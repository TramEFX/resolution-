use serde_json::json;

use super::*;

const SAMPLES: [[f32; 3]; 5] =
    [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.2, 0.5, 0.8], [0.9, 0.1, 0.3], [0.435, 0.435, 0.435]];

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn node(patch: serde_json::Value) -> Node {
    let mut n = Node::default();
    n.patch(&patch).unwrap();
    n
}

#[test]
fn a_new_grade_is_one_neutral_node() {
    let g = Grade::default();
    assert_eq!(g.nodes.len(), 1);
    assert!(g.is_neutral());
    for c in SAMPLES {
        let out = g.apply(c);
        assert!((0..3).all(|k| close(out[k], c[k], 1e-6)), "{c:?} → {out:?}");
    }
}

#[test]
fn lift_moves_black_and_leaves_white() {
    let n = node(json!({"lift": {"y": 0.2}}));
    assert!(close(n.tone(0, 0.0), 0.2, 1e-6));
    assert!(close(n.tone(1, 1.0), 1.0, 1e-6));
    assert!(n.tone(2, 0.5) > 0.5);
}

#[test]
fn gain_scales_from_black() {
    let n = node(json!({"gain": {"y": 0.5}}));
    assert!(close(n.tone(0, 0.0), 0.0, 1e-6));
    assert!(close(n.tone(0, 1.0), 0.5, 1e-6));
    assert!(close(n.tone(0, 0.4), 0.2, 1e-6));
}

#[test]
fn gamma_moves_the_mids_and_leaves_black_and_white() {
    let up = node(json!({"gamma": {"y": 0.25}}));
    let down = node(json!({"gamma": {"y": -0.25}}));
    for n in [&up, &down] {
        assert!(close(n.tone(0, 0.0), 0.0, 1e-6) && close(n.tone(0, 1.0), 1.0, 1e-6));
    }
    assert!(up.tone(0, 0.5) > 0.5 && down.tone(0, 0.5) < 0.5);
}

#[test]
fn offset_shifts_the_whole_signal() {
    let n = node(json!({"offset": {"y": 35.0}}));
    assert!(close(n.tone(0, 0.0), 0.1, 1e-6));
    assert!(close(n.tone(0, 0.5), 0.6, 1e-6));
}

#[test]
fn contrast_turns_about_the_pivot_without_clipping() {
    let n = node(json!({"contrast": 1.5, "pivot": 0.4}));
    assert!(close(n.tone(0, 0.0), 0.0, 1e-6) && close(n.tone(0, 1.0), 1.0, 1e-6));
    assert!(close(n.tone(0, 0.4), 0.4, 1e-6), "the pivot stays");
    assert!(n.tone(0, 0.3) < 0.3 && n.tone(0, 0.6) > 0.6, "darks darker, lights lighter");
    let flat = node(json!({"contrast": 0.0, "pivot": 0.4}));
    assert!(close(flat.tone(0, 0.9), 0.4, 1e-6), "contrast 0 is flat grey at the pivot");
}

#[test]
fn the_balance_moves_colour_and_keeps_luma() {
    let mut w = Wheel::splat(0.0);
    w.set_balance([0.1, 0.3], 0.0, 0.5);
    assert!(close(luma([w.r, w.g, w.b]), 0.0, 1e-6), "a balance move does not brighten or darken");
    assert!(w.r > 0.0 && w.g < 0.0, "towards red: red up, green down");
    let back = w.balance(0.0, 0.5);
    assert!(close(back[0], 0.1, 1e-5) && close(back[1], 0.3, 1e-5), "{back:?}");
    // the same about gain's neutral of 1
    let mut g = Wheel::splat(1.0);
    g.set_balance([-0.2, 0.0], 1.0, 0.5);
    assert!(g.b < 1.0 && close(luma([g.r - 1.0, g.g - 1.0, g.b - 1.0]), 0.0, 1e-6));
}

#[test]
fn temp_warms_and_tint_turns_magenta() {
    let warm = node(json!({"temp": 2000.0}));
    let wb = warm.white_balance();
    assert!(wb[0] > 1.0 && wb[2] < 1.0 && close(wb[1], 1.0, 1e-6));
    let magenta = node(json!({"tint": 50.0}));
    let wb = magenta.white_balance();
    assert!(wb[0] > 1.0 && wb[2] > 1.0 && wb[1] < 1.0);
}

#[test]
fn saturation_keeps_luma_and_zero_is_grey() {
    let grey = node(json!({"saturation": 0.0}));
    let c = [0.9, 0.2, 0.3];
    let out = grey.apply(c);
    assert!(close(out[0], out[1], 1e-6) && close(out[1], out[2], 1e-6), "{out:?}");
    assert!(close(out[0], luma(c), 1e-6));
    let double = node(json!({"saturation": 100.0}));
    let out = double.apply([0.5, 0.4, 0.4]);
    let l = luma([0.5, 0.4, 0.4]);
    assert!(close(out[0] - l, 2.0 * (0.5 - l), 1e-5));
}

#[test]
fn color_boost_lifts_dull_colours_more_than_vivid_ones() {
    let n = node(json!({"color_boost": 50.0}));
    let gain = |c: [f32; 3]| {
        let out = n.apply(c);
        let l = luma(c);
        (out[0] - luma(out)) / (c[0] - l)
    };
    assert!(gain([0.55, 0.45, 0.45]) > gain([0.9, 0.1, 0.1]));
}

#[test]
fn hue_turns_the_colour_wheel() {
    let n = node(json!({"hue": 100.0}));
    let out = n.apply([0.8, 0.2, 0.2]);
    assert!(out[0] < out[1] && close(out[1], out[2], 1e-5), "half a turn makes red cyan: {out:?}");
}

#[test]
fn curves_pass_through_their_points_and_are_monotone_between_them() {
    let c = Curve(vec![[0.25, 0.15], [0.75, 0.85]]);
    assert!(close(c.eval(0.25), 0.15, 1e-6) && close(c.eval(0.75), 0.85, 1e-6));
    assert!(close(c.eval(0.0), 0.0, 1e-6) && close(c.eval(1.0), 1.0, 1e-6), "black and white are implied");
    let mut last = -1.0;
    for i in 0..=100 {
        let y = c.eval(i as f32 / 100.0);
        assert!(y >= last - 1e-6, "monotone at {i}");
        last = y;
    }
    assert!(Curve::default().is_identity() && Curve(vec![[0.3, 0.3]]).is_identity());
    let n = node(json!({"curves": {"r": [[0.5, 0.7]]}}));
    assert!(n.tone(0, 0.5) > 0.69 && close(n.tone(1, 0.5), 0.5, 1e-6), "the red curve moves red only");
    let y = node(json!({"curves": {"y": [[0.5, 0.3]]}}));
    assert!((0..3).all(|k| y.tone(k, 0.5) < 0.31), "the Y curve moves every channel");
}

#[test]
fn tone_points_reproduce_the_tone_function() {
    // what an engine does with them: a monotone cubic through the points, sampled finely
    let grades = [
        json!({"lift": {"y": 0.1, "r": 0.03}, "gamma": {"y": 0.2}, "gain": {"y": 1.2, "b": 0.9}}),
        json!({"gamma": {"y": 0.5}}),
        json!({"contrast": 1.6, "pivot": 0.3, "offset": {"y": 22.0}}),
        json!({"curves": {"y": [[0.3, 0.2], [0.7, 0.85]], "g": [[0.5, 0.45]]}, "temp": -1500.0}),
    ];
    for g in grades {
        let n = node(g.clone());
        for k in 0..3 {
            let pts = n.tone_points(k);
            assert!(pts.len() <= MAX_TONE_POINTS, "{} points", pts.len());
            let worst = (0..1024)
                .map(|i| {
                    let x = i as f32 / 1023.0;
                    (monotone_cubic(&pts, x) - n.tone(k, x)).abs()
                })
                .fold(0.0f32, f32::max);
            assert!(worst < 0.5 / 255.0, "{g} channel {k}: off by {worst} with {} points", pts.len());
        }
    }
    assert_eq!(Node::default().tone_points(0).len(), 33, "a straight line needs no more");
}

#[test]
fn patching_keeps_other_controls_holds_ranges_and_refuses_unknown_fields() {
    let mut n = node(json!({"gain": {"r": 1.1}}));
    n.patch(&json!({"gain": {"b": 0.9}, "saturation": 500.0})).unwrap();
    assert_eq!((n.gain.r, n.gain.b, n.gain.y), (1.1, 0.9, 1.0), "the other numbers of the wheel stay");
    assert_eq!(n.saturation, 100.0, "held to its range");
    assert!(n.patch(&json!({"saturaton": 60.0})).is_err(), "a typo is refused");
    assert!(n.patch(&json!({"gain": {"x": 1.0}})).is_err());
    assert!(n.patch(&json!(5)).is_err());
    n.patch(&json!({"curves": {"g": [[0.9, 2.0], [0.1, 0.2]]}})).unwrap();
    assert_eq!(n.curves.g.0, vec![[0.1, 0.2], [0.9, 1.0]], "curve points are sorted and held to 0…1");
}

#[test]
fn grades_round_trip_as_json_and_refuse_unknown_fields() {
    let mut g = Grade::default();
    g.nodes[0].patch(&json!({"lift": {"y": 0.1}, "label": "Base"})).unwrap();
    g.add_after(0);
    let text = serde_json::to_string(&g).unwrap();
    assert_eq!(serde_json::from_str::<Grade>(&text).unwrap(), g);
    assert!(serde_json::from_str::<Grade>(r#"{"nodes": [], "extra": 1}"#).is_err());
}

#[test]
fn nodes_are_added_deleted_and_reset_as_in_resolve() {
    let mut g = Grade::default();
    g.nodes[0].patch(&json!({"saturation": 70.0, "label": "Skin"})).unwrap();
    assert_eq!(g.add_after(0), 1);
    assert_eq!(g.add_before(0), 0);
    assert_eq!(g.nodes.len(), 3);
    assert_eq!(g.nodes[1].label, "Skin", "the first node moved down one");
    g.reset_keep_nodes();
    assert_eq!(g.nodes.len(), 3);
    assert!(g.is_neutral() && g.nodes[1].label == "Skin", "controls reset, labels kept");
    g.delete(0);
    g.delete(0);
    g.nodes[0].patch(&json!({"saturation": 70.0})).unwrap();
    g.delete(0);
    assert_eq!(g.nodes.len(), 1, "the last node is reset, not deleted");
    assert!(g.is_neutral());
}

#[test]
fn a_disabled_node_passes_the_picture_through() {
    let mut n = node(json!({"gain": {"y": 0.2}}));
    n.enabled = false;
    assert_eq!(n.apply([0.3, 0.6, 0.9]), [0.3, 0.6, 0.9]);
    let g = Grade { nodes: vec![n] };
    assert!(g.is_neutral());
}

#[test]
fn grades_stored_before_windows_still_load() {
    // a node as D-028 stored it: no `windows` field
    let mut v = serde_json::to_value(Node::default()).unwrap();
    v.as_object_mut().unwrap().remove("windows");
    let old = json!({ "nodes": [v] });
    let g: Grade = serde_json::from_value(old).unwrap();
    assert!(g.nodes[0].windows.is_empty());
    // and a node without windows is stored without the field
    assert!(serde_json::to_value(Node::default()).unwrap().get("windows").is_none());
}

#[test]
fn windows_are_set_whole_and_held_to_their_ranges() {
    let mut n = Node::default();
    let mut w = Window::new(Shape::Circle);
    w.softness = 5.0;
    w.opacity = -3.0;
    n.patch(&json!({ "windows": [w] })).unwrap();
    assert_eq!(n.windows.len(), 1);
    assert_eq!((n.windows[0].softness, n.windows[0].opacity), (1.0, 0.0));
    assert!(n.patch(&json!({ "windows": [{ "shape": "star" }] })).is_err(), "unknown shape");
    n.patch(&json!({ "windows": [] })).unwrap();
    assert!(n.windows.is_empty());
    let mut r = Node::default();
    r.patch(&json!({ "windows": [Window::new(Shape::Linear)], "saturation": 70.0 })).unwrap();
    r.reset();
    assert!(r.windows.is_empty(), "resetting a node clears its windows");
}

#[test]
fn a_qualifier_keys_the_colours_in_its_ranges() {
    let q = Qualifier::picked([0.8, 0.4, 0.2]); // an orange
    assert!(q.key([0.8, 0.4, 0.2]) > 0.99, "the picked colour is in the key");
    assert!(q.key([0.2, 0.4, 0.8]) < 0.01, "a blue is not");
    assert!(q.key([0.5, 0.5, 0.5]) < 0.01, "nor a grey (no saturation)");
    // the hue's soft edge: a little further round the wheel is partly in
    let edge = Qualifier {
        hue_center: 0.0,
        hue_width: 10.0,
        hue_soft: 10.0,
        sat_on: false,
        lum_on: false,
        ..Qualifier::default()
    };
    let at = |turns: f32| edge.key(hsl_to_rgb([turns, 1.0, 0.5]));
    assert!(at(0.04) > 0.99 && at(0.15) < 0.01, "inside half the width, outside width and soft");
    let mid = at(0.10);
    assert!(mid > 0.05 && mid < 0.95, "between: {mid}");
    assert!((at(0.97) - 1.0).abs() < 1e-6, "red is both ends of the wheel");
    let inverted = Qualifier { invert: true, ..q };
    assert!(inverted.key([0.2, 0.4, 0.8]) > 0.99);
}

#[test]
fn a_qualified_node_changes_only_what_its_key_takes() {
    let mut n = node(json!({"gain": {"y": 0.0}}));
    n.qualifier = Some(Qualifier::picked([0.8, 0.4, 0.2]));
    assert_eq!(n.apply([0.8, 0.4, 0.2]).map(|v| (v * 1000.0).round()), [0.0; 3], "the keyed orange goes black");
    assert_eq!(n.apply([0.2, 0.4, 0.8]), [0.2, 0.4, 0.8], "the blue is untouched");
    n.qualifier = Some(Qualifier { enabled: false, ..Qualifier::picked([0.8, 0.4, 0.2]) });
    assert_eq!(
        n.apply([0.2, 0.4, 0.8]).map(|v| (v * 1000.0).round()),
        [0.0; 3],
        "a qualifier that is off keys everything"
    );
}

#[test]
fn the_table_holds_the_node_at_its_points() {
    let mut n = node(json!({"gamma": {"y": 0.3}, "saturation": 70.0}));
    n.qualifier = Some(Qualifier::picked([0.8, 0.4, 0.2]));
    let t = n.table(5);
    assert_eq!(t.len(), 125);
    // red changes fastest: index 1 is (0.25, 0, 0); index 5 is (0, 0.25, 0); index 25 is (0, 0, 0.25)
    for (i, rgb) in [(1, [0.25, 0.0, 0.0]), (5, [0.0, 0.25, 0.0]), (25, [0.0, 0.0, 0.25]), (124, [1.0, 1.0, 1.0])] {
        assert_eq!(t[i], n.apply(rgb), "point {i}");
    }
}

#[test]
fn grades_stored_before_qualifiers_still_load() {
    let v = serde_json::to_value(Node::default()).unwrap();
    assert!(v.get("qualifier").is_none(), "a node without one is stored without the field");
    let g: Grade = serde_json::from_value(json!({ "nodes": [v] })).unwrap();
    assert!(g.nodes[0].qualifier.is_none());
    let mut n = Node::default();
    n.patch(
        &json!({"qualifier": {"enabled": true, "invert": false, "hue_on": true, "hue_center": 150.0, "hue_width": 10.0,
        "hue_soft": 5.0, "sat_on": true, "sat_low": 20.0, "sat_high": 100.0, "sat_low_soft": 10.0, "sat_high_soft": 0.0,
        "lum_on": true, "lum_low": 10.0, "lum_high": 90.0, "lum_low_soft": 10.0, "lum_high_soft": 10.0}}),
    )
    .unwrap();
    assert_eq!(n.qualifier.map(|q| q.hue_center), Some(50.0), "hue goes round: 150 is 50");
}

// ---- the hue curves (LOG D-043)

fn hsl(rgb: [f32; 3]) -> [f32; 3] {
    rgb_to_hsl(rgb)
}

#[test]
fn a_hue_curve_runs_round_the_wheel_and_the_middle_line_changes_nothing() {
    let empty = HslCurve::default();
    assert!(empty.is_identity() && close(empty.eval(0.3, true), 0.5, 0.0));
    let c = HslCurve(vec![[0.0, 0.8], [0.5, 0.2]]);
    // the last point joins the first: red at both ends of the wheel
    assert!(close(c.eval(0.0, true), 0.8, 1e-5) && close(c.eval(1.0, true), 0.8, 1e-5));
    assert!(close(c.eval(0.999, true), c.eval(0.0, true), 0.01), "no step where the wheel closes");
    // over lightness or saturation the ends are held instead
    let held = HslCurve(vec![[0.2, 0.7], [0.8, 0.3]]);
    assert!(close(held.eval(0.0, false), 0.7, 1e-6) && close(held.eval(1.0, false), 0.3, 1e-6));
    assert!(HslCurve(vec![[0.1, 0.5], [0.7, 0.5]]).is_identity());
}

#[test]
fn hue_vs_sat_takes_the_colour_out_of_one_hue_only() {
    // reds grey, the rest of the wheel as it was
    let n = node(json!({ "curves": { "hue_sat": [[0.0, 0.0], [1.0 / 6.0, 0.5], [0.5, 0.5], [5.0 / 6.0, 0.5]] } }));
    assert!(!n.is_neutral());
    let red = n.apply([0.8, 0.2, 0.2]);
    assert!(hsl(red)[1] < 1e-3, "red loses its colour: {red:?}");
    assert!(close(hsl(red)[2], hsl([0.8, 0.2, 0.2])[2], 1e-4), "and keeps its lightness");
    let blue = [0.2, 0.3, 0.8];
    let out = n.apply(blue);
    assert!((0..3).all(|k| close(out[k], blue[k], 1e-4)), "blue untouched: {out:?}");
}

#[test]
fn hue_vs_hue_turns_one_hue_and_hue_vs_lum_lightens_one() {
    // green turned an eighth of a turn; green lightened by the most Hue vs Lum gives
    let flat = |x: f32, y: f32| json!([[0.0, 0.5], [x - 1.0 / 6.0, 0.5], [x, y], [x + 1.0 / 6.0, 0.5]]);
    let green = [0.2, 0.7, 0.2];
    let n = node(json!({ "curves": { "hue_hue": flat(1.0 / 3.0, 0.625) } }));
    let out = hsl(n.apply(green));
    assert!(close(out[0], 1.0 / 3.0 + 0.125, 1e-3), "hue {}", out[0]);
    let n = node(json!({ "curves": { "hue_lum": flat(1.0 / 3.0, 1.0) } }));
    assert!(close(hsl(n.apply(green))[2], hsl(green)[2] + 0.25, 1e-3));
}

#[test]
fn lum_vs_sat_and_sat_vs_sat_follow_lightness_and_saturation() {
    // the darkest colours grey; the most saturated ones doubled
    let n = node(json!({ "curves": { "lum_sat": [[0.0, 0.0], [0.4, 0.5], [1.0, 0.5]] } }));
    // a dark red (saturation 0.5, lightness 0.04, near the curve's grey end) loses most of it
    assert!(hsl(n.apply([0.06, 0.02, 0.02]))[1] < 0.1, "a dark red goes nearly grey");
    let bright = [0.9, 0.5, 0.5];
    assert!(close(hsl(n.apply(bright))[1], hsl(bright)[1], 1e-4), "a light one keeps its colour");
    let n = node(json!({ "curves": { "sat_sat": [[0.0, 0.5], [0.3, 0.5], [1.0, 1.0]] } }));
    let dull = [0.5, 0.45, 0.45];
    assert!(close(hsl(n.apply(dull))[1], hsl(dull)[1], 1e-3), "a dull colour as it was");
}

#[test]
fn hue_rotation_and_hue_vs_hue_add_up_as_one_turn() {
    let rotated = node(json!({ "hue": 60.0 }));
    let both = node(json!({ "hue": 60.0, "curves": { "hue_hue": [[0.0, 0.6], [0.5, 0.6]] } }));
    let c = [0.7, 0.3, 0.2];
    // the curve adds a tenth of a turn everywhere on top of Hue's tenth
    let (a, b) = (hsl(rotated.apply(c))[0], hsl(both.apply(c))[0]);
    assert!(close((b - a).rem_euclid(1.0), 0.1, 1e-3), "{a} then {b}");
    // and without hue curves a node is what it was before they existed
    let plain = node(json!({ "saturation": 70.0, "hue": 55.0, "color_boost": 20.0 }));
    for c in SAMPLES {
        let (old, new) = (plain.saturation().apply(c), plain.color(c));
        assert!((0..3).all(|k| close(old[k], new[k], 1e-6)), "{c:?}: {old:?} against {new:?}");
    }
}

#[test]
fn hue_curves_are_tidied_and_grades_stored_before_them_still_load() {
    let n = node(json!({ "curves": { "hue_sat": [[1.25, 1.4], [0.5, -0.2]], "lum_sat": [[1.5, 0.3]] } }));
    assert_eq!(n.curves.hue_sat.0, vec![[0.25, 1.0], [0.5, 0.0]], "over hue, 1.25 is a quarter turn");
    assert_eq!(n.curves.lum_sat.0, vec![[1.0, 0.3]]);
    let old = r#"{"nodes":[{"label":"","enabled":true,"lift":{"y":0.0,"r":0.0,"g":0.0,"b":0.0},"gamma":{"y":0.0,"r":0.0,"g":0.0,"b":0.0},"gain":{"y":1.0,"r":1.0,"g":1.0,"b":1.0},"offset":{"y":25.0,"r":25.0,"g":25.0,"b":25.0},"contrast":1.0,"pivot":0.435,"saturation":50.0,"hue":50.0,"color_boost":0.0,"temp":0.0,"tint":0.0,"curves":{"y":[],"r":[],"g":[],"b":[]}}]}"#;
    let g: Grade = serde_json::from_str(old).unwrap();
    assert!(g.is_neutral());
    assert_eq!(serde_json::to_string(&g).unwrap(), old, "written back as it was");
}
