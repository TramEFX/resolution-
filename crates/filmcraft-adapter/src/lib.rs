//! Resolution's [`Engine`] on top of FilmCraft's headless `Session`.

mod color;
mod gpu;
pub mod map;
pub mod playback;
mod scopes;

use std::sync::Arc;

use filmcraft_engine::time::Tick;
use filmcraft_engine::{EngineError as FcError, Event, FsServices, Session};
pub use gpu::{frame_counts, use_gpu_renderer};
use map::Target;
pub use playback::use_hardware_decoders;
use resolution_engine_api::{CommandInfo, Engine, EngineError, EngineEvent, Frame, Outcome, Playback, Result, Risk};
use serde_json::Value;

/// Raw FilmCraft commands are reachable as `raw.filmcraft/<id>`.
pub const RAW_PREFIX: &str = "raw.filmcraft/";

pub struct FilmCraftEngine {
    session: Session,
}

impl Default for FilmCraftEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// The bin holding `item` in a `project.inspect` tree under `node` (whose id is `bin`):
/// `Some(None)` at the top of the project, `None` if it is not there.
fn bin_of(node: &Value, bin: Option<u64>, item: u64) -> Option<Option<u64>> {
    node["children"].as_array().into_iter().flatten().find_map(|c| {
        if c["item"].as_u64() == Some(item) {
            Some(bin)
        } else if c.get("children").is_some() {
            bin_of(c, c["id"].as_u64(), item)
        } else {
            None
        }
    })
}

impl FilmCraftEngine {
    /// A headless session on the native filesystem.
    pub fn new() -> Self {
        Self { session: Session::new(Arc::new(FsServices)) }
    }

    /// Identity of the newest undo step, to tell whether a command added one.
    fn undo_top(&self) -> (usize, Option<*const filmcraft_engine::project::Project>) {
        let undo = &self.session.history.undo;
        (undo.len(), undo.last().map(|(_, p)| Arc::as_ptr(p)))
    }

    fn run(&mut self, resolution_id: &str, fc_id: &str, params: Value) -> Result<Outcome> {
        let before = self.undo_top();
        let result = self.session.execute(fc_id, params).map_err(|e| convert(resolution_id, e))?;
        let undo_label = if self.undo_top() != before {
            self.session.history.undo.last().map(|(label, _)| label.clone())
        } else {
            None
        };
        Ok(Outcome { result, revision: self.session.revision, undo_label })
    }

    /// `timeline.inspect`: FilmCraft's `sequence.inspect`, plus what the track headers and the
    /// timeline toolbar show and FilmCraft keeps outside the sequence: which tracks are targeted
    /// (auto track selectors), where source edits go (destination controls), and whether
    /// snapping and linked selection are on.
    fn inspect_timeline(&mut self) -> Result<Outcome> {
        let mut out = self.run("timeline.inspect", "sequence.inspect", Value::Object(Default::default()))?;
        let s = &self.session;
        let tg = s.targeting();
        if let Value::Object(m) = &mut out.result {
            m.insert(
                "targeting".into(),
                serde_json::json!({
                    "targeted": tg.targeted.iter().map(|t| t.0).collect::<Vec<_>>(),
                    "videoDest": tg.video_dest.map(|t| t.0),
                    "audioDest": tg.audio_dest.map(|t| t.0),
                }),
            );
            m.insert("snapping".into(), Value::Bool(s.state.snapping));
            m.insert("linkedSelection".into(), Value::Bool(s.state.linked_selection));
        }
        Ok(out)
    }

    /// Commands FilmCraft runs on the project selection: select the items named, then run it.
    /// `timeline.duplicate` (`item`, default the open timeline) is `edit.duplicate`, with the copy
    /// moved to the original's bin as Resolve does; `media.delete` (`items`) is `project.delete`.
    fn on_selection(&mut self, id: &str, params: Value) -> Result<Outcome> {
        let items: Vec<u64> = match id {
            "timeline.duplicate" => {
                params["item"].as_u64().or(self.session.state.active_sequence.map(|s| s.0)).into_iter().collect()
            }
            _ => params["items"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
        };
        if items.is_empty() {
            return Err(EngineError::BadParams { cmd: id.to_string(), msg: "name the items".into() });
        }
        self.run(id, "project.select", serde_json::json!({ "items": items }))?;
        if id == "media.delete" {
            return self.run(id, "project.delete", serde_json::json!({ "items": items }));
        }
        // FilmCraft puts the copy at the top of the project, Resolve in the original's bin: move
        // it there, folding the move into the duplicate's undo step as FilmCraft folds its own
        // (`clip_ops`: the first step's "before" project stands for both). If the move fails the
        // copy stays where FilmCraft put it.
        let n0 = self.session.history.undo.len();
        let mut out = self.run(id, "edit.duplicate", serde_json::json!({ "items": items }))?;
        let tree = self.run(id, "project.inspect", Value::Object(Default::default()))?.result;
        let bin = |item| bin_of(&tree["root"], None, item);
        if let Some(copy) = out.result["items"][0].as_u64()
            && let Some(to) = bin(items[0])
            && bin(copy) != Some(to)
            && self.run(id, "project.moveToBin", serde_json::json!({ "items": [copy], "bin": to })).is_ok()
        {
            let undo = &mut self.session.history.undo;
            if undo.len() > n0 + 1 {
                let first = undo[n0].clone();
                undo.truncate(n0);
                undo.push(first);
            }
            out.revision = self.session.revision;
        }
        Ok(out)
    }

    /// `source.inspect`: FilmCraft's, plus the clip's name and its exact frame rate (FilmCraft
    /// gives a float `fps`, too coarse for frame arithmetic at 23.976).
    fn inspect_source(&mut self) -> Result<Outcome> {
        let mut out = self.run("source.inspect", "source.inspect", Value::Object(Default::default()))?;
        let s = &self.session;
        let item = s.state.source_item.and_then(|i| s.project.item(i));
        if let (Value::Object(m), Some(item)) = (&mut out.result, item) {
            let rate = item.frame_rate();
            m.insert("name".into(), Value::String(item.name.clone()));
            m.insert("frameRate".into(), serde_json::json!({ "num": rate.num, "den": rate.den }));
            // the media's own timecode at its first frame, in its frames (the Sony clip's 02:01:12:00)
            let tc = item.as_media().and_then(|media| media.info.start_timecode);
            m.insert("startTimecodeFrames".into(), serde_json::json!(tc));
        }
        Ok(out)
    }
}

fn convert(id: &str, e: FcError) -> EngineError {
    match e {
        FcError::UnknownCommand(_) => EngineError::UnknownCommand(id.to_string()),
        FcError::Disabled(_, why) => EngineError::Disabled(id.to_string(), why),
        FcError::BadParams { msg, .. } => EngineError::BadParams { cmd: id.to_string(), msg },
        other => EngineError::Engine(other.to_string()),
    }
}

/// Risk of a raw FilmCraft command: queries read; file, export and delete commands are
/// destructive (they touch disk or replace the open project); everything else is an undoable edit.
fn raw_risk(spec: &filmcraft_engine::CommandSpec) -> Risk {
    let id = spec.id;
    if !spec.journal {
        Risk::Read
    } else if id.starts_with("file.")
        || id.starts_with("export.")
        || id.contains("delete")
        || id.contains("Delete")
        || id.contains("relink")
        || id.contains("replaceFootage")
    {
        Risk::Destructive
    } else {
        Risk::Edit
    }
}

impl Engine for FilmCraftEngine {
    fn name(&self) -> &'static str {
        "filmcraft"
    }

    fn execute(&mut self, id: &str, params: Value) -> Result<Outcome> {
        if let Some(raw) = id.strip_prefix(RAW_PREFIX) {
            return self.run(id, raw, params);
        }
        match map::target(id) {
            Some(Target::Engine(fc) | Target::Approx(fc, _)) => self.run(id, fc, params),
            Some(Target::Adapter) if id == "timeline.inspect" => self.inspect_timeline(),
            Some(Target::Adapter) if id == "source.inspect" => self.inspect_source(),
            Some(Target::Adapter) if id == "timeline.duplicate" || id == "media.delete" => {
                self.on_selection(id, params)
            }
            Some(Target::Adapter) if id.starts_with("color.") => self.color(id, params),
            Some(Target::Adapter) => {
                // Undo and redo move through the history; they add no step of their own (LOG B-001).
                let label = if id == "edit.undo" { self.undo() } else { self.redo() };
                Ok(Outcome { result: Value::from(label), revision: self.revision(), undo_label: None })
            }
            Some(Target::Ui) => Err(EngineError::Unsupported(id.to_string(), "headless (handled by the front end)")),
            Some(Target::Missing(_)) => Err(EngineError::Unsupported(id.to_string(), "filmcraft")),
            None => Err(EngineError::UnknownCommand(id.to_string())),
        }
    }

    fn commands(&self) -> Vec<CommandInfo> {
        let mapped = resolution_engine_api::COMMANDS.iter().filter_map(|c| {
            let (fc, note) = match map::target(c.id)? {
                Target::Engine(fc) => (filmcraft_engine::find_command(fc), None),
                Target::Approx(fc, note) => (filmcraft_engine::find_command(fc), Some(note.to_string())),
                Target::Adapter => (None, None),
                Target::Ui | Target::Missing(_) => return None,
            };
            Some(CommandInfo {
                id: c.id.to_string(),
                label: c.label.to_string(),
                params: if c.params.is_empty() { fc.map(|f| f.params).unwrap_or_default() } else { c.params }
                    .to_string(),
                risk: resolution_engine_api::commands::risk(c.id).unwrap_or(Risk::Edit),
                page: Some(c.page),
                note,
            })
        });
        let raw = filmcraft_engine::command_specs().iter().map(|f| CommandInfo {
            id: format!("{RAW_PREFIX}{}", f.id),
            label: f.label.to_string(),
            params: f.params.to_string(),
            risk: raw_risk(f),
            page: None,
            note: None,
        });
        mapped.chain(raw).collect()
    }

    fn undo(&mut self) -> Option<String> {
        self.session.undo()
    }

    fn redo(&mut self) -> Option<String> {
        self.session.redo()
    }

    fn revision(&self) -> u64 {
        self.session.revision
    }

    fn render_frame(&self, at_seconds: Option<f64>, max_side: u32) -> Result<Frame> {
        let (w, h) = self
            .session
            .active_sequence()
            .map(|q| (q.settings.width, q.settings.height))
            .ok_or_else(|| EngineError::Disabled("render".into(), "no timeline is open".into()))?;
        let scale = (max_side.max(1) as f32 / w.max(h).max(1) as f32).min(1.0);
        let t = at_seconds.map_or_else(|| self.session.playhead(), Tick::from_seconds_f64);
        let img = self.session.try_render_program_at(scale, t).map_err(|e| EngineError::Engine(e.to_string()))?;
        Ok(Frame { width: img.w as u32, height: img.h as u32, rgba: img.over_black_rgba8() })
    }

    fn playback(&self) -> Option<Arc<dyn Playback>> {
        playback::Snapshot::of(&self.session).map(|s| Arc::new(s) as Arc<dyn Playback>)
    }

    fn source_playback(&self) -> Option<Arc<dyn Playback>> {
        playback::SourceSnapshot::of(&self.session).map(|s| Arc::new(s) as Arc<dyn Playback>)
    }

    fn scopes(&self) -> Option<Arc<dyn resolution_engine_api::Scopes>> {
        Some(Arc::new(scopes::FilmCraftScopes))
    }

    fn drain_events(&mut self) -> Vec<EngineEvent> {
        self.session
            .drain_events()
            .into_iter()
            .filter_map(|e| match e {
                Event::ProjectChanged { revision } => Some(EngineEvent::ProjectChanged { revision }),
                Event::Toast { message, error } => Some(EngineEvent::Notice { message, error }),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolution_engine_api::RenderHints;
    use serde_json::json;

    fn demo() -> FilmCraftEngine {
        let mut e = FilmCraftEngine::new();
        e.execute("raw.filmcraft/file.openDemoProject", json!({})).expect("demo project");
        e
    }

    #[test]
    fn unknown_and_missing_commands_are_errors() {
        let mut e = FilmCraftEngine::new();
        assert!(matches!(e.execute("nope.nothing", json!({})), Err(EngineError::UnknownCommand(_))));
        assert!(matches!(e.execute("edit.fitToFill", json!({})), Err(EngineError::Unsupported(..))));
    }

    #[test]
    fn queries_run_headless() {
        let mut e = FilmCraftEngine::new();
        let out = e.execute("project.inspect", json!({})).expect("inspect");
        assert!(out.result.is_object());
        assert_eq!(out.undo_label, None);
    }

    #[test]
    fn raw_commands_are_listed_and_runnable() {
        let mut e = FilmCraftEngine::new();
        let cmds = e.commands();
        let raw = cmds.iter().find(|c| c.id == "raw.filmcraft/project.inspect").expect("raw inspect listed");
        assert_eq!(raw.risk, Risk::Read);
        assert!(cmds.iter().any(|c| c.id == "raw.filmcraft/file.save" && c.risk == Risk::Destructive));
        assert!(e.execute("raw.filmcraft/project.inspect", json!({})).is_ok());
    }

    #[test]
    fn edits_report_an_undo_step_and_undo_reverts_them() {
        let mut e = demo();
        e.execute("playhead.set", json!({"seconds": 2.0})).expect("playhead");
        let before = e.execute("timeline.inspect", json!({})).expect("inspect").result;
        let out = e.execute("timeline.split", json!({})).expect("split");
        assert!(out.undo_label.is_some(), "a split is an undo step");
        assert_ne!(e.execute("timeline.inspect", json!({})).expect("inspect").result, before);
        assert!(e.undo().is_some());
        assert_eq!(e.execute("timeline.inspect", json!({})).expect("inspect").result, before);
    }

    /// What the track headers show and change: lock, mute, targeting, destination; and the
    /// snapping and linked-selection toggles.
    #[test]
    fn track_headers_round_trip_through_timeline_inspect() {
        let mut e = demo();
        let tl = |e: &mut FilmCraftEngine| e.execute("timeline.inspect", json!({})).expect("inspect").result;
        let v1 = tl(&mut e)["video"][0]["id"].as_u64().expect("V1 id");
        let a1 = tl(&mut e)["audio"][0]["id"].as_u64().expect("A1 id");
        e.execute("track.set", json!({"track": "V1", "locked": true})).expect("lock");
        e.execute("track.set", json!({"track": "A1", "muted": true})).expect("mute");
        let t = tl(&mut e);
        assert_eq!((t["video"][0]["locked"].as_bool(), t["audio"][0]["muted"].as_bool()), (Some(true), Some(true)));
        e.execute("track.target", json!({"track": "A1", "targeted": false})).expect("untarget");
        e.execute("track.target", json!({"track": "V1", "sourcePatch": true})).expect("patch");
        let t = tl(&mut e);
        let targeted: Vec<u64> =
            t["targeting"]["targeted"].as_array().expect("targeted").iter().filter_map(Value::as_u64).collect();
        assert!(!targeted.contains(&a1), "A1 untargeted: {targeted:?}");
        assert_eq!(t["targeting"]["videoDest"].as_u64(), Some(v1));
        let snapping = t["snapping"].as_bool().expect("snapping");
        e.execute("timeline.snapping", json!({"on": !snapping})).expect("snap");
        assert_eq!(tl(&mut e)["snapping"].as_bool(), Some(!snapping));
        assert!(tl(&mut e)["linkedSelection"].is_boolean());
    }

    /// The mouse-editing commands, each one undo step: ripple, roll, slip, slide, move, blade.
    #[test]
    fn mouse_edits_trim_roll_slip_slide_move_and_blade() {
        let mut e = demo();
        let tl = |e: &mut FilmCraftEngine| e.execute("timeline.inspect", json!({})).expect("inspect").result;
        let v1 = |t: &Value, i: usize, key: &str| t["video"][0]["items"][i][key].as_i64().expect(key);
        let t0 = tl(&mut e);
        let (a, b) = (v1(&t0, 0, "clip"), v1(&t0, 1, "clip"));
        let fps_ticks = v1(&t0, 0, "duration") / v1(&t0, 0, "durationFrames");
        assert_eq!(v1(&t0, 0, "end"), v1(&t0, 1, "start"), "the demo's first two clips are adjacent");

        // the demo's music on A2 spans the cuts and A2 is sync-locked, which blocks rippling
        e.execute("track.set", json!({"track": "A2", "syncLock": false})).expect("sync lock off");
        // ripple the first clip's out point 10 frames earlier: everything after moves left
        let out = e
            .execute("timeline.trimEdge", json!({"clip": a, "edge": "out", "mode": "ripple", "deltaFrames": -10}))
            .expect("ripple");
        assert!(out.undo_label.is_some());
        let t = tl(&mut e);
        assert_eq!(v1(&t, 1, "start"), v1(&t0, 1, "start") - 10 * fps_ticks);
        e.undo();

        // roll the cut 5 frames later: the timeline's length stays
        e.execute("timeline.rollEdit", json!({"left": a, "right": b, "deltaFrames": 5})).expect("roll");
        let t = tl(&mut e);
        assert_eq!(v1(&t, 0, "end"), v1(&t0, 0, "end") + 5 * fps_ticks);
        assert_eq!(t["duration"], t0["duration"]);
        e.undo();

        // slip the second clip: same place, different media
        e.execute("timeline.slipClip", json!({"clip": b, "deltaFrames": 4})).expect("slip");
        let t = tl(&mut e);
        assert_eq!(
            (v1(&t, 1, "start"), v1(&t, 1, "sourceIn")),
            (v1(&t0, 1, "start"), v1(&t0, 1, "sourceIn") + 4 * fps_ticks)
        );
        e.undo();

        // slide the second clip 3 frames later: its neighbours give and take
        e.execute("timeline.slideClip", json!({"clip": b, "deltaFrames": 3})).expect("slide");
        let t = tl(&mut e);
        assert_eq!(
            (v1(&t, 1, "start"), v1(&t, 0, "end")),
            (v1(&t0, 1, "start") + 3 * fps_ticks, v1(&t0, 0, "end") + 3 * fps_ticks)
        );
        e.undo();

        // move the second clip up to V2
        let start = v1(&t0, 1, "start");
        e.execute("timeline.moveClips", json!({"moves": [{"clip": b, "track": "V2", "time": start}], "insert": false}))
            .expect("move");
        let t = tl(&mut e);
        assert!(t["video"][1]["items"].as_array().expect("V2").iter().any(|i| i["clip"].as_i64() == Some(b)));
        e.undo();

        // blade the first clip in the middle: one more clip on V1
        let mid = (v1(&t0, 0, "start") + v1(&t0, 0, "end")) / 2;
        e.execute("timeline.bladeClip", json!({"clip": a, "time": mid})).expect("blade");
        let n = |t: &Value| t["video"][0]["items"].as_array().map_or(0, Vec::len);
        assert_eq!(n(&tl(&mut e)), n(&t0) + 1);
    }

    /// The source viewer: a clip opened from the Media Pool renders and sounds on its own, and
    /// its playhead and In point move through the vocabulary.
    #[test]
    fn the_source_clip_plays_and_marks_on_its_own() {
        let mut e = demo();
        assert!(e.source_playback().is_none(), "nothing open yet");
        let project = e.execute("project.inspect", json!({})).expect("project").result;
        fn find(node: &Value, name: &str) -> Option<u64> {
            node["children"]
                .as_array()?
                .iter()
                .find_map(|c| if c["name"] == name { c["item"].as_u64() } else { find(c, name) })
        }
        let clip = find(&project["root"], "Ocean_Sunset.mp4").expect("demo clip");
        e.execute("source.open", json!({"item": clip})).expect("open");
        let s = e.execute("source.inspect", json!({})).expect("inspect").result;
        assert_eq!(s["name"], "Ocean_Sunset.mp4");
        let (num, den) = (s["frameRate"]["num"].as_i64().expect("num"), s["frameRate"]["den"].as_i64().expect("den"));
        assert!(num > 0 && den > 0);

        let p = e.source_playback().expect("source playback");
        assert_eq!(p.rate(), resolution_engine_api::Rate::new(num, den));
        assert!(p.frames() > 24);
        let f = p.render(10, 64, &RenderHints::default()).expect("source frame");
        assert!(f.width <= 64 && f.width > 0);
        let mut mix = p.mixer(48_000);
        let mut buf = vec![0f32; 2 * 4800];
        mix(48_000, &mut buf, 2);

        e.execute("source.seek", json!({"frame": 12})).expect("seek");
        e.execute("mark.in", json!({"target": "source"})).expect("mark in");
        let s = e.execute("source.inspect", json!({})).expect("inspect").result;
        assert_eq!(s["playhead"], s["markIn"]);
        assert!(s["markIn"].as_i64().is_some_and(|t| t > 0));
    }

    /// Waveforms for the timeline's clips: peaks of a media item's sound, one per 256 samples.
    #[test]
    fn peaks_summarise_a_clips_sound() {
        let e = demo();
        let t = e.session.active_sequence().expect("demo timeline").clone();
        let music =
            t.audio_tracks.iter().flat_map(|tr| &tr.items).find(|i| i.name == "Ambient_Score.wav").expect("demo music");
        let p = e.playback().expect("playback");
        let peaks = p.peaks(music.item.0).expect("the music has sound");
        // about 60 s of music at 48 kHz, 256 samples a peak
        let seconds = peaks.len() as f64 * resolution_engine_api::PEAK_SAMPLES as f64 / 48_000.0;
        assert!((seconds - 60.0).abs() < 2.0, "{seconds} s of peaks");
        let loudest = peaks.iter().fold(0f32, |m, (lo, hi)| m.max(lo.abs()).max(hi.abs()));
        assert!(loudest > 0.01 && loudest <= 1.0, "loudest peak {loudest}");
        assert!(peaks.iter().all(|(lo, hi)| lo <= hi));
    }

    /// The steps of an agent draft: duplicate the open timeline, rename the copy, delete one.
    #[test]
    fn timelines_duplicate_rename_and_delete() {
        let mut e = demo();
        let project = |e: &mut FilmCraftEngine| e.execute("project.inspect", json!({})).expect("project").result;
        fn timelines(node: &Value, out: &mut Vec<(u64, String)>) {
            for c in node["children"].as_array().into_iter().flatten() {
                if c["type"] == "Sequence" {
                    out.push((
                        c["item"].as_u64().unwrap_or_default(),
                        c["name"].as_str().unwrap_or_default().to_string(),
                    ));
                }
                timelines(c, out);
            }
        }
        let list = |e: &mut FilmCraftEngine| {
            let mut out = Vec::new();
            timelines(&project(e)["root"], &mut out);
            out
        };
        let original = project(&mut e)["activeSequence"].as_u64().expect("open timeline");
        let before = list(&mut e).len();
        let out = e.execute("timeline.duplicate", json!({})).expect("duplicate");
        let copy = out.result["items"][0].as_u64().expect("the copy");
        assert!(out.undo_label.is_some());
        assert_eq!(list(&mut e).len(), before + 1);
        // in the original's bin, as Resolve does, and one undo step
        let tree = project(&mut e);
        let original_bin = bin_of(&tree["root"], None, original);
        assert!(matches!(original_bin, Some(Some(_))), "the demo keeps its timelines in a bin");
        assert_eq!(bin_of(&tree["root"], None, copy), original_bin);
        assert_eq!(e.undo(), out.undo_label);
        assert_eq!(list(&mut e).len(), before, "one undo takes the copy away");
        assert_eq!(e.redo(), out.undo_label);
        assert_eq!(bin_of(&project(&mut e)["root"], None, copy), original_bin);
        e.execute("media.rename", json!({"item": copy, "name": "Main Edit (agent draft)"})).expect("rename");
        assert!(list(&mut e).iter().any(|(i, n)| *i == copy && n == "Main Edit (agent draft)"));
        e.execute("timeline.open", json!({"item": copy})).expect("open the copy");
        e.execute("media.delete", json!({"items": [original]})).expect("delete the original");
        let after = list(&mut e);
        assert!(after.iter().all(|(i, _)| *i != original), "the original is gone: {after:?}");
        assert_eq!(project(&mut e)["activeSequence"].as_u64(), Some(copy));
    }

    /// LOG B-001: undo and redo move through the history; they add no step of their own.
    #[test]
    fn undo_and_redo_commands_add_no_undo_step() {
        let mut e = demo();
        e.execute("playhead.set", json!({"seconds": 2.0})).expect("playhead");
        let split = e.execute("timeline.split", json!({})).expect("split").undo_label.expect("split label");
        let undo = e.execute("edit.undo", json!({})).expect("undo");
        assert_eq!(undo.undo_label, None);
        assert_eq!(undo.result, json!(split));
        let redo = e.execute("edit.redo", json!({})).expect("redo");
        assert_eq!(redo.undo_label, None);
        assert_eq!(redo.result, json!(split));
    }

    #[test]
    fn renders_a_frame_without_moving_the_playhead() {
        let e = demo();
        let f = e.render_frame(Some(1.0), 64).expect("frame");
        assert!(f.width <= 64 && f.height <= 64 && f.width > 0);
        assert_eq!(f.rgba.len(), (f.width * f.height * 4) as usize);
    }

    #[test]
    fn playback_renders_the_same_frames_on_other_threads() {
        let e = demo();
        let p = e.playback().expect("a timeline is open");
        assert_eq!(p.revision(), e.revision());
        let rate = p.rate();
        assert!(p.frames() > 24);
        let frames: Vec<i64> = vec![0, 1, 23, 24, p.frames() / 2];
        let threads: Vec<_> = frames
            .iter()
            .map(|&f| {
                let p = p.clone();
                std::thread::spawn(move || p.render(f, 96, &RenderHints::default()).expect("frame"))
            })
            .collect();
        for (f, t) in frames.iter().zip(threads) {
            let from_thread = t.join().expect("render thread");
            let from_engine = e.render_frame(Some(rate.seconds_of(*f)), 96).expect("frame");
            assert_eq!((from_thread.width, from_thread.height), (from_engine.width, from_engine.height), "frame {f}");
            // playback draws on the GPU where it can (LOG D-036), the engine on the CPU: the same
            // picture, within the two renderers' rounding and filtering when shrinking
            let mut d: Vec<u8> = from_thread.rgba.iter().zip(&from_engine.rgba).map(|(a, b)| a.abs_diff(*b)).collect();
            let mean = d.iter().map(|v| f32::from(*v)).sum::<f32>() / d.len() as f32;
            d.sort_unstable();
            let p99 = d[d.len() * 99 / 100];
            assert!(mean < 1.0 && p99 <= 3, "frame {f}: mean {mean}, 99% {p99}");
        }
        let cancelled = RenderHints::default();
        cancelled.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(p.render(5, 96, &cancelled).is_err(), "a cancelled render is not a frame");
        let timed = RenderHints::default();
        p.render(30, 96, &timed).expect("frame");
        assert!(timed.decode_ns.load(std::sync::atomic::Ordering::Relaxed) > 0, "getting source pictures is timed");
    }

    /// Phone video is stored sideways with a rotation tag, and FilmCraft's decoder turns each
    /// picture upright with rayon while holding its lock. Two frame workers rendering it at once
    /// deadlocked when their renders shared one pool (LOG B-015).
    #[test]
    fn playback_renders_a_rotated_clip_from_two_threads() {
        let mut e = FilmCraftEngine::new();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/rotated.mp4");
        let imported = e.execute("media.import", json!({ "paths": [path] })).expect("import");
        let item = imported.result["items"][0].clone();
        e.execute("timeline.createFromClips", json!({ "items": [item] })).expect("a timeline");
        let p = e.playback().expect("a timeline is open");
        assert_eq!(p.size(), (360, 640), "the clip's rotation tag is applied");
        let (done, finished) = std::sync::mpsc::channel();
        for worker in 0..2 {
            let (p, done) = (p.clone(), done.clone());
            std::thread::spawn(move || {
                for i in 0..120 {
                    p.render((2 * i + worker) % p.frames(), 640, &RenderHints::default()).expect("frame");
                }
                let _ = done.send(());
            });
        }
        for _ in 0..2 {
            finished.recv_timeout(std::time::Duration::from_secs(60)).expect("both workers finish");
        }
    }

    #[test]
    fn playback_mixes_the_timeline_sound_at_any_rate() {
        let e = demo();
        let p = e.playback().expect("a timeline is open");
        let start = p.rate().sample_of(24, 48_000);
        for rate in [48_000, 44_100] {
            let mut mix = p.mixer(rate);
            let mut buf = vec![f32::NAN; 2 * 4800];
            std::thread::spawn(move || {
                mix(start, &mut buf, 2);
                buf
            })
            .join()
            .map(|buf| {
                assert!(buf.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{rate} Hz");
                assert!(buf.iter().any(|x| x.abs() > 1e-3), "the demo's tone and score are heard at {rate} Hz");
            })
            .expect("mix thread");
        }
    }
}
