use filmcraft_engine::render::render_sequence;
use resolution_engine_api::Engine;
use serde_json::{Value, json};

use super::*;
use crate::FilmCraftEngine;

fn demo() -> FilmCraftEngine {
    let mut e = FilmCraftEngine::new();
    e.execute("raw.filmcraft/file.openDemoProject", json!({})).unwrap();
    e.execute("playhead.set", json!({"seconds": 2.0})).unwrap();
    e
}

/// The frame at the playhead drawn both ways at a 960-pixel viewer: the GPU renderer's (from
/// the graded copy of the timeline) and FilmCraft's CPU renderer's (from the project, grades
/// compiled to Lumetri). `None` on a machine without a GPU.
fn both(e: &FilmCraftEngine) -> Option<(Frame, Frame)> {
    both_at(e, 960.0)
}

fn both_at(e: &FilmCraftEngine, side: f32) -> Option<(Frame, Frame)> {
    if renderer().is_none() {
        eprintln!("no GPU here: skipped");
        return None;
    }
    let s = &e.session;
    let seq = s.state.active_sequence.unwrap();
    let t = s.playhead();
    let (w, h) = s.project.sequence(seq).map(|q| (q.settings.width, q.settings.height)).unwrap();
    let opts = RenderOptions { scale: side / w.max(h) as f32, captions: true, ..Default::default() };
    let provider = s.media.provider(s.project.clone(), s.services.clone());
    let gpu = Graded::of(&s.project, seq).render(seq, t, opts, &provider).expect("the GPU draws this frame");
    let img = render_sequence(&s.project, seq, t, opts, &provider);
    Some((gpu, Frame { width: img.w as u32, height: img.h as u32, rgba: img.over_black_rgba8() }))
}

/// How far two frames differ, in code values: the mean, and the 99th percentile.
fn differ(a: &Frame, b: &Frame) -> (f32, f32) {
    assert_eq!((a.width, a.height), (b.width, b.height));
    let mut d: Vec<f32> = a
        .rgba
        .chunks(4)
        .zip(b.rgba.chunks(4))
        .map(|(p, q)| (0..3).map(|k| f32::from(p[k].abs_diff(q[k]))).fold(0.0, f32::max))
        .collect();
    let mean = d.iter().sum::<f32>() / d.len() as f32;
    d.sort_by(f32::total_cmp);
    (mean, d[d.len() * 99 / 100])
}

/// How far each colour channel of `a` leans from `b` on average, in code values.
fn lean(a: &Frame, b: &Frame) -> [f32; 3] {
    let n = (a.width * a.height) as f32;
    [0, 1, 2].map(|k| {
        a.rgba.chunks(4).zip(b.rgba.chunks(4)).map(|(p, q)| f32::from(p[k]) - f32::from(q[k])).sum::<f32>() / n
    })
}

fn adjust(e: &mut FilmCraftEngine, controls: Value) {
    e.execute("color.adjust", json!({"node": 1, "controls": controls})).unwrap();
}

#[test]
fn the_gpu_draws_the_demo_as_the_cpu_does() {
    let e = demo();
    let Some((gpu, cpu)) = both(&e) else { return };
    let (mean, p99) = differ(&gpu, &cpu);
    eprintln!("ungraded: mean {mean:.2}, 99% {p99:.1} code values");
    assert!(mean < 1.0 && p99 <= 3.0, "mean {mean}, 99% {p99}");
}

#[test]
fn a_graded_clip_draws_on_the_gpu_as_its_compiled_grade_does() {
    let mut e = demo();
    adjust(
        &mut e,
        json!({"lift": {"y": 0.04}, "gamma": {"y": 0.2, "b": -0.04}, "gain": {"y": 0.9}, "contrast": 1.2, "saturation": 70.0}),
    );
    let Some((gpu, cpu)) = both(&e) else { return };
    let (mean, p99) = differ(&gpu, &cpu);
    eprintln!("graded: mean {mean:.2}, 99% {p99:.1} code values");
    assert!(mean < 1.0 && p99 <= 3.0, "mean {mean}, 99% {p99}");
    let before = frame_counts();
    let _ = both(&e);
    assert!(frame_counts().0 > before.0, "counted as a GPU frame");
}

#[test]
fn a_qualifier_draws_on_the_gpu_close_to_its_baked_lut() {
    let mut e = demo();
    adjust(
        &mut e,
        json!({"gain": {"y": 0.5}, "saturation": 40.0, "qualifier": resolution_grade::Qualifier::picked([0.8, 0.4, 0.2])}),
    );
    let Some((gpu, cpu)) = both(&e) else { return };
    let (mean, p99) = differ(&gpu, &cpu);
    eprintln!("qualified: mean {mean:.2}, 99% {p99:.1} code values");
    // The CPU's baked LUT strays from the key's math by up to 6.5 code values at the key's soft
    // edge (LOG D-033), and at a viewer's size the CPU grades before shrinking the picture while
    // the GPU grades after: on a steep key edge the two differ. The GPU's own accuracy against
    // the math is tested in `resolution-renderer` (within a code value).
    assert!(mean < 1.0 && p99 <= 10.0, "mean {mean}, 99% {p99}");
}

#[test]
fn a_frame_with_a_foreign_lumetri_falls_back_to_the_cpu() {
    let e = demo();
    let s = &e.session;
    let seq = s.state.active_sequence.unwrap();
    // the demo's Desert Dunes clip carries a Lumetri of its own, not a Resolution grade
    let (clip, start) = s
        .project
        .sequence(seq)
        .and_then(|q| {
            q.video_tracks
                .iter()
                .flat_map(|t| &t.items)
                .find(|it| it.effects.first().is_some_and(|f| f.effect == "lumetri"))
                .map(|it| (it.id, it.start))
        })
        .unwrap();
    let graded = Graded::of(&s.project, seq);
    assert!(!graded.grades.contains_key(&clip), "a foreign Lumetri is not a grade");
    let provider = s.media.provider(s.project.clone(), s.services.clone());
    let opts = RenderOptions { scale: 0.25, captions: true, ..Default::default() };
    if renderer().is_some() {
        assert!(
            graded.render(seq, start, opts, &provider).is_none(),
            "its Lumetri runs on the CPU: the whole frame does"
        );
    }
}

/// Camera video is Y'CbCr, which the demo's pictures are not (LOG B-035).
#[test]
fn camera_video_draws_on_the_gpu_as_on_the_cpu() {
    let mut e = FilmCraftEngine::new();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/rotated.mp4");
    let imported = e.execute("media.import", json!({ "paths": [path] })).unwrap();
    let item = imported.result["items"][0].clone();
    e.execute("timeline.createFromClips", json!({ "items": [item] })).unwrap();
    e.execute("playhead.set", json!({"seconds": 0.5})).unwrap();
    {
        let s = &e.session;
        let seq = s.state.active_sequence.unwrap();
        let provider = s.media.provider(s.project.clone(), s.services.clone());
        let plan = plan_frame(&s.project, seq, s.playhead(), RenderOptions::default(), &provider);
        let FramePlan::Layers { layers, .. } = plan else { panic!("a plain frame") };
        assert!(matches!(layers[0].frame.data, PixelData::Yuv8 { .. }), "the decoder hands over Y'CbCr");
    }
    let Some((gpu, cpu)) = both_at(&e, 640.0) else { return };
    // The clip is a test pattern of hard-edged colour bars. The CPU takes each pixel's nearest
    // chroma sample and the GPU blends them, so along the edges they differ by design: compare
    // the flat parts, where the colour conversion alone decides the picture.
    let (w, h) = (cpu.width as usize, cpu.height as usize);
    let at = |f: &Frame, x: usize, y: usize, k: usize| f.rgba[(y * w + x) * 4 + k];
    let flat = |x: usize, y: usize| {
        (0..3).all(|k| {
            let around = (y - 2..=y + 2).flat_map(|yy| (x - 2..=x + 2).map(move |xx| (xx, yy)));
            let (lo, hi) = around.fold((255u8, 0u8), |(lo, hi), (xx, yy)| {
                let v = at(&cpu, xx, yy, k);
                (lo.min(v), hi.max(v))
            });
            hi - lo <= 2
        })
    };
    let (mut worst, mut n) = (0u8, 0usize);
    for y in 2..h - 2 {
        for x in 2..w - 2 {
            if flat(x, y) {
                n += 1;
                worst = worst.max((0..3).map(|k| at(&gpu, x, y, k).abs_diff(at(&cpu, x, y, k))).max().unwrap_or(0));
            }
        }
    }
    let lean = lean(&gpu, &cpu);
    eprintln!("camera video: {n} flat pixels, worst {worst}; lean {lean:.2?} code values");
    assert!(n > w * h / 2, "most of the picture is flat");
    assert!(worst <= 2, "the flat parts agree: worst {worst}");
    assert!(lean.iter().all(|v| v.abs() < 0.25), "no colour leans: {lean:?}");
}
