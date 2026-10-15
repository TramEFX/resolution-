use resolution_engine_api::Engine;
use resolution_grade::Node;

use super::*;

fn demo() -> FilmCraftEngine {
    let mut e = FilmCraftEngine::new();
    e.execute("raw.filmcraft/file.openDemoProject", json!({})).unwrap();
    e.execute("playhead.set", json!({"seconds": 2.0})).unwrap();
    e
}

fn node(controls: Value) -> Node {
    let mut n = Node::default();
    n.patch(&controls).unwrap();
    n
}

/// The frame at the playhead, `side` pixels on its long side, as FilmCraft renders the project
/// itself: through the grades compiled to Lumetri, which these tests are about. `render_frame`
/// draws the grade's own math instead (LOG D-044, B-037).
fn frame(e: &FilmCraftEngine, side: u32) -> Vec<[f32; 3]> {
    let s = &e.session.active_sequence().unwrap().settings;
    let scale = (side as f32 / s.width.max(s.height) as f32).min(1.0);
    let img = e.session.try_render_program_at(scale, e.session.playhead()).unwrap();
    img.over_black_rgba8().chunks(4).map(|p| [p[0], p[1], p[2]].map(|v| f32::from(v) / 255.0)).collect()
}

fn adjust(e: &mut FilmCraftEngine, controls: Value) -> Outcome {
    e.execute("color.adjust", json!({"node": 1, "controls": controls})).unwrap()
}

#[test]
fn lumetris_curves_reproduce_the_tone_functions() {
    // Lumetri samples a curve at 1024 points (FilmCraft's own curve code) and interpolates
    let lumetri = |pts: &[[f32; 2]], x: f32| {
        let lut = filmcraft_engine::render::effects::curve_lut(pts, 1024);
        let p = x.clamp(0.0, 1.0) * 1023.0;
        let (i, j) = (p as usize, (p as usize + 1).min(1023));
        lut[i] + (lut[j] - lut[i]) * (p - i as f32)
    };
    for controls in [
        json!({"lift": {"y": 0.08, "b": 0.04}, "gamma": {"y": 0.15, "r": -0.05}, "gain": {"y": 1.1}}),
        json!({"contrast": 1.4, "pivot": 0.4, "offset": {"r": 28.0}}),
        json!({"gain": {"y": 1.3}, "curves": {"y": [[0.25, 0.2], [0.75, 0.82]]}}),
    ] {
        let n = node(controls.clone());
        let e = &compile(&Grade { nodes: vec![n.clone()] }, (1920, 1080))[0];
        for (k, id) in ["curve_red", "curve_green", "curve_blue"].into_iter().enumerate() {
            let pts = e.params[id].value.as_curve().unwrap().to_vec();
            let worst = (0..=500)
                .map(|i| i as f32 / 500.0)
                .map(|x| (lumetri(&pts, x) - n.tone(k, x)).abs())
                .fold(0.0, f32::max);
            assert!(worst < 0.5 / 255.0, "{controls} {id}: off by {worst}");
        }
    }
}

#[test]
fn a_neutral_or_disabled_node_compiles_to_nothing() {
    assert!(compile(&Grade::default(), (1920, 1080)).is_empty());
    let mut n = node(json!({"gain": {"y": 0.5}, "saturation": 60.0}));
    assert_eq!(compile(&Grade { nodes: vec![n.clone()] }, (1920, 1080)).len(), 2, "tone, then saturation");
    n.enabled = false;
    assert!(compile(&Grade { nodes: vec![n] }, (1920, 1080)).is_empty());
    assert_eq!(compile(&Grade { nodes: vec![node(json!({"hue": 60.0}))] }, (1920, 1080)).len(), 1);
}

/// At full size: smaller renders grade at an in-between size and then scale down, and averaging
/// after a curve is not the curve of the average.
#[test]
fn the_rendered_frame_follows_the_grade() {
    let mut e = demo();
    let before = frame(&e, 1920);
    let controls = json!({"lift": {"y": 0.05}, "gamma": {"y": 0.2, "b": -0.05}, "gain": {"y": 0.9}, "contrast": 1.2, "saturation": 70.0});
    adjust(&mut e, controls.clone());
    let after = frame(&e, 1920);
    let n = node(controls);
    let worst = before
        .iter()
        .zip(&after)
        .map(|(b, a)| {
            let want = n.apply(*b);
            (0..3).map(|k| (want[k] - a[k]).abs()).fold(0.0, f32::max)
        })
        .fold(0.0, f32::max);
    assert!(worst <= 1.0 / 255.0, "the render strays from the grade's math by {:.1} code values", worst * 255.0);
    assert!(before.iter().zip(&after).any(|(b, a)| (b[0] - a[0]).abs() > 0.02), "the grade changed the picture");
}

/// The hue curves compiled to Lumetri's HSL curves (LOG D-043): the CPU's render follows the
/// grade's math.
#[test]
fn the_rendered_frame_follows_the_hue_curves() {
    let mut e = demo();
    let before = frame(&e, 1920);
    let controls = json!({
        "saturation": 60.0, "hue": 54.0,
        "curves": {
            "hue_hue": [[0.0, 0.55], [0.33, 0.45], [0.66, 0.5]],
            "hue_sat": [[0.05, 0.2], [0.4, 0.7], [0.75, 0.5]],
            "hue_lum": [[0.1, 0.6], [0.6, 0.35]],
            "lum_sat": [[0.0, 0.3], [0.5, 0.55], [1.0, 0.4]],
            "sat_sat": [[0.2, 0.5], [0.9, 0.8]]
        }
    });
    adjust(&mut e, controls.clone());
    let after = frame(&e, 1920);
    let n = node(controls);
    let mut off: Vec<f32> = before
        .iter()
        .zip(&after)
        .map(|(b, a)| {
            let want = n.apply(*b);
            (0..3).map(|k| (want[k] - a[k]).abs()).fold(0.0, f32::max) * 255.0
        })
        .collect();
    off.sort_by(f32::total_cmp);
    let (p99, worst) = (off[off.len() * 99 / 100], off[off.len() - 1]);
    eprintln!("hue curves through Lumetri: 99% within {p99:.2}, worst {worst:.2} code values");
    // (sampled to the ends' edges: fully saturated colours, many after more Saturation, read
    // Sat vs Sat's end, not Lumetri's join between its ends)
    assert!(p99 <= 1.0 && worst <= 1.5, "99% {p99}, worst {worst}");
    assert!(before.iter().zip(&after).any(|(b, a)| (b[0] - a[0]).abs() > 0.02), "the grade changed the picture");
}

/// Sat vs Lum (LOG D-047), which Lumetri has no curve for: the node is baked into a table, as a
/// qualified node is, and FilmCraft's render follows the math.
#[test]
fn sat_vs_lum_is_baked_into_a_table() {
    let mut e = demo();
    let before = frame(&e, 1920);
    let controls = json!({"saturation": 70.0, "curves": {"sat_lum": [[0.1, 0.5], [0.6, 0.3], [1.0, 0.2]]}});
    adjust(&mut e, controls.clone());
    let n = node(controls);
    assert_eq!(compile(&Grade { nodes: vec![n.clone()] }, (1920, 1080)).len(), 1, "one effect: the table");
    let luts: Vec<&ProjectLut> = e.session.project.luts.iter().filter(|l| l.id.starts_with(LUT_PREFIX)).collect();
    assert_eq!(luts.len(), 1, "one table in the project");
    assert_eq!(luts[0].name, "Resolution node");
    let after = frame(&e, 1920);
    let mut off: Vec<f32> = before
        .iter()
        .zip(&after)
        .map(|(b, a)| {
            let want = n.apply(*b);
            (0..3).map(|k| (want[k] - a[k]).abs()).fold(0.0, f32::max) * 255.0
        })
        .collect();
    off.sort_by(f32::total_cmp);
    let (p99, worst) = (off[off.len() * 99 / 100], off[off.len() - 1]);
    eprintln!("Sat vs Lum through its table: 99% within {p99:.2}, worst {worst:.2} code values");
    // measured 2026-10-14: 99 % within 5.4, worst 7.0, a table being coarse where saturation moves
    // fast: the pixels more than 3 off are darker than the frame (lightness 0.17 against 0.30)
    assert!(p99 <= 6.0 && worst <= 8.0, "99% {p99}, worst {worst}");
    assert!(before.iter().zip(&after).any(|(b, a)| (b[0] - a[0]).abs() > 0.05), "the grade changed the picture");
}

#[test]
fn gain_zero_is_black_and_saturation_zero_is_grey() {
    let mut e = demo();
    adjust(&mut e, json!({"gain": {"y": 0.0}}));
    assert!(frame(&e, 96).iter().all(|p| p.iter().all(|v| *v <= 2.0 / 255.0)), "gain 0 is black");
    let mut e = demo();
    adjust(&mut e, json!({"saturation": 0.0}));
    assert!(
        frame(&e, 96).iter().all(|p| (p[0] - p[1]).abs() <= 2.0 / 255.0 && (p[1] - p[2]).abs() <= 2.0 / 255.0),
        "saturation 0 is grey"
    );
}

#[test]
fn a_grade_edit_is_one_undo_step_and_a_drag_merges() {
    let mut e = demo();
    let out = adjust(&mut e, json!({"saturation": 60.0}));
    assert_eq!(out.undo_label.as_deref(), Some("Adjust Node"));
    let inspect = |e: &mut FilmCraftEngine| e.execute("color.inspect", json!({})).unwrap().result;
    assert_eq!(inspect(&mut e)["graded"], true);
    assert_eq!(e.undo().as_deref(), Some("Adjust Node"));
    assert_eq!(inspect(&mut e)["graded"], false, "undo takes the grade and its effects away together");
    // a drag: calls with one merge key in a row are one step
    for sat in [55.0, 60.0, 65.0, 70.0] {
        e.execute("color.adjust", json!({"node": 1, "controls": {"saturation": sat}, "merge": "sat"})).unwrap();
    }
    assert_eq!(inspect(&mut e)["nodes"][0]["saturation"], 70.0);
    e.undo();
    assert_eq!(inspect(&mut e)["graded"], false, "one undo takes the whole drag back");
}

#[test]
fn nodes_are_added_toggled_and_reset_by_number() {
    let mut e = demo();
    e.execute("color.addSerialNode", json!({})).unwrap();
    adjust(&mut e, json!({"gain": {"y": 0.5}}));
    e.execute("color.adjust", json!({"node": 2, "controls": {"saturation": 80.0, "label": "Look"}})).unwrap();
    let g = e.execute("color.inspect", json!({})).unwrap().result;
    assert_eq!(g["nodes"].as_array().map(Vec::len), Some(2));
    assert_eq!((g["nodes"][1]["number"].clone(), g["nodes"][1]["label"].clone()), (json!(2), json!("Look")));
    e.execute("color.toggleNodes", json!({"nodes": [1, 2]})).unwrap();
    assert_eq!(e.execute("color.inspect", json!({})).unwrap().result["graded"], false, "both nodes off");
    e.execute("color.toggleAllNodes", json!({})).unwrap();
    e.execute("color.resetNode", json!({"node": 1})).unwrap();
    let g = e.execute("color.inspect", json!({})).unwrap().result;
    assert_eq!((g["nodes"][0]["neutral"].clone(), g["nodes"][1]["enabled"].clone()), (json!(true), json!(true)));
    e.execute("color.resetAll", json!({})).unwrap();
    assert_eq!(e.execute("color.inspect", json!({})).unwrap().result["nodes"].as_array().map(Vec::len), Some(1));
}

#[test]
fn a_split_copy_keeps_the_grade() {
    let mut e = demo();
    adjust(&mut e, json!({"gamma": {"y": 0.3}}));
    e.execute("playhead.set", json!({"seconds": 1.0})).unwrap();
    e.execute("timeline.split", json!({})).unwrap();
    // both halves, by the playhead before and after the cut
    for at in [0.5, 1.5] {
        e.execute("playhead.set", json!({"seconds": at})).unwrap();
        let g = e.execute("color.inspect", json!({})).unwrap().result;
        assert_eq!(g["graded"], true, "at {at} s: {g}");
        assert_eq!(g["foreign"], false);
        assert_eq!(g["nodes"][0]["gamma"]["y"], 0.3);
    }
}

#[test]
fn mistakes_are_refused() {
    let mut e = demo();
    let bad = |r: Result<Outcome>| matches!(r, Err(EngineError::BadParams { .. }));
    assert!(bad(e.execute("color.adjust", json!({"node": 2, "controls": {}}))), "no node 2");
    assert!(bad(e.execute("color.adjust", json!({"node": 1, "controls": {"saturaton": 5}}))), "a typo");
    assert!(bad(e.execute("color.adjust", json!({"node": 1}))), "no controls");
    assert!(bad(e.execute("color.inspect", json!({"clip": 999_999}))));
    e.execute("playhead.set", json!({"seconds": 3600.0})).unwrap();
    assert!(bad(e.execute("color.inspect", json!({}))), "no clip under the playhead");
}

/// The demo frame's pixel at `(x, y)` of 1920 x 1080.
fn at(f: &[[f32; 3]], x: usize, y: usize) -> [f32; 3] {
    f[y * 1920 + x]
}

#[test]
fn a_window_limits_the_node_to_its_shape() {
    let mut e = demo();
    let before = frame(&e, 1920);
    let circle = json!({"shape": "circle", "enabled": true, "invert": false, "center": [0.5, 0.5], "size": [0.3, 0.3],
        "rotation": 0.0, "softness": 0.0, "opacity": 100.0});
    let out = adjust(&mut e, json!({"gain": {"y": 0.0}, "windows": [circle.clone()]}));
    assert_eq!((out.result["width"].clone(), out.result["height"].clone()), (json!(1920), json!(1080)));
    let inside = frame(&e, 1920);
    assert!(at(&inside, 960, 540).iter().all(|v| *v < 0.01), "black inside the circle");
    assert_eq!(at(&inside, 30, 30), at(&before, 30, 30), "untouched outside it");
    // the circle is round: 0.3 of the width is 576 pixels across, so 260 above the centre is inside
    assert!(at(&inside, 960, 540 - 260).iter().all(|v| *v < 0.01), "round, not squashed to the frame's height");
    let mut inverted = circle;
    inverted["invert"] = json!(true);
    adjust(&mut e, json!({"windows": [inverted]}));
    let outside = frame(&e, 1920);
    assert_eq!(at(&outside, 960, 540), at(&before, 960, 540), "inverted: untouched inside");
    assert!(at(&outside, 30, 30).iter().all(|v| *v < 0.01), "and black outside");
}

/// A Gradient window (LOG D-052) through the compile: the half above its line, a quarter turn
/// clockwise the right half.
#[test]
fn a_gradient_window_compiles_to_its_half_of_the_picture() {
    let mut e = demo();
    let before = frame(&e, 1920);
    let gradient = json!({"shape": "gradient", "enabled": true, "invert": false, "center": [0.5, 0.5], "size": [1.0, 1.0],
        "rotation": 0.0, "softness": 0.0, "opacity": 100.0});
    adjust(&mut e, json!({"gain": {"y": 0.0}, "windows": [gradient.clone()]}));
    let f = frame(&e, 1920);
    assert!(at(&f, 960, 300).iter().all(|v| *v < 0.01), "black above the line");
    assert_eq!(at(&f, 960, 800), at(&before, 960, 800), "untouched below it");
    let mut turned = gradient;
    turned["rotation"] = json!(90.0);
    adjust(&mut e, json!({"windows": [turned]}));
    let f = frame(&e, 1920);
    assert!(at(&f, 1500, 540).iter().all(|v| *v < 0.01), "a quarter turn: black on the right");
    assert_eq!(at(&f, 400, 540), at(&before, 400, 540), "untouched on the left");
}

#[test]
fn a_turned_linear_window_turns_its_rectangle() {
    let w = |rotation: f32| {
        let mut n = node(json!({"gain": {"y": 0.5}}));
        let mut win = Window::new(Shape::Linear);
        win.rotation = rotation;
        n.windows = vec![win];
        compile(&Grade { nodes: vec![n] }, (1920, 1080))
    };
    let corners = |e: &[EffectInstance]| match &e[0].masks[0].path.value {
        ParamValue::Path(p) => p.vertices.iter().map(|v| (v.p.x.round(), v.p.y.round())).collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    assert_eq!(corners(&w(0.0))[0], (576.0, 300.0), "0.4 x 0.25 of the width, centred");
    // a quarter turn clockwise: the top-left corner, 384 left of the centre and 240 above it,
    // goes to 240 right of it and 384 above
    let turned = corners(&w(90.0));
    assert_eq!(turned[0], (1200.0, 156.0), "{turned:?}");
}

/// The most orange and the most blue pixels of a frame, by index.
fn orange_and_blue(f: &[[f32; 3]]) -> (usize, usize) {
    let best =
        |score: fn(&[f32; 3]) -> f32| (0..f.len()).max_by(|a, b| score(&f[*a]).total_cmp(&score(&f[*b]))).unwrap();
    (best(|p| p[0] - p[2]), best(|p| p[2] - p[0]))
}

#[test]
fn a_qualifier_limits_the_node_to_the_colours_it_keys() {
    let mut e = demo();
    let before = frame(&e, 1920);
    let (orange, blue) = orange_and_blue(&before);
    let q = resolution_grade::Qualifier::picked(before[orange]);
    adjust(&mut e, json!({"gain": {"y": 0.0}, "qualifier": q}));
    let after = frame(&e, 1920);
    assert!(after[orange].iter().all(|v| *v < 0.05), "the keyed orange goes black: {:?}", after[orange]);
    let d = (0..3).map(|k| (after[blue][k] - before[blue][k]).abs()).fold(0.0, f32::max);
    assert!(d <= 1.0 / 255.0, "the blue is untouched (off by {d})");
    // one LUT in the project carries the node
    let luts: Vec<String> =
        e.session.project.luts.iter().map(|l| l.id.clone()).filter(|id| id.starts_with(LUT_PREFIX)).collect();
    assert_eq!(luts.len(), 1, "{luts:?}");
    // turning the qualifier off drops the LUT and grades everything
    adjust(&mut e, json!({"qualifier": {"enabled": false}}));
    assert!(e.session.project.luts.iter().all(|l| !l.id.starts_with(LUT_PREFIX)), "the unused LUT is dropped");
    assert!(frame(&e, 96).iter().all(|p| p.iter().all(|v| *v <= 2.0 / 255.0)), "gain 0 everywhere");
    // undo brings the qualifier and its LUT back together
    e.undo();
    assert_eq!(e.session.project.luts.iter().filter(|l| l.id.starts_with(LUT_PREFIX)).count(), 1);
}

#[test]
fn moving_a_window_keeps_a_qualified_nodes_lut() {
    let mut n = node(json!({"gain": {"y": 0.5}}));
    n.qualifier = Some(resolution_grade::Qualifier::default());
    let mut moved = n.clone();
    moved.windows = vec![Window::new(Shape::Circle)];
    moved.label = "Skin".into();
    assert_eq!(lut_id(&n), lut_id(&moved), "the table does not depend on windows or the label");
    let mut other = n.clone();
    other.gain.y = 0.6;
    assert_ne!(lut_id(&n), lut_id(&other));
    let compiled = compile(&Grade { nodes: vec![moved] }, (1920, 1080));
    assert_eq!(compiled.len(), 1, "one effect: the LUT");
    assert_eq!(compiled[0].masks.len(), 1, "with the window as its mask");
}

/// How closely a qualified node's 33-point LUT follows the node's own math over a whole frame:
/// interpolation strays most where the key's soft edge runs between table points.
#[test]
fn a_qualified_nodes_lut_follows_its_math() {
    let mut e = demo();
    let before = frame(&e, 1920);
    let (orange, _) = orange_and_blue(&before);
    let controls = json!({"gamma": {"y": 0.3}, "saturation": 80.0, "qualifier": resolution_grade::Qualifier::picked(before[orange])});
    adjust(&mut e, controls.clone());
    let after = frame(&e, 1920);
    let n = node(controls);
    let mut errs: Vec<f32> = before
        .iter()
        .zip(&after)
        .map(|(b, a)| {
            let want = n.apply(*b);
            (0..3).map(|k| (want[k] - a[k]).abs()).fold(0.0, f32::max) * 255.0
        })
        .collect();
    errs.sort_by(f32::total_cmp);
    let pct = |p: f32| errs[((errs.len() - 1) as f32 * p) as usize];
    // measured 2026-10-14 (LOG D-032): median 0, 99 % 2.3, 99.9 % 5.1, worst 6.5 code values
    assert_eq!(pct(0.5), 0.0, "most pixels exact");
    assert!(pct(0.99) <= 3.0, "99 % within three code values: {}", pct(0.99));
    assert!(pct(1.0) <= 8.0, "none more than eight off: {}", pct(1.0));
}
