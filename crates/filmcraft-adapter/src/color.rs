//! The Color page's grades (LOG D-028).
//!
//! A clip's grade is a [`Grade`] in Resolution's own types. It is kept as JSON in the timeline's
//! metadata (`resolution.grade.<clip id>`) and compiled onto the clip as Lumetri Color effects at
//! the front of its effect stack. The viewers, frame renders and Deliver draw the grade by its own
//! math (`gpu.rs`, LOG D-044); the compile is what FilmCraft shows, and what renders without a GPU
//! use. Each edit is one FilmCraft undo step holding both, so undo keeps them in step.
//!
//! A node compiles to at most two Lumetri effects: its tone functions as the R, G and B curves
//! (every other section off), then its saturation step as Lumetri's Saturation and Vibrance (both
//! keep luma), with Hue as a constant Hue vs Hue curve and the hue curves as Lumetri's HSL curves
//! (LOG D-043). A neutral or disabled node compiles to nothing, so an ungraded clip costs nothing
//! to render.
//!
//! A qualified node (LOG D-032) cannot be a curve and a saturation: Lumetri cannot limit them to a
//! colour key. It becomes one Lumetri effect whose input LUT is the node itself, key and all,
//! baked into a 33-point 3D LUT kept in the project's LUT library (`resolution-<hash>`). LUTs no
//! clip uses any more are dropped at the next grade edit.
//!
//! A node's windows (LOG D-031) become effect masks on its Lumetri effects, in the clip's pixels:
//! a Circle window an ellipse, a Linear window a rectangle, turned, with its softness as the
//! feather. Masks add up, as Resolve's windows do.
//!
//! FilmCraft copies a clip's effects when it copies the clip (a split, a paste) but not our
//! metadata. A clip with no stored grade whose Lumetri effects are exactly what another of the
//! timeline's grades compiles to takes that grade. Lumetri effects that match no grade are
//! "foreign" (made outside the Color page); the next grade edit replaces them.

use filmcraft_engine::project::{
    ClipId, EffectInstance, ItemId, ItemKind, Mask, MaskPath, Param, ParamValue, Project, ProjectLut, find_effect,
};
use filmcraft_geom::Vec2;
use resolution_engine_api::{EngineError, Outcome, Result};
use resolution_grade::{Grade, Node, Shape, Window};
use serde_json::{Value, json};

use crate::FilmCraftEngine;

const KEY: &str = "resolution.grade.";

/// The project LUTs that carry qualified nodes.
const LUT_PREFIX: &str = "resolution-";

/// Points per side of a qualified node's LUT.
const LUT_POINTS: usize = 33;
/// Points a compiled hue curve is sampled at (LOG D-043).
const HSL_SAMPLES: usize = 96;

fn key(clip: ClipId) -> String {
    format!("{KEY}{}", clip.0)
}

impl FilmCraftEngine {
    /// The `color.*` commands.
    pub(crate) fn color(&mut self, id: &str, params: Value) -> Result<Outcome> {
        let bad = |msg: String| EngineError::BadParams { cmd: id.to_string(), msg };
        let seq = self
            .session
            .state
            .active_sequence
            .ok_or_else(|| EngineError::Disabled(id.to_string(), "no timeline is open".into()))?;
        let clip = match params.get("clip") {
            Some(v) => ClipId(v.as_u64().ok_or_else(|| bad("`clip` is a clip id".into()))?),
            None => self
                .clip_at_playhead(seq)
                .ok_or_else(|| bad("no video clip under the playhead; name the `clip`".into()))?,
        };
        let (mut grade, foreign) =
            self.grade_of(seq, clip).ok_or_else(|| bad(format!("no clip {} on the open timeline", clip.0)))?;
        let size = self.clip_size(seq, clip);
        if id == "color.inspect" {
            return Ok(Outcome {
                result: view(clip, &grade, foreign, size),
                revision: self.session.revision,
                undo_label: None,
            });
        }
        let count = grade.nodes.len();
        // nodes are numbered from 1, as in the Node Editor
        let number = |v: &Value| v.as_u64().filter(|n| (1..=count as u64).contains(n)).map(|n| n as usize - 1);
        let node =
            |name: &str| number(&params[name]).ok_or_else(|| bad(format!("`{name}` is a node number, 1 to {count}")));
        let maybe_node = |name: &str, default: usize| match params.get(name) {
            None => Ok(default),
            Some(v) => number(v).ok_or_else(|| bad(format!("`{name}` is a node number, 1 to {count}"))),
        };
        let label = match id {
            "color.adjust" => {
                let i = node("node")?;
                let controls = params.get("controls").ok_or_else(|| bad("name the `controls` to set".into()))?;
                grade.nodes[i].patch(controls).map_err(bad)?;
                "Adjust Node"
            }
            "color.addSerialNode" => {
                grade.add_after(maybe_node("node", count - 1)?);
                "Add Serial Node"
            }
            "color.addNodeBefore" => {
                grade.add_before(maybe_node("node", 0)?);
                "Add Before Current"
            }
            "color.appendNode" => {
                grade.add_after(count - 1);
                "Append Node"
            }
            "color.deleteNode" => {
                grade.delete(node("node")?);
                "Delete Node"
            }
            "color.toggleNodes" => {
                let nodes =
                    params["nodes"].as_array().ok_or_else(|| bad("`nodes` is a list of node numbers".into()))?;
                for n in nodes {
                    let i = number(n).ok_or_else(|| bad(format!("node numbers run 1 to {count}")))?;
                    grade.nodes[i].enabled = !grade.nodes[i].enabled;
                }
                "Enable/Disable Selected Nodes"
            }
            "color.toggleAllNodes" => {
                // as in Resolve: all off, or all back on when every node is already off
                let on = grade.nodes.iter().all(|n| !n.enabled);
                grade.nodes.iter_mut().for_each(|n| n.enabled = on);
                "Enable/Disable All Nodes"
            }
            "color.resetNode" => {
                grade.nodes[node("node")?].reset();
                "Reset Selected Node Grade"
            }
            "color.resetGradesKeepNodes" => {
                grade.reset_keep_nodes();
                "Reset Grades and Keep Nodes"
            }
            "color.resetAll" => {
                grade = Grade::default();
                "Reset All Grades and Nodes"
            }
            _ => return Err(EngineError::UnknownCommand(id.to_string())),
        };
        let merge = params.get("merge").and_then(Value::as_str).map(|k| format!("color:{}:{k}", clip.0));
        let undo_label = self.store(id, seq, clip, &grade, label, merge.as_deref())?;
        Ok(Outcome { result: view(clip, &grade, false, size), revision: self.session.revision, undo_label })
    }

    /// The clip under the playhead on the highest video track that shows one.
    fn clip_at_playhead(&self, seq: ItemId) -> Option<ClipId> {
        let t = self.session.playhead();
        let s = self.session.project.sequence(seq)?;
        s.video_tracks
            .iter()
            .rev()
            .filter(|tr| tr.enabled)
            .find_map(|tr| tr.items.iter().find(|it| it.enabled && it.start <= t && t < it.end()).map(|it| it.id))
    }

    /// The clip's picture size, which windows are measured on: its media's, else the timeline's.
    fn clip_size(&self, seq: ItemId, clip: ClipId) -> (u32, u32) {
        let p = &self.session.project;
        let s = p.sequence(seq);
        s.and_then(|s| s.find_item(clip))
            .and_then(|(_, it)| p.source_size(it.item))
            .or_else(|| s.map(|s| (s.settings.width, s.settings.height)))
            .unwrap_or((1920, 1080))
    }

    /// The clip's grade, and whether its Lumetri effects are foreign (see the module notes).
    /// `None` when the open timeline has no such clip.
    fn grade_of(&self, seq: ItemId, clip: ClipId) -> Option<(Grade, bool)> {
        grade_in(&self.session.project, seq, clip, self.clip_size(seq, clip))
    }

    /// Store `grade` for `clip` and compile it onto the clip: one undo step labelled `label`, or
    /// part of the last one when `merge` repeats its key. Returns the new undo step's label.
    fn store(
        &mut self,
        id: &str,
        seq: ItemId,
        clip: ClipId,
        grade: &Grade,
        label: &str,
        merge: Option<&str>,
    ) -> Result<Option<String>> {
        let json = serde_json::to_string(grade).map_err(|e| EngineError::Engine(e.to_string()))?;
        let effects = compile(grade, self.clip_size(seq, clip));
        // the tables of the grade's qualified nodes, made only when the project lacks them
        let have: Vec<String> = self.session.project.luts.iter().map(|l| l.id.clone()).collect();
        let luts: Vec<ProjectLut> = grade
            .nodes
            .iter()
            .filter(|n| n.enabled && n.keyed().is_some() && !n.is_neutral())
            .filter(|n| !have.contains(&lut_id(n)))
            .map(|n| ProjectLut {
                id: lut_id(n),
                name: "Resolution qualified node".into(),
                source_path: None,
                format: "cube".into(),
                text: cube(n).into(),
            })
            .collect();
        let neutral = *grade == Grade::default();
        let missing = || filmcraft_engine::EngineError::Other("the clip is gone".into());
        let edit = move |p: &mut Project, _: &mut filmcraft_engine::EditorState| {
            let s = p.sequence_mut(seq).ok_or_else(missing)?;
            let (_, it) = s.find_item_mut(clip).ok_or_else(missing)?;
            let n = it.effects.iter().take_while(|e| e.effect == "lumetri").count();
            it.effects.splice(0..n, effects);
            // grades of clips no longer on the timeline go
            let live: std::collections::BTreeSet<String> =
                s.video_tracks.iter().flat_map(|t| &t.items).map(|i| key(i.id)).collect();
            let item = p.item_mut(seq).ok_or_else(missing)?;
            item.metadata.retain(|k, _| !k.starts_with(KEY) || live.contains(k));
            if neutral {
                item.metadata.remove(&key(clip));
            } else {
                item.metadata.insert(key(clip), json);
            }
            p.luts.extend(luts);
            prune_luts(p);
            Ok(())
        };
        let before = self.undo_top();
        match merge {
            Some(m) => self.session.edit_merged(label, m, edit),
            None => self.session.edit(label, edit),
        }
        .map_err(|e| crate::convert(id, e))?;
        Ok((self.undo_top() != before).then(|| label.to_string()))
    }
}

/// The grade of `clip` on timeline `seq` of `p`, whose picture is `size`, and whether its Lumetri
/// effects are foreign (see the module notes). `None` when the timeline has no such clip.
pub(crate) fn grade_in(p: &Project, seq: ItemId, clip: ClipId, size: (u32, u32)) -> Option<(Grade, bool)> {
    let (_, it) = p.sequence(seq)?.find_item(clip)?;
    let front = lumetri_front(&it.effects);
    let meta = &p.item(seq)?.metadata;
    if let Some(g) = meta.get(&key(clip)).and_then(|j| serde_json::from_str::<Grade>(j).ok()) {
        let foreign = compile(&g, size) != front;
        return Some((g, foreign));
    }
    if front.is_empty() {
        return Some((Grade::default(), false));
    }
    // a copy of a graded clip
    let adopted = meta
        .iter()
        .filter(|(k, _)| k.starts_with(KEY))
        .filter_map(|(_, j)| serde_json::from_str::<Grade>(j).ok())
        .find(|g| compile(g, size) == front);
    Some(adopted.map_or((Grade::default(), true), |g| (g, false)))
}

/// The run of Lumetri effects at the front of a clip's effect stack: its grade.
fn lumetri_front(effects: &[EffectInstance]) -> Vec<EffectInstance> {
    effects.iter().take_while(|e| e.effect == "lumetri").cloned().collect()
}

/// A grade as `color.inspect` shows it: the nodes numbered from 1, and the size of the clip's
/// picture that windows are measured on.
fn view(clip: ClipId, grade: &Grade, foreign: bool, (width, height): (u32, u32)) -> Value {
    let nodes: Vec<Value> = grade
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // through text, so the numbers read as typed (0.3, not 0.30000001192092896)
            let mut v: Value =
                serde_json::to_string(n).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            v["number"] = json!(i + 1);
            v["neutral"] = json!(n.is_neutral());
            v
        })
        .collect();
    json!({
        "clip": clip.0,
        "nodes": nodes,
        "graded": !grade.is_neutral(),
        "foreign": foreign,
        "width": width,
        "height": height,
    })
}

/// The grade as Lumetri effects, in node order, for a clip whose picture is `size` (see the
/// module notes).
pub(crate) fn compile(grade: &Grade, size: (u32, u32)) -> Vec<EffectInstance> {
    let Some(def) = find_effect("lumetri") else {
        return Vec::new();
    };
    let off = || {
        let mut e = def.instance();
        for section in ["basic_on", "creative_on", "curves_on", "wheels_on", "hsl_on", "vignette_on"] {
            set(&mut e, section, ParamValue::Bool(false));
        }
        e
    };
    let mut out = Vec::new();
    for n in grade.nodes.iter().filter(|n| n.enabled) {
        let masks: Vec<Mask> =
            n.windows.iter().filter(|w| w.enabled).enumerate().map(|(i, w)| mask(w, size, i)).collect();
        let start = out.len();
        if n.keyed().is_some() {
            // the whole node, key and all, as the effect's input LUT
            if !n.is_neutral() {
                let mut e = off();
                set(&mut e, "basic_on", ParamValue::Bool(true));
                set(&mut e, "input_lut", ParamValue::Text(format!("lib:{}", lut_id(n))));
                e.masks = masks;
                out.push(e);
            }
            continue;
        }
        if !n.tone_is_identity() {
            let mut e = off();
            set(&mut e, "curves_on", ParamValue::Bool(true));
            for (k, curve) in ["curve_red", "curve_green", "curve_blue"].into_iter().enumerate() {
                set(&mut e, curve, ParamValue::Curve(n.tone_points(k)));
            }
            out.push(e);
        }
        let s = n.saturation();
        let c = &n.curves;
        if !s.is_identity() || !c.hsl_is_identity() {
            let mut e = off();
            set(&mut e, "basic_on", ParamValue::Bool(true));
            set(&mut e, "saturation", ParamValue::Float(f64::from(s.amount) * 100.0));
            set(&mut e, "creative_on", ParamValue::Bool(true));
            set(&mut e, "vibrance", ParamValue::Float(f64::from(s.boost) * 100.0));
            // Hue's rotation rides on Hue vs Hue (LOG D-043)
            if s.hue_turns.abs() > 1e-6 && c.hue_hue.is_identity() {
                set(&mut e, "curves_on", ParamValue::Bool(true));
                set(&mut e, "hue_vs_hue", ParamValue::Curve(vec![[0.5, 0.5 + s.hue_turns]]));
            } else if !c.hue_hue.is_identity() {
                set(&mut e, "curves_on", ParamValue::Bool(true));
                set(
                    &mut e,
                    "hue_vs_hue",
                    ParamValue::Curve(hsl_points(|x| c.hue_hue.eval(x, true) + s.hue_turns, true)),
                );
            }
            for (curve, id, periodic) in [
                (&c.hue_sat, "hue_vs_sat", true),
                (&c.hue_lum, "hue_vs_luma", true),
                (&c.lum_sat, "luma_vs_sat", false),
                (&c.sat_sat, "sat_vs_sat", false),
            ] {
                if !curve.is_identity() {
                    set(&mut e, "curves_on", ParamValue::Bool(true));
                    set(&mut e, id, ParamValue::Curve(hsl_points(|x| curve.eval(x, periodic), periodic)));
                }
            }
            out.push(e);
        }
        for e in &mut out[start..] {
            e.masks = masks.clone();
        }
    }
    out
}

/// A hue curve as points for Lumetri to join (LOG D-043). Lumetri joins every HSL curve as if it
/// ran round the wheel, its last point to its first. A curve over hue is sampled round the wheel.
/// One over lightness or saturation is sampled from just inside 0 to just inside 1: Lumetri's
/// table of the curve runs from 0.0003 to 0.9997, so it never reaches the join between the two
/// ends (where a fully saturated colour, common after more Saturation, would take a value halfway
/// to the other end's).
fn hsl_points(f: impl Fn(f32) -> f32, periodic: bool) -> Vec<[f32; 2]> {
    const EDGE: f32 = 1e-4;
    (0..HSL_SAMPLES)
        .map(|i| {
            let x = if periodic {
                i as f32 / HSL_SAMPLES as f32
            } else {
                EDGE + (1.0 - 2.0 * EDGE) * i as f32 / (HSL_SAMPLES - 1) as f32
            };
            [x, f(x)]
        })
        .collect()
}

/// The id of the LUT that carries qualified node `n`: a hash of what the table depends on (not
/// its windows or label, which do not change it), so moving a window keeps the table.
fn lut_id(n: &Node) -> String {
    let mut table = n.clone();
    table.windows.clear();
    table.label.clear();
    // nor Denoise and Blur Radius, which a table cannot hold (LOG D-046)
    if let Some(q) = &mut table.qualifier {
        (q.denoise, q.blur) = (0.0, 0.0);
    }
    let text = serde_json::to_string(&table).unwrap_or_default();
    // FNV-1a
    let h = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    format!("{LUT_PREFIX}{h:016x}")
}

/// Node `n` as a `.cube` file of [`LUT_POINTS`] per side.
fn cube(n: &Node) -> String {
    use std::fmt::Write as _;
    let mut s = format!("TITLE \"Resolution qualified node\"\nLUT_3D_SIZE {LUT_POINTS}\n");
    for [r, g, b] in n.table(LUT_POINTS) {
        let _ = writeln!(s, "{r:.6} {g:.6} {b:.6}");
    }
    s
}

/// Drop the qualified-node LUTs that no clip of any timeline uses.
fn prune_luts(p: &mut Project) {
    let used: std::collections::BTreeSet<String> = p
        .items
        .values()
        .filter_map(|it| match &it.kind {
            ItemKind::Sequence(s) => Some(s),
            _ => None,
        })
        .flat_map(|s| s.video_tracks.iter().flat_map(|t| &t.items))
        .flat_map(|it| &it.effects)
        .filter_map(|e| match &e.params.get("input_lut")?.value {
            ParamValue::Text(t) => t.strip_prefix("lib:").map(str::to_string),
            _ => None,
        })
        .collect();
    p.luts.retain(|l| !l.id.starts_with(LUT_PREFIX) || used.contains(&l.id));
}

/// A window as an effect mask in the pixels of a `w` x `h` picture.
fn mask(win: &Window, (w, h): (u32, u32), n: usize) -> Mask {
    let (w, h) = (f64::from(w), f64::from(h));
    let c = Vec2::new(f64::from(win.center[0]) * w, f64::from(win.center[1]) * h);
    // sizes are fractions of the picture's width on both axes
    let r = Vec2::new(f64::from(win.size[0]) * w / 2.0, f64::from(win.size[1]) * w / 2.0);
    let mut path = match win.shape {
        Shape::Circle => MaskPath::ellipse(c, r),
        Shape::Linear => MaskPath::rect(c.x - r.x, c.y - r.y, c.x + r.x, c.y + r.y),
    };
    let a = f64::from(win.rotation).to_radians();
    if a != 0.0 {
        // y runs down, so a positive angle turns clockwise on screen
        let (sin, cos) = a.sin_cos();
        let turn = |v: Vec2| Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
        for v in &mut path.vertices {
            v.p = c + turn(v.p - c);
            v.t_in = turn(v.t_in);
            v.t_out = turn(v.t_out);
        }
    }
    let mut m = Mask::new(format!("Window {}", n + 1), path);
    m.feather = Param::new(ParamValue::Float(f64::from(win.softness) * w));
    m.opacity = Param::new(ParamValue::Float(f64::from(win.opacity)));
    m.inverted = win.invert;
    m
}

fn set(e: &mut EffectInstance, id: &str, value: ParamValue) {
    e.params.insert(id.to_string(), Param::new(value));
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
