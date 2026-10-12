//! Resolution's [`Engine`] on top of FilmCraft's headless `Session`.

pub mod map;

use std::sync::Arc;

use filmcraft_engine::time::Tick;
use filmcraft_engine::{EngineError as FcError, Event, FsServices, Session};
use map::Target;
use resolution_engine_api::{CommandInfo, Engine, EngineError, EngineEvent, Frame, Outcome, Result, Risk};
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
}
