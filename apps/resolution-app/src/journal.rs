//! The action journal (LOG D-034, D-054): every command the engine thread runs, who sent it (the
//! user or an agent), when it started, how long it ran and how it ended. A freeze shows as a
//! command that started and never ended, a slow action by its time, a silent failure by its
//! error, and an agent's work step by step.
//!
//! The newest entries are kept in memory. With a file (`--journal <path>`), each start and each
//! end is also written there as a line of JSON, flushed at once, so the file is current even if
//! the editor stops or freezes.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use serde_json::{Value, json};

/// Entries kept in memory.
pub const KEEP: usize = 2000;
/// A command's parameters are kept to this many characters of JSON.
const PARAMS_CHARS: usize = 400;

/// Who sent a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    User,
    Agent,
}

/// How a command ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Ended {
    /// It ran, making this undo step if it made one.
    Ok {
        undo: Option<String>,
    },
    Err(String),
}

/// One command.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its number, from 1, in the order commands started.
    pub seq: u64,
    pub who: Who,
    /// The vocabulary id, or for the few things that are not commands a description in brackets.
    pub command: String,
    /// Its parameters as JSON, cut short when long.
    pub params: String,
    /// When it started, in milliseconds since the journal began.
    pub started_ms: f64,
    /// How long it ran, and how it ended; `None` while it runs.
    pub ended: Option<(f64, Ended)>,
}

#[derive(Clone)]
pub struct Journal {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    entries: VecDeque<Entry>,
    next: u64,
    t0: Instant,
    file: Option<BufWriter<File>>,
}

impl Journal {
    /// A journal in memory, and in `file` when one is given (created or emptied).
    pub fn new(file: Option<&Path>) -> io::Result<Self> {
        let file = file.map(File::create).transpose()?.map(BufWriter::new);
        Ok(Self { inner: Arc::new(Mutex::new(Inner { entries: VecDeque::new(), next: 1, t0: Instant::now(), file })) })
    }

    /// A journal in memory only.
    pub fn in_memory() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner { entries: VecDeque::new(), next: 1, t0: Instant::now(), file: None })),
        }
    }

    /// Notes that `command` started; returns its number for [`Journal::end`].
    pub fn start(&self, who: Who, command: &str, params: &Value) -> u64 {
        let mut j = self.lock();
        let seq = j.next;
        j.next += 1;
        let started_ms = j.t0.elapsed().as_secs_f64() * 1000.0;
        let mut text = params.to_string();
        if text.len() > PARAMS_CHARS {
            let cut = (0..=PARAMS_CHARS).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
            text.truncate(cut);
            text.push('…');
        }
        let who_name = match who {
            Who::User => "user",
            Who::Agent => "agent",
        };
        j.write(json!({
            "seq": seq, "t_ms": round(started_ms), "event": "start", "who": who_name,
            "command": command, "params": params,
        }));
        j.entries.push_back(Entry { seq, who, command: command.to_string(), params: text, started_ms, ended: None });
        while j.entries.len() > KEEP {
            j.entries.pop_front();
        }
        seq
    }

    /// Notes how command `seq` ended.
    pub fn end(&self, seq: u64, ended: Ended) {
        let mut j = self.lock();
        let now = j.t0.elapsed().as_secs_f64() * 1000.0;
        let Some(e) = j.entries.iter_mut().rev().find(|e| e.seq == seq) else { return };
        let ms = now - e.started_ms;
        e.ended = Some((ms, ended.clone()));
        let mut line = json!({ "seq": seq, "t_ms": round(now), "event": "end", "ms": round(ms) });
        match ended {
            Ended::Ok { undo } => {
                line["ok"] = json!(true);
                if let Some(u) = undo {
                    line["undo"] = json!(u);
                }
            }
            Ended::Err(e) => {
                line["ok"] = json!(false);
                line["error"] = json!(e);
            }
        }
        j.write(line);
    }

    /// The entries kept, oldest first (for tests, and the journal's panel to come).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn entries(&self) -> Vec<Entry> {
        self.lock().entries.iter().cloned().collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    /// One line of the file, flushed at once; a journal that cannot be written stops writing.
    fn write(&mut self, line: Value) {
        if let Some(f) = self.file.as_mut()
            && (writeln!(f, "{line}").is_err() || f.flush().is_err())
        {
            self.file = None;
        }
    }
}

/// Milliseconds to the hundredth.
fn round(ms: f64) -> f64 {
    (ms * 100.0).round() / 100.0
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_noted_when_it_starts_and_when_it_ends() {
        let path = std::env::temp_dir().join(format!("resolution-journal-{}.jsonl", std::process::id()));
        let j = Journal::new(Some(&path)).unwrap();
        let a = j.start(Who::User, "timeline.split", &json!({}));
        let b = j.start(Who::Agent, "color.adjust", &json!({ "controls": { "saturation": 60.0 } }));
        j.end(a, Ended::Ok { undo: Some("Split".into()) });
        // b has not ended: what a freeze looks like
        let entries = j.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!((entries[0].seq, entries[0].who, entries[0].command.as_str()), (1, Who::User, "timeline.split"));
        assert!(matches!(&entries[0].ended, Some((ms, Ended::Ok { undo: Some(u) })) if *ms >= 0.0 && u == "Split"));
        assert_eq!((entries[1].who, entries[1].ended.clone()), (Who::Agent, None));
        let lines: Vec<Value> =
            std::fs::read_to_string(&path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 3, "two starts and one end, written as they happened");
        assert_eq!((lines[0]["event"].as_str(), lines[0]["command"].as_str()), (Some("start"), Some("timeline.split")));
        assert_eq!(
            (lines[1]["who"].as_str(), lines[1]["params"]["controls"]["saturation"].as_f64()),
            (Some("agent"), Some(60.0))
        );
        assert_eq!((lines[2]["seq"].as_u64(), lines[2]["undo"].as_str()), (Some(a), Some("Split")));
        j.end(b, Ended::Err("no clip".into()));
        assert!(matches!(&j.entries()[1].ended, Some((_, Ended::Err(e))) if e == "no clip"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn only_the_newest_entries_are_kept_and_long_parameters_are_cut() {
        let j = Journal::new(None).unwrap();
        for _ in 0..KEEP + 10 {
            let s = j.start(Who::User, "playhead.set", &json!({ "frame": 1 }));
            j.end(s, Ended::Ok { undo: None });
        }
        let entries = j.entries();
        assert_eq!(entries.len(), KEEP);
        assert_eq!(entries[0].seq, 11, "the oldest went first");
        let s = j.start(Who::User, "media.import", &json!({ "paths": ["é".repeat(500)] }));
        let e = j.entries().into_iter().find(|e| e.seq == s).unwrap();
        assert!(e.params.ends_with('…') && e.params.chars().count() <= PARAMS_CHARS + 1, "{}", e.params);
    }
}
