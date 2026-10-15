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
fn sat_vs_lum_lightens_by_saturation() {
    // the most saturated colours lightened by the most Sat vs Lum gives; dull ones as they were
    let n = node(json!({ "curves": { "sat_lum": [[0.0, 0.5], [0.5, 0.5], [0.9, 1.0]] } }));
    assert!(!n.is_neutral());
    let red = n.apply([1.0, 0.0, 0.0]);
    let out = hsl(red);
    assert!(close(out[2], 0.75, 1e-3), "a full red lightened by a quarter: {red:?}");
    // (hue in turns: 0.99999994 is red too)
    let turn = out[0].rem_euclid(1.0);
    assert!(close(out[1], 1.0, 1e-3) && turn.min(1.0 - turn) < 1e-3, "and still a full red: {out:?}");
    for dull in [[0.6, 0.4, 0.4], [0.5, 0.5, 0.5]] {
        let o = n.apply(dull);
        assert!((0..3).all(|k| close(o[k], dull[k], 1e-4)), "{dull:?} as it was: {o:?}");
    }
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

// ---- the qualifier's matte finesse (LOG D-046)

/// A qualifier keying saturated reds hard (greys, whose hue reads as red, are below its
/// saturation), nothing else.
fn reds() -> Qualifier {
    Qualifier {
        hue_width: 10.0,
        hue_soft: 0.0,
        sat_low: 30.0,
        sat_high: 100.0,
        sat_low_soft: 0.0,
        sat_high_soft: 0.0,
        lum_on: false,
        ..Qualifier::picked([0.8, 0.1, 0.1])
    }
}

#[test]
fn clean_black_and_clean_white_stretch_the_key() {
    let q = Qualifier { clean_black: 40.0, clean_white: 40.0, ..reds() };
    // black point 0.2, white point 0.8
    assert!(close(q.clean(0.15), 0.0, 1e-6) && close(q.clean(0.85), 1.0, 1e-6));
    assert!(close(q.clean(0.5), 0.5, 1e-6) && close(q.clean(0.35), 0.25, 1e-6));
    assert_eq!(reds().clean(0.3), 0.3, "none set: the key as it was");
    assert!(!q.is_spatial(), "levels need no neighbours");
}

#[test]
fn denoise_takes_out_a_lone_keyed_pixel() {
    // grey everywhere but one red pixel: keyed alone, gone after one pass of medians
    let (w, h) = (9, 9);
    let mut px = vec![[0.5, 0.5, 0.5]; w * h];
    px[4 * w + 4] = [0.8, 0.1, 0.1];
    assert_eq!(reds().matte(&px, w, h)[4 * w + 4], 1.0);
    let q = Qualifier { denoise: 10.0, ..reds() };
    assert_eq!(q.denoise_passes(w), 1);
    assert!(q.matte(&px, w, h).iter().all(|k| *k == 0.0), "the speck is gone");
    // a keyed block survives
    for y in 2..7 {
        for x in 2..7 {
            px[y * w + x] = [0.8, 0.1, 0.1];
        }
    }
    assert_eq!(q.matte(&px, w, h)[4 * w + 4], 1.0, "the block's middle is still keyed");
}

#[test]
fn blur_radius_softens_the_key_in_proportion_to_the_picture() {
    // red on the left half, grey on the right: a hard edge, softened either side of it
    let edge = |w: usize| -> Vec<[f32; 3]> {
        (0..w * 4).map(|i| if i % w < w / 2 { [0.8, 0.1, 0.1] } else { [0.5, 0.5, 0.5] }).collect()
    };
    let q = Qualifier { blur: 50.0, ..reds() };
    for w in [768usize, 1536] {
        let k = q.matte(&edge(w), w, 4);
        let sigma = q.blur_sigma(w);
        assert!(close(sigma, w as f32 / 384.0, 1e-5));
        assert!(close(k[w / 2 - 1] + k[w / 2], 1.0, 0.02), "the edge's two sides add to one");
        // a standard deviation into the grey, about 16 % keyed, at either width
        let x = w / 2 + sigma.round() as usize;
        assert!((0.08..0.25).contains(&k[x]), "{w} wide: {}", k[x]);
        assert!(k[2] > 0.999 && k[w - 3] < 0.001, "far from the edge as keyed");
    }
}

#[test]
fn a_picture_without_spatial_mattes_is_graded_pixel_by_pixel() {
    let mut n = Node::default();
    n.patch(&json!({ "gain": {"y": 0.6}, "qualifier": Qualifier { clean_black: 20.0, ..Qualifier::picked([0.8, 0.4, 0.2]) } }))
        .unwrap();
    let g = Grade { nodes: vec![n] };
    let mut px: Vec<[f32; 3]> = (0..64).map(|i| [i as f32 / 63.0, 0.4, 1.0 - i as f32 / 63.0]).collect();
    let want: Vec<[f32; 3]> = px.iter().map(|p| g.apply(*p)).collect();
    g.apply_image(&mut px, 8, 8);
    assert_eq!(px, want);
}

#[test]
fn qualifiers_stored_before_matte_finesse_still_load() {
    let q = reds();
    let text = serde_json::to_string(&q).unwrap();
    assert!(!text.contains("blur") && !text.contains("denoise"), "none set: none written");
    let back: Qualifier = serde_json::from_str(&text).unwrap();
    assert_eq!(back, q);
    let mut n = Node::default();
    n.patch(&json!({ "qualifier": { "blur": 250.0, "denoise": -3.0 } })).unwrap_err();
    n.patch(&json!({ "qualifier": Qualifier { blur: 250.0, denoise: -3.0, ..reds() } })).unwrap();
    let q = n.qualifier.unwrap();
    assert_eq!((q.blur, q.denoise), (100.0, 0.0), "held to 0..100");
}

#[test]
fn the_highlight_view_shows_what_a_node_keys() {
    // a red and a blue pixel, keyed on red, the node darkening what it keys
    let px = vec![[0.8, 0.2, 0.2], [0.2, 0.3, 0.8]];
    let mut n = node(json!({ "gain": { "y": 0.5 } }));
    n.qualifier = Some(Qualifier::picked(px[0]));
    let g = Grade { nodes: vec![n] };
    let mut graded = px.clone();
    g.apply_image(&mut graded, 2, 1);
    let mut grey = px.clone();
    g.highlight_image(&mut grey, 2, 1, 0, false);
    assert!((0..3).all(|k| close(grey[0][k], graded[0][k], 1e-6)), "the keyed red as graded: {grey:?}");
    assert!(grey[1].iter().all(|v| close(*v, HIGHLIGHT_GRAY, 1e-6)), "the blue flat grey: {grey:?}");
    let mut bw = px.clone();
    g.highlight_image(&mut bw, 2, 1, 0, true);
    assert!(bw[0].iter().all(|v| close(*v, 1.0, 1e-6)) && bw[1].iter().all(|v| close(*v, 0.0, 1e-6)), "{bw:?}");
    // a node without a qualifier keys everything; a disabled node, or none, shows the grade
    let open = Grade { nodes: vec![node(json!({ "gain": { "y": 0.5 } }))] };
    let mut all = px.clone();
    open.highlight_image(&mut all, 2, 1, 0, true);
    assert!(all.iter().flatten().all(|v| close(*v, 1.0, 1e-6)), "{all:?}");
    let mut off = g.clone();
    off.nodes[0].enabled = false;
    for (grade, at) in [(&off, 0), (&g, 5)] {
        let (mut want, mut got) = (px.clone(), px.clone());
        grade.apply_image(&mut want, 2, 1);
        grade.highlight_image(&mut got, 2, 1, at, true);
        assert_eq!(got, want, "node {at}");
    }
}

#[test]
fn a_gradient_window_is_written_as_gradient() {
    let w = Window::new(Shape::Gradient);
    let j = serde_json::to_value(&w).unwrap();
    assert_eq!(j["shape"], "gradient");
    let back: Window = serde_json::from_value(j).unwrap();
    assert_eq!(back, w);
}

#[test]
fn a_polygon_window_keeps_three_to_128_corners_and_other_shapes_none() {
    let w = Window::new(Shape::Polygon);
    assert_eq!(w.points.len(), 4, "a new polygon is a rectangle");
    let j = serde_json::to_value(&w).unwrap();
    assert_eq!(j["shape"], "polygon");
    assert_eq!(serde_json::from_value::<Window>(j).unwrap(), w);
    // a circle stored before polygons existed has no points, and writes none
    let circle = serde_json::to_value(Window::new(Shape::Circle)).unwrap();
    assert!(circle.get("points").is_none());
    let window = |points: serde_json::Value, shape: &str| {
        let mut w = serde_json::to_value(Window::new(Shape::Polygon)).unwrap();
        w["points"] = points;
        w["shape"] = json!(shape);
        node(json!({ "windows": [w] })).windows[0].clone()
    };
    assert_eq!(window(json!([[0.0, -1.0], [1.0, 1.0]]), "polygon").points.len(), 4, "two corners: the rectangle again");
    let many: Vec<[f32; 2]> = (0..200).map(|i| [(i as f32 / 31.8).cos(), (i as f32 / 31.8).sin()]).collect();
    assert_eq!(window(json!(many), "polygon").points.len(), MAX_POLYGON_POINTS);
    assert!(window(json!([[0.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]), "circle").points.is_empty());
}

/// Inside and Outside Softness (LOG D-063): a Polygon's or a Curve's own, else half the softness
/// each way; written only when set, and dropped from the other shapes.
#[test]
fn inside_and_outside_softness_default_to_half_the_softness_each_way() {
    let w = Window::new(Shape::Polygon);
    assert_eq!(w.edge_softness(), (w.softness / 2.0, w.softness / 2.0));
    let j = serde_json::to_value(&w).unwrap();
    assert!(j.get("inside_softness").is_none() && j.get("outside_softness").is_none(), "{j}");
    let own = Window { inside_softness: Some(0.0), outside_softness: Some(0.3), ..w.clone() };
    assert_eq!(own.edge_softness(), (0.0, 0.3));
    let j = serde_json::to_value(&own).unwrap();
    assert_eq!(serde_json::from_value::<Window>(j.clone()).unwrap(), own);
    // only one of them: the other is still half the softness
    let one = Window { outside_softness: Some(0.3), ..w.clone() };
    assert_eq!(one.edge_softness(), (w.softness / 2.0, 0.3));
    // on a node: kept in range, and a circle has none
    let window = |shape: &str| {
        let mut j = j.clone();
        j["shape"] = json!(shape);
        j["inside_softness"] = json!(-1.0);
        j["outside_softness"] = json!(3.0);
        node(json!({ "windows": [j] })).windows[0].clone()
    };
    for shape in ["polygon", "curve"] {
        assert_eq!(window(shape).edge_softness(), (0.0, 1.0), "{shape}");
    }
    let circle = window("circle");
    assert_eq!((circle.inside_softness, circle.outside_softness), (None, None));
    assert_eq!(circle.edge_softness(), (circle.softness / 2.0, circle.softness / 2.0));
}

#[test]
fn shadows_and_highlights_move_the_ends_and_keep_black_white_and_mid_grey() {
    let ramp: Vec<f32> = (0..=200).map(|i| i as f32 / 200.0).collect();
    for (s, h) in [(100.0, 0.0), (-100.0, 0.0), (0.0, 100.0), (0.0, -100.0), (60.0, -40.0)] {
        let n = node(json!({ "shadows": s, "highlights": h }));
        assert!(!n.is_neutral() && !n.is_spatial());
        let out: Vec<f32> = ramp.iter().map(|x| n.tone(0, *x)).collect();
        assert!(close(out[0], 0.0, 1e-6) && close(out[100], 0.5, 1e-6) && close(out[200], 1.0, 1e-6), "{s} {h}");
        assert!(out.windows(2).all(|p| p[1] >= p[0] - 1e-6), "rising, at Shadows {s} and Highlights {h}");
    }
    // raised Shadows lighten the dark tones, most a third of the way to mid grey
    let n = node(json!({ "shadows": 100.0 }));
    assert!(close(n.tone(0, 1.0 / 6.0), 1.0 / 6.0 + 0.5 / 3.0 * 4.0 / 9.0, 1e-5));
    assert!(close(n.tone(0, 0.8), 0.8, 1e-6), "the highlights untouched");
    // lowered Highlights pull the light tones down
    assert!(node(json!({ "highlights": -100.0 })).tone(0, 0.85) < 0.8);
}

#[test]
fn midtone_detail_sharpens_an_edge_and_lowered_softens_only_faint_detail() {
    // a dark and a light half, with a faint ripple on the dark one
    let (w, h) = (400usize, 8usize);
    let px: Vec<[f32; 3]> = (0..w * h)
        .map(|i| {
            let x = i % w;
            let v = if x < 200 { 0.35 + 0.01 * ((x % 4) as f32 - 1.5) } else { 0.65 };
            [v; 3]
        })
        .collect();
    let row = |p: &[[f32; 3]], x: usize| p[4 * w + x][0];
    let up = Grade { nodes: vec![node(json!({ "midtone_detail": 100.0 }))] };
    assert!(up.nodes[0].is_spatial() && !up.nodes[0].is_neutral());
    let mut sharp = px.clone();
    up.apply_image(&mut sharp, w, h);
    assert!(
        row(&sharp, 197) < row(&px, 197) - 0.01 && row(&sharp, 202) > row(&px, 202) + 0.01,
        "more contrast at the edge"
    );
    assert!((row(&sharp, 390) - row(&px, 390)).abs() < 1e-3, "flat far from it");
    // per pixel it is left out
    assert_eq!(up.apply(px[0]), px[0]);
    let down = Grade { nodes: vec![node(json!({ "midtone_detail": -100.0 }))] };
    let mut soft = px.clone();
    down.apply_image(&mut soft, w, h);
    let ripple = |p: &[[f32; 3]]| {
        (100..104).map(|x| row(p, x)).fold(0.0f32, f32::max) - (100..104).map(|x| row(p, x)).fold(1.0f32, f32::min)
    };
    assert!(ripple(&soft) < ripple(&px) * 0.8, "the faint ripple softened: {} from {}", ripple(&soft), ripple(&px));
    assert!((row(&soft, 205) - row(&px, 205)).abs() < 0.01, "the strong edge's side kept");
}

#[test]
fn morph_shrinks_grows_opens_and_closes_the_key_and_clip_sets_its_levels() {
    // red where keyed: a 20 x 12 block with a 2-pixel hole, and a lone speck, on grey
    let (w, h) = (192usize, 40usize);
    let red = [0.8, 0.15, 0.1];
    let px: Vec<[f32; 3]> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let block = (40..60).contains(&x) && (10..22).contains(&y) && !((49..51).contains(&x) && y == 15);
            if block || (x, y) == (120, 30) { red } else { [0.5, 0.5, 0.5] }
        })
        .collect();
    let base = Qualifier { sat_low: 30.0, ..Qualifier::picked(red) };
    let matte = |morph: Morph| Qualifier { morph, morph_radius: 100.0, ..base }.matte(&px, w, h);
    let at = |k: &[f32], x: usize, y: usize| k[y * w + x];
    // 10 pixels at 1920 wide: 1 here
    assert_eq!(Qualifier { morph: Morph::Grow, morph_radius: 100.0, ..base }.morph_pixels(w), 1);
    let plain = base.matte(&px, w, h);
    assert!(at(&plain, 120, 30) > 0.99 && at(&plain, 49, 15) < 0.01, "a speck and a hole");
    let shrunk = matte(Morph::Shrink);
    assert!(at(&shrunk, 120, 30) < 0.01 && at(&shrunk, 40, 16) < 0.01 && at(&shrunk, 45, 16) > 0.99);
    let grown = matte(Morph::Grow);
    assert!(at(&grown, 39, 16) > 0.99 && at(&grown, 49, 15) > 0.99, "edges out, the hole filled");
    let opened = matte(Morph::Opening);
    assert!(at(&opened, 120, 30) < 0.01 && at(&opened, 40, 16) > 0.99, "the speck gone, the block kept");
    let closed = matte(Morph::Closing);
    assert!(at(&closed, 49, 15) > 0.99 && at(&closed, 39, 16) < 0.01, "the hole filled, the edge kept");
    // Clip: levels on the finished matte, pixel by pixel too
    let q = Qualifier { black_clip: 20.0, white_clip: 80.0, ..base };
    assert!(q.is_spatial() && q.is_clipped());
    assert!((q.clip(0.5) - 0.5).abs() < 1e-6 && q.clip(0.15) == 0.0 && q.clip(0.85) == 1.0);
    // a qualifier from before these fields writes none of them
    let j = serde_json::to_value(base).unwrap();
    for k in ["morph", "morph_radius", "black_clip", "white_clip"] {
        assert!(j.get(k).is_none(), "{k}");
    }
    assert_eq!(serde_json::from_value::<Qualifier>(j).unwrap(), base);
}

#[test]
fn a_curve_runs_smoothly_through_its_points_and_closes() {
    let square = Window::new(Shape::Curve).points;
    assert_eq!(square.len(), 4);
    let spans = curve_spans(&square);
    assert_eq!(spans.len(), 4);
    for (i, s) in spans.iter().enumerate() {
        assert_eq!((s[0], s[3]), (square[i], square[(i + 1) % 4]), "through each point, closed");
    }
    // the tangent at a point is a sixth of the line from the point before to the point after
    assert!((spans[0][1][0] - (-1.0 + 2.0 / 6.0)).abs() < 1e-6 && (spans[0][1][1] - (-1.0 - 2.0 / 6.0)).abs() < 1e-6);
    // through a square's corners it bulges past each side's middle by a quarter
    let corners = curve_corners(&square);
    assert_eq!(corners.len(), 4 * 16);
    let top = corners.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    assert!((top + 1.25).abs() < 1e-3, "{top}");
    // many points share the corners out
    let many: Vec<[f32; 2]> = (0..100).map(|i| [(i as f32 / 15.9).cos(), (i as f32 / 15.9).sin()]).collect();
    assert_eq!(curve_corners(&many).len(), 200);
    let j = serde_json::to_value(Window::new(Shape::Curve)).unwrap();
    assert_eq!(j["shape"], "curve");
    assert_eq!(node(json!({ "windows": [j] })).windows[0].points.len(), 4, "a curve keeps its points");
}
