//! The engine on its own thread. The UI never waits on it: it sends requests and reads replies
//! each frame, so a slow 4K seek or render cannot freeze the window. Renders are coalesced: only
//! the newest request in a batch is rendered.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use resolution_engine_api::{Engine, EngineEvent, Frame, Outcome};
use serde_json::{Value, json};

pub enum Request {
    Run { id: String, params: Value },
    Render { seconds: f64, max_side: u32 },
    Quit,
}

pub enum Reply {
    Ran {
        id: String,
        result: Result<Outcome, String>,
    },
    /// The project after a batch of commands. `timeline` is `None` when none is open.
    Snapshot {
        revision: u64,
        timeline: Option<Value>,
        project: Option<Value>,
    },
    Frame {
        seconds: f64,
        frame: Result<Frame, String>,
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

    pub fn render(&self, seconds: f64, max_side: u32) {
        let _ = self.tx.send(Request::Render { seconds, max_side });
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
        let (mut ran, mut render) = (false, None);
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
                    ran = true;
                    let _ = replies.send(Reply::Ran { id, result });
                }
                Request::Render { seconds, max_side } => render = Some((seconds, max_side)),
            }
        }
        if ran {
            let _ = replies.send(snapshot(engine.as_mut()));
        }
        if let Some((seconds, max_side)) = render {
            let frame = catch_unwind(AssertUnwindSafe(|| engine.render_frame(Some(seconds), max_side)))
                .unwrap_or_else(|_| Err(resolution_engine_api::EngineError::Engine("the engine panicked".into())))
                .map_err(|e| e.to_string());
            let _ = replies.send(Reply::Frame { seconds, frame });
        }
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
    Reply::Snapshot { revision: engine.revision(), timeline, project }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolution_filmcraft_adapter::FilmCraftEngine;
    use std::time::{Duration, Instant};

    fn wait_for(w: &Worker, mut want: impl FnMut(&Reply) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if w.poll().iter().any(&mut want) {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn runs_commands_and_renders_off_the_ui_thread() {
        let w = Worker::spawn(Box::new(FilmCraftEngine::new()), || {});
        w.run("raw.filmcraft/file.openDemoProject", json!({}));
        assert!(
            wait_for(&w, |r| matches!(r, Reply::Snapshot { timeline: Some(_), .. })),
            "a snapshot with the demo timeline"
        );
        // two renders in a row: at least the newest arrives, at the size asked for
        w.render(0.5, 64);
        w.render(1.0, 64);
        assert!(wait_for(
            &w,
            |r| matches!(r, Reply::Frame { seconds, frame: Ok(f) } if *seconds == 1.0 && f.width <= 64)
        ));
        w.run("no.such.command", json!({}));
        assert!(wait_for(&w, |r| matches!(r, Reply::Ran { result: Err(_), .. })), "errors come back as replies");
    }
}
