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
