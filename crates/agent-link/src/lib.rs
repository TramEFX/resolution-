//! The agent link (ROADMAP step 4, LOG D-026): how an agent's MCP server reaches the editor that
//! is open on screen, so an agent edits the timeline the user is looking at, in the same undo
//! history.
//!
//! The editor listens on 127.0.0.1 at a port the system picks, and writes the port with a random
//! token to a file in the user's own application-data folder ([`default_path`]). `resolution mcp
//! --editor` reads that file, connects, and serves MCP over a [`RemoteEngine`], which forwards
//! every [`Engine`] call to the editor. A call without the token is refused. Calls are one JSON
//! object per line each way.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use resolution_engine_api::{CommandInfo, Engine, EngineError, EngineEvent, Frame, Outcome, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Where the running editor says how to reach it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkInfo {
    pub port: u16,
    pub token: String,
    /// The editor's process id, so a stale file can be told apart.
    pub pid: u32,
}

/// The link file's usual place: `%LOCALAPPDATA%\Resolution` on Windows,
/// `~/Library/Application Support/Resolution` on macOS, `$XDG_STATE_HOME/resolution` (or
/// `~/.local/state/resolution`) elsewhere.
pub fn default_path() -> PathBuf {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let dir = if cfg!(windows) {
        var("LOCALAPPDATA").map(|d| d.join("Resolution"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support/Resolution"))
    } else {
        var("XDG_STATE_HOME")
            .map(|d| d.join("resolution"))
            .or_else(|| var("HOME").map(|h| h.join(".local/state/resolution")))
    };
    dir.unwrap_or_else(std::env::temp_dir).join("agent-link.json")
}

/// One call from the agent's side: an [`Engine`] method and its arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "camelCase")]
pub enum Call {
    Name,
    Execute { id: String, params: Value },
    Commands,
    Undo,
    Redo,
    Revision,
    RenderFrame { at_seconds: Option<f64>, max_side: u32 },
    DrainEvents,
}

#[derive(Serialize, Deserialize)]
struct Request {
    token: String,
    #[serde(flatten)]
    call: Call,
}

/// The answer to a [`Call`]: its value as JSON, or the engine's error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Response {
    Ok(Value),
    Err(WireError),
}

/// An [`EngineError`] on the wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireError {
    pub kind: String,
    pub what: String,
    pub why: String,
}

impl From<&EngineError> for WireError {
    fn from(e: &EngineError) -> Self {
        let (kind, what, why) = match e {
            EngineError::UnknownCommand(id) => ("unknownCommand", id.clone(), String::new()),
            EngineError::Unsupported(id, engine) => ("unsupported", id.clone(), engine.to_string()),
            EngineError::Disabled(id, why) => ("disabled", id.clone(), why.clone()),
            EngineError::BadParams { cmd, msg } => ("badParams", cmd.clone(), msg.clone()),
            EngineError::Engine(msg) => ("engine", msg.clone(), String::new()),
        };
        Self { kind: kind.into(), what, why }
    }
}

impl From<WireError> for EngineError {
    fn from(w: WireError) -> Self {
        match w.kind.as_str() {
            "unknownCommand" => EngineError::UnknownCommand(w.what),
            // the engine's name is a static string: the editor's engine, or the front end
            "unsupported" => EngineError::Unsupported(
                w.what,
                if w.why.starts_with("headless") { "headless (handled by the front end)" } else { "filmcraft" },
            ),
            "disabled" => EngineError::Disabled(w.what, w.why),
            "badParams" => EngineError::BadParams { cmd: w.what, msg: w.why },
            _ => EngineError::Engine(w.what),
        }
    }
}

/// Answer `call` on `engine`: the editor runs this on its engine thread. Engine events are the
/// caller's to collect (the editor shows them as well), so [`Call::DrainEvents`] is answered by
/// the caller, not here.
pub fn answer(engine: &mut dyn Engine, call: Call) -> Response {
    let ok = |v: Value| Response::Ok(v);
    let result: Result<Value> = match call {
        Call::Name => Ok(json!(engine.name())),
        Call::Execute { id, params } => engine.execute(&id, params).map(|o| serde_json::to_value(o).unwrap_or_default()),
        Call::Commands => Ok(serde_json::to_value(engine.commands()).unwrap_or_default()),
        Call::Undo => Ok(json!(engine.undo())),
        Call::Redo => Ok(json!(engine.redo())),
        Call::Revision => Ok(json!(engine.revision())),
        Call::RenderFrame { at_seconds, max_side } => engine.render_frame(at_seconds, max_side).map(|f| {
            json!({ "width": f.width, "height": f.height, "rgba": base64::engine::general_purpose::STANDARD.encode(&f.rgba) })
        }),
        Call::DrainEvents => Ok(json!([])),
    };
    match result {
        Ok(v) => ok(v),
        Err(e) => Response::Err(WireError::from(&e)),
    }
}

/// The editor's side: accepts agents on 127.0.0.1 while it lives, and removes its link file when
/// dropped.
pub struct Listener {
    pub info: LinkInfo,
    path: PathBuf,
}

impl Drop for Listener {
    fn drop(&mut self) {
        // only our own file: another editor may have started since
        let ours = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str::<LinkInfo>(&s).ok())
            .is_some_and(|i| i == self.info);
        if ours {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Listen for agents, writing how to reach this editor to `path`. Each call goes to `handle`,
/// which runs it on the engine and returns the answer; connections are served on threads of
/// their own.
pub fn listen(path: &Path, handle: impl Fn(Call) -> Response + Send + Sync + 'static) -> std::io::Result<Listener> {
    let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let info = LinkInfo { port: socket.local_addr()?.port(), token: token(), pid: std::process::id() };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string(&info).unwrap_or_default())?;
    let handle = Arc::new(handle);
    let token = info.token.clone();
    std::thread::Builder::new().name("agent-link".into()).spawn(move || {
        for stream in socket.incoming().flatten() {
            let (handle, token) = (handle.clone(), token.clone());
            let _ = std::thread::Builder::new()
                .name("agent-link-client".into())
                .spawn(move || serve(stream, &token, &*handle));
        }
    })?;
    Ok(Listener { info, path: path.to_path_buf() })
}

fn serve(stream: TcpStream, token: &str, handle: &dyn Fn(Call) -> Response) {
    let Ok(mut out) = stream.try_clone() else { return };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(r) if r.token == token => handle(r.call),
            Ok(_) => Response::Err(WireError {
                kind: "engine".into(),
                what: "wrong agent-link token".into(),
                why: String::new(),
            }),
            Err(e) => Response::Err(WireError {
                kind: "engine".into(),
                what: format!("bad agent-link call: {e}"),
                why: String::new(),
            }),
        };
        let text = serde_json::to_string(&response).unwrap_or_default();
        if writeln!(out, "{text}").and_then(|_| out.flush()).is_err() {
            return;
        }
    }
}

/// 128 bits from the system's random hash keys: enough to keep other local users and web pages
/// (which cannot read the user's files) off the link.
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let salt = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    (0..2u8)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u128(salt);
            h.write_u8(i);
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// The agent's side: an [`Engine`] that is the open editor's, reached over the link.
pub struct RemoteEngine {
    conn: Mutex<(BufReader<TcpStream>, TcpStream)>,
    token: String,
}

impl RemoteEngine {
    /// Connect to the editor that wrote `path` (usually [`default_path`]).
    pub fn connect(path: &Path) -> std::result::Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|_| format!("no editor is running (no {}): start resolution-app first", path.display()))?;
        let info: LinkInfo = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, info.port))
            .map_err(|e| format!("the editor (process {}) is not answering on port {}: {e}", info.pid, info.port))?;
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        Ok(Self { conn: Mutex::new((reader, stream)), token: info.token })
    }

    fn call(&self, call: Call) -> Result<Value> {
        let lost = |e: std::io::Error| EngineError::Engine(format!("lost the editor: {e}"));
        let request = serde_json::to_string(&Request { token: self.token.clone(), call }).unwrap_or_default();
        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let (reader, writer) = &mut *conn;
        writeln!(writer, "{request}").and_then(|_| writer.flush()).map_err(lost)?;
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(lost)? == 0 {
            return Err(EngineError::Engine("the editor closed the link".into()));
        }
        match serde_json::from_str::<Response>(&line) {
            Ok(Response::Ok(v)) => Ok(v),
            Ok(Response::Err(w)) => Err(w.into()),
            Err(e) => Err(EngineError::Engine(format!("bad answer from the editor: {e}"))),
        }
    }
}

impl Engine for RemoteEngine {
    fn name(&self) -> &'static str {
        "editor"
    }

    fn execute(&mut self, id: &str, params: Value) -> Result<Outcome> {
        let v = self.call(Call::Execute { id: id.to_string(), params })?;
        serde_json::from_value(v).map_err(|e| EngineError::Engine(format!("bad outcome from the editor: {e}")))
    }

    fn commands(&self) -> Vec<CommandInfo> {
        self.call(Call::Commands).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }

    fn undo(&mut self) -> Option<String> {
        self.call(Call::Undo).ok().and_then(|v| v.as_str().map(str::to_string))
    }

    fn redo(&mut self) -> Option<String> {
        self.call(Call::Redo).ok().and_then(|v| v.as_str().map(str::to_string))
    }

    fn revision(&self) -> u64 {
        self.call(Call::Revision).ok().and_then(|v| v.as_u64()).unwrap_or(0)
    }

    fn render_frame(&self, at_seconds: Option<f64>, max_side: u32) -> Result<Frame> {
        let v = self.call(Call::RenderFrame { at_seconds, max_side })?;
        let rgba = base64::engine::general_purpose::STANDARD
            .decode(v["rgba"].as_str().unwrap_or_default())
            .map_err(|e| EngineError::Engine(format!("bad frame from the editor: {e}")))?;
        Ok(Frame {
            width: v["width"].as_u64().unwrap_or(0) as u32,
            height: v["height"].as_u64().unwrap_or(0) as u32,
            rgba,
        })
    }

    fn drain_events(&mut self) -> Vec<EngineEvent> {
        self.call(Call::DrainEvents).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolution_filmcraft_adapter::FilmCraftEngine;

    fn link_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("resolution-agent-link-{name}-{}.json", std::process::id()))
    }

    /// An editor with the demo project behind a link, its engine on one thread as the editor's is.
    fn editor(path: &Path) -> Listener {
        let engine = Arc::new(Mutex::new(FilmCraftEngine::new()));
        engine.lock().expect("engine").execute("raw.filmcraft/file.openDemoProject", json!({})).expect("demo");
        listen(path, move |call| answer(&mut *engine.lock().expect("engine"), call)).expect("listen")
    }

    #[test]
    fn an_agent_edits_the_editors_timeline_through_the_link() {
        let path = link_file("edit");
        let link = editor(&path);
        let mut remote = RemoteEngine::connect(&path).expect("connect");
        assert_eq!(remote.name(), "editor");
        assert!(remote.commands().iter().any(|c| c.id == "timeline.split"));
        let clips = |e: &mut RemoteEngine| {
            e.execute("timeline.inspect", json!({})).expect("inspect").result["video"][0]["items"]
                .as_array()
                .map_or(0, Vec::len)
        };
        let before = clips(&mut remote);
        remote.execute("playhead.set", json!({"seconds": 2.0})).expect("playhead");
        let out = remote.execute("timeline.split", json!({})).expect("split");
        assert!(out.undo_label.is_some());
        assert_eq!(clips(&mut remote), before + 1);
        assert!(remote.undo().is_some());
        assert_eq!(clips(&mut remote), before);
        let f = remote.render_frame(Some(1.0), 64).expect("frame");
        assert_eq!(f.rgba.len(), (f.width * f.height * 4) as usize);
        // errors keep their kind
        assert!(matches!(remote.execute("no.such.command", json!({})), Err(EngineError::UnknownCommand(_))));
        drop(link);
        assert!(!path.exists(), "the editor removes its link file when it closes");
    }

    #[test]
    fn calls_without_the_token_are_refused() {
        let path = link_file("token");
        let link = editor(&path);
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, link.info.port)).expect("connect");
        writeln!(stream, r#"{{"token": "guess", "call": "revision"}}"#).expect("send");
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).expect("answer");
        assert!(line.contains("wrong agent-link token"), "{line}");
        assert_eq!(link.info.token.len(), 32);
    }

    #[test]
    fn with_no_editor_running_connecting_says_so() {
        let err = RemoteEngine::connect(&link_file("none")).err().expect("no editor");
        assert!(err.contains("no editor is running"), "{err}");
    }
}
