// Resolution's GPU renderer (LOG D-035).
//
// Layers are drawn as transformed quads into a premultiplied linear accumulator. The source
// sampling (manual bilinear loads, so any float format works; Y'CbCr decoding; N x N supersampling
// over the pixel footprint when minifying) follows FilmCraft's compositor (`filmcraft-gpu`,
// composite.wgsl; MIT or Apache-2.0). The grade stage is Resolution's own: the nodes of
// `resolution-grade`, run on the display-encoded signal exactly as `Node::apply` runs them, with
// each node limited by its qualifier's key (on the node's input) and its windows.

struct U {
    m0: vec4<f32>,   // a b c d
    m1: vec4<f32>,   // e f out_w out_h
    src: vec4<f32>,  // src_w src_h chroma_w chroma_h
    p0: vec4<f32>,   // opacity, kind (0 rgba8 srgb straight, 1 rgba premul linear, 2 yuv), taps, transfer (0 srgb, 1 linear)
    p1: vec4<f32>,   // y_off y_scale c_off c_scale (code units)
    p2: vec4<f32>,   // kr kb code_scale footprint
    p3: vec4<f32>,   // alpha-plane scale (0: none), nodes, unused, unused
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var tex0: texture_2d<f32>;
@group(0) @binding(2) var tex1: texture_2d<f32>;
@group(0) @binding(3) var tex2: texture_2d<f32>;
@group(0) @binding(4) var tex3: texture_2d<f32>;
// The grade: NODE_FLOATS numbers per node, and TONE_N samples of each node's three tone functions.
@group(0) @binding(5) var<storage, read> nodes: array<f32>;
@group(0) @binding(6) var<storage, read> tones: array<f32>;

const TONE_N: u32 = 4096u;
const NODE_FLOATS: u32 = 104u;

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
    let cp = p * u.src.zw / u.src.xy;
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
    var lin = vec3(R, G, B);
    if u.p0.w < 0.5 {
        lin = srgb_to_linear(clamp(lin, vec3(0.0), vec3(1.0)));
    }
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
    let base = (n * 3u + ch) * TONE_N;
    return mix(tones[base + i], tones[base + j], p - floor(p));
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
        var dist: f32;
        if nf(n, o) > 0.5 {
            dist = (length(q / r) - 1.0) * min(r.x, r.y);
        } else {
            dist = max(abs(q.x) - r.x, abs(q.y) - r.y);
        }
        var m: f32;
        if soft <= 1e-3 {
            m = select(0.0, 1.0, dist <= 0.0);
        } else {
            m = clamp(0.5 - dist / soft, 0.0, 1.0);
        }
        if nf(n, o + 8u) > 0.5 {
            m = 1.0 - m;
        }
        m *= nf(n, o + 7u);
        acc = acc + m - acc * m;
    }
    return acc;
}

// The grade on one pixel's signal (0..1), at source position sp (`Grade::apply`, node by node).
fn grade(v_in: vec3<f32>, sp: vec2<f32>) -> vec3<f32> {
    var v = clamp(v_in, vec3(0.0), vec3(1.0));
    let count = u32(u.p3.y);
    for (var n = 0u; n < count; n++) {
        let inp = v;
        var o = inp;
        if nf(n, 0u) > 0.5 {
            o = vec3(tone(n, 0u, o.r), tone(n, 1u, o.g), tone(n, 2u, o.b));
        }
        let amount = nf(n, 1u);
        let boost = nf(n, 2u);
        let turns = nf(n, 3u);
        if abs(amount - 1.0) > 1e-6 || abs(boost) > 1e-6 || abs(turns) > 1e-6 {
            let l = luma(o);
            let spread = clamp(max(max(o.r, o.g), o.b) - min(min(o.r, o.g), o.b), 0.0, 1.0);
            let s = amount * (1.0 + boost * (1.0 - spread));
            o = clamp(vec3(l) + (o - vec3(l)) * s, vec3(0.0), vec3(1.0));
            if abs(turns) > 1e-6 {
                var h = rgb_to_hsl(o);
                h.x = h.x + turns - floor(h.x + turns);
                o = clamp(hsl_to_rgb(h), vec3(0.0), vec3(1.0));
            }
        }
        var k = 1.0;
        if nf(n, 4u) > 0.5 {
            k = key(n, inp);
        }
        k *= coverage(n, sp);
        v = inp + (o - inp) * k;
    }
    return v;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    var c = layer_color(in.sp);
    if u.p3.y > 0.5 && c.a > 1e-6 {
        let signal = linear_to_srgb(c.rgb / c.a);
        let graded = grade(signal, in.sp);
        c = vec4(srgb_to_linear(graded) * c.a, c.a);
    }
    return c * u.p0.x;
}

// ---- the resolve: the accumulator over black, into the sRGB output

@group(0) @binding(0) var accum: texture_2d<f32>;

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[vi], 0.0, 1.0);
}

@fragment
fn fs_full(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(accum, vec2<i32>(pos.xy), 0);
    return vec4(c.rgb, 1.0);
}
