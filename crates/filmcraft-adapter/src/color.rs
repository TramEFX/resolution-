//! The Color page's grades (LOG D-028).
//!
//! A clip's grade is a [`Grade`] in Resolution's own types. It is kept as JSON in the timeline's
//! metadata (`resolution.grade.<clip id>`) and compiled onto the clip as Lumetri Color effects at
//! the front of its effect stack, so playback, frame renders and Deliver show it with nothing more
//! to do. Each edit is one FilmCraft undo step holding both, so undo keeps them in step.
//!
//! A node compiles to at most two Lumetri effects: its tone functions as the R, G and B curves
//! (every other section off), then its saturation step as Lumetri's Saturation and Vibrance (both
//! keep luma), with Hue as a constant Hue vs Hue curve. A neutral or disabled node compiles to
//! nothing, so an ungraded clip costs nothing to render.
//!
//! FilmCraft copies a clip's effects when it copies the clip (a split, a paste) but not our
//! metadata. A clip with no stored grade whose Lumetri effects are exactly what another of the
//! timeline's grades compiles to takes that grade. Lumetri effects that match no grade are
//! "foreign" (made outside the Color page); the next grade edit replaces them.

use filmcraft_engine::project::{ClipId, EffectInstance, ItemId, Param, ParamValue, Project, find_effect};
use resolution_engine_api::{EngineError, Outcome, Result};
use resolution_grade::Grade;
use serde_json::{Value, json};

use crate::FilmCraftEngine;

const KEY: &str = "resolution.grade.";

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
        if id == "color.inspect" {
            return Ok(Outcome {
                result: view(clip, &grade, foreign),
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
        Ok(Outcome { result: view(clip, &grade, false), revision: self.session.revision, undo_label })
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

    /// The clip's grade, and whether its Lumetri effects are foreign (see the module notes).
    /// `None` when the open timeline has no such clip.
    fn grade_of(&self, seq: ItemId, clip: ClipId) -> Option<(Grade, bool)> {
        let p = &self.session.project;
        let (_, it) = p.sequence(seq)?.find_item(clip)?;
        let front = lumetri_front(&it.effects);
        let meta = &p.item(seq)?.metadata;
        if let Some(g) = meta.get(&key(clip)).and_then(|j| serde_json::from_str::<Grade>(j).ok()) {
            let foreign = compile(&g) != front;
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
            .find(|g| compile(g) == front);
        Some(adopted.map_or((Grade::default(), true), |g| (g, false)))
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
        let effects = compile(grade);
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

/// The run of Lumetri effects at the front of a clip's effect stack: its grade.
fn lumetri_front(effects: &[EffectInstance]) -> Vec<EffectInstance> {
    effects.iter().take_while(|e| e.effect == "lumetri").cloned().collect()
}

/// A grade as `color.inspect` shows it: the nodes numbered from 1.
fn view(clip: ClipId, grade: &Grade, foreign: bool) -> Value {
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
    json!({ "clip": clip.0, "nodes": nodes, "graded": !grade.is_neutral(), "foreign": foreign })
}

/// The grade as Lumetri effects, in node order (see the module notes).
pub(crate) fn compile(grade: &Grade) -> Vec<EffectInstance> {
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
        if !n.tone_is_identity() {
            let mut e = off();
            set(&mut e, "curves_on", ParamValue::Bool(true));
            for (k, curve) in ["curve_red", "curve_green", "curve_blue"].into_iter().enumerate() {
                set(&mut e, curve, ParamValue::Curve(n.tone_points(k)));
            }
            out.push(e);
        }
        let s = n.saturation();
        if !s.is_identity() {
            let mut e = off();
            set(&mut e, "basic_on", ParamValue::Bool(true));
            set(&mut e, "saturation", ParamValue::Float(f64::from(s.amount) * 100.0));
            set(&mut e, "creative_on", ParamValue::Bool(true));
            set(&mut e, "vibrance", ParamValue::Float(f64::from(s.boost) * 100.0));
            if s.hue_turns.abs() > 1e-6 {
                set(&mut e, "curves_on", ParamValue::Bool(true));
                set(&mut e, "hue_vs_hue", ParamValue::Curve(vec![[0.5, 0.5 + s.hue_turns]]));
            }
            out.push(e);
        }
    }
    out
}

fn set(e: &mut EffectInstance, id: &str, value: ParamValue) {
    e.params.insert(id.to_string(), Param::new(value));
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
