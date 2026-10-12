//! The engine on its own thread. The UI never waits on it: it sends commands and reads replies
//! each frame, so a slow command cannot freeze the window. After each batch of commands the
//! engine sends a snapshot: the timeline and project to draw, and a [`Playback`] handle the
//! viewer renders and plays from on other threads (LOG D-013).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use resolution_engine_api::{Engine, EngineEvent, Outcome, Playback};
use serde_json::{Value, json};

pub enum Request {
    Run { id: String, params: Value },
    Quit,
}

pub enum Reply {
    Ran {
        id: String,
        result: Result<Outcome, String>,
    },
    /// The project after a batch of commands. `timeline` and `playback` are `None` when no
    /// timeline is open.
    Snapshot {
        revision: u64,
        timeline: Option<Value>,
        project: Option<Value>,
        playback: Option<Arc<dyn Playback>>,
    },
    Notice {
        message: String,
        error: bool,
    },
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

    pub fn poll(&self) -> Vec<Reply> {
        self.rx.try_iter().collect()
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
                    let _ = replies.send(Reply::Ran { id, result });
                }
            }
        }
        let _ = replies.send(snapshot(engine.as_mut()));
        for event in engine.drain_events() {
            if let EngineEvent::Notice { message, error } = event {
                let _ = replies.send(Reply::Notice { message, error });
            }
        }
        repaint();
    }
}

fn snapshot(engine: &mut dyn Engine) -> Reply {
    let mut query = |id: &str| engine.execute(id, json!({})).ok().map(|o| o.result);
    let timeline = query("timeline.inspect");
    let project = query("project.inspect");
    Reply::Snapshot { revision: engine.revision(), timeline, project, playback: engine.playback() }
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
}
