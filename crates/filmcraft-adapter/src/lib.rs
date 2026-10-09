//! Resolution's [`Engine`] on top of FilmCraft's headless `Session`.

pub mod map;

use std::sync::Arc;

use filmcraft_engine::{EngineError as FcError, Event, FsServices, Session};
use map::Target;
use resolution_engine_api::{
    CommandInfo, Engine, EngineError, EngineEvent, Frame, Outcome, Result,
};
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
        Self {
            session: Session::new(Arc::new(FsServices)),
        }
    }

    fn run(&mut self, resolution_id: &str, fc_id: &str, params: Value) -> Result<Outcome> {
        let before = self.session.revision;
        let result = self
            .session
            .execute(fc_id, params)
            .map_err(|e| convert(resolution_id, e))?;
        let changed = self.session.revision != before;
        let undo_label = if changed {
            self.session
                .history
                .undo
                .last()
                .map(|(label, _)| label.clone())
        } else {
            None
        };
        Ok(Outcome {
            result,
            revision: self.session.revision,
            undo_label,
        })
    }
}

fn convert(id: &str, e: FcError) -> EngineError {
    match e {
        FcError::UnknownCommand(_) => EngineError::UnknownCommand(id.to_string()),
        FcError::Disabled(_, why) => EngineError::Disabled(id.to_string(), why),
        FcError::BadParams { msg, .. } => EngineError::BadParams {
            cmd: id.to_string(),
            msg,
        },
        other => EngineError::Engine(other.to_string()),
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
                let label = if id == "edit.undo" {
                    self.undo()
                } else {
                    self.redo()
                };
                Ok(Outcome {
                    result: Value::from(label.clone()),
                    revision: self.revision(),
                    undo_label: label,
                })
            }
            Some(Target::Ui) => Err(EngineError::Unsupported(
                id.to_string(),
                "headless (handled by the front end)",
            )),
            Some(Target::Missing(_)) => Err(EngineError::Unsupported(id.to_string(), "filmcraft")),
            None => Err(EngineError::UnknownCommand(id.to_string())),
        }
    }

    fn commands(&self) -> Vec<CommandInfo> {
        let mapped = resolution_engine_api::COMMANDS.iter().filter_map(|c| {
            let fc = match map::target(c.id)? {
                Target::Engine(fc) | Target::Approx(fc, _) => filmcraft_engine::find_command(fc),
                Target::Adapter => None,
                Target::Ui | Target::Missing(_) => return None,
            };
            Some(CommandInfo {
                id: c.id.to_string(),
                label: c.label.to_string(),
                params: if c.params.is_empty() {
                    fc.map(|f| f.params).unwrap_or_default().to_string()
                } else {
                    c.params.to_string()
                },
                mutates: fc.is_none_or(|f| f.journal),
            })
        });
        let raw = filmcraft_engine::command_specs()
            .iter()
            .map(|f| CommandInfo {
                id: format!("{RAW_PREFIX}{}", f.id),
                label: f.label.to_string(),
                params: f.params.to_string(),
                mutates: f.journal,
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

    fn render_program(&self, scale: f32) -> Result<Frame> {
        let t = self.session.playhead();
        let img = self
            .session
            .try_render_program_at(scale, t)
            .map_err(|e| EngineError::Engine(e.to_string()))?;
        Ok(Frame {
            width: img.w as u32,
            height: img.h as u32,
            rgba: img.to_rgba8(),
        })
    }

    fn drain_events(&mut self) -> Vec<EngineEvent> {
        self.session
            .drain_events()
            .into_iter()
            .filter_map(|e| match e {
                Event::ProjectChanged { revision } => {
                    Some(EngineEvent::ProjectChanged { revision })
                }
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

    #[test]
    fn unknown_and_missing_commands_are_errors() {
        let mut e = FilmCraftEngine::new();
        assert!(matches!(
            e.execute("nope.nothing", json!({})),
            Err(EngineError::UnknownCommand(_))
        ));
        assert!(matches!(
            e.execute("edit.fitToFill", json!({})),
            Err(EngineError::Unsupported(..))
        ));
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
        assert!(
            e.commands()
                .iter()
                .any(|c| c.id == "raw.filmcraft/project.inspect")
        );
        assert!(
            e.execute("raw.filmcraft/project.inspect", json!({}))
                .is_ok()
        );
    }
}
