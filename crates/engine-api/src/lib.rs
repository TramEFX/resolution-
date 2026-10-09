//! The engine boundary.
//!
//! Everything above this crate (the Resolve-style UI, the keymap, the MCP server, AI features)
//! speaks Resolution's own command vocabulary ([`commands`]) through the [`Engine`] trait and
//! never names an engine type. An adapter crate (today `resolution-filmcraft-adapter`) maps that
//! vocabulary onto a concrete engine. Swapping engines means writing a new adapter, nothing else.

pub mod commands;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use commands::{COMMANDS, CommandSpec, Page};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Not in Resolution's vocabulary, and not a namespaced raw engine command.
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    /// In the vocabulary, but this engine has no equivalent yet.
    #[error("`{0}` is not supported by the {1} engine yet")]
    Unsupported(String, &'static str),
    /// The command exists but cannot run in the current state (no timeline open, empty selection…).
    #[error("`{0}` is not available right now: {1}")]
    Disabled(String, String),
    #[error("invalid parameters for `{cmd}`: {msg}")]
    BadParams { cmd: String, msg: String },
    #[error("{0}")]
    Engine(String),
}

pub type Result<T> = std::result::Result<T, EngineError>;

/// A command as the engine actually offers it right now.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandInfo {
    /// Resolution id (`timeline.blade`), or a raw engine id (`raw.filmcraft/sequence.nest`).
    pub id: String,
    pub label: String,
    /// Free-form parameter description for agents.
    pub params: String,
    /// Changes the project (and is undoable), as opposed to a pure query.
    pub mutates: bool,
}

/// What every command returns: the engine's result plus what an agent needs to reason about undo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub result: Value,
    /// Project revision after the command; unchanged for queries.
    pub revision: u64,
    /// Label of the undo step this command created, if any.
    pub undo_label: Option<String>,
}

/// An 8-bit sRGB RGBA frame, ready to encode as PNG for an agent or upload as a texture.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Things the UI and agents should react to.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EngineEvent {
    ProjectChanged { revision: u64 },
    Notice { message: String, error: bool },
}

/// One editing session on one project. Implemented once per engine.
pub trait Engine {
    /// Short engine name for messages (`"filmcraft"`).
    fn name(&self) -> &'static str;

    /// Run a command by Resolution id, or a raw engine command as `raw.<engine>/<id>`.
    /// Raw commands are an escape hatch for capabilities the vocabulary does not cover yet;
    /// nothing that must survive an engine swap may depend on them.
    fn execute(&mut self, id: &str, params: Value) -> Result<Outcome>;

    /// The commands this engine supports now: mapped vocabulary entries plus raw commands.
    fn commands(&self) -> Vec<CommandInfo>;

    fn undo(&mut self) -> Option<String>;
    fn redo(&mut self) -> Option<String>;
    fn revision(&self) -> u64;

    /// The program (record) viewer at the playhead, scaled by `scale` (1.0 = timeline resolution).
    fn render_program(&self, scale: f32) -> Result<Frame>;

    /// Events since the last call.
    fn drain_events(&mut self) -> Vec<EngineEvent>;
}
