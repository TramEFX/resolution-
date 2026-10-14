//! Colour grades in Resolution's own types (LOG D-028): the corrector nodes of the Color page and
//! the controls on them. A clip's grade is a chain of serial nodes; each holds the Primaries
//! palette's controls and the Custom Curves.
//!
//! **The math is ours.** Resolve does not publish its formulas, so each control does what the
//! Resolve manual says it does, by formulas defined here:
//!
//! - The signal is display-encoded RGB, 0 black and 1 white (the grading signal of an SDR
//!   timeline), held to 0…1 after each node.
//! - Every control except Saturation, Hue and Color Boost acts on each colour channel on its own,
//!   so a node is a **tone function** per channel ([`Node::tone`]) followed by a **saturation
//!   step** ([`Node::saturation`]). These two parts are what an engine has to reproduce.
//!
//! Numbers are in Resolve's units where the manual gives them: Lift and Gamma 0, Gain 1 and
//! Offset 25 are neutral; Contrast 1 and Pivot 0.435; Saturation and Hue 50; Color Boost, Temp
//! and Tint 0.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The most points [`Node::tone_points`] gives for one channel.
pub const MAX_TONE_POINTS: usize = 129;

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

    /// The grade applied to one pixel.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        self.nodes.iter().fold(rgb, |c, n| n.apply(c))
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
    pub curves: Curves,
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
            curves: Curves::default(),
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
        self.tone_is_identity() && self.saturation().is_identity()
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

    /// The node applied to one pixel (a disabled node passes it through).
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        if !self.enabled {
            return rgb;
        }
        let toned = [self.tone(0, rgb[0]), self.tone(1, rgb[1]), self.tone(2, rgb[2])];
        self.saturation().apply(toned)
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
        for c in [&mut self.curves.y, &mut self.curves.r, &mut self.curves.g, &mut self.curves.b] {
            c.tidy();
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

/// The Custom Curves: Y (applied to all three channels) then R, G and B.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curves {
    pub y: Curve,
    pub r: Curve,
    pub g: Curve,
    pub b: Curve,
}

impl Curves {
    pub fn is_identity(&self) -> bool {
        [&self.y, &self.r, &self.g, &self.b].iter().all(|c| c.is_identity())
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
        let l = luma(rgb);
        let spread = (rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2])).clamp(0.0, 1.0);
        let s = self.amount * (1.0 + self.boost * (1.0 - spread));
        let mut v = rgb.map(|q| (l + (q - l) * s).clamp(0.0, 1.0));
        if self.hue_turns.abs() > 1e-6 {
            let mut h = rgb_to_hsl(v);
            h[0] = (h[0] + self.hue_turns).rem_euclid(1.0);
            v = hsl_to_rgb(h).map(|q| q.clamp(0.0, 1.0));
        }
        v
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
