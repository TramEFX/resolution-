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

/// The frame at the playhead, `side` pixels on its long side.
fn frame(e: &FilmCraftEngine, side: u32) -> Vec<[f32; 3]> {
    let f = e.render_frame(None, side).unwrap();
    f.rgba.chunks(4).map(|p| [p[0], p[1], p[2]].map(|v| f32::from(v) / 255.0)).collect()
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
        let e = &compile(&Grade { nodes: vec![n.clone()] })[0];
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
    assert!(compile(&Grade::default()).is_empty());
    let mut n = node(json!({"gain": {"y": 0.5}, "saturation": 60.0}));
    assert_eq!(compile(&Grade { nodes: vec![n.clone()] }).len(), 2, "tone, then saturation");
    n.enabled = false;
    assert!(compile(&Grade { nodes: vec![n] }).is_empty());
    assert_eq!(compile(&Grade { nodes: vec![node(json!({"hue": 60.0}))] }).len(), 1);
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
