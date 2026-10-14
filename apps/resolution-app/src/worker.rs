//! The engine on its own thread. The UI never waits on it: it sends commands and reads replies
//! each frame, so a slow command cannot freeze the window. After each batch of commands the
//! engine sends a snapshot: the timeline and project to draw, and a [`Playback`] handle the
//! viewer renders and plays from on other threads (LOG D-013).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use resolution_agent_link::{Call, Response, WireError};
use resolution_engine_api::{Engine, EngineEvent, Outcome, Playback};
use serde_json::{Value, json};

pub enum Request {
    Run {
        id: String,
        params: Value,
    },
    /// A call from an agent over the agent link (LOG D-026), answered on `reply`.
    Agent {
        call: Call,
        reply: Sender<Response>,
    },
    /// The user keeps (`accept`) or throws away the agent's draft (LOG D-027).
    Decide {
        accept: bool,
    },
    Quit,
}

/// An agent's draft (LOG D-027): the copy of a timeline that an agent's edits go to, until the
/// user accepts it (it replaces the original) or rejects it (it is deleted).
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// The timeline the agent started from, and its name.
    pub original: u64,
    pub name: String,
    /// The copy the agent edits.
    pub draft: u64,
    /// The undo steps made since the draft was, oldest first, each with whether an agent made it;
    /// and the steps undone since, most recent last (what redo brings back). Every step goes
    /// through the engine thread, so this follows the one history exactly.
    steps: Vec<(String, bool)>,
    undone: Vec<(String, bool)>,
}

/// How a command moved the undo history.
enum Step {
    /// A new step, by an agent (`true`) or the user.
    Made(String, bool),
    Undone(String),
    Redone(String),
}

impl Draft {
    /// The agent's edits in the draft, by their undo labels, oldest first: what the user is
    /// accepting. An undone edit drops out until it is redone.
    pub fn edits(&self) -> Vec<&str> {
        self.steps.iter().filter(|(_, agent)| *agent).map(|(l, _)| l.as_str()).collect()
    }

    fn record(&mut self, step: Option<Step>) {
        match step {
            Some(Step::Made(label, agent)) => {
                self.steps.push((label, agent));
                self.undone.clear();
            }
            // an undo past the draft's start (its copy and rename) is not one of its steps
            Some(Step::Undone(label)) => {
                if let Some(s) = self.steps.pop_if(|(l, _)| *l == label) {
                    self.undone.push(s);
                }
            }
            Some(Step::Redone(label)) => {
                if let Some(s) = self.undone.pop_if(|(l, _)| *l == label) {
                    self.steps.push(s);
                }
            }
            None => {}
        }
    }
}

/// What command `id` did to the undo history, from its result and undo label.
fn history_step(id: &str, result: &Value, undo_label: Option<&str>, agent: bool) -> Option<Step> {
    match id {
        "edit.undo" => result.as_str().map(|l| Step::Undone(l.to_string())),
        "edit.redo" => result.as_str().map(|l| Step::Redone(l.to_string())),
        _ => undo_label.map(|l| Step::Made(l.to_string(), agent)),
    }
}

pub enum Reply {
    Ran {
        id: String,
        result: Result<Outcome, String>,
    },
    /// The project after a batch of commands. `timeline` and `playback` are `None` when no
    /// timeline is open; `source_playback` when no clip is open in the source viewer.
    Snapshot {
        revision: u64,
        timeline: Option<Value>,
        project: Option<Value>,
        source: Option<Value>,
        playback: Option<Arc<dyn Playback>>,
        source_playback: Option<Arc<dyn Playback>>,
    },
    Notice {
        message: String,
        error: bool,
    },
    /// The agent's draft, after every batch (`None`: there is none).
    Draft(Option<Draft>),
}

pub struct Worker {
    tx: Sender<Request>,
    rx: Receiver<Reply>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    /// Start the engine thread. `repaint` wakes the UI when replies are waiting.
    pub fn spawn(engine: Box<dyn Engine + Send>, repaint: impl Fn() + Send + 'static) -> Self {
        let (tx, requests) = mpsc::channel();
        let (replies, rx) = mpsc::channel();
        let handle = thread::Builder::new()
            .name("engine".into())
            .spawn(move || serve(engine, &requests, &replies, &repaint))
            .ok();
        Self { tx, rx, handle }
    }

    pub fn run(&self, id: &str, params: Value) {
        let _ = self.tx.send(Request::Run { id: id.to_string(), params });
    }

    /// Keep or throw away the agent's draft. The user's call only: no command does this, so an
    /// agent cannot accept its own work.
    pub fn decide(&self, accept: bool) {
        let _ = self.tx.send(Request::Decide { accept });
    }

    pub fn poll(&self) -> Vec<Reply> {
        self.rx.try_iter().collect()
    }

    /// What the agent link calls: hands an agent's call to the engine thread and waits for the
    /// answer. Agents' edits land in the same engine, history and snapshots as the user's.
    pub fn agent_handle(&self) -> impl Fn(Call) -> Response + Send + Sync + 'static {
        let tx = self.tx.clone();
        move |call| {
            let (reply, answer) = mpsc::channel();
            let closed = || {
                Response::Err(WireError {
                    kind: "engine".into(),
                    what: "the editor is closing".into(),
                    why: String::new(),
                })
            };
            if tx.send(Request::Agent { call, reply }).is_err() {
                return closed();
            }
            answer.recv().unwrap_or_else(|_| closed())
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Quit);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn serve(
    mut engine: Box<dyn Engine + Send>,
    requests: &Receiver<Request>,
    replies: &Sender<Reply>,
    repaint: &dyn Fn(),
) {
    let _ = replies.send(snapshot(engine.as_mut()));
    repaint();
    // engine notices from agents' calls, kept until the agent asks for them
    let mut agent_events: Vec<EngineEvent> = Vec::new();
    let mut draft: Option<Draft> = None;
    while let Ok(first) = requests.recv() {
        let batch: Vec<Request> = std::iter::once(first).chain(requests.try_iter()).collect();
        for request in batch {
            match request {
                Request::Quit => return,
                Request::Run { id, params } => {
                    // A panic inside the engine fails this command; the editor keeps running.
                    let result = catch_unwind(AssertUnwindSafe(|| engine.execute(&id, params)))
                        .unwrap_or_else(|_| {
                            Err(resolution_engine_api::EngineError::Engine("the engine panicked".into()))
                        })
                        .map_err(|e| e.to_string());
                    if let (Some(d), Ok(o)) = (draft.as_mut(), &result) {
                        d.record(history_step(&id, &o.result, o.undo_label.as_deref(), false));
                    }
                    let _ = replies.send(Reply::Ran { id, result });
                }
                Request::Agent { call, reply } => {
                    // the command, for the draft's record; undo and redo answer with the label itself
                    let history = match &call {
                        Call::Execute { id, .. } => Some((id.clone(), true)),
                        Call::Undo => Some(("edit.undo".to_string(), false)),
                        Call::Redo => Some(("edit.redo".to_string(), false)),
                        _ => None,
                    };
                    let response = if call == Call::DrainEvents {
                        Response::Ok(serde_json::to_value(std::mem::take(&mut agent_events)).unwrap_or_default())
                    } else if let Call::Execute { id, params } = call {
                        agent_edit(engine.as_mut(), &mut draft, &mut agent_events, &id, &params)
                    } else {
                        catch_unwind(AssertUnwindSafe(|| resolution_agent_link::answer(engine.as_mut(), call)))
                            .unwrap_or_else(|_| panicked())
                    };
                    if let (Some((id, outcome)), Some(d), Response::Ok(v)) = (&history, draft.as_mut(), &response) {
                        let result = if *outcome { &v["result"] } else { v };
                        d.record(history_step(id, result, v["undo_label"].as_str(), true));
                    }
                    let edits = history.is_some();
                    if edits {
                        // the user sees what an agent did; its notices go to it as well as here
                        if let Response::Ok(v) = &response
                            && let Some(label) = v["undo_label"].as_str()
                        {
                            let _ = replies.send(Reply::Notice { message: format!("Agent: {label}"), error: false });
                        }
                        for event in engine.drain_events() {
                            if let EngineEvent::Notice { message, error } = &event {
                                let _ = replies.send(Reply::Notice { message: message.clone(), error: *error });
                            }
                            agent_events.push(event);
                        }
                    }
                    let _ = reply.send(response);
                }
                Request::Decide { accept } => {
                    if let Some(d) = draft.take() {
                        let (message, error) = match decide(engine.as_mut(), &d, accept) {
                            Ok(()) if accept => (format!("Accepted the agent's draft of {}", d.name), false),
                            Ok(()) => (format!("Rejected the agent's draft of {}", d.name), false),
                            Err(e) => (format!("The agent's draft: {e}"), true),
                        };
                        let _ = replies.send(Reply::Notice { message, error });
                    }
                }
            }
        }
        // a draft the user deleted, or undid away, is gone
        if draft.as_ref().is_some_and(|d| !has_item(engine.as_mut(), d.draft)) {
            draft = None;
        }
        // the timeline first: whoever sees the draft's edits sees them on screen too
        let _ = replies.send(snapshot(engine.as_mut()));
        let _ = replies.send(Reply::Draft(draft.clone()));
        for event in engine.drain_events() {
            if let EngineEvent::Notice { message, error } = event {
                let _ = replies.send(Reply::Notice { message, error });
            }
        }
        repaint();
    }
}

fn panicked() -> Response {
    Response::Err(WireError { kind: "engine".into(), what: "the engine panicked".into(), why: String::new() })
}

/// An agent's command (LOG D-027). Edits go to the agent's draft, a copy of the open timeline,
/// made at its first edit: the command runs once on the open timeline, and if that added an undo
/// step it is undone there and made again on a fresh copy. Commands that change nothing (looking,
/// moving the playhead) never make a draft. The caller records the step in the draft.
fn agent_edit(
    engine: &mut dyn Engine,
    draft: &mut Option<Draft>,
    events: &mut Vec<EngineEvent>,
    id: &str,
    params: &Value,
) -> Response {
    let run = |engine: &mut dyn Engine| {
        let call = Call::Execute { id: id.to_string(), params: params.clone() };
        catch_unwind(AssertUnwindSafe(|| resolution_agent_link::answer(engine, call))).unwrap_or_else(|_| panicked())
    };
    let label = |r: &Response| match r {
        Response::Ok(v) => v["undo_label"].as_str().map(str::to_string),
        Response::Err(_) => None,
    };
    // the draft is where agents work: open it if the user has moved to another timeline
    if let Some(d) = draft.as_ref() {
        if !has_item(engine, d.draft) {
            *draft = None;
        } else if active_timeline(engine) != Some(d.draft) {
            let _ = engine.execute("timeline.open", json!({ "item": d.draft }));
        }
    }
    let response = run(engine);
    if label(&response).is_some() && draft.is_none() && active_timeline(engine).is_some() {
        engine.undo();
        match start_draft(engine) {
            Ok(d) => {
                events.push(EngineEvent::Notice {
                    message: format!(
                        "Your edits go to a draft of the timeline \"{}\", which the user will accept or reject.",
                        d.name
                    ),
                    error: false,
                });
                *draft = Some(d);
                return run(engine);
            }
            // no draft could be made: the edit stays where it was made. Redo brings it back unless
            // the failed attempt made a step (a copy), which drops the redo; then it runs again.
            Err(_) => {
                if engine.redo().is_none() {
                    return run(engine);
                }
            }
        }
    }
    response
}

/// Copy the open timeline, name the copy "<name> (agent draft)", open it at the same playhead.
fn start_draft(engine: &mut dyn Engine) -> Result<Draft, String> {
    let tl = engine.execute("timeline.inspect", json!({})).map_err(|e| e.to_string())?.result;
    let original = tl["id"].as_u64().ok_or("no timeline is open")?;
    let name = tl["name"].as_str().unwrap_or("Timeline").to_string();
    let copy = engine.execute("timeline.duplicate", json!({ "item": original })).map_err(|e| e.to_string())?;
    let copy = copy.result["items"][0].as_u64().ok_or("the timeline was not copied")?;
    engine
        .execute("media.rename", json!({ "item": copy, "name": format!("{name} (agent draft)") }))
        .map_err(|e| e.to_string())?;
    engine.execute("timeline.open", json!({ "item": copy })).map_err(|e| e.to_string())?;
    if let Some(t) = tl["playhead"].as_i64() {
        let _ = engine.execute("playhead.set", json!({ "time": t }));
    }
    Ok(Draft { original, name, draft: copy, steps: Vec::new(), undone: Vec::new() })
}

/// Accept: the draft takes the original's place and name. Reject: the original opens again and
/// the draft is deleted. Both are undoable edits.
fn decide(engine: &mut dyn Engine, d: &Draft, accept: bool) -> Result<(), String> {
    let run = |engine: &mut dyn Engine, id: &str, params: Value| {
        engine.execute(id, params).map(|_| ()).map_err(|e| e.to_string())
    };
    if accept {
        run(engine, "timeline.open", json!({ "item": d.draft }))?;
        run(engine, "media.delete", json!({ "items": [d.original] }))?;
        run(engine, "media.rename", json!({ "item": d.draft, "name": d.name }))
    } else {
        run(engine, "timeline.open", json!({ "item": d.original }))?;
        run(engine, "media.delete", json!({ "items": [d.draft] }))
    }
}

fn active_timeline(engine: &mut dyn Engine) -> Option<u64> {
    engine.execute("project.inspect", json!({})).ok()?.result["activeSequence"].as_u64()
}

/// Whether the project still holds item `item`.
fn has_item(engine: &mut dyn Engine, item: u64) -> bool {
    fn find(node: &Value, item: u64) -> bool {
        node["children"].as_array().into_iter().flatten().any(|c| c["item"].as_u64() == Some(item) || find(c, item))
    }
    engine.execute("project.inspect", json!({})).is_ok_and(|o| find(&o.result["root"], item))
}

fn snapshot(engine: &mut dyn Engine) -> Reply {
    let mut query = |id: &str| engine.execute(id, json!({})).ok().map(|o| o.result);
    let timeline = query("timeline.inspect");
    let project = query("project.inspect");
    let source = query("source.inspect");
    Reply::Snapshot {
        revision: engine.revision(),
        timeline,
        project,
        source,
        playback: engine.playback(),
        source_playback: engine.source_playback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolution_engine_api::RenderHints;
    use resolution_filmcraft_adapter::FilmCraftEngine;
    use std::time::{Duration, Instant};

    fn wait_for<T>(w: &Worker, mut want: impl FnMut(Reply) -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if let Some(found) = w.poll().into_iter().find_map(&mut want) {
                return Some(found);
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    }

    #[test]
    fn runs_commands_off_the_ui_thread_and_hands_out_playback() {
        let w = Worker::spawn(Box::new(FilmCraftEngine::new()), || {});
        w.run("raw.filmcraft/file.openDemoProject", json!({}));
        let playback = wait_for(&w, |r| match r {
            Reply::Snapshot { timeline: Some(_), playback: Some(p), .. } => Some(p),
            _ => None,
        })
        .expect("a snapshot with the demo timeline and its playback");
        // the snapshot renders on this thread while the engine thread waits for commands
        let f = playback.render(12, 64, &RenderHints::default()).expect("frame");
        assert!(f.width <= 64 && f.width > 0);
        w.run("no.such.command", json!({}));
        assert!(
            wait_for(&w, |r| matches!(r, Reply::Ran { result: Err(_), .. }).then_some(())).is_some(),
            "errors come back as replies"
        );
    }

    /// The demo, on an engine that cannot rename: a draft's copy is made, then naming it fails.
    struct NoRename(FilmCraftEngine);

    impl Engine for NoRename {
        fn name(&self) -> &'static str {
            self.0.name()
        }
        fn execute(&mut self, id: &str, params: Value) -> resolution_engine_api::Result<Outcome> {
            if id == "media.rename" {
                return Err(resolution_engine_api::EngineError::Engine("no renaming here".into()));
            }
            self.0.execute(id, params)
        }
        fn commands(&self) -> Vec<resolution_engine_api::CommandInfo> {
            self.0.commands()
        }
        fn undo(&mut self) -> Option<String> {
            self.0.undo()
        }
        fn redo(&mut self) -> Option<String> {
            self.0.redo()
        }
        fn revision(&self) -> u64 {
            self.0.revision()
        }
        fn render_frame(
            &self,
            at: Option<f64>,
            max_side: u32,
        ) -> resolution_engine_api::Result<resolution_engine_api::Frame> {
            self.0.render_frame(at, max_side)
        }
        fn drain_events(&mut self) -> Vec<EngineEvent> {
            self.0.drain_events()
        }
    }

    #[test]
    fn an_edit_whose_draft_cannot_be_made_stays_on_the_timeline() {
        let mut e = NoRename(FilmCraftEngine::new());
        e.execute("raw.filmcraft/file.openDemoProject", json!({})).expect("demo");
        e.execute("playhead.set", json!({ "seconds": 2.0 })).expect("playhead");
        let clips = |e: &mut NoRename| {
            let t = e.execute("timeline.inspect", json!({})).expect("timeline").result;
            t["video"][0]["items"].as_array().map_or(0, Vec::len)
        };
        let before = clips(&mut e);
        let (mut draft, mut events) = (None, Vec::new());
        let r = agent_edit(&mut e, &mut draft, &mut events, "timeline.split", &json!({}));
        assert!(matches!(&r, Response::Ok(v) if v["undo_label"].is_string()), "{r:?}");
        assert!(draft.is_none());
        assert_eq!(clips(&mut e), before + 1, "the split is on the open timeline, not lost");
    }
}
