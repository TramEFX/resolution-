// Resolution's GPU renderer (LOG D-035).
//
// Layers are drawn as transformed quads into a premultiplied linear accumulator. The source
// sampling (manual bilinear loads, so any float format works; Y'CbCr decoding; N x N supersampling
// over the pixel footprint when minifying), the colour management of HDR and wide-gamut Y'CbCr,
// and the blend modes follow FilmCraft's compositor (`filmcraft-gpu`, composite.wgsl; MIT or
// Apache-2.0), whose code they port. The grade stage is Resolution's own: the nodes of
// `resolution-grade`, run on the display-encoded signal exactly as `Node::apply` runs them, with
// each node limited by its qualifier's key (on the node's input) and its windows.

struct U {
    m0: vec4<f32>,   // a b c d
    m1: vec4<f32>,   // e f out_w out_h
    src: vec4<f32>,  // src_w src_h chroma_w chroma_h
    p0: vec4<f32>,   // opacity, kind (0 rgba8 srgb straight, 1 rgba premul linear, 2 yuv), taps, transfer (0 srgb, 1 linear, 2 pq, 3 hlg)
    p1: vec4<f32>,   // y_off y_scale c_off c_scale (code units)
    p2: vec4<f32>,   // kr kb code_scale footprint
    p3: vec4<f32>,   // alpha-plane scale (0: none), nodes, blend mode (`Blend::index`), chroma siting (0 centre, 1 left)
    // Colour management of HDR / wide-gamut Y'CbCr (`Managed`):
    q0: vec4<f32>,   // on, curve (0 sdr, 1 linear, 2 pq, 3 hlg), HLG OOTF scale (0: none), reference white cd/m²
    q1: vec4<f32>,   // HLG OOTF luma weights, gamut matrix on
    q2: vec4<f32>,   // gamut matrix row 0, tone mapping on
    q3: vec4<f32>,   // gamut matrix row 1, tone-mapping source peak cd/m²
    q4: vec4<f32>,   // gamut matrix row 2, tone-mapping target peak cd/m²
    q5: vec4<f32>,   // desaturation luma weights, desaturation on
    // The grade stage (LOG D-046): the node, the axis (0 across, 1 down), Blur Radius's sigma in
    // working pixels, and whether the node's matte comes from the mask (1).
    g0: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var tex0: texture_2d<f32>;
@group(0) @binding(2) var tex1: texture_2d<f32>;
@group(0) @binding(3) var tex2: texture_2d<f32>;
@group(0) @binding(4) var tex3: texture_2d<f32>;
// The grade: NODE_FLOATS numbers per node, and TONE_N samples of each node's three tone functions.
@group(0) @binding(5) var<storage, read> nodes: array<f32>;
@group(0) @binding(6) var<storage, read> tones: array<f32>;
// The accumulator under the layer (blend modes other than Normal and Dissolve only).
@group(0) @binding(7) var backdrop: texture_2d<f32>;

const TONE_N: u32 = 4096u;
const NODE_FLOATS: u32 = 104u;
// Tables per node: three tone functions, then the hue curves (Hue vs Hue, Hue vs Sat, Hue vs Lum,
// Lum vs Sat, Sat vs Sat, Sat vs Lum).
const TABLES: u32 = 9u;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) sp: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0));
    let c = corners[vi];
    let s = c * u.src.xy;
    let o = vec2(u.m0.x * s.x + u.m0.z * s.y + u.m1.x, u.m0.y * s.x + u.m0.w * s.y + u.m1.y);
    var out: VOut;
    out.pos = vec4(o.x / u.m1.z * 2.0 - 1.0, 1.0 - o.y / u.m1.w * 2.0, 0.0, 1.0);
    out.sp = s;
    return out;
}

// The four texels around source position p and how far p lies between them.
struct Quad {
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    d: vec4<f32>,
    f: vec2<f32>,
}

fn quad(t: texture_2d<f32>, p: vec2<f32>) -> Quad {
    let d = vec2<i32>(textureDimensions(t));
    var q = p - 0.5;
    let rq = round(q);
    if abs(q.x - rq.x) < 1e-4 { q.x = rq.x; }
    if abs(q.y - rq.y) < 1e-4 { q.y = rq.y; }
    let i = vec2<i32>(floor(q));
    var out: Quad;
    out.a = textureLoad(t, clamp(i, vec2(0), d - 1), 0);
    out.b = textureLoad(t, clamp(i + vec2(1, 0), vec2(0), d - 1), 0);
    out.c = textureLoad(t, clamp(i + vec2(0, 1), vec2(0), d - 1), 0);
    out.d = textureLoad(t, clamp(i + vec2(1, 1), vec2(0), d - 1), 0);
    out.f = q - floor(q);
    return out;
}

fn bilerp(a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, d: vec4<f32>, f: vec2<f32>) -> vec4<f32> {
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn load4(t: texture_2d<f32>, p: vec2<f32>) -> vec4<f32> {
    let q = quad(t, p);
    return bilerp(q.a, q.b, q.c, q.d, q.f);
}

fn srgb_to_linear(v: vec3<f32>) -> vec3<f32> {
    let lo = v / 12.92;
    let hi = pow((max(v, vec3(0.0)) + 0.055) / 1.055, vec3(2.4));
    return select(hi, lo, v <= vec3(0.04045));
}

fn linear_to_srgb(v: vec3<f32>) -> vec3<f32> {
    let c = max(v, vec3(0.0));
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3(0.0031308));
}

fn pq_eotf(e: vec3<f32>) -> vec3<f32> {
    let m1 = 0.1593017578125; let m2 = 78.84375; let c1 = 0.8359375; let c2 = 18.8515625; let c3 = 18.6875;
    let p = pow(max(e, vec3(0.0)), vec3(1.0 / m2));
    return pow(max(p - c1, vec3(0.0)) / (c2 - c3 * p), vec3(1.0 / m1)) * 100.0;
}

fn hlg_inv(e: vec3<f32>) -> vec3<f32> {
    let a = 0.17883277; let b = 0.28466892; let c = 0.55991073;
    let lo = e * e / 3.0;
    let hi = (exp((e - c) / a) + b) / 12.0;
    return select(hi, lo, e <= vec3(0.5));
}

fn to_linear(v: vec3<f32>) -> vec3<f32> {
    let t = u32(u.p0.w);
    if t == 1u { return v; }
    if t == 2u { return pq_eotf(v); }
    if t == 3u { return hlg_inv(v); }
    return srgb_to_linear(clamp(v, vec3(0.0), vec3(1.0)));
}

// ---- colour management (FilmCraft's `InputTransform`), for `u.q0.x` = 1

// SMPTE ST 2084 on a 0..1 scale (1 = 10 000 cd/m²) and back.
fn pq1(e: f32) -> f32 {
    let m1 = 0.1593017578125; let m2 = 78.84375; let c1 = 0.8359375; let c2 = 18.8515625; let c3 = 18.6875;
    let p = pow(max(e, 0.0), 1.0 / m2);
    return pow(max(p - c1, 0.0) / (c2 - c3 * p), 1.0 / m1);
}

fn pq1_inv(y: f32) -> f32 {
    let m1 = 0.1593017578125; let m2 = 78.84375; let c1 = 0.8359375; let c2 = 18.8515625; let c3 = 18.6875;
    let p = pow(clamp(y, 0.0, 1.0), m1);
    return pow((c1 + c2 * p) / (1.0 + c3 * p), m2);
}

// ITU-R BT.2390 EETF, source peak s to target peak d, in cd/m².
fn eetf(nits: f32, s: f32, d: f32) -> f32 {
    if d >= s || nits <= 0.0 { return clamp(nits, 0.0, max(s, d)); }
    let smax = pq1_inv(s / 10000.0);
    let e1 = min(pq1_inv(nits / 10000.0) / smax, 1.0);
    let max_lum = pq1_inv(d / 10000.0) / smax;
    let ks = 1.5 * max_lum - 0.5;
    var e2 = e1;
    if e1 >= ks {
        let t = (e1 - ks) / (1.0 - ks);
        let t2 = t * t; let t3 = t2 * t;
        e2 = (2.0 * t3 - 3.0 * t2 + 1.0) * ks + (t3 - 2.0 * t2 + t) * (1.0 - ks) + (-2.0 * t3 + 3.0 * t2) * max_lum;
    }
    return pq1(e2 * smax) * 10000.0;
}

// Signal to working linear: the curve, HLG's OOTF, the gamut matrix, tone mapping, and
// desaturation into the working gamut. (FilmCraft's CPU decode tables clamp the signal to 0..1.)
fn managed(rgb: vec3<f32>) -> vec3<f32> {
    let v = clamp(rgb, vec3(0.0), vec3(1.0));
    let curve = u32(u.q0.y);
    let white = u.q0.w;
    var c = v;
    if curve == 0u { c = srgb_to_linear(v); }
    if curve == 2u { c = vec3(pq1(v.r), pq1(v.g), pq1(v.b)) * (10000.0 / white); }
    if curve == 3u { c = hlg_inv(v); }
    if u.q0.z > 0.0 {
        let ys = max(dot(u.q1.xyz, c), 0.0);
        var g = 0.0;
        if ys > 0.0 { g = u.q0.z * pow(ys, 0.2); }
        c = c * g;
    }
    if u.q1.w > 0.5 {
        c = vec3(dot(u.q2.xyz, c), dot(u.q3.xyz, c), dot(u.q4.xyz, c));
    }
    if u.q2.w > 0.5 {
        let m = max(max(c.r, c.g), c.b);
        if m > 0.0 {
            let nits = m * white;
            var k = 1.0;
            if nits >= u.q3.w {
                k = u.q4.w / white / m;
            } else if nits > 1e-6 {
                k = eetf(nits, u.q3.w, u.q4.w) / nits;
            }
            c = c * k;
        }
    }
    if u.q5.w > 0.5 {
        let mn = min(min(c.r, c.g), c.b);
        if mn < 0.0 {
            let y = dot(u.q5.xyz, c);
            if y <= 0.0 {
                c = vec3(0.0);
            } else {
                c = max(y + (c - y) * (y / (y - mn)), vec3(0.0));
            }
        }
    }
    return c;
}

// An 8-bit sRGB texel with straight alpha, linear and premultiplied.
fn srgb_texel(c: vec4<f32>) -> vec4<f32> {
    return vec4(srgb_to_linear(clamp(c.rgb, vec3(0.0), vec3(1.0))) * c.a, c.a);
}

// One linear premultiplied sample at source position p.
fn sample(p: vec2<f32>) -> vec4<f32> {
    let kind = u32(u.p0.y);
    if kind == 0u {
        // 8-bit sRGB codes, straight alpha: decoded here, not by the texture unit, so a grade
        // re-encoding them sees exactly the codes that went in; each texel decoded and
        // premultiplied before they are blended, as the texture unit and FilmCraft's CPU do
        let q = quad(tex0, p);
        return bilerp(srgb_texel(q.a), srgb_texel(q.b), srgb_texel(q.c), srgb_texel(q.d), q.f);
    }
    if kind == 1u {
        return load4(tex0, p);
    }
    let cs = u.p2.z;
    let yc = load4(tex0, p).r * cs;
    // the chroma planes' position of this luma position: their samples halfway between luma
    // samples, or (sited left) on the first of each run horizontally
    var cp = p * u.src.zw / u.src.xy;
    if u.p3.w > 0.5 {
        cp.x = (p.x - 0.5) * u.src.z / u.src.x + 0.5;
    }
    let cb = load4(tex1, cp).r * cs;
    let cr = load4(tex2, cp).r * cs;
    let y = (yc - u.p1.x) / u.p1.y;
    let b = (cb - u.p1.z) / u.p1.w;
    let r = (cr - u.p1.z) / u.p1.w;
    let kr = u.p2.x; let kb = u.p2.y; let kg = 1.0 - kr - kb;
    let R = y + 2.0 * (1.0 - kr) * r;
    let B = y + 2.0 * (1.0 - kb) * b;
    let G = (y - kr * R - kb * B) / kg;
    var a = 1.0;
    if u.p3.x > 0.0 {
        a = clamp(load4(tex3, p).r * u.p3.x, 0.0, 1.0);
    }
    var lin = to_linear(vec3(R, G, B));
    if u.q0.x > 0.5 { lin = managed(vec3(R, G, B)); }
    return vec4(lin * a, a);
}

// The layer's premultiplied linear colour at this source position, before opacity.
fn layer_color(sp: vec2<f32>) -> vec4<f32> {
    let n = max(u32(u.p0.z), 1u);
    let fp = u.p2.w;
    var acc = vec4(0.0);
    for (var j = 0u; j < n; j++) {
        for (var i = 0u; i < n; i++) {
            let off = (vec2(f32(i), f32(j)) + 0.5) / f32(n) - 0.5;
            acc += sample(sp + off * fp);
        }
    }
    return acc / f32(n * n);
}

// ---- the grade (`resolution-grade`)

fn nf(n: u32, i: u32) -> f32 {
    return nodes[n * NODE_FLOATS + i];
}

// A tone function, sampled at TONE_N points over 0..1, between samples linearly.
fn tone(n: u32, ch: u32, x: f32) -> f32 {
    let p = clamp(x, 0.0, 1.0) * f32(TONE_N - 1u);
    let i = u32(floor(p));
    let j = min(i + 1u, TONE_N - 1u);
    let base = (n * TABLES + ch) * TONE_N;
    return mix(tones[base + i], tones[base + j], p - floor(p));
}

// Hue curve `c` of node `n` at hue `x` (turns): sampled at TONE_N points round the wheel.
fn round_curve(n: u32, c: u32, x: f32) -> f32 {
    let p = fract(x) * f32(TONE_N);
    let i = min(u32(floor(p)), TONE_N - 1u);
    let j = (i + 1u) % TONE_N;
    let base = (n * TABLES + 3u + c) * TONE_N;
    return mix(tones[base + i], tones[base + j], p - floor(p));
}

// Lum vs Sat (`c` 3), Sat vs Sat (4) or Sat vs Lum (5) of node `n` at `x`: sampled end to end.
fn held_curve(n: u32, c: u32, x: f32) -> f32 {
    return tone(n, 3u + c, x);
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

// Hue (turns), saturation, lightness: `resolution_grade::rgb_to_hsl`.
fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let mx = max(max(c.r, c.g), c.b);
    let mn = min(min(c.r, c.g), c.b);
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-9 {
        return vec3(0.0, 0.0, l);
    }
    var s = d / (mx + mn);
    if l > 0.5 {
        s = d / (2.0 - mx - mn);
    }
    var h: f32;
    if mx == c.r {
        h = (c.g - c.b) / d;
        if c.g < c.b { h += 6.0; }
    } else if mx == c.g {
        h = (c.b - c.r) / d + 2.0;
    } else {
        h = (c.r - c.g) / d + 4.0;
    }
    return vec3(h / 6.0, s, l);
}

fn hue_channel(p: f32, q: f32, t0: f32) -> f32 {
    let t = t0 - floor(t0);
    if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
    if t < 0.5 { return q; }
    if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    return p;
}

fn hsl_to_rgb(h: vec3<f32>) -> vec3<f32> {
    let s = h.y;
    let l = h.z;
    if s < 1e-9 {
        return vec3(l);
    }
    var q = l + s - l * s;
    if l < 0.5 { q = l * (1.0 + s); }
    let p = 2.0 * l - q;
    return vec3(hue_channel(p, q, h.x + 1.0 / 3.0), hue_channel(p, q, h.x), hue_channel(p, q, h.x - 1.0 / 3.0));
}

// 1 inside [lo, hi], falling to 0 over each soft width outside (`resolution_grade`'s `range`).
fn fall(d: f32, soft: f32) -> f32 {
    if d <= 0.0 { return 1.0; }
    if soft <= 1e-6 { return 0.0; }
    let t = clamp(1.0 - d / soft, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn range(v: f32, lo: f32, hi: f32, lo_soft: f32, hi_soft: f32) -> f32 {
    return fall(lo - v, lo_soft) * fall(v - hi, hi_soft);
}

// The qualifier's key of a node's input (`Qualifier::key`).
fn key(n: u32, rgb: vec3<f32>) -> f32 {
    let c = clamp(rgb, vec3(0.0), vec3(1.0));
    let hsl = rgb_to_hsl(c);
    var k = 1.0;
    if nf(n, 6u) > 0.5 {
        let x = hsl.x * 100.0 - nf(n, 7u);
        let d0 = x - 100.0 * floor(x / 100.0);
        let d = min(d0, 100.0 - d0);
        k *= range(d, -1.0, nf(n, 8u) / 2.0, 0.0, nf(n, 9u));
    }
    if nf(n, 10u) > 0.5 {
        k *= range(hsl.y * 100.0, nf(n, 11u), nf(n, 12u), nf(n, 13u), nf(n, 14u));
    }
    if nf(n, 15u) > 0.5 {
        k *= range(luma(c) * 100.0, nf(n, 16u), nf(n, 17u), nf(n, 18u), nf(n, 19u));
    }
    if nf(n, 5u) > 0.5 {
        k = 1.0 - k;
    }
    return k;
}

// How much of a node the windows let through at source position sp: each window's shape (turned
// clockwise about its centre), its soft edge centred on the edge, inverted and scaled by its
// opacity; windows add up. 1 when the node has none.
// How much a soft edge `soft` pixels wide, centred on the shape's edge, lets through at signed
// distance `d` from it (negative inside); hard below a thousandth of a pixel.
fn soft_edge(d: f32, soft: f32) -> f32 {
    if soft <= 1e-3 {
        return select(0.0, 1.0, d <= 0.0);
    }
    return clamp(0.5 - d / soft, 0.0, 1.0);
}

// The signed distance from `p` to the polygon whose corners are at `at` in the tables (their
// count, then x and y for each, in the picture's pixels): negative inside (even-odd), positive
// outside.
fn polygon_distance(at: u32, p: vec2<f32>) -> f32 {
    let count = u32(tones[at]);
    var best = 1e9;
    var inside = false;
    for (var i = 0u; i < count; i++) {
        let j = (i + count - 1u) % count;
        let a = vec2(tones[at + 1u + 2u * i], tones[at + 2u + 2u * i]);
        let b = vec2(tones[at + 1u + 2u * j], tones[at + 2u + 2u * j]);
        let e = b - a;
        let t = clamp(dot(p - a, e) / max(dot(e, e), 1e-12), 0.0, 1.0);
        best = min(best, length(p - a - e * t));
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    return select(best, -best, inside);
}

fn coverage(n: u32, sp: vec2<f32>) -> f32 {
    let count = u32(nf(n, 20u));
    if count == 0u {
        return 1.0;
    }
    var acc = 0.0;
    for (var w = 0u; w < count; w++) {
        let o = 24u + w * 10u;
        let c = vec2(nf(n, o + 1u), nf(n, o + 2u));
        let r = max(vec2(nf(n, o + 3u), nf(n, o + 4u)), vec2(1e-4));
        let rot = nf(n, o + 5u);
        let soft = nf(n, o + 6u);
        let d = sp - c;
        let cs = cos(rot);
        let sn = sin(rot);
        let q = vec2(d.x * cs + d.y * sn, -d.x * sn + d.y * cs);
        let shape = nf(n, o);
        var dist: f32;
        if shape > 2.5 {
            // a Polygon (LOG D-053): its corners, placed, are in the tables
            dist = polygon_distance(u32(nf(n, o + 9u)), sp);
        } else if shape > 1.5 {
            // a Gradient (LOG D-052): keyed above its line, at Rotate 0
            dist = q.y;
        } else if shape > 0.5 {
            dist = (length(q / r) - 1.0) * min(r.x, r.y);
        } else {
            dist = max(abs(q.x) - r.x, abs(q.y) - r.y);
        }
        var m: f32;
        if shape > 2.5 {
            // a Polygon's or a Curve's own reach into the shape and out of it (LOG D-063)
            let inside = nf(n, o + 3u);
            let outside = nf(n, o + 4u);
            if inside + outside <= 1e-3 {
                m = select(0.0, 1.0, dist <= 0.0);
            } else {
                m = clamp((outside - dist) / (inside + outside), 0.0, 1.0);
            }
        } else if shape < 0.5 {
            // a Linear window's Soft 1 to 4 (LOG D-065): each side's own, in the tables as top,
            // bottom, left and right; with all four the same, as one softness
            let at = u32(nf(n, o + 9u));
            let sx = select(tones[at + 3u], tones[at + 2u], q.x < 0.0);
            let sy = select(tones[at + 1u], tones[at], q.y < 0.0);
            m = min(soft_edge(abs(q.x) - r.x, sx), soft_edge(abs(q.y) - r.y, sy));
        } else {
            m = soft_edge(dist, soft);
        }
        if nf(n, o + 8u) > 0.5 {
            m = 1.0 - m;
        }
        m *= nf(n, o + 7u);
        acc = acc + m - acc * m;
    }
    return acc;
}

// Clean Black and Clean White on a key value (`Qualifier::clean`).
fn clean(n: u32, k: f32) -> f32 {
    let b = nf(n, 22u);
    let wc = nf(n, 23u);
    if b <= 0.0 && wc <= 0.0 {
        return k;
    }
    return clamp((k - b) / max(1.0 - wc - b, 1e-3), 0.0, 1.0);
}

// Node n's correction of a pixel's signal, before its key and windows (`Node::tone`, then
// `Node::color`).
fn node_color(n: u32, inp: vec3<f32>) -> vec3<f32> {
    var o = inp;
    if nf(n, 0u) > 0.5 {
        o = vec3(tone(n, 0u, o.r), tone(n, 1u, o.g), tone(n, 2u, o.b));
    }
    let amount = nf(n, 1u);
    let boost = nf(n, 2u);
    let turns = nf(n, 3u);
    let curves = u32(nf(n, 21u));
    if abs(amount - 1.0) > 1e-6 || abs(boost) > 1e-6 {
        let l = luma(o);
        let spread = clamp(max(max(o.r, o.g), o.b) - min(min(o.r, o.g), o.b), 0.0, 1.0);
        let s = amount * (1.0 + boost * (1.0 - spread));
        o = clamp(vec3(l) + (o - vec3(l)) * s, vec3(0.0), vec3(1.0));
    }
    // Hue's rotation and the hue curves together, each curve read where the pixel was
    // (`Node::color`)
    if abs(turns) > 1e-6 || curves != 0u {
        let h0 = rgb_to_hsl(o);
        var hue = h0.x + turns;
        if (curves & 1u) != 0u { hue += round_curve(n, 0u, h0.x) - 0.5; }
        var gain = 1.0;
        if (curves & 2u) != 0u { gain *= 2.0 * round_curve(n, 1u, h0.x); }
        if (curves & 8u) != 0u { gain *= 2.0 * held_curve(n, 3u, h0.z); }
        if (curves & 16u) != 0u { gain *= 2.0 * held_curve(n, 4u, h0.y); }
        var light = h0.z;
        if (curves & 4u) != 0u { light += (round_curve(n, 2u, h0.x) - 0.5) * 0.5; }
        if (curves & 32u) != 0u { light += (held_curve(n, 5u, h0.y) - 0.5) * 0.5; }
        light = clamp(light, 0.0, 1.0);
        o = clamp(hsl_to_rgb(vec3(fract(hue), clamp(h0.y * gain, 0.0, 1.0), light)), vec3(0.0), vec3(1.0));
    }
    return o;
}

// The grade on one pixel's signal (0..1), at source position sp (`Grade::apply`, node by node).
fn grade(v_in: vec3<f32>, sp: vec2<f32>) -> vec3<f32> {
    var v = clamp(v_in, vec3(0.0), vec3(1.0));
    let count = u32(u.p3.y);
    for (var n = 0u; n < count; n++) {
        let inp = v;
        let o = node_color(n, inp);
        var k = 1.0;
        if nf(n, 4u) > 0.5 {
            k = clean(n, key(n, inp));
        }
        k *= coverage(n, sp);
        v = inp + (o - inp) * k;
    }
    return v;
}

// ---- the grade stage (LOG D-046): a layer whose qualifiers clean their key over the picture
// is graded first at a working size, into textures of straight, display-encoded colour (tex0),
// each node's matte in a mask (tex1), pass by pass. Per-pixel passes draw a full-screen
// triangle (`vs_full`); positions are working pixels.

fn texel(t: texture_2d<f32>, p: vec2<i32>) -> vec4<f32> {
    let d = vec2<i32>(textureDimensions(t));
    return textureLoad(t, clamp(p, vec2(0), d - 1), 0);
}

// The layer drawn at the working size: its colour, straight and display-encoded.
@fragment
fn fs_signal(in: VOut) -> @location(0) vec4<f32> {
    let c = layer_color(in.sp);
    if c.a <= 1e-6 {
        return vec4(0.0);
    }
    return vec4(clamp(linear_to_srgb(c.rgb / c.a), vec3(0.0), vec3(1.0)), c.a);
}

// The node's key of each pixel.
@fragment
fn fs_key(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let s = texel(tex0, vec2<i32>(pos.xy));
    return vec4(key(u32(u.g0.x), s.rgb), 0.0, 0.0, 1.0);
}

// Denoise: the median of each mask value and its two neighbours across or down.
@fragment
fn fs_median(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    var d = vec2(1, 0);
    if u.g0.y > 0.5 { d = vec2(0, 1); }
    let a = texel(tex1, p - d).r;
    let b = texel(tex1, p).r;
    let c = texel(tex1, p + d).r;
    return vec4(max(min(a, b), min(max(a, b), c)), 0.0, 0.0, 1.0);
}

// Clean Black and Clean White on the mask.
@fragment
fn fs_clean(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4(clean(u32(u.g0.x), texel(tex1, vec2<i32>(pos.xy)).r), 0.0, 0.0, 1.0);
}

// Blur Radius: the mask convolved across or down with a Gaussian to three sigmas
// (`resolution_grade::gaussian`).
@fragment
fn fs_blur(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    var d = vec2(1, 0);
    if u.g0.y > 0.5 { d = vec2(0, 1); }
    let sigma = u.g0.z;
    let r = max(i32(ceil(3.0 * sigma)), 1);
    var total = 1.0;
    var acc = texel(tex1, p).r;
    for (var i = 1; i <= r; i++) {
        let w = exp(-f32(i * i) / (2.0 * sigma * sigma));
        acc += w * (texel(tex1, p - d * i).r + texel(tex1, p + d * i).r);
        total += 2.0 * w;
    }
    return vec4(acc / total, 0.0, 0.0, 1.0);
}

// The Highlight view (LOG D-048) of node g0.x on the graded picture (tex0): its matte (tex1 when
// g0.z is 1, else everything keyed) with its windows; the picture where keyed and flat grey g0.w
// elsewhere, or with g0.y the matte in black and white (`Grade::highlight_image`).
@fragment
fn fs_highlight(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let s = texel(tex0, p);
    let n = u32(u.g0.x);
    var k = 1.0;
    if u.g0.z > 0.5 {
        k = texel(tex1, p).r;
    }
    let dims = vec2<f32>(textureDimensions(tex0));
    k *= coverage(n, pos.xy * u.src.xy / dims);
    if u.g0.y > 0.5 {
        return vec4(vec3(k), s.a);
    }
    return vec4(mix(vec3(u.g0.w), s.rgb, k), s.a);
}

// Morph (LOG D-058; `resolution_grade::Morph`): the most (g0.w = 1) or the least of each mask
// value and its neighbours up to g0.z away, across or down (g0.y).
@fragment
fn fs_morph(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    var d = vec2(1, 0);
    if u.g0.y > 0.5 { d = vec2(0, 1); }
    let r = i32(u.g0.z);
    let most = u.g0.w > 0.5;
    var v = texel(tex1, p).r;
    for (var i = 1; i <= r; i++) {
        let a = texel(tex1, p - d * i).r;
        let b = texel(tex1, p + d * i).r;
        if most {
            v = max(v, max(a, b));
        } else {
            v = min(v, min(a, b));
        }
    }
    return vec4(v, 0.0, 0.0, 1.0);
}

// Black Clip and White Clip (LOG D-058; `Qualifier::clip`): the mask stretched between g0.y and
// g0.z.
@fragment
fn fs_clip(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let k = texel(tex1, vec2<i32>(pos.xy)).r;
    return vec4(clamp((k - u.g0.y) / max(u.g0.z - u.g0.y, 1e-3), 0.0, 1.0), 0.0, 0.0, 1.0);
}

// Midtone Detail (LOG D-055): the luma of each pixel of the node's input, to be blurred.
@fragment
fn fs_luma(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let s = texel(tex0, vec2<i32>(pos.xy));
    return vec4(luma(clamp(s.rgb, vec3(0.0), vec3(1.0))), 0.0, 0.0, 1.0);
}

// The node on each pixel: its correction blended by its matte (from the mask, or keyed here) and
// its windows, at the pixel's place in the layer's own pixels.
@fragment
fn fs_node(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let s = texel(tex0, p);
    let n = u32(u.g0.x);
    let inp = clamp(s.rgb, vec3(0.0), vec3(1.0));
    var o = node_color(n, inp);
    // Midtone Detail (LOG D-055; `Node::detailed`): the input's luma against its blur (tex2),
    // Midtone Detail / 100 in g0.y, the detail that stops softening in g0.z
    let md = u.g0.y;
    if abs(md) > 1e-6 {
        let l = luma(inp);
        let d = l - texel(tex2, p).r;
        var a = md;
        if md < 0.0 {
            a = md * clamp(1.0 - abs(d) / u.g0.z, 0.0, 1.0);
        }
        o = clamp(o + vec3(a * d * 4.0 * l * (1.0 - l)), vec3(0.0), vec3(1.0));
    }
    var k = 1.0;
    if nf(n, 4u) > 0.5 {
        if u.g0.w > 0.5 {
            k = texel(tex1, p).r;
        } else {
            k = clean(n, key(n, inp));
        }
    }
    let dims = vec2<f32>(textureDimensions(tex0));
    k *= coverage(n, pos.xy * u.src.xy / dims);
    return vec4(inp + (o - inp) * k, s.a);
}

// The layer's premultiplied linear colour at a pixel, graded, opacity applied.
fn shaded(sp: vec2<f32>) -> vec4<f32> {
    var c = layer_color(sp);
    if u.p3.y > 0.5 && c.a > 1e-6 {
        let signal = linear_to_srgb(c.rgb / c.a);
        let graded = grade(signal, sp);
        c = vec4(srgb_to_linear(graded) * c.a, c.a);
    }
    return c * u.p0.x;
}

// ---- Dissolve: FilmCraft's 64-bit pixel hash (`blend::hash2`), on 32-bit halves (lo, hi)

// The full 64-bit product of two u32.
fn mul32(a: u32, b: u32) -> vec2<u32> {
    let a0 = a & 0xffffu; let a1 = a >> 16u; let b0 = b & 0xffffu; let b1 = b >> 16u;
    let p00 = a0 * b0; let p01 = a0 * b1; let p10 = a1 * b0; let p11 = a1 * b1;
    let mid = (p00 >> 16u) + (p01 & 0xffffu) + (p10 & 0xffffu);
    return vec2((mid << 16u) | (p00 & 0xffffu), p11 + (p01 >> 16u) + (p10 >> 16u) + (mid >> 16u));
}

// A wrapping 64-bit multiply.
fn mul64(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
    let p = mul32(a.x, b.x);
    return vec2(p.x, p.y + a.x * b.y + a.y * b.x);
}

// A 64-bit shift right by 0 < n < 32.
fn shr64(a: vec2<u32>, n: u32) -> vec2<u32> {
    return vec2((a.x >> n) | (a.y << (32u - n)), a.y >> n);
}

fn hash2(x: u32, y: u32) -> f32 {
    var h = mul64(vec2(x, 0u), vec2(0x7F4A7C15u, 0x9E3779B9u)) ^ mul64(vec2(y, 0u), vec2(0x27D4EB4Fu, 0xC2B2AE3Du));
    h = h ^ shr64(h, 31u);
    h = mul64(h, vec2(0x1CE4E5B9u, 0xBF58476Du));
    h = h ^ shr64(h, 29u);
    return f32(h.y >> 8u) / 16777216.0;
}

// Normal and Dissolve: fixed-function premultiplied "over".
@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let c = shaded(in.sp);
    if u32(u.p3.z) == 1u {
        // Dissolve: the pixel is kept with probability alpha, then drawn opaque
        let sa = c.a;
        if hash2(u32(in.pos.x), u32(in.pos.y)) >= sa {
            discard;
        }
        return vec4(c.rgb / sa, 1.0);
    }
    return c;
}

// ---- the other blend modes: the W3C Compositing and Blending formulas on straight,
// sRGB-encoded colour, as FilmCraft's `blend::composite` (no recursion in WGSL, so helpers are
// split out)

fn screen(b: f32, s: f32) -> f32 { return b + s - b * s; }

fn color_burn(b: f32, s: f32) -> f32 {
    if b >= 1.0 { return 1.0; }
    if s <= 0.0 { return 0.0; }
    return 1.0 - min((1.0 - b) / s, 1.0);
}

fn color_dodge(b: f32, s: f32) -> f32 {
    if b <= 0.0 { return 0.0; }
    if s >= 1.0 { return 1.0; }
    return min(b / (1.0 - s), 1.0);
}

fn hard_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 { return b * 2.0 * s; }
    return screen(b, 2.0 * s - 1.0);
}

fn vivid_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 { return color_burn(b, 2.0 * s); }
    return color_dodge(b, 2.0 * s - 1.0);
}

fn sep(mode: u32, b: f32, s: f32) -> f32 {
    switch mode {
        case 2u: { return min(b, s); }                       // Darken
        case 3u: { return b * s; }                           // Multiply
        case 4u: { return color_burn(b, s); }
        case 5u: { return max(b + s - 1.0, 0.0); }           // Linear Burn
        case 7u: { return max(b, s); }                       // Lighten
        case 8u: { return screen(b, s); }
        case 9u: { return color_dodge(b, s); }
        case 10u: { return min(b + s, 1.0); }                // Linear Dodge (Add)
        case 12u: { return hard_light(s, b); }               // Overlay
        case 13u: {                                          // Soft Light
            if s <= 0.5 { return b - (1.0 - 2.0 * s) * b * (1.0 - b); }
            var d = sqrt(b);
            if b <= 0.25 { d = ((16.0 * b - 12.0) * b + 4.0) * b; }
            return b + (2.0 * s - 1.0) * (d - b);
        }
        case 14u: { return hard_light(b, s); }
        case 15u: { return vivid_light(b, s); }
        case 16u: { return clamp(b + 2.0 * s - 1.0, 0.0, 1.0); } // Linear Light
        case 17u: {                                          // Pin Light
            if s <= 0.5 { return min(b, 2.0 * s); }
            return max(b, 2.0 * s - 1.0);
        }
        case 18u: { return select(0.0, 1.0, vivid_light(b, s) >= 0.5); } // Hard Mix
        case 19u: { return abs(b - s); }                     // Difference
        case 20u: { return b + s - 2.0 * b * s; }            // Exclusion
        case 21u: { return max(b - s, 0.0); }                // Subtract
        case 22u: {                                          // Divide
            if s <= 0.0 { return 1.0; }
            return min(b / s, 1.0);
        }
        default: { return s; }
    }
}

fn lum(c: vec3<f32>) -> f32 { return 0.3 * c.r + 0.59 * c.g + 0.11 * c.b; }

fn clip_color(c: vec3<f32>) -> vec3<f32> {
    let l = lum(c);
    let n = min(min(c.r, c.g), c.b);
    let x = max(max(c.r, c.g), c.b);
    var o = c;
    if n < 0.0 { o = l + (o - l) * l / max(l - n, 1e-6); }
    if x > 1.0 { o = l + (o - l) * (1.0 - l) / max(x - l, 1e-6); }
    return o;
}

fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> { return clip_color(c + (l - lum(c))); }

fn sat(c: vec3<f32>) -> f32 { return max(max(c.r, c.g), c.b) - min(min(c.r, c.g), c.b); }

fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let mx = max(max(c.r, c.g), c.b);
    let mn = min(min(c.r, c.g), c.b);
    if mx - mn <= 1e-6 { return vec3(0.0); }
    return (c - mn) * s / (mx - mn);
}

fn blend_rgb(mode: u32, b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    switch mode {
        case 23u: { return set_lum(set_sat(s, sat(b)), lum(b)); } // Hue
        case 24u: { return set_lum(set_sat(b, sat(s)), lum(b)); } // Saturation
        case 25u: { return set_lum(s, lum(b)); }                  // Color
        case 26u: { return set_lum(b, lum(s)); }                  // Luminosity
        case 6u: { return select(b, s, lum(s) < lum(b)); }        // Darker Color
        case 11u: { return select(b, s, lum(s) > lum(b)); }       // Lighter Color
        default: { return vec3(sep(mode, b.r, s.r), sep(mode, b.g, s.g), sep(mode, b.b, s.b)); }
    }
}

// Blend modes that need the colour under the layer: written without fixed-function blending.
@fragment
fn fs_blend(in: VOut) -> @location(0) vec4<f32> {
    let sp = shaded(in.sp);
    let sa = sp.a;
    if sa <= 0.0 {
        discard;
    }
    let d = textureLoad(backdrop, vec2<i32>(in.pos.xy), 0);
    let da = d.a;
    if da <= 0.0 {
        return sp + d * (1.0 - sa);
    }
    let cs = linear_to_srgb(sp.rgb / sa);
    let cb = linear_to_srgb(d.rgb / da);
    let mixed = srgb_to_linear(clamp(blend_rgb(u32(u.p3.z), cb, cs), vec3(0.0), vec3(1.0)));
    return vec4(sp.rgb * (1.0 - da) + d.rgb * (1.0 - sa) + sa * da * mixed, sa + da - sa * da);
}

// ---- the resolve: the accumulator over black, into the sRGB output

@group(0) @binding(0) var accum: texture_2d<f32>;

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[vi], 0.0, 1.0);
}

// sRGB-encoded into a plain 8-bit target (what egui takes as a native texture)
@fragment
fn fs_full(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = clamp(textureLoad(accum, vec2<i32>(pos.xy), 0).rgb, vec3(0.0), vec3(1.0));
    return vec4(linear_to_srgb(c), 1.0);
}
