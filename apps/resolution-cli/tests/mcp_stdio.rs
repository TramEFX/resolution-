//! MCP over stdio, end to end: the built `resolution` binary, driven the way an MCP client such as
//! Claude Code drives it. The tests in `crates/mcp` call `AgentSession` directly; these cover the
//! wire layer on top of it: the handshake, argument checking, image content, and both protocol
//! modes (LOG D-006).

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{Value, json};

/// Longest wait for one response, so a hung server fails the test instead of hanging it.
const TIMEOUT: Duration = Duration::from_secs(60);

/// `resolution mcp --demo` on FilmCraft's demo project; killed on drop.
struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
    /// Sent as `_meta` with every request in the 2026-07-28 mode.
    meta: Option<Value>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resolution"))
            .args(["mcp", "--demo"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start `resolution mcp --demo`");
        let stdin = child.stdin.take().expect("server stdin");
        let stdout = child.stdout.take().expect("server stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, stdin, lines, next_id: 0, meta: None }
    }

    /// The handshake mode (2025-11-25 and earlier): `initialize`, then `notifications/initialized`.
    fn initialized() -> (Self, Value) {
        let mut s = Self::start();
        let init = s.request(
            "initialize",
            json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "e2e", "version": "0"}}),
        );
        s.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        (s, init)
    }

    /// The 2026-07-28 mode: no handshake; every request names the protocol version in `_meta`.
    fn stateless() -> Self {
        let mut s = Self::start();
        s.meta = Some(json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {},
        }));
        s
    }

    fn send(&mut self, msg: &Value) {
        writeln!(self.stdin, "{msg}").expect("write to server");
        self.stdin.flush().expect("flush to server");
    }

    /// Send a request and return its `result`. A JSON-RPC error or a timeout fails the test.
    fn request(&mut self, method: &str, mut params: Value) -> Value {
        self.next_id += 1;
        if let Some(meta) = &self.meta {
            params["_meta"] = meta.clone();
        }
        self.send(&json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}));
        loop {
            let line = self.lines.recv_timeout(TIMEOUT).unwrap_or_else(|e| panic!("no response to `{method}`: {e}"));
            let msg: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON-RPC ({e}): {line}"));
            if msg["id"] == self.next_id {
                assert!(msg.get("error").is_none(), "`{method}` failed: {msg}");
                return msg["result"].clone();
            }
        }
    }

    /// Call a tool: its JSON reply, or its error text.
    fn call(&mut self, tool: &str, args: Value) -> Result<Value, String> {
        let r = self.request("tools/call", json!({"name": tool, "arguments": args}));
        let text = r["content"]
            .as_array()
            .and_then(|c| c.iter().find(|c| c["type"] == "text"))
            .and_then(|c| c["text"].as_str())
            .unwrap_or_default()
            .to_string();
        if r["isError"] == true { Err(text) } else { Ok(serde_json::from_str(&text).unwrap_or(Value::String(text))) }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn two_cuts() -> Value {
    json!([
        {"command": "playhead.set", "params": {"seconds": 2.0}},
        {"command": "timeline.split"},
        {"command": "playhead.set", "params": {"seconds": 4.0}},
        {"command": "timeline.split"},
    ])
}

#[test]
fn handshake_offers_the_ten_tools() {
    let (mut s, init) = Server::initialized();
    assert_eq!(init["protocolVersion"], "2025-11-25");
    assert_eq!(init["serverInfo"]["name"], "resolution");
    assert!(init["instructions"].as_str().is_some_and(|i| i.contains("list_pages")), "{init}");
    let tools = s.request("tools/list", json!({}));
    let mut names: Vec<&str> =
        tools["tools"].as_array().expect("tools").iter().filter_map(|t| t["name"].as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "checkpoint",
            "inspect",
            "list_pages",
            "redo",
            "render_frame",
            "restore_checkpoint",
            "run",
            "run_batch",
            "search_commands",
            "undo"
        ]
    );
}

/// The HANDOFF example: cut at 2 s and 4 s as one batch, dry-run it first, then look at a frame.
#[test]
fn batch_dry_run_then_apply_then_render() {
    let (mut s, _) = Server::initialized();
    let before = s.call("inspect", json!({"what": "timeline"})).expect("inspect");

    let dry = s.call("run_batch", json!({"steps": two_cuts(), "dry_run": true})).expect("dry run");
    assert_eq!(dry["ok"], true, "{dry}");
    assert!(dry["timeline_changes"]["changed"].as_u64().unwrap_or(0) > 0, "{dry}");
    let after_dry = s.call("inspect", json!({"what": "timeline"})).expect("inspect");
    assert_eq!(after_dry["video"], before["video"], "a dry run leaves the clips as they were");
    assert_eq!(after_dry["audio"], before["audio"]);

    let applied = s.call("run_batch", json!({"steps": two_cuts()})).expect("cuts");
    assert_eq!(applied["undo_steps"], 2, "{applied}");
    let session = s.call("inspect", json!({"what": "session"})).expect("session");
    assert_eq!(session["agent_edits"].as_array().map(Vec::len), Some(2), "{session}");

    let frame = s.request("tools/call", json!({"name": "render_frame", "arguments": {"seconds": 2.5, "max_side": 64}}));
    assert_ne!(frame["isError"], true, "{frame}");
    let image = frame["content"].as_array().and_then(|c| c.iter().find(|c| c["type"] == "image")).expect("image");
    assert_eq!(image["mimeType"], "image/png");
    assert!(image["data"].as_str().is_some_and(|d| d.starts_with("iVBORw0KGgo")), "base64 of the PNG signature");
}

/// LOG B-001 and D-005 over the wire.
#[test]
fn undo_through_run_is_not_an_agent_edit() {
    let (mut s, _) = Server::initialized();
    s.call("run_batch", json!({"steps": two_cuts()})).expect("cuts");
    let cut = s.call("inspect", json!({})).expect("inspect");

    let dry = s.call("run", json!({"command": "edit.undo", "dry_run": true})).expect("dry-run undo");
    assert_eq!(dry["reverted"], true, "{dry}");
    assert_eq!(s.call("inspect", json!({})).expect("inspect"), cut, "a dry run steps back exactly once");

    let undo = s.call("run", json!({"command": "edit.undo"})).expect("undo");
    assert_eq!(undo["undo_label"], Value::Null, "{undo}");
    let session = s.call("inspect", json!({"what": "session"})).expect("session");
    assert_eq!(session["agent_edits"].as_array().map(Vec::len), Some(1), "{session}");

    let refused = s.call("run_batch", json!({"steps": [{"command": "edit.undo"}]})).expect_err("undo in a batch");
    assert!(refused.contains("cannot be a batch step"), "{refused}");
}

/// LOG D-004: a mistyped argument is an error, never a silent default.
#[test]
fn unknown_argument_fields_are_rejected() {
    let (mut s, _) = Server::initialized();
    let e = s.call("run", json!({"command": "timeline.split", "dryrun": true})).expect_err("typo");
    assert!(e.contains("unknown field `dryrun`"), "{e}");
    let session = s.call("inspect", json!({"what": "session"})).expect("session");
    assert!(session["agent_edits"].as_array().is_some_and(Vec::is_empty), "nothing ran: {session}");
}

/// The 2026-07-28 mode: no handshake, and the cache hints `server.rs` adds to list and read results.
#[test]
fn stateless_requests_get_cache_hints() {
    let mut s = Server::stateless();
    let list = s.request("resources/list", json!({}));
    assert_eq!(list["ttlMs"], 600_000, "{list}");
    assert_eq!(list["cacheScope"], "private");
    let read = s.request("resources/read", json!({"uri": "resolution://timeline"}));
    assert_eq!(read["ttlMs"], 0, "the timeline changes with every edit");
    let text = read["contents"][0]["text"].as_str().expect("timeline text");
    assert!(serde_json::from_str::<Value>(text).is_ok_and(|t| t["video"].is_array()), "{text}");
    let pages = s.call("list_pages", json!({})).expect("tools work without a handshake");
    assert_eq!(pages["pages"].as_array().map(Vec::len), Some(8), "{pages}");
}
