use resolution_grade::{Node, Qualifier, Window};
use serde_json::json;

use super::*;

/// The GPU renderer, or `None` (with a note) on a machine without a GPU.
fn renderer() -> Option<Renderer> {
    let r = Renderer::new();
    if r.is_none() {
        eprintln!("no GPU here: skipped");
    }
    r
}

const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// An opaque 8-bit layer of `px` (RGB triples), `w` × `h`, drawn as it is.
fn rgba_layer<'a>(w: u32, h: u32, rgba: &'a [u8], grade: Option<&'a Grade>) -> Layer<'a> {
    Layer {
        width: w,
        height: h,
        pixels: Pixels::Rgba8(rgba),
        coding: Coding::REC709,
        matrix: IDENTITY,
        opacity: 1.0,
        blend: Blend::Normal,
        managed: None,
        grade,
    }
}

/// Deterministic pseudo-random colours, opaque.
fn noise(n: usize) -> Vec<u8> {
    let mut s = 0x2545_f491u32;
    (0..n)
        .flat_map(|_| {
            let mut c = [0u8; 4];
            for v in c.iter_mut().take(3) {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                *v = (s >> 24) as u8;
            }
            c[3] = 255;
            c
        })
        .collect()
}

fn node(controls: serde_json::Value) -> Node {
    let mut n = Node::default();
    n.patch(&controls).unwrap();
    n
}

/// The largest difference, in code values, between the GPU's frame and `want` per input pixel.
fn worst(input: &[u8], out: &[u8], want: impl Fn([f32; 3]) -> [f32; 3]) -> f32 {
    input
        .chunks(4)
        .zip(out.chunks(4))
        .map(|(i, o)| {
            let w = want([i[0], i[1], i[2]].map(|v| f32::from(v) / 255.0));
            (0..3).map(|k| (w[k] * 255.0 - f32::from(o[k])).abs()).fold(0.0, f32::max)
        })
        .fold(0.0, f32::max)
}

#[test]
fn an_rgba_picture_comes_back_as_it_went_in() {
    let Some(r) = renderer() else { return };
    let px = noise(64 * 36);
    let out = r.render((64, 36), &[rgba_layer(64, 36, &px, None)]).unwrap();
    assert!(worst(&px, &out, |c| c) <= 1.0, "off by {}", worst(&px, &out, |c| c));
}

#[test]
fn video_range_rec709_decodes_to_white_black_and_red() {
    let Some(r) = renderer() else { return };
    for ((y, cb, cr), want) in
        [((235u8, 128u8, 128u8), [255u8, 255, 255]), ((16, 128, 128), [0, 0, 0]), ((63, 102, 240), [255, 0, 0])]
    {
        let (w, h) = (8u32, 8u32);
        let luma = vec![y; 64];
        let (u, v) = (vec![cb; 16], vec![cr; 16]);
        let layer = Layer {
            width: w,
            height: h,
            pixels: Pixels::Yuv8 { planes: [&luma, &u, &v], chroma: (1, 1), alpha: None },
            coding: Coding::REC709,
            matrix: IDENTITY,
            opacity: 1.0,
            blend: Blend::Normal,
            managed: None,
            grade: None,
        };
        let out = r.render((w, h), &[layer]).unwrap();
        let got = [out[0], out[1], out[2]];
        assert!(
            got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2),
            "Y'CbCr {y} {cb} {cr} gave {got:?}, not {want:?}"
        );
    }
}

/// Y'CbCr codes spread over the mid-tones, `n` × `n`, at `bits`; and the R'G'B' codes (0 to 255)
/// the conversion's formula gives for them (FilmCraft's `ycbcr_to_rgb`).
fn ycbcr_sweep(n: usize, bits: u32, full: bool) -> ([Vec<u16>; 3], Vec<[f32; 3]>) {
    let levels = (1u32 << bits) as f32;
    let s = levels / 256.0;
    let (mut y, mut cb, mut cr, mut want) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for i in 0..n * n {
        let (fy, fc, fr) = ((i % n) as f32 / n as f32, (i / n) as f32 / n as f32, ((i * 7) % n) as f32 / n as f32);
        let (yc, bc, rc) = ((40.0 + 180.0 * fy) * s, (96.0 + 64.0 * fc) * s, (96.0 + 64.0 * fr) * s);
        let (yc, bc, rc) = (yc.round(), bc.round(), rc.round());
        let (yn, bn, rn) = if full {
            (yc / (levels - 1.0), (bc - levels / 2.0) / (levels - 1.0), (rc - levels / 2.0) / (levels - 1.0))
        } else {
            ((yc - 16.0 * s) / (219.0 * s), (bc - 128.0 * s) / (224.0 * s), (rc - 128.0 * s) / (224.0 * s))
        };
        let (kr, kb) = (0.2126f32, 0.0722f32);
        let r = yn + 2.0 * (1.0 - kr) * rn;
        let b = yn + 2.0 * (1.0 - kb) * bn;
        let g = (yn - kr * r - kb * b) / (1.0 - kr - kb);
        want.push([r, g, b].map(|v| v.clamp(0.0, 1.0) * 255.0));
        y.push(yc as u16);
        cb.push(bc as u16);
        cr.push(rc as u16);
    }
    ([y, cb, cr], want)
}

/// The largest difference from `want` and the mean signed one, in code values.
fn off(out: &[u8], want: &[[f32; 3]]) -> (f32, f32) {
    let d: Vec<f32> = out.chunks(4).zip(want).flat_map(|(o, w)| (0..3).map(move |k| f32::from(o[k]) - w[k])).collect();
    (d.iter().fold(0.0, |m, v| v.abs().max(m)), d.iter().sum::<f32>() / d.len() as f32)
}

#[test]
fn ycbcr_mid_tones_decode_as_the_formula_says() {
    let Some(r) = renderer() else { return };
    let n = 16;
    for (bits, full) in [(8, false), (8, true), (10, false), (10, true)] {
        let ([y, cb, cr], want) = ycbcr_sweep(n, bits, full);
        let coding = Coding { full_range: full, ..Coding::REC709 };
        let (y8, cb8, cr8): (Vec<u8>, Vec<u8>, Vec<u8>) = (
            y.iter().map(|v| *v as u8).collect(),
            cb.iter().map(|v| *v as u8).collect(),
            cr.iter().map(|v| *v as u8).collect(),
        );
        let pixels = if bits == 8 {
            Pixels::Yuv8 { planes: [&y8, &cb8, &cr8], chroma: (0, 0), alpha: None }
        } else {
            Pixels::Yuv16 { planes: [&y, &cb, &cr], chroma: (0, 0), bits, alpha: None }
        };
        let layer = Layer {
            width: n as u32,
            height: n as u32,
            pixels,
            coding,
            matrix: IDENTITY,
            opacity: 1.0,
            blend: Blend::Normal,
            managed: None,
            grade: None,
        };
        let out = r.render((n as u32, n as u32), &[layer]).unwrap();
        let (worst, bias) = off(&out, &want);
        eprintln!("{bits}-bit, full range {full}: worst {worst:.2}, mean {bias:.3} code values");
        // within rounding, and no lean either way (a half-code chroma offset leans about one)
        assert!(worst <= 1.0 && bias.abs() < 0.15, "{bits}-bit, full {full}: worst {worst}, mean {bias}");
    }
}

#[test]
fn ten_bit_white_is_white() {
    let Some(r) = renderer() else { return };
    let luma = vec![940u16; 64];
    let c = vec![512u16; 16];
    let layer = Layer {
        width: 8,
        height: 8,
        pixels: Pixels::Yuv16 { planes: [&luma, &c, &c], chroma: (1, 1), bits: 10, alpha: None },
        coding: Coding::REC709,
        matrix: IDENTITY,
        opacity: 1.0,
        blend: Blend::Normal,
        managed: None,
        grade: None,
    };
    let out = r.render((8, 8), &[layer]).unwrap();
    assert!(out[..3].iter().all(|v| *v >= 253), "{:?}", &out[..3]);
}

#[test]
fn the_grade_matches_its_own_math() {
    let Some(r) = renderer() else { return };
    let px = noise(64 * 64);
    let n = node(json!({
        "lift": {"y": 0.05, "b": 0.02}, "gamma": {"y": 0.15}, "gain": {"y": 1.1, "r": 1.05}, "contrast": 1.2,
        "saturation": 70.0, "hue": 55.0, "color_boost": 20.0, "curves": {"y": [[0.3, 0.25], [0.7, 0.78]]}
    }));
    let g = Grade { nodes: vec![n.clone(), node(json!({"temp": 900.0, "offset": {"y": 23.0}}))] };
    let out = r.render((64, 64), &[rgba_layer(64, 64, &px, Some(&g))]).unwrap();
    let e = worst(&px, &out, |c| g.apply(c));
    assert!(e <= 1.0, "the GPU strays from the grade's math by {e} code values");
}

/// The hue curves (LOG D-043), all five at once with Hue's rotation, on noise.
#[test]
fn the_hue_curves_match_their_own_math() {
    let Some(r) = renderer() else { return };
    let px = noise(64 * 64);
    let n = node(json!({
        "saturation": 60.0, "hue": 54.0,
        "curves": {
            "hue_hue": [[0.0, 0.55], [0.33, 0.45], [0.66, 0.5]],
            "hue_sat": [[0.05, 0.2], [0.4, 0.7], [0.75, 0.5]],
            "hue_lum": [[0.1, 0.6], [0.6, 0.35]],
            "lum_sat": [[0.0, 0.3], [0.5, 0.55], [1.0, 0.4]],
            "sat_sat": [[0.2, 0.5], [0.9, 0.8]]
        }
    }));
    let g = Grade { nodes: vec![n] };
    let out = r.render((64, 64), &[rgba_layer(64, 64, &px, Some(&g))]).unwrap();
    let e = worst(&px, &out, |c| g.apply(c));
    assert!(e <= 1.0, "the GPU strays from the hue curves' math by {e} code values");
}

#[test]
fn a_qualifier_is_a_real_key_on_the_gpu() {
    let Some(r) = renderer() else { return };
    let px = noise(64 * 64);
    let mut n = node(json!({"gain": {"y": 0.0}, "saturation": 30.0}));
    n.qualifier = Some(Qualifier { hue_width: 30.0, ..Qualifier::picked([0.8, 0.4, 0.2]) });
    let g = Grade { nodes: vec![n] };
    let out = r.render((64, 64), &[rgba_layer(64, 64, &px, Some(&g))]).unwrap();
    let e = worst(&px, &out, |c| g.apply(c));
    assert!(e <= 1.0, "the key strays from its math by {e} code values");
    // and it did key something
    assert!(px.chunks(4).zip(out.chunks(4)).any(|(i, o)| i[0] > 100 && o[0] < 5), "some oranges went black");
}

#[test]
fn a_window_limits_the_grade_to_its_shape() {
    let Some(r) = renderer() else { return };
    let (w, h) = (200u32, 100u32);
    let px: Vec<u8> = (0..w * h).flat_map(|_| [128u8, 128, 128, 255]).collect();
    let mut n = node(json!({"gain": {"y": 0.0}}));
    // 0.3 of the width across: a radius of 30 pixels about the middle
    n.windows = vec![Window { softness: 0.0, ..Window::new(resolution_grade::Shape::Circle) }];
    n.windows[0].size = [0.3, 0.3];
    let g = Grade { nodes: vec![n] };
    let out = r.render((w, h), &[rgba_layer(w, h, &px, Some(&g))]).unwrap();
    let at = |x: u32, y: u32| out[((y * w + x) * 4) as usize];
    assert!(at(100, 50) < 3, "black in the middle");
    assert!(at(100, 50 - 25) < 3, "and 25 pixels above it: round on a 2:1 picture");
    assert!(at(5, 5).abs_diff(128) <= 1 && at(100, 50 - 35).abs_diff(128) <= 1, "untouched outside");
}

#[test]
fn layers_go_over_one_another_with_their_opacity() {
    let Some(r) = renderer() else { return };
    let red: Vec<u8> = (0..16).flat_map(|_| [255u8, 0, 0, 255]).collect();
    let blue: Vec<u8> = (0..16).flat_map(|_| [0u8, 0, 255, 255]).collect();
    let mut top = rgba_layer(4, 4, &blue, None);
    top.opacity = 0.5;
    let out = r.render((4, 4), &[rgba_layer(4, 4, &red, None), top]).unwrap();
    // half and half in linear light: 0.5 of each, which is 188 in sRGB
    assert!(out[0].abs_diff(188) <= 2 && out[2].abs_diff(188) <= 2 && out[1] <= 1, "{:?}", &out[..3]);
}

#[test]
fn a_shrunk_picture_averages_its_pixels() {
    let Some(r) = renderer() else { return };
    // a fine checkerboard of black and white, drawn at a quarter of its size
    let px: Vec<u8> =
        (0..64 * 64).flat_map(|i| if (i % 64 + i / 64) % 2 == 0 { [255u8; 4] } else { [0, 0, 0, 255] }).collect();
    let mut l = rgba_layer(64, 64, &px, None);
    l.matrix = [0.25, 0.0, 0.0, 0.25, 0.0, 0.0];
    let out = r.render((16, 16), &[l]).unwrap();
    // the mean in linear light (0.5) in sRGB is 188
    assert!(out.chunks(4).all(|p| p[0].abs_diff(188) <= 8), "{:?}", &out[..12]);
}

#[test]
fn an_enlarged_picture_blends_its_pixels_in_linear_light() {
    let Some(r) = renderer() else { return };
    // black then white, drawn twice as wide: the middle pixels are a quarter and three quarters
    // of the way across, which in linear light are 137 and 225 in sRGB (not 64 and 191)
    let px = [0u8, 0, 0, 255, 255, 255, 255, 255];
    let mut l = rgba_layer(2, 1, &px, None);
    l.matrix = [2.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let out = r.render((4, 1), &[l]).unwrap();
    assert!(out[4].abs_diff(137) <= 2 && out[8].abs_diff(225) <= 2, "{out:?}");
    // opaque red beside a clear pixel whose (unseen) colour is green: no green bleeds in
    let px = [255u8, 0, 0, 255, 0, 255, 0, 0];
    let mut l = rgba_layer(2, 1, &px, None);
    l.matrix = [2.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let out = r.render((4, 1), &[l]).unwrap();
    assert!(out[5] <= 1 && out[9] <= 1, "{out:?}");
}

#[test]
fn a_frame_left_on_the_gpu_reads_back_as_rendered() {
    let Some(r) = renderer() else { return };
    let px = noise(32 * 16);
    let mut l = rgba_layer(32, 16, &px, None);
    l.matrix = [0.75, 0.0, 0.0, 0.75, 1.5, 0.5];
    let rendered = r.render((24, 12), &[l]).unwrap();
    let picture = r.draw_steps((24, 12), &[Step::Layers(vec![l])]);
    assert_eq!(picture.size(), (24, 12));
    assert_eq!(picture.read(), Some(&rendered[..]), "the same frame");
    assert_eq!(picture.read(), Some(&rendered[..]), "read once, kept");
}

#[test]
fn a_picture_shrunk_sixteen_times_averages_all_its_pixels() {
    let Some(r) = renderer() else { return };
    // white stripes 2 pixels wide every 8, at each phase, shrunk 16 times: a quarter of the light
    // in every output pixel (137 in sRGB), wherever the stripes fall
    for phase in 0..8u32 {
        let (w, h) = (256u32, 32u32);
        let px: Vec<u8> =
            (0..w * h).flat_map(|i| if ((i % w) + phase) % 8 < 2 { [255u8; 4] } else { [0, 0, 0, 255] }).collect();
        let mut l = rgba_layer(w, h, &px, None);
        l.matrix = [1.0 / 16.0, 0.0, 0.0, 1.0 / 16.0, 0.0, 0.0];
        let out = r.render((16, 2), &[l]).unwrap();
        let worst = out.chunks(4).map(|p| p[0].abs_diff(137)).max().unwrap();
        assert!(worst <= 3, "phase {phase}: {:?}", out.chunks(4).map(|p| p[0]).collect::<Vec<_>>());
    }
}

/// 4:2:0 chroma taken at the left one of each pair of luma columns (as H.264 and HEVC site it by
/// default) is read back where it was taken: a sharp edge between two colours comes back sharp
/// when read sited left, and smeared by a column when read as centred (LOG D-040).
#[test]
fn chroma_sited_left_is_read_where_it_was_taken() {
    let Some(r) = renderer() else { return };
    let (w, h) = (16u32, 4u32);
    // full-resolution Y'CbCr of a red | blue edge after column `edge`, in video-range codes
    let ycc = |rgb: [f32; 3]| {
        let (kr, kb) = (0.2126f32, 0.0722f32);
        let y = kr * rgb[0] + (1.0 - kr - kb) * rgb[1] + kb * rgb[2];
        let cb = (rgb[2] - y) / (2.0 * (1.0 - kb));
        let cr = (rgb[0] - y) / (2.0 * (1.0 - kr));
        [16.0 + 219.0 * y, 128.0 + 224.0 * cb, 128.0 + 224.0 * cr].map(|v| v.round() as u8)
    };
    for edge in [6u32, 7] {
        let colour = |x: u32| if x < edge { [0.8, 0.1, 0.1] } else { [0.1, 0.2, 0.9] };
        let luma: Vec<u8> = (0..h).flat_map(|_| (0..w).map(|x| ycc(colour(x))[0])).collect();
        // chroma sampled at even columns (sited left), every other row
        let cw = w / 2;
        let cb: Vec<u8> = (0..h / 2).flat_map(|_| (0..cw).map(|k| ycc(colour(2 * k))[1])).collect();
        let cr: Vec<u8> = (0..h / 2).flat_map(|_| (0..cw).map(|k| ycc(colour(2 * k))[2])).collect();
        let draw = |siting| {
            let l = Layer {
                width: w,
                height: h,
                pixels: Pixels::Yuv8 { planes: [&luma, &cb, &cr], chroma: (1, 1), alpha: None },
                coding: Coding { siting, ..Coding::REC709 },
                matrix: IDENTITY,
                opacity: 1.0,
                blend: Blend::Normal,
                managed: None,
                grade: None,
            };
            r.render((w, h), &[l]).unwrap()
        };
        // the columns sampled for chroma (even ones) must come back as their own colour
        let error = |out: &[u8]| {
            (0..w)
                .step_by(2)
                .map(|x| {
                    let want = colour(x).map(|v| (v.powf(1.0) * 255.0) as i32);
                    let px = &out[(x * 4) as usize..(x * 4 + 3) as usize];
                    (0..3).map(|k| (i32::from(px[k]) - want[k]).abs()).max().unwrap_or(0)
                })
                .max()
                .unwrap_or(0)
        };
        let (left, centre) = (error(&draw(Siting::Left)), error(&draw(Siting::Center)));
        eprintln!("edge after column {edge}: sited left worst {left}, read as centred worst {centre}");
        assert!(left <= 4, "edge {edge}: read sited left, {left} off");
        assert!(centre > 30, "edge {edge}: read as centred smears the edge ({centre})");
    }
}

#[test]
fn the_linear_canvas_keeps_alpha_and_matches_the_frame() {
    let Some(r) = renderer() else { return };
    // premultiplied linear: an opaque left half, a half-clear right half
    let px: Vec<f32> =
        (0..8 * 4).flat_map(|i| if i % 8 < 4 { [0.6, 0.2, 0.1, 1.0] } else { [0.15, 0.05, 0.0, 0.5] }).collect();
    let l = Layer {
        width: 8,
        height: 4,
        pixels: Pixels::RgbaF32(&px),
        coding: Coding { transfer: Transfer::Linear, ..Coding::REC709 },
        matrix: IDENTITY,
        opacity: 1.0,
        blend: Blend::Normal,
        managed: None,
        grade: None,
    };
    let steps = [Step::Layers(vec![l])];
    let lin = r.render_linear((8, 4), &steps).unwrap();
    assert_eq!(lin.len(), 8 * 4 * 4);
    for (i, p) in lin.chunks(4).enumerate() {
        let want = &px[i * 4..i * 4 + 4];
        assert!(p.iter().zip(want).all(|(a, b)| (a - b).abs() < 2e-3), "pixel {i}: {p:?}, not {want:?}");
    }
    // the 8-bit frame is the canvas over black, encoded
    let frame = r.render_steps((8, 4), &steps).unwrap();
    let enc =
        |v: f32| (if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 } * 255.0).round() as i32;
    for (p, f) in lin.chunks(4).zip(frame.chunks(4)) {
        assert!((0..3).all(|k| (enc(p[k]) - i32::from(f[k])).abs() <= 1), "{p:?} against {f:?}");
    }
}

/// The grade stage (LOG D-046): a qualifier cleaning its key over the picture (Denoise, Clean
/// Black and White, Blur Radius), against the CPU's `Grade::apply_image`, drawn one to one.
#[test]
fn a_matte_cleaned_over_the_picture_matches_its_math() {
    let Some(r) = renderer() else { return };
    // wide enough for Blur Radius to spread a few pixels (it scales with the picture's width)
    let (w, h) = (768u32, 64u32);
    // grey noise, a red block, and red specks in the grey
    let mut px = noise((w * h) as usize);
    for (i, p) in px.chunks_mut(4).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        if (300..460).contains(&x) && (16..48).contains(&y) {
            p[..3].copy_from_slice(&[204, 40, 30]);
        } else if (x * 7 + y * 13) % 41 == 0 {
            p[..3].copy_from_slice(&[210, 30, 30]);
        } else {
            let g = p[0] / 2 + 64;
            p[..3].copy_from_slice(&[g, g, g]);
        }
    }
    let mut q = Qualifier::picked([0.8, 0.16, 0.12]);
    (q.denoise, q.clean_black, q.clean_white, q.blur) = (40.0, 20.0, 20.0, 60.0);
    assert!(q.is_spatial());
    let n = node(json!({ "gain": {"y": 0.6, "b": 1.4}, "saturation": 30.0, "qualifier": q }));
    let g = Grade { nodes: vec![n, node(json!({ "contrast": 1.2 }))] };
    let out = r.render((w, h), &[rgba_layer(w, h, &px, Some(&g))]).unwrap();
    let mut cpu: Vec<[f32; 3]> = px.chunks(4).map(|p| [p[0], p[1], p[2]].map(|v| f32::from(v) / 255.0)).collect();
    g.apply_image(&mut cpu, w as usize, h as usize);
    let worst = cpu
        .iter()
        .zip(out.chunks(4))
        .map(|(c, o)| (0..3).map(|k| (c[k] * 255.0 - f32::from(o[k])).abs()).fold(0.0, f32::max))
        .fold(0.0, f32::max);
    eprintln!("the grade stage against its math: worst {worst:.2} code values");
    assert!(worst <= 1.5, "the GPU's cleaned matte strays from the math by {worst} code values");
    // and the cleaning matters: pixel by pixel (no Denoise, no Blur) is far off
    let plain: Vec<[f32; 3]> =
        px.chunks(4).map(|p| g.apply([p[0], p[1], p[2]].map(|v| f32::from(v) / 255.0))).collect();
    let apart = plain
        .iter()
        .zip(&cpu)
        .map(|(a, b)| (0..3).map(|k| (a[k] - b[k]).abs() * 255.0).fold(0.0, f32::max))
        .fold(0.0, f32::max);
    assert!(apart > 20.0, "Denoise and Blur changed nothing ({apart})");
}
