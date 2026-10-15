//! Colour grades in Resolution's own types (LOG D-028): the corrector nodes of the Color page and
//! the controls on them. A clip's grade is a chain of serial nodes; each holds the Primaries
//! palette's controls, the Custom Curves and the hue curves.
//!
//! **The math is ours.** Resolve does not publish its formulas, so each control does what the
//! Resolve manual says it does, by formulas defined here:
//!
//! - The signal is display-encoded RGB, 0 black and 1 white (the grading signal of an SDR
//!   timeline), held to 0…1 after each node.
//! - Every control except Saturation, Hue, Color Boost and the hue curves acts on each colour
//!   channel on its own, so a node is a **tone function** per channel ([`Node::tone`]) followed by
//!   a **saturation step** ([`Node::saturation`]) and the **hue curves** ([`HslCurves`]), run
//!   together in [`Node::color`]. These parts are what an engine has to reproduce.
//!
//! Numbers are in Resolve's units where the manual gives them: Lift and Gamma 0, Gain 1 and
//! Offset 25 are neutral; Contrast 1 and Pivot 0.435; Saturation and Hue 50; Color Boost, Temp
//! and Tint 0.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The most points [`Node::tone_points`] gives for one channel.
pub const MAX_TONE_POINTS: usize = 129;

/// How much local detail (luma against its blur) keeps lowered Midtone Detail from softening a
/// pixel: none at all from this much (LOG D-055).
pub const DETAIL_EDGE: f32 = 0.08;

/// Shadows and Highlights (LOG D-055) on a tone `x`, each -100 to 100. Below mid grey, `x` moves
/// by Shadows / 100 × 0.5·t·(1 − t)² with t = x / 0.5: most a third of the way up, nothing at
/// black or mid grey. Above it, mirrored from white, by Highlights. For any setting in range the
/// result rises with `x`.
pub fn shadows_highlights(x: f32, shadows: f32, highlights: f32) -> f32 {
    const MID: f32 = 0.5;
    let bump = |t: f32| MID * t * (1.0 - t) * (1.0 - t);
    let mut v = x;
    if shadows != 0.0 && x < MID {
        v += shadows / 100.0 * bump(x / MID);
    }
    if highlights != 0.0 && x > MID {
        v += highlights / 100.0 * bump((1.0 - x) / MID);
    }
    v
}

/// The flat grey the Highlight view shows where a node's matte does not key the picture
/// ([`Grade::highlight_image`]), display-encoded.
pub const HIGHLIGHT_GRAY: f32 = 0.5;

/// How far [`Node::tone_points`] lets the joined points stray from the tone function: a quarter
/// of an 8-bit code value.
const TONE_TOLERANCE: f32 = 0.25 / 255.0;

/// Gamma +1 raises the signal to the power 1/4 (much brighter mids), −1 to the power 4.
const GAMMA_BASE: f32 = 4.0;

/// A clip's grade: corrector nodes, applied in order (serial).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grade {
    pub nodes: Vec<Node>,
}

impl Default for Grade {
    /// One neutral node, as a clip starts in Resolve.
    fn default() -> Self {
        Self { nodes: vec![Node::default()] }
    }
}

impl Grade {
    /// Whether the grade changes nothing (every node neutral or disabled).
    pub fn is_neutral(&self) -> bool {
        self.nodes.iter().all(|n| !n.enabled || n.is_neutral())
    }

    /// The grade applied to one pixel. A qualifier's Denoise and Blur Radius need the pixel's
    /// neighbours and are left out here (see [`Grade::apply_image`]).
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        self.nodes.iter().fold(rgb, |c, n| n.apply(c))
    }

    /// The grade applied to a whole picture, `w` × `h` pixels of straight, display-encoded RGB,
    /// row by row: pixel by pixel as [`Grade::apply`], except that each qualifier's key is cleaned
    /// over the picture by its Denoise and Blur Radius ([`Qualifier::matte`]). Windows are left
    /// out (they need the picture's place in the clip's).
    pub fn apply_image(&self, px: &mut [[f32; 3]], w: usize, h: usize) {
        self.image(px, w, h, None);
    }

    /// The Highlight view of node `node` (its index in `nodes`, LOG D-048) over a whole picture,
    /// as [`Grade::apply_image`] grades it: where the node's matte keys the picture, the graded
    /// picture; elsewhere flat grey ([`HIGHLIGHT_GRAY`]), in proportion to the matte. With
    /// `black_white`, the matte itself, white where keyed. A node without a qualifier keys
    /// everything; a disabled node, or none at `node`, leaves the graded picture as it is.
    pub fn highlight_image(&self, px: &mut [[f32; 3]], w: usize, h: usize, node: usize, black_white: bool) {
        self.image(px, w, h, Some((node, black_white)));
    }

    fn image(&self, px: &mut [[f32; 3]], w: usize, h: usize, highlight: Option<(usize, bool)>) {
        let mut matte = None;
        for (i, n) in self.nodes.iter().enumerate().filter(|(_, n)| n.enabled) {
            let keys = n.keyed().map(|q| q.matte(px, w, h));
            if highlight.is_some_and(|(at, _)| at == i) {
                matte = Some(keys.clone().unwrap_or_else(|| vec![1.0; px.len()]));
            }
            // Midtone Detail's blurred luma of the node's input (LOG D-055)
            let blurred = (n.midtone_detail != 0.0).then(|| {
                let l: Vec<f32> = px.iter().map(|p| luma(*p)).collect();
                let weights = gaussian(Node::detail_sigma(w));
                convolve(&convolve(&l, w, h, &weights, true), w, h, &weights, false)
            });
            for (i, p) in px.iter_mut().enumerate() {
                *p = n.apply_detailed(*p, keys.as_ref().map(|k| k[i]), blurred.as_ref().map(|b| b[i]));
            }
        }
        let (Some(matte), Some((_, black_white))) = (matte, highlight) else { return };
        for (p, k) in px.iter_mut().zip(matte) {
            *p = if black_white { [k; 3] } else { p.map(|c| HIGHLIGHT_GRAY + (c - HIGHLIGHT_GRAY) * k) };
        }
    }

    /// Add a neutral node after node `at` (0-based); returns the new node's index.
    pub fn add_after(&mut self, at: usize) -> usize {
        let i = (at + 1).min(self.nodes.len());
        self.nodes.insert(i, Node::default());
        i
    }

    /// Add a neutral node before node `at`; returns its index.
    pub fn add_before(&mut self, at: usize) -> usize {
        let i = at.min(self.nodes.len());
        self.nodes.insert(i, Node::default());
        i
    }

    /// Delete node `at`. The last node is reset instead: a grade keeps one node, as in Resolve.
    pub fn delete(&mut self, at: usize) {
        if self.nodes.len() > 1 {
            if at < self.nodes.len() {
                self.nodes.remove(at);
            }
        } else if let Some(n) = self.nodes.first_mut() {
            *n = Node::default();
        }
    }

    /// Reset every node's controls, keeping the nodes (Reset Grades and Keep Nodes).
    pub fn reset_keep_nodes(&mut self) {
        self.nodes.iter_mut().for_each(Node::reset);
    }
}

/// One corrector node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// The node's label in the Node Editor ("" for none).
    pub label: String,
    pub enabled: bool,
    pub lift: Wheel,
    pub gamma: Wheel,
    pub gain: Wheel,
    pub offset: Wheel,
    pub contrast: f32,
    pub pivot: f32,
    pub saturation: f32,
    pub hue: f32,
    pub color_boost: f32,
    pub temp: f32,
    pub tint: f32,
    /// Shadows and Highlights (LOG D-055), -100 to 100, 0 neutral: the darkest and the lightest
    /// tones moved, black, white and the midtones kept. Grades stored before them have none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shadows: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub highlights: f32,
    /// Midtone Detail (LOG D-055), -100 to 100, 0 neutral: raised, local contrast in the
    /// midtones (definition); lowered, low-detail regions softened. It needs a pixel's
    /// neighbours, so it is left out of [`Node::apply`] (see [`Grade::apply_image`]).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub midtone_detail: f32,
    pub curves: Curves,
    /// Where the node's correction applies (LOG D-031): inside the enabled windows, which add
    /// up; everywhere when none is enabled. Grades stored before windows existed have none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<Window>,
    /// A colour key (LOG D-032): the correction applies by how much each pixel's colour is in its
    /// ranges. Grades stored before qualifiers existed have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<Qualifier>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            label: String::new(),
            enabled: true,
            lift: Wheel::splat(0.0),
            gamma: Wheel::splat(0.0),
            gain: Wheel::splat(1.0),
            offset: Wheel::splat(25.0),
            contrast: 1.0,
            pivot: 0.435,
            saturation: 50.0,
            hue: 50.0,
            color_boost: 0.0,
            temp: 0.0,
            tint: 0.0,
            shadows: 0.0,
            highlights: 0.0,
            midtone_detail: 0.0,
            curves: Curves::default(),
            windows: Vec::new(),
            qualifier: None,
        }
    }
}

impl Node {
    /// Back to neutral controls, keeping the label and whether the node is on (Reset Selected
    /// Node Grade).
    pub fn reset(&mut self) {
        *self = Self { label: std::mem::take(&mut self.label), enabled: self.enabled, ..Self::default() };
    }

    /// Whether the node changes nothing.
    pub fn is_neutral(&self) -> bool {
        self.tone_is_identity()
            && self.saturation().is_identity()
            && self.curves.hsl_is_identity()
            && self.midtone_detail == 0.0
    }

    /// Whether the node needs a pixel's neighbours: Midtone Detail, or a qualifier cleaned over
    /// the picture (LOG D-046, D-055).
    pub fn is_spatial(&self) -> bool {
        self.midtone_detail != 0.0 || self.keyed().is_some_and(Qualifier::is_spatial)
    }

    /// Midtone Detail's blur, a Gaussian's standard deviation in pixels of a picture `width`
    /// wide: 10 pixels at 1920, in proportion at other widths.
    pub fn detail_sigma(width: usize) -> f32 {
        width as f32 / 192.0
    }

    /// Midtone Detail on a pixel's correction `out`, from its input's luma `luma` and that luma
    /// blurred, `blurred` (LOG D-055): the local detail, `luma - blurred`, added to each channel
    /// by Midtone Detail / 100, most in the midtones (by 4·l·(1 − l)). Lowered, it moves the
    /// pixel towards the blur instead, the less the more detail there is (none past 0.08), so
    /// edges stay.
    pub fn detailed(&self, out: [f32; 3], luma: f32, blurred: f32) -> [f32; 3] {
        let md = self.midtone_detail / 100.0;
        if md == 0.0 {
            return out;
        }
        let d = luma - blurred;
        let a = if md < 0.0 { md * (1.0 - d.abs() / DETAIL_EDGE).clamp(0.0, 1.0) } else { md };
        let w = 4.0 * luma * (1.0 - luma);
        out.map(|v| (v + a * d * w).clamp(0.0, 1.0))
    }

    /// Whether every channel's tone function is the identity.
    pub fn tone_is_identity(&self) -> bool {
        let d = Self::default();
        self.lift == d.lift
            && self.gamma == d.gamma
            && self.gain == d.gain
            && self.offset == d.offset
            && self.contrast == d.contrast
            && self.temp == 0.0
            && self.tint == 0.0
            && self.shadows == 0.0
            && self.highlights == 0.0
            && self.curves.is_identity()
    }

    /// The tone function of `channel` (0 red, 1 green, 2 blue), in this order:
    ///
    /// 1. **Offset** adds `(Y − 25) + (channel − 25)` hundredths of the range.
    /// 2. **Lift** and **Gain**: `gain × (v + lift × (1 − v))`. Lift moves black and leaves white;
    ///    Gain scales from black. Temp and Tint are gains (see [`Node::white_balance`]).
    /// 3. **Gamma** raises the signal to the power `4^−gamma`: black and white stay, mids move.
    /// 4. **Contrast** about **Pivot**, as an S-curve that cannot clip (Resolve's default, "Use
    ///    S-curve for contrast").
    /// 5. The **Custom Curves**: Y, then the channel's own.
    ///
    /// Y is the master ring under each wheel and R, G and B the wheel's balance. They combine by
    /// adding for Lift, Gamma and Offset and by multiplying for Gain. (In Resolve the master ring
    /// moves all four numbers; here it moves Y alone, so R, G and B show the balance only.)
    pub fn tone(&self, channel: usize, x: f32) -> f32 {
        let k = channel.min(2);
        let lift = self.lift.y + self.lift.rgb()[k];
        let gamma = self.gamma.y + self.gamma.rgb()[k];
        let gain = self.gain.y * self.gain.rgb()[k] * self.white_balance()[k];
        let offset = (self.offset.y - 25.0 + self.offset.rgb()[k] - 25.0) / 100.0;
        let mut v = x + offset;
        v = gain * (v + lift * (1.0 - v));
        v = v.max(0.0).powf(GAMMA_BASE.powf(-gamma));
        v = contrast(v.clamp(0.0, 1.0), self.contrast, self.pivot);
        v = shadows_highlights(v, self.shadows, self.highlights);
        v = self.curves.y.eval(v);
        v = self.curves.channel(k).eval(v);
        v.clamp(0.0, 1.0)
    }

    /// Temp (±4000) as a gain towards orange (red up, blue down) or blue, and Tint (±100) towards
    /// magenta (red and blue up, green down) or green: the gains per channel.
    pub fn white_balance(&self) -> [f32; 3] {
        let t = self.temp / 4000.0;
        let m = self.tint / 100.0;
        [(1.0 + 0.25 * t) * (1.0 + 0.1 * m), 1.0 - 0.2 * m, (1.0 - 0.25 * t) * (1.0 + 0.1 * m)]
    }

    /// The tone function of `channel` as curve points for an engine to join with a monotone
    /// cubic. It starts from 33 samples, closer together near black and white where gamma and
    /// contrast bend the curve most, then halves each stretch where the joined points stray from
    /// the function by more than a quarter of an 8-bit code value (where a clip to black or white
    /// makes a corner), up to [`MAX_TONE_POINTS`].
    pub fn tone_points(&self, channel: usize) -> Vec<[f32; 2]> {
        const START: usize = 33;
        let at = |x: f32| [x, self.tone(channel, x)];
        let mut pts: Vec<[f32; 2]> =
            (0..START).map(|i| at(0.5 - 0.5 * (PI * i as f32 / (START - 1) as f32).cos())).collect();
        for _ in 0..8 {
            let stray: Vec<usize> = (0..pts.len() - 1)
                .filter(|&i| {
                    [0.25, 0.5, 0.75].iter().any(|t| {
                        let x = pts[i][0] + t * (pts[i + 1][0] - pts[i][0]);
                        (monotone_cubic(&pts, x) - self.tone(channel, x)).abs() > TONE_TOLERANCE
                    })
                })
                .collect();
            if stray.is_empty() || pts.len() + stray.len() > MAX_TONE_POINTS {
                break;
            }
            for &i in stray.iter().rev() {
                pts.insert(i + 1, at((pts[i][0] + pts[i + 1][0]) / 2.0));
            }
        }
        pts
    }

    /// The saturation step: Saturation (50 is unchanged, 100 doubles), Color Boost and Hue (50 is
    /// unchanged; 0 and 100 turn the hues half way round).
    pub fn saturation(&self) -> Saturation {
        Saturation {
            amount: self.saturation / 50.0,
            boost: self.color_boost / 100.0,
            hue_turns: (self.hue - 50.0) / 100.0,
        }
    }

    /// The node applied to one pixel inside its windows (a disabled node passes it through).
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        if !self.enabled {
            return rgb;
        }
        self.apply_keyed(rgb, self.keyed().map(|q| q.clean(q.key(rgb))))
    }

    /// The node applied to one pixel inside its windows, its correction blended by `key` (the
    /// qualifier's matte at that pixel) when it has one.
    pub fn apply_keyed(&self, rgb: [f32; 3], key: Option<f32>) -> [f32; 3] {
        self.apply_detailed(rgb, key, None)
    }

    /// [`Node::apply_keyed`] with the pixel's blurred input luma for Midtone Detail (LOG D-055).
    pub fn apply_detailed(&self, rgb: [f32; 3], key: Option<f32>, blurred: Option<f32>) -> [f32; 3] {
        if !self.enabled {
            return rgb;
        }
        let toned = [self.tone(0, rgb[0]), self.tone(1, rgb[1]), self.tone(2, rgb[2])];
        let mut out = self.color(toned);
        if let Some(b) = blurred {
            out = self.detailed(out, luma(rgb), b);
        }
        match key {
            // the correction, blended by how much the node's input is in the key
            Some(k) => [0, 1, 2].map(|c| rgb[c] + (out[c] - rgb[c]) * k),
            None => out,
        }
    }

    /// The saturation step and then the hue curves, on one pixel (after the tone functions).
    ///
    /// The hue curves and Hue's rotation run together in HSL, each curve read at the pixel's own
    /// hue, saturation or lightness as they were before any of them (as Lumetri runs its HSL
    /// curves, so a grade compiles to them exactly):
    ///
    /// - hue turns by Hue's rotation plus Hue vs Hue's distance from the middle line (± half a
    ///   turn);
    /// - saturation is multiplied by twice each of Hue vs Sat, Lum vs Sat and Sat vs Sat (the
    ///   middle line is ×1, the top ×2, the bottom grey);
    /// - lightness moves by half of Hue vs Lum's distance from the middle line, and by half of
    ///   Sat vs Lum's (LOG D-047).
    pub fn color(&self, rgb: [f32; 3]) -> [f32; 3] {
        let s = self.saturation();
        let mut v = s.boosted(rgb);
        let c = &self.curves;
        if s.hue_turns.abs() > 1e-6 || !c.hsl_is_identity() {
            let [h0, s0, l0] = rgb_to_hsl(v);
            let hue = (h0 + s.hue_turns + c.hue_hue.eval(h0, true) - 0.5).rem_euclid(1.0);
            let gain = 8.0 * c.hue_sat.eval(h0, true) * c.lum_sat.eval(l0, false) * c.sat_sat.eval(s0, false);
            let light = l0 + (c.hue_lum.eval(h0, true) - 0.5) * 0.5 + (c.sat_lum.eval(s0, false) - 0.5) * 0.5;
            v = hsl_to_rgb([hue, (s0 * gain).clamp(0.0, 1.0), light.clamp(0.0, 1.0)]).map(|q| q.clamp(0.0, 1.0));
        }
        v
    }

    /// The node's qualifier, when it is on.
    pub fn keyed(&self) -> Option<&Qualifier> {
        self.qualifier.as_ref().filter(|q| q.enabled)
    }

    /// The node (inside its windows) as a 3D table of `n` points per side, red changing fastest,
    /// as a `.cube` file lists it: how an engine without colour keys carries a qualified node
    /// (LOG D-032).
    pub fn table(&self, n: usize) -> Vec<[f32; 3]> {
        let n = n.max(2);
        let at = |i: usize| i as f32 / (n - 1) as f32;
        (0..n * n * n).map(|i| self.apply([at(i % n), at(i / n % n), at(i / (n * n))])).collect()
    }

    /// Set the controls named in `patch`, a JSON object of this node's fields; a wheel may name
    /// only some of `y`, `r`, `g` and `b`, and `curves` only some curves. The other controls keep
    /// their values; values are held to their ranges. Unknown fields are refused.
    pub fn patch(&mut self, patch: &Value) -> Result<(), String> {
        if !patch.is_object() {
            return Err("a node's controls are a JSON object".into());
        }
        let mut v = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        merge(&mut v, patch);
        let mut n: Node = serde_json::from_value(v).map_err(|e| e.to_string())?;
        n.clamp();
        *self = n;
        Ok(())
    }

    /// Hold every control to its range.
    fn clamp(&mut self) {
        self.lift = self.lift.clamped(-1.0, 1.0);
        self.gamma = self.gamma.clamped(-1.0, 1.0);
        self.gain = self.gain.clamped(0.0, 4.0);
        self.offset = self.offset.clamped(0.0, 100.0);
        self.contrast = self.contrast.clamp(0.0, 2.0);
        self.pivot = self.pivot.clamp(0.0, 1.0);
        self.saturation = self.saturation.clamp(0.0, 100.0);
        self.hue = self.hue.clamp(0.0, 100.0);
        self.color_boost = self.color_boost.clamp(-100.0, 100.0);
        self.temp = self.temp.clamp(-4000.0, 4000.0);
        self.tint = self.tint.clamp(-100.0, 100.0);
        self.shadows = self.shadows.clamp(-100.0, 100.0);
        self.highlights = self.highlights.clamp(-100.0, 100.0);
        self.midtone_detail = self.midtone_detail.clamp(-100.0, 100.0);
        self.windows.iter_mut().for_each(Window::clamp);
        if let Some(q) = &mut self.qualifier {
            q.clamp();
        }
        for c in [&mut self.curves.y, &mut self.curves.r, &mut self.curves.g, &mut self.curves.b] {
            c.tidy();
        }
        for (c, periodic) in self.curves.hsl_mut() {
            c.tidy(periodic);
        }
    }
}

/// Merge `patch` into `v`: objects key by key, anything else replaced.
fn merge(v: &mut Value, patch: &Value) {
    match (v, patch) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, pv) in b {
                match a.get_mut(k) {
                    Some(av) => merge(av, pv),
                    None => {
                        a.insert(k.clone(), pv.clone());
                    }
                }
            }
        }
        (v, p) => *v = p.clone(),
    }
}

/// Contrast `c` about `pivot` as an S-curve: a power of the distance from black below the pivot
/// and from white above it, so black and white stay put and the slope at the pivot is `c`.
fn contrast(v: f32, c: f32, pivot: f32) -> f32 {
    if (c - 1.0).abs() < 1e-6 {
        return v;
    }
    let p = pivot.clamp(0.01, 0.99);
    if v <= p { p * (v / p).powf(c) } else { 1.0 - (1.0 - p) * ((1.0 - v) / (1.0 - p)).powf(c) }
}

/// An HSL qualifier (LOG D-032): a key on the node's input picking colours by hue, saturation
/// and luminance. Every number runs 0 to 100, as in Resolve's palette. Hue goes once round the
/// colour wheel from red (0) through yellow, green, cyan, blue and magenta back to red (100);
/// saturation is HSL saturation, luminance Rec. 709 luma. Each range is full inside and falls to
/// nothing over its soft width outside; a range that is off keys everything.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qualifier {
    pub enabled: bool,
    /// Key the colours outside the ranges instead.
    pub invert: bool,
    pub hue_on: bool,
    pub hue_center: f32,
    /// The hue range's full width.
    pub hue_width: f32,
    pub hue_soft: f32,
    pub sat_on: bool,
    pub sat_low: f32,
    pub sat_high: f32,
    pub sat_low_soft: f32,
    pub sat_high_soft: f32,
    pub lum_on: bool,
    pub lum_low: f32,
    pub lum_high: f32,
    pub lum_low_soft: f32,
    pub lum_high_soft: f32,
    /// Matte finesse (LOG D-046), cleaning the key in this order; 0 leaves it as keyed. Grades
    /// stored before it have none. Denoise: passes of a small median over the key, more for more.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub denoise: f32,
    /// Clean Black: the key's black point raised, so faint keying goes.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub clean_black: f32,
    /// Clean White: the key's white point lowered, so nearly keyed is keyed.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub clean_white: f32,
    /// Blur Radius: the key softened by a Gaussian.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub blur: f32,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl Default for Qualifier {
    /// Reds of some saturation in the midtones, until a colour is picked.
    fn default() -> Self {
        Self {
            enabled: true,
            invert: false,
            hue_on: true,
            hue_center: 0.0,
            hue_width: 10.0,
            hue_soft: 5.0,
            sat_on: true,
            sat_low: 20.0,
            sat_high: 100.0,
            sat_low_soft: 10.0,
            sat_high_soft: 0.0,
            lum_on: true,
            lum_low: 10.0,
            lum_high: 90.0,
            lum_low_soft: 10.0,
            lum_high_soft: 10.0,
            denoise: 0.0,
            clean_black: 0.0,
            clean_white: 0.0,
            blur: 0.0,
        }
    }
}

/// 1 inside `[lo, hi]`, falling to 0 over `lo_soft` below it and `hi_soft` above, smoothly.
fn range(v: f32, lo: f32, hi: f32, lo_soft: f32, hi_soft: f32) -> f32 {
    let fall = |d: f32, soft: f32| {
        if d <= 0.0 {
            1.0
        } else if soft <= 1e-6 {
            0.0
        } else {
            let t = (1.0 - d / soft).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }
    };
    fall(lo - v, lo_soft) * fall(v - hi, hi_soft)
}

impl Qualifier {
    /// How much of `rgb` (0 to 1) the key takes: 0 to 1.
    pub fn key(&self, rgb: [f32; 3]) -> f32 {
        let hsl = rgb_to_hsl(rgb.map(|v| v.clamp(0.0, 1.0)));
        let mut k = 1.0;
        if self.hue_on {
            // distance round the wheel, in hundredths of a turn
            let d = (hsl[0] * 100.0 - self.hue_center).rem_euclid(100.0);
            let d = d.min(100.0 - d);
            k *= range(d, -1.0, self.hue_width / 2.0, 0.0, self.hue_soft);
        }
        if self.sat_on {
            k *= range(hsl[1] * 100.0, self.sat_low, self.sat_high, self.sat_low_soft, self.sat_high_soft);
        }
        if self.lum_on {
            let l = luma(rgb.map(|v| v.clamp(0.0, 1.0))) * 100.0;
            k *= range(l, self.lum_low, self.lum_high, self.lum_low_soft, self.lum_high_soft);
        }
        if self.invert { 1.0 - k } else { k }
    }

    /// Clean Black and Clean White on a key value: the black point raised by Clean Black / 200 and
    /// the white point lowered by Clean White / 200, the key stretched between them.
    pub fn clean(&self, k: f32) -> f32 {
        if self.clean_black <= 0.0 && self.clean_white <= 0.0 {
            return k;
        }
        let (b, w) = (self.clean_black / 200.0, 1.0 - self.clean_white / 200.0);
        ((k - b) / (w - b).max(1e-3)).clamp(0.0, 1.0)
    }

    /// Whether the matte depends on a pixel's neighbours (Denoise or Blur Radius set).
    pub fn is_spatial(&self) -> bool {
        self.denoise > 0.0 || self.blur > 0.0
    }

    /// Denoise's passes of a three-pixel median across, then down, for a picture `width` pixels
    /// wide: up to three at 1920 pixels, in proportion at other widths, at least one when set.
    pub fn denoise_passes(&self, width: usize) -> usize {
        if self.denoise <= 0.0 {
            return 0;
        }
        ((self.denoise / 100.0 * 3.0 * width as f32 / 1920.0).round() as usize).max(1)
    }

    /// Blur Radius as a Gaussian's standard deviation in pixels of a picture `width` wide: up to
    /// 10 pixels at 1920, in proportion at other widths.
    pub fn blur_sigma(&self, width: usize) -> f32 {
        self.blur / 100.0 * width as f32 / 192.0
    }

    /// The matte over a picture (`w` × `h`, row by row): the key of each pixel, then Denoise, then
    /// Clean Black and Clean White, then Blur Radius. A pixel's neighbours beyond the picture's
    /// edge are the edge's.
    pub fn matte(&self, px: &[[f32; 3]], w: usize, h: usize) -> Vec<f32> {
        let mut k: Vec<f32> = px.iter().map(|p| self.key(*p)).collect();
        for _ in 0..self.denoise_passes(w) {
            k = median3(&k, w, h, true);
            k = median3(&k, w, h, false);
        }
        for v in &mut k {
            *v = self.clean(*v);
        }
        let sigma = self.blur_sigma(w);
        if sigma > 0.0 {
            let weights = gaussian(sigma);
            k = convolve(&k, w, h, &weights, true);
            k = convolve(&k, w, h, &weights, false);
        }
        k
    }

    /// A qualifier centred on one picked colour (Resolve's qualifier picker): its hue, its
    /// saturation and its luminance, with some room either side.
    pub fn picked(rgb: [f32; 3]) -> Self {
        let hsl = rgb_to_hsl(rgb.map(|v| v.clamp(0.0, 1.0)));
        let (s, l) = (hsl[1] * 100.0, luma(rgb.map(|v| v.clamp(0.0, 1.0))) * 100.0);
        Self {
            hue_center: hsl[0] * 100.0,
            hue_width: 8.0,
            hue_soft: 6.0,
            sat_low: (s - 20.0).max(0.0),
            sat_high: (s + 20.0).min(100.0),
            sat_low_soft: 10.0,
            sat_high_soft: 10.0,
            lum_low: (l - 25.0).max(0.0),
            lum_high: (l + 25.0).min(100.0),
            lum_low_soft: 10.0,
            lum_high_soft: 10.0,
            ..Self::default()
        }
    }

    fn clamp(&mut self) {
        self.hue_center = self.hue_center.rem_euclid(100.0);
        for v in [
            &mut self.hue_width,
            &mut self.hue_soft,
            &mut self.sat_low,
            &mut self.sat_high,
            &mut self.sat_low_soft,
            &mut self.sat_high_soft,
            &mut self.lum_low,
            &mut self.lum_high,
            &mut self.lum_low_soft,
            &mut self.lum_high_soft,
            &mut self.denoise,
            &mut self.clean_black,
            &mut self.clean_white,
            &mut self.blur,
        ] {
            *v = v.clamp(0.0, 100.0);
        }
    }
}

/// The value at `(x, y)` of a `w` × `h` map, coordinates held to its edges.
fn at(k: &[f32], w: usize, h: usize, x: isize, y: isize) -> f32 {
    k[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize]
}

/// The median of each value and its two neighbours across (`across`) or down.
fn median3(k: &[f32], w: usize, h: usize, across: bool) -> Vec<f32> {
    let (dx, dy) = if across { (1, 0) } else { (0, 1) };
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let (a, b, c) = (at(k, w, h, x - dx, y - dy), k[i], at(k, w, h, x + dx, y + dy));
            a.min(b).max(a.max(b).min(c))
        })
        .collect()
}

/// A Gaussian's weights from its centre outwards, to three standard deviations, summing to 1
/// over both sides.
pub fn gaussian(sigma: f32) -> Vec<f32> {
    let r = (3.0 * sigma).ceil().max(1.0) as usize;
    let mut wts: Vec<f32> = (0..=r).map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp()).collect();
    let total = wts[0] + 2.0 * wts[1..].iter().sum::<f32>();
    for v in &mut wts {
        *v /= total;
    }
    wts
}

/// `k` convolved across (`across`) or down with the symmetric `weights` (centre first).
fn convolve(k: &[f32], w: usize, h: usize, weights: &[f32], across: bool) -> Vec<f32> {
    let (dx, dy) = if across { (1, 0) } else { (0, 1) };
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let mut v = weights[0] * k[i];
            for (j, wt) in weights.iter().enumerate().skip(1) {
                let j = j as isize;
                v += wt * (at(k, w, h, x - j * dx, y - j * dy) + at(k, w, h, x + j * dx, y + j * dy));
            }
            v
        })
        .collect()
}

/// A window's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// Resolve's Linear window: a rectangle.
    Linear,
    /// Resolve's Circle window: an ellipse.
    Circle,
    /// Resolve's Gradient window (LOG D-052): the picture divided by a line through the window's
    /// centre, the side above it keyed (at Rotate 0; turning clockwise with Rotate), soft across
    /// the line. Its size is not used.
    Gradient,
    /// Resolve's Polygon window (LOG D-053): straight sides through its `points`.
    Polygon,
}

/// The most corners a Polygon window has (Resolve's limit).
pub const MAX_POLYGON_POINTS: usize = 128;

/// A window (LOG D-031): limits its node's correction to a shape on the clip's picture, with a
/// soft edge. Positions are fractions of the picture: `center` across its width and down its
/// height, from the top left. Sizes are fractions of the picture's *width* on both axes, so equal
/// numbers are a circle or a square however wide the picture is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub shape: Shape,
    pub enabled: bool,
    /// Apply the correction outside the shape instead.
    pub invert: bool,
    pub center: [f32; 2],
    /// Width and height (not used by a Gradient).
    pub size: [f32; 2],
    /// Degrees, clockwise.
    pub rotation: f32,
    /// The soft edge's width, centred on the shape's edge, as a fraction of the picture's width.
    pub softness: f32,
    /// How strongly the window lets the correction through, 0 to 100.
    pub opacity: f32,
    /// A Polygon's corners, in order, in the window's own space (LOG D-053): -1 to 1 across its
    /// size, before it is turned and placed, as a Linear window's corners are at ±1. So Pan,
    /// Tilt, Size, Aspect and Rotate move a polygon as they move the other shapes. Other shapes
    /// have none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f32; 2]>,
}

impl Window {
    /// A new window in the middle of the picture: a third of its width across.
    pub fn new(shape: Shape) -> Self {
        let size = match shape {
            Shape::Circle => [0.33, 0.33],
            Shape::Linear | Shape::Polygon => [0.4, 0.25],
            Shape::Gradient => [1.0, 1.0],
        };
        Self {
            shape,
            enabled: true,
            invert: false,
            center: [0.5, 0.5],
            size,
            rotation: 0.0,
            // a gradient is soft: a fifth of the picture's width across its line
            softness: if shape == Shape::Gradient { 0.2 } else { 0.02 },
            opacity: 100.0,
            // a polygon starts as Resolve's does: a rectangle of four corners
            points: if shape == Shape::Polygon { Self::SQUARE.to_vec() } else { Vec::new() },
        }
    }

    const SQUARE: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];

    fn clamp(&mut self) {
        self.center = self.center.map(|v| v.clamp(-1.0, 2.0));
        self.size = self.size.map(|v| v.clamp(0.002, 4.0));
        self.rotation = self.rotation.clamp(-360.0, 360.0);
        self.softness = self.softness.clamp(0.0, 1.0);
        self.opacity = self.opacity.clamp(0.0, 100.0);
        if self.shape == Shape::Polygon {
            for p in &mut self.points {
                *p = p.map(|v| v.clamp(-4.0, 4.0));
            }
            self.points.truncate(MAX_POLYGON_POINTS);
            // fewer than three corners enclose nothing
            if self.points.len() < 3 {
                self.points = Self::SQUARE.to_vec();
            }
        } else {
            self.points.clear();
        }
    }
}

/// The four numbers under a Primaries wheel: `y`, the master (the ring under the wheel), and `r`,
/// `g` and `b`, the colour balance (the wheel itself).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wheel {
    pub y: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Wheel {
    pub const fn splat(v: f32) -> Self {
        Self { y: v, r: v, g: v, b: v }
    }

    fn rgb(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }

    fn clamped(self, lo: f32, hi: f32) -> Self {
        Self { y: self.y.clamp(lo, hi), r: self.r.clamp(lo, hi), g: self.g.clamp(lo, hi), b: self.b.clamp(lo, hi) }
    }

    /// The colour balance as a point on the wheel, from how R, G and B differ from `neutral`:
    /// x towards blue and y towards red, as on a vectorscope (Rec. 709 Cb and Cr); `scale` is
    /// the balance at the wheel's edge.
    pub fn balance(&self, neutral: f32, scale: f32) -> [f32; 2] {
        let d = [self.r - neutral, self.g - neutral, self.b - neutral];
        let y = luma(d);
        [(d[2] - y) / 1.8556 / scale, (d[0] - y) / 1.5748 / scale]
    }

    /// Set R, G and B for a point on the wheel (see [`Wheel::balance`]) without changing luma:
    /// moving the balance does not brighten or darken.
    pub fn set_balance(&mut self, xy: [f32; 2], neutral: f32, scale: f32) {
        let dr = 1.5748 * xy[1] * scale;
        let db = 1.8556 * xy[0] * scale;
        let dg = -(0.2126 * dr + 0.0722 * db) / 0.7152;
        self.r = neutral + dr;
        self.g = neutral + dg;
        self.b = neutral + db;
    }
}

/// The Custom Curves (Y, applied to all three channels, then R, G and B) and the hue curves
/// (LOG D-043; see [`Node::color`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curves {
    pub y: Curve,
    pub r: Curve,
    pub g: Curve,
    pub b: Curve,
    /// Hue vs Hue: by hue, how far it turns. Grades stored before the hue curves have none.
    #[serde(default, skip_serializing_if = "HslCurve::is_empty")]
    pub hue_hue: HslCurve,
    /// Hue vs Sat: by hue, the saturation.
    #[serde(default, skip_serializing_if = "HslCurve::is_empty")]
    pub hue_sat: HslCurve,
    /// Hue vs Lum: by hue, the lightness.
    #[serde(default, skip_serializing_if = "HslCurve::is_empty")]
    pub hue_lum: HslCurve,
    /// Lum vs Sat: by lightness, the saturation.
    #[serde(default, skip_serializing_if = "HslCurve::is_empty")]
    pub lum_sat: HslCurve,
    /// Sat vs Sat: by saturation, the saturation.
    #[serde(default, skip_serializing_if = "HslCurve::is_empty")]
    pub sat_sat: HslCurve,
    /// Sat vs Lum: by saturation, the lightness (LOG D-047).
    #[serde(default, skip_serializing_if = "HslCurve::is_empty")]
    pub sat_lum: HslCurve,
}

/// The hue curves' JSON names, in order, and whether each runs round the colour wheel.
pub const HSL_CURVES: [(&str, bool); 6] = [
    ("hue_hue", true),
    ("hue_sat", true),
    ("hue_lum", true),
    ("lum_sat", false),
    ("sat_sat", false),
    ("sat_lum", false),
];

impl Curves {
    /// Whether the Custom Curves are all the identity (the hue curves aside).
    pub fn is_identity(&self) -> bool {
        [&self.y, &self.r, &self.g, &self.b].iter().all(|c| c.is_identity())
    }

    /// Whether every hue curve leaves the picture as it is.
    pub fn hsl_is_identity(&self) -> bool {
        self.hsl().iter().all(|(c, _)| c.is_identity())
    }

    /// The hue curves in [`HSL_CURVES`]' order, each with whether it runs round the wheel.
    pub fn hsl(&self) -> [(&HslCurve, bool); 6] {
        [
            (&self.hue_hue, true),
            (&self.hue_sat, true),
            (&self.hue_lum, true),
            (&self.lum_sat, false),
            (&self.sat_sat, false),
            (&self.sat_lum, false),
        ]
    }

    fn hsl_mut(&mut self) -> [(&mut HslCurve, bool); 6] {
        [
            (&mut self.hue_hue, true),
            (&mut self.hue_sat, true),
            (&mut self.hue_lum, true),
            (&mut self.lum_sat, false),
            (&mut self.sat_sat, false),
            (&mut self.sat_lum, false),
        ]
    }

    fn channel(&self, k: usize) -> &Curve {
        match k {
            0 => &self.r,
            1 => &self.g,
            _ => &self.b,
        }
    }
}

/// A custom curve: control points (input, output) in 0…1, joined by a monotone cubic (it does not
/// overshoot between points). Black (0, 0) and white (1, 1) are implied where no point sits at 0
/// or 1, so no points, or points on the diagonal, is the identity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Curve(pub Vec<[f32; 2]>);

impl Curve {
    pub fn is_identity(&self) -> bool {
        self.0.iter().all(|p| (p[0] - p[1]).abs() < 1e-6)
    }

    /// The points with black and white added where missing, sorted: what a curve editor shows.
    pub fn knots(&self) -> Vec<[f32; 2]> {
        let mut k = self.0.clone();
        k.sort_by(|a, b| a[0].total_cmp(&b[0]));
        if k.first().is_none_or(|p| p[0] > 0.0) {
            k.insert(0, [0.0, 0.0]);
        }
        if k.last().is_none_or(|p| p[0] < 1.0) {
            k.push([1.0, 1.0]);
        }
        k
    }

    /// The curve at `x`.
    pub fn eval(&self, x: f32) -> f32 {
        if self.0.is_empty() {
            return x;
        }
        monotone_cubic(&self.knots(), x)
    }

    /// Points into 0…1, sorted, one per input.
    fn tidy(&mut self) {
        for p in &mut self.0 {
            *p = [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)];
        }
        self.0.sort_by(|a, b| a[0].total_cmp(&b[0]));
        self.0.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-6);
    }
}

/// A hue curve (LOG D-043): control points (input, output) in 0…1, where an output of ½ (the
/// middle line) changes nothing. No points is the middle line. Joined by a monotone cubic; a
/// curve over hue runs round the colour wheel (the last point joins the first), the others hold
/// their end points' values beyond them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HslCurve(pub Vec<[f32; 2]>);

impl HslCurve {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_identity(&self) -> bool {
        self.0.iter().all(|p| (p[1] - 0.5).abs() < 1e-6)
    }

    /// The curve at `x`; `periodic` for a curve over hue.
    pub fn eval(&self, x: f32, periodic: bool) -> f32 {
        let mut pts = self.0.clone();
        pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        match pts.len() {
            0 => 0.5,
            1 => pts[0][1],
            _ if periodic => {
                // a turn's worth of points either side, so the ends join
                let around: Vec<[f32; 2]> =
                    [-1.0f32, 0.0, 1.0].iter().flat_map(|o| pts.iter().map(move |p| [p[0] + o, p[1]])).collect();
                monotone_cubic(&around, x.rem_euclid(1.0))
            }
            _ => monotone_cubic(&pts, x),
        }
    }

    /// Points into 0…1 (over hue, 1 is 0 again), sorted, one per input.
    fn tidy(&mut self, periodic: bool) {
        for p in &mut self.0 {
            let x = if periodic { p[0].rem_euclid(1.0) } else { p[0].clamp(0.0, 1.0) };
            *p = [x, p[1].clamp(0.0, 1.0)];
        }
        self.0.sort_by(|a, b| a[0].total_cmp(&b[0]));
        self.0.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-6);
    }
}

/// The monotone cubic (Fritsch–Carlson) through `p` (sorted by x, at least two points) at `x`;
/// beyond the ends it holds the end values.
pub fn monotone_cubic(p: &[[f32; 2]], x: f32) -> f32 {
    let n = p.len();
    if n == 0 {
        return x;
    }
    if n == 1 || x <= p[0][0] {
        return p[0][1];
    }
    if x >= p[n - 1][0] {
        return p[n - 1][1];
    }
    let m = tangents(p);
    let i = p.partition_point(|q| q[0] <= x).clamp(1, n - 1) - 1;
    let h = p[i + 1][0] - p[i][0];
    if h <= 0.0 {
        return p[i + 1][1];
    }
    let t = (x - p[i][0]) / h;
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * p[i][1]
        + (t3 - 2.0 * t2 + t) * h * m[i]
        + (-2.0 * t3 + 3.0 * t2) * p[i + 1][1]
        + (t3 - t2) * h * m[i + 1]
}

/// Fritsch–Carlson tangents: the mean of the neighbouring slopes, zero at a turn, limited so the
/// curve stays monotone between monotone points.
fn tangents(p: &[[f32; 2]]) -> Vec<f32> {
    let n = p.len();
    let d: Vec<f32> = (0..n - 1).map(|i| (p[i + 1][1] - p[i][1]) / (p[i + 1][0] - p[i][0]).max(1e-9)).collect();
    let mut m = vec![0.0; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 { 0.0 } else { (d[i - 1] + d[i]) / 2.0 };
    }
    for i in 0..n - 1 {
        if d[i].abs() < 1e-9 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let (a, b) = (m[i] / d[i], m[i + 1] / d[i]);
        let s = a * a + b * b;
        if s > 9.0 {
            let k = 3.0 / s.sqrt();
            m[i] = k * a * d[i];
            m[i + 1] = k * b * d[i];
        }
    }
    m
}

/// A node's saturation step (see [`Node::saturation`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Saturation {
    /// Saturation as a factor: 1 unchanged, 0 grey, 2 double.
    pub amount: f32,
    /// Color Boost, −1…1: extra saturation that falls off as colours get more saturated.
    pub boost: f32,
    /// Hue rotation in turns, −½…½.
    pub hue_turns: f32,
}

impl Saturation {
    pub fn is_identity(&self) -> bool {
        (self.amount - 1.0).abs() < 1e-6 && self.boost.abs() < 1e-6 && self.hue_turns.abs() < 1e-6
    }

    /// Saturation and Color Boost about the pixel's Rec. 709 luma (luma is kept), then the hue
    /// rotation (in HSL); held to 0…1 after each.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let mut v = self.boosted(rgb);
        if self.hue_turns.abs() > 1e-6 {
            let mut h = rgb_to_hsl(v);
            h[0] = (h[0] + self.hue_turns).rem_euclid(1.0);
            v = hsl_to_rgb(h).map(|q| q.clamp(0.0, 1.0));
        }
        v
    }
}

impl Saturation {
    /// Saturation and Color Boost about the pixel's Rec. 709 luma (luma is kept), held to 0…1.
    pub fn boosted(&self, rgb: [f32; 3]) -> [f32; 3] {
        let l = luma(rgb);
        let spread = (rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2])).clamp(0.0, 1.0);
        let s = self.amount * (1.0 + self.boost * (1.0 - spread));
        rgb.map(|q| (l + (q - l) * s).clamp(0.0, 1.0))
    }
}

/// Rec. 709 luma.
pub fn luma(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// RGB to hue (turns), saturation and lightness.
pub fn rgb_to_hsl(c: [f32; 3]) -> [f32; 3] {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-9 {
        return [0.0, 0.0, l];
    }
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == c[0] {
        (c[1] - c[2]) / d + if c[1] < c[2] { 6.0 } else { 0.0 }
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h / 6.0, s, l]
}

/// Hue (turns), saturation and lightness to RGB.
pub fn hsl_to_rgb(h: [f32; 3]) -> [f32; 3] {
    let (hue, s, l) = (h[0], h[1], h[2]);
    if s < 1e-9 {
        return [l, l, l];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let ch = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [ch(hue + 1.0 / 3.0), ch(hue), ch(hue - 1.0 / 3.0)]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
