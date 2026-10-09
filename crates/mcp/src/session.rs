//! What an agent can do, independent of the MCP wire format: find commands, run them safely
//! (risk checks, dry runs, atomic batches), look at the result, and back out (undo, checkpoints).

use std::collections::BTreeMap;

use resolution_engine_api::{CommandInfo, Engine, EngineEvent, Page, Risk, commands as vocabulary};
use resolution_keymap::Keymap;
use serde_json::{Value, json};

/// Most leaf changes listed in a `changes` report.
const MAX_CHANGES: usize = 25;

pub struct AgentSession {
    engine: Box<dyn Engine + Send>,
    catalog: Vec<CommandInfo>,
    keymap: Keymap,
    /// Undo labels of the edits made through this session, oldest first.
    steps: Vec<String>,
    /// Labels undone through this session, most recent last (what `redo` brings back).
    undone: Vec<String>,
    /// Checkpoint name → number of `steps` when it was taken.
    checkpoints: BTreeMap<String, usize>,
}

pub struct BatchStep {
    pub command: String,
    pub params: Value,
}

impl AgentSession {
    pub fn new(engine: Box<dyn Engine + Send>) -> Self {
        let catalog = engine.commands();
        Self {
            engine,
            catalog,
            keymap: Keymap::resolve(),
            steps: Vec::new(),
            undone: Vec::new(),
            checkpoints: BTreeMap::new(),
        }
    }

    pub fn engine_name(&self) -> &'static str {
        self.engine.name()
    }

    fn info(&self, id: &str) -> Option<&CommandInfo> {
        self.catalog.iter().find(|c| c.id == id)
    }

    fn describe(&self, c: &CommandInfo) -> Value {
        let keys: Vec<&str> = self.keymap.keys_for(&c.id).collect();
        let mut v = json!({
            "id": c.id,
            "label": c.label,
            "page": c.page.map(Page::name),
            "risk": c.risk,
            "params": c.params,
        });
        if !keys.is_empty() {
            v["resolve_keys"] = json!(keys);
        }
        if let Some(note) = &c.note {
            v["differs_from_resolve"] = json!(note);
        }
        v
    }

    /// Resolve's pages, with how many commands each offers on this engine.
    pub fn pages(&self) -> Value {
        let pages: Vec<Value> = Page::ALL
            .iter()
            .map(|p| {
                let n = self.catalog.iter().filter(|c| c.page == Some(*p)).count();
                json!({"page": p.name(), "description": p.description(), "commands": n})
            })
            .collect();
        let raw = self.catalog.iter().filter(|c| c.page.is_none()).count();
        json!({
            "pages": pages,
            "raw_engine_commands": raw,
            "hint": "search_commands with `page` to list a page's commands; include_raw reaches the engine's own commands",
        })
    }

    /// Commands matching `query` (id, label or Resolve key, case-insensitive), optionally on one page.
    pub fn search(
        &self,
        query: Option<&str>,
        page: Option<&str>,
        include_raw: bool,
        limit: usize,
    ) -> Result<Value, String> {
        let page = match page {
            Some(p) => Some(
                Page::ALL
                    .into_iter()
                    .find(|x| x.name().eq_ignore_ascii_case(p))
                    .ok_or_else(|| format!("unknown page `{p}`; one of: {}", page_names()))?,
            ),
            None => None,
        };
        let q = query.unwrap_or("").to_ascii_lowercase();
        let matches: Vec<&CommandInfo> = self
            .catalog
            .iter()
            .filter(|c| include_raw || c.page.is_some())
            .filter(|c| page.is_none() || c.page == page)
            .filter(|c| {
                q.is_empty()
                    || c.id.to_ascii_lowercase().contains(&q)
                    || c.label.to_ascii_lowercase().contains(&q)
                    || self.keymap.keys_for(&c.id).any(|k| k.eq_ignore_ascii_case(&q))
            })
            .collect();
        let total = matches.len();
        let shown: Vec<Value> = matches.into_iter().take(limit).map(|c| self.describe(c)).collect();
        Ok(json!({"total": total, "shown": shown.len(), "commands": shown}))
    }

    /// Check a command before running it. Returns its risk.
    fn preflight(&self, id: &str, dry_run: bool, acknowledge: bool) -> Result<Risk, String> {
        let Some(info) = self.info(id) else {
            if vocabulary::find(id).is_some() {
                return Err(format!(
                    "`{id}` is a Resolution command, but the {} engine cannot do it headless yet (it is front-end only or not implemented)",
                    self.engine.name()
                ));
            }
            return Err(format!("unknown command `{id}`{}", self.suggest(id)));
        };
        match info.risk {
            Risk::Destructive if dry_run => {
                Err(format!("`{id}` writes files or replaces the project, so it cannot be dry-run or undone"))
            }
            Risk::Destructive if !acknowledge => Err(format!(
                "`{id}` is destructive (writes files or replaces the open project; not undoable). Confirm with the user, then re-run with acknowledge: true"
            )),
            r => Ok(r),
        }
    }

    fn suggest(&self, id: &str) -> String {
        let needle = id.rsplit(['.', '/']).next().unwrap_or(id).to_ascii_lowercase();
        if needle.is_empty() {
            return String::new();
        }
        let near: Vec<&str> = self
            .catalog
            .iter()
            .filter(|c| c.page.is_some() && c.id.to_ascii_lowercase().contains(&needle))
            .map(|c| c.id.as_str())
            .take(5)
            .collect();
        if near.is_empty() { "; try search_commands".into() } else { format!("; did you mean: {}", near.join(", ")) }
    }

    fn timeline_snapshot(&mut self) -> Option<Value> {
        self.engine.execute("timeline.inspect", json!({})).ok().map(|o| o.result)
    }

    fn notices(&mut self) -> Vec<Value> {
        self.engine
            .drain_events()
            .into_iter()
            .filter_map(|e| match e {
                EngineEvent::Notice { message, error } => Some(json!({"message": message, "error": error})),
                EngineEvent::ProjectChanged { .. } => None,
            })
            .collect()
    }

    /// Run one command. Edits report what changed on the timeline; `dry_run` reverts the edit
    /// after measuring it.
    pub fn run(&mut self, id: &str, params: Value, dry_run: bool, acknowledge: bool) -> Result<Value, String> {
        let risk = self.preflight(id, dry_run, acknowledge)?;
        let before = (risk == Risk::Edit).then(|| self.timeline_snapshot()).flatten();
        let out = self.engine.execute(id, params).map_err(|e| e.to_string())?;
        let changes = before.and_then(|b| self.timeline_snapshot().map(|a| diff(&b, &a)));
        let mut reverted = false;
        if let Some(label) = &out.undo_label {
            if dry_run {
                self.engine.undo();
                reverted = true;
            } else {
                self.steps.push(label.clone());
                self.undone.clear();
            }
        }
        let mut v = json!({
            "ok": true,
            "command": id,
            "risk": risk,
            "result": out.result,
            "undo_label": out.undo_label,
            "revision": self.engine.revision(),
        });
        if let Some(c) = changes {
            v["timeline_changes"] = c;
        }
        if dry_run {
            v["dry_run"] = json!(true);
            v["reverted"] = json!(reverted);
            if !reverted && risk == Risk::Edit {
                v["warning"] = json!(
                    "this command made no undo step (it changes view state such as the playhead or selection), so nothing was reverted"
                );
            }
        }
        if let Some(note) = self.info(id).and_then(|c| c.note.clone()) {
            v["differs_from_resolve"] = json!(note);
        }
        let notices = self.notices();
        if !notices.is_empty() {
            v["notices"] = json!(notices);
        }
        Ok(v)
    }

    /// Run several commands in order. Everything is checked before anything runs. With `atomic`,
    /// a failure undoes the steps that already ran; with `dry_run`, every step is undone at the end.
    pub fn run_batch(
        &mut self,
        steps: Vec<BatchStep>,
        dry_run: bool,
        acknowledge: bool,
        atomic: bool,
    ) -> Result<Value, String> {
        let mut risks = Vec::with_capacity(steps.len());
        for (i, s) in steps.iter().enumerate() {
            let r = self.preflight(&s.command, dry_run, acknowledge).map_err(|e| format!("step {i}: {e}"))?;
            if r == Risk::Destructive && i + 1 != steps.len() && atomic {
                return Err(format!(
                    "step {i}: a destructive command can only be the last step of an atomic batch (it cannot be rolled back)"
                ));
            }
            risks.push(r);
        }
        let before = self.timeline_snapshot();
        let mut made = Vec::new();
        let mut results = Vec::new();
        let mut failure = None;
        for (i, s) in steps.into_iter().enumerate() {
            match self.engine.execute(&s.command, s.params) {
                Ok(out) => {
                    if let Some(l) = &out.undo_label {
                        made.push(l.clone());
                    }
                    results.push(json!({"step": i, "command": s.command, "ok": true, "result": out.result, "undo_label": out.undo_label}));
                }
                Err(e) => {
                    results.push(json!({"step": i, "command": s.command, "ok": false, "error": e.to_string()}));
                    failure = Some(i);
                    break;
                }
            }
        }
        let after = self.timeline_snapshot();
        let rollback = dry_run || (atomic && failure.is_some());
        if rollback {
            for _ in 0..made.len() {
                self.engine.undo();
            }
        } else {
            self.undone.clear();
            self.steps.extend(made.iter().cloned());
        }
        let mut v = json!({
            "ok": failure.is_none(),
            "completed": results.iter().filter(|r| r["ok"] == true).count(),
            "results": results,
            "undo_steps": made.len(),
            "rolled_back": rollback && !made.is_empty(),
            "revision": self.engine.revision(),
        });
        if let (Some(b), Some(a)) = (before, after) {
            v["timeline_changes"] = diff(&b, &a);
        }
        if dry_run {
            v["dry_run"] = json!(true);
        }
        if let Some(i) = failure {
            v["failed_step"] = json!(i);
        }
        Ok(v)
    }

    pub fn inspect(&mut self, what: &str) -> Result<Value, String> {
        match what {
            "project" => self.engine.execute("project.inspect", json!({})).map(|o| o.result).map_err(|e| e.to_string()),
            "timeline" => {
                self.engine.execute("timeline.inspect", json!({})).map(|o| o.result).map_err(|e| e.to_string())
            }
            "session" => Ok(json!({
                "engine": self.engine.name(),
                "revision": self.engine.revision(),
                "agent_edits": self.steps,
                "redo_available": self.undone,
                "checkpoints": self.checkpoints,
            })),
            other => Err(format!("unknown inspect target `{other}`; one of: project, timeline, session")),
        }
    }

    /// The program viewer as PNG.
    pub fn render_png(&self, seconds: Option<f64>, max_side: u32) -> Result<Vec<u8>, String> {
        let f = self.engine.render_frame(seconds, max_side).map_err(|e| e.to_string())?;
        let img = image::RgbaImage::from_raw(f.width, f.height, f.rgba).ok_or("engine returned a malformed frame")?;
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(png.into_inner())
    }

    pub fn undo(&mut self, n: usize) -> Value {
        let mut labels = Vec::new();
        for _ in 0..n {
            let Some(l) = self.engine.undo() else { break };
            if self.steps.last() == Some(&l) {
                self.steps.pop();
            }
            self.undone.push(l.clone());
            labels.push(l);
        }
        json!({"undone": labels, "revision": self.engine.revision()})
    }

    pub fn redo(&mut self, n: usize) -> Value {
        let mut labels = Vec::new();
        for _ in 0..n {
            let Some(l) = self.engine.redo() else { break };
            if self.undone.last() == Some(&l) {
                self.undone.pop();
            }
            self.steps.push(l.clone());
            labels.push(l);
        }
        json!({"redone": labels, "revision": self.engine.revision()})
    }

    pub fn checkpoint(&mut self, name: &str) -> Value {
        self.checkpoints.insert(name.to_string(), self.steps.len());
        json!({"checkpoint": name, "agent_edits_so_far": self.steps.len()})
    }

    /// Undo every edit made through this session since the checkpoint.
    pub fn restore(&mut self, name: &str) -> Result<Value, String> {
        let at = *self.checkpoints.get(name).ok_or_else(|| format!("no checkpoint `{name}`"))?;
        if self.steps.len() < at {
            return Err(format!("history was undone past checkpoint `{name}`; use redo or a newer checkpoint"));
        }
        let mut v = self.undo(self.steps.len() - at);
        v["restored"] = json!(name);
        Ok(v)
    }
}

fn page_names() -> String {
    Page::ALL.iter().map(|p| p.name()).collect::<Vec<_>>().join(", ")
}

/// A short list of what changed between two JSON documents (`path: old → new`).
pub fn diff(before: &Value, after: &Value) -> Value {
    let mut out = Vec::new();
    let mut total = 0usize;
    walk("", before, after, &mut out, &mut total);
    if total == 0 {
        return json!({"changed": 0});
    }
    let mut v = json!({"changed": total, "changes": out});
    if total > out.len() {
        v["more"] = json!(total - out.len());
    }
    v
}

fn walk(path: &str, a: &Value, b: &Value, out: &mut Vec<String>, total: &mut usize) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                let p = format!("{path}.{k}");
                match y.get(k) {
                    Some(vb) => walk(&p, va, vb, out, total),
                    None => note(out, total, format!("{p}: removed")),
                }
            }
            for (k, vb) in y {
                if !x.contains_key(k) {
                    note(out, total, format!("{path}.{k}: added {}", short(vb)));
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if key_of(x).is_some() && key_of(x) == key_of(y) => {
            // Lists of things with ids (tracks, clips, markers): match by id, so one cut reports
            // the clips it touched rather than every clip after it.
            let k = key_of(x).unwrap_or("id");
            for va in x {
                let p = format!("{path}[{k}={}]", va[k]);
                match y.iter().find(|vb| vb[k] == va[k]) {
                    Some(vb) => walk(&p, va, vb, out, total),
                    None => note(out, total, format!("{p}: removed")),
                }
            }
            for vb in y {
                if !x.iter().any(|va| va[k] == vb[k]) {
                    note(out, total, format!("{path}[{k}={}]: added {}", vb[k], short(vb)));
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                note(out, total, format!("{path}: {} → {} items", x.len(), y.len()));
            }
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                walk(&format!("{path}[{i}]"), va, vb, out, total);
            }
        }
        _ => note(out, total, format!("{path}: {} → {}", short(a), short(b))),
    }
}

/// The identity field shared by every element (`id`, or `clip` for timeline items), if any.
fn key_of(v: &[Value]) -> Option<&'static str> {
    ["id", "clip"].into_iter().find(|k| !v.is_empty() && v.iter().all(|e| e.get(*k).is_some_and(|id| !id.is_null())))
}

fn note(out: &mut Vec<String>, total: &mut usize, line: String) {
    *total += 1;
    if out.len() < MAX_CHANGES {
        out.push(line.trim_start_matches('.').to_string());
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 60 { format!("{}…", s.chars().take(60).collect::<String>()) } else { s }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use resolution_filmcraft_adapter::FilmCraftEngine;

    fn demo() -> AgentSession {
        let mut s = AgentSession::new(Box::new(FilmCraftEngine::new()));
        s.run("raw.filmcraft/file.openDemoProject", json!({}), false, true).expect("demo project");
        s.run("playhead.set", json!({"seconds": 2.0}), false, false).expect("playhead");
        s
    }

    #[test]
    fn search_finds_resolve_commands_with_their_keys() {
        let s = AgentSession::new(Box::new(FilmCraftEngine::new()));
        let v = s.search(Some("ripple"), Some("edit"), false, 50).expect("search");
        let split = s.search(Some("Mod+B"), None, false, 50).expect("search by key");
        assert!(v["total"].as_u64().unwrap_or(0) >= 1);
        assert_eq!(split["commands"][0]["id"], "timeline.split");
        assert!(s.search(None, Some("nope"), false, 5).is_err());
    }

    #[test]
    fn dry_run_measures_then_reverts() {
        let mut s = demo();
        let before = s.inspect("timeline").expect("timeline");
        let v = s.run("timeline.split", json!({}), true, false).expect("dry run");
        assert_eq!(v["reverted"], true);
        assert!(v["timeline_changes"]["changed"].as_u64().unwrap_or(0) > 0, "{v}");
        assert_eq!(s.inspect("timeline").expect("timeline"), before);
        assert!(s.inspect("session").expect("session")["agent_edits"].as_array().is_some_and(Vec::is_empty));
    }

    #[test]
    fn destructive_commands_need_acknowledgement_and_cannot_be_dry_run() {
        let mut s = demo();
        let path = std::env::temp_dir().join("resolution-mcp-test.fcproj");
        let p = json!({"path": path.to_string_lossy()});
        assert!(s.run("project.saveAs", p.clone(), false, false).unwrap_err().contains("acknowledge"));
        assert!(s.run("project.saveAs", p.clone(), true, true).unwrap_err().contains("dry-run"));
        assert!(s.run("project.saveAs", p, false, true).is_ok());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn atomic_batch_rolls_back_on_failure() {
        let mut s = demo();
        let before = s.inspect("timeline").expect("timeline");
        let steps = vec![
            BatchStep { command: "timeline.split".into(), params: json!({}) },
            BatchStep { command: "playhead.set".into(), params: json!({"nonsense": true}) },
        ];
        let v = s.run_batch(steps, false, false, true).expect("batch ran");
        assert_eq!(v["ok"], false);
        assert_eq!(v["failed_step"], 1);
        assert_eq!(v["rolled_back"], true);
        assert_eq!(s.inspect("timeline").expect("timeline"), before);
    }

    #[test]
    fn checkpoints_restore_agent_edits() {
        let mut s = demo();
        let before = s.inspect("timeline").expect("timeline");
        s.checkpoint("start");
        s.run("timeline.split", json!({}), false, false).expect("split");
        s.run("playhead.set", json!({"seconds": 4.0}), false, false).expect("playhead");
        s.run("timeline.split", json!({}), false, false).expect("split");
        let v = s.restore("start").expect("restore");
        assert_eq!(v["undone"].as_array().map(Vec::len), Some(2));
        let after = s.inspect("timeline").expect("timeline");
        assert!(before["video"].is_array());
        assert_eq!(after["video"], before["video"]);
        assert_eq!(after["audio"], before["audio"]);
    }

    #[test]
    fn unknown_commands_get_suggestions() {
        let mut s = demo();
        let e = s.run("timeline.rippledelete", json!({}), false, false).unwrap_err();
        assert!(e.contains("timeline.rippleDelete") || e.contains("search_commands"), "{e}");
        assert!(s.run("edit.fitToFill", json!({}), false, false).unwrap_err().contains("cannot do it headless"));
    }

    #[test]
    fn renders_png() {
        let s = demo();
        let png = s.render_png(Some(1.0), 128).expect("png");
        assert_eq!(&png[1..4], b"PNG");
    }

    #[test]
    fn diff_reports_paths() {
        let d = diff(&json!({"a": 1, "b": [1, 2]}), &json!({"a": 2, "b": [1], "c": true}));
        assert_eq!(d["changed"], 3);
        // keyed lists match by id: inserting one item reports one addition, not a shifted tail
        let a = json!({"clips": [{"id": 1, "d": 5}, {"id": 2, "d": 5}]});
        let b = json!({"clips": [{"id": 1, "d": 3}, {"id": 9, "d": 2}, {"id": 2, "d": 5}]});
        let d = diff(&a, &b);
        assert_eq!(d["changed"], 2, "{d}");
    }
}
