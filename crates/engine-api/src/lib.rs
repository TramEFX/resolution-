//! The engine boundary.
//!
//! Everything above this crate (the Resolve-style UI, the keymap, the MCP server, AI features)
//! speaks Resolution's own command vocabulary ([`commands`]) through the [`Engine`] trait and
//! never names an engine type. An adapter crate (today `resolution-filmcraft-adapter`) maps that
//! vocabulary onto a concrete engine. Swapping engines means writing a new adapter, nothing else.

pub mod commands;
pub mod playback;
pub mod scopes;

use std::any::Any;
use std::borrow::Cow;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use commands::{COMMANDS, CommandSpec, Page, Risk};
pub use playback::{Mixer, PEAK_RATE, PEAK_SAMPLES, Playback, Rate, RenderHints};
pub use scopes::{PARADE_GAP, ScopeKind, Scopes, VECTORSCOPE_EXTENT};

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
    pub risk: Risk,
    /// The Resolve page that owns it; `None` for raw engine commands.
    pub page: Option<Page>,
    /// How this engine's behaviour differs from Resolve's, when it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What every command returns: the engine's result plus what an agent needs to reason about undo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub result: Value,
    /// Project revision after the command; unchanged for queries.
    pub revision: u64,
    /// Label of the undo step this command created, if any. `None` for undo and redo themselves:
    /// they move through the history without adding to it.
    pub undo_label: Option<String>,
}

/// An 8-bit sRGB RGBA frame, ready to encode as PNG for an agent or upload as a texture.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// The pixels; empty for a frame left on the GPU (`gpu`): [`Frame::pixels`] reads either.
    pub rgba: Vec<u8>,
    /// The frame as the renderer left it on the GPU, for a front end drawing on the same device
    /// to show without a copy (LOG D-039). Only given when asked for ([`RenderHints::gpu`]).
    pub gpu: Option<Arc<dyn GpuPicture>>,
}

impl Frame {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self { width, height, rgba, gpu: None }
    }

    /// The pixels, straight 8-bit sRGB RGBA: [`Frame::rgba`], or read back from the GPU (once,
    /// waiting for it). Empty when the GPU could not be read.
    pub fn pixels(&self) -> Cow<'_, [u8]> {
        match &self.gpu {
            Some(g) if self.rgba.is_empty() => Cow::Owned(g.read().unwrap_or_default()),
            _ => Cow::Borrowed(&self.rgba),
        }
    }

    /// The memory the frame holds, on the GPU or not.
    pub fn bytes(&self) -> usize {
        self.rgba.len().max(if self.gpu.is_some() { self.width as usize * self.height as usize * 4 } else { 0 })
    }
}

/// A frame on the GPU (LOG D-039).
pub trait GpuPicture: Send + Sync + std::fmt::Debug {
    /// The texture, as the renderer's GPU library has it (with wgpu: a `wgpu::TextureView` of
    /// format `Rgba8Unorm` holding sRGB-encoded colour, the form egui's renderer takes).
    fn texture(&self) -> &(dyn Any + Send + Sync);
    /// The pixels, read back (waits for the GPU).
    fn read(&self) -> Option<Vec<u8>>;
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

    /// The program (record) viewer at `at_seconds` (default: the playhead), opaque, with its
    /// longest side at most `max_side` pixels. Never moves the playhead.
    fn render_frame(&self, at_seconds: Option<f64>, max_side: u32) -> Result<Frame>;

    /// The open timeline as it is now, for real-time playback on other threads; `None` when no
    /// timeline is open or the engine cannot play back.
    fn playback(&self) -> Option<Arc<dyn Playback>> {
        None
    }

    /// The clip open in the source viewer, played the same way: its frames from the first of
    /// its range, its sound from the same point. `None` when no clip is open.
    fn source_playback(&self) -> Option<Arc<dyn Playback>> {
        None
    }

    /// Video scopes for the viewers' frames (LOG D-030); `None` when the engine has none.
    fn scopes(&self) -> Option<Arc<dyn Scopes>> {
        None
    }

    /// Events since the last call.
    fn drain_events(&mut self) -> Vec<EngineEvent>;
}
