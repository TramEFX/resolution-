//! The editor window: Resolve's page bar and Edit page layout over the engine's command API.
//! Every edit is a vocabulary command run by the engine thread, the same commands agents use over
//! MCP (ARCHITECTURE invariant 3).
//!
//! The Edit page has a fixed layout, as in Resolve (LOG D-016): the interface toolbar shows and
//! hides panels in fixed places (a browser on the left, the Inspector or Metadata on the right,
//! the Mixer beside the timeline) rather than letting them dock anywhere. The look is the Black
//! Box skin in a choice of colour themes (LOG D-018, `theme.rs`).
//!
//! Two viewers sit side by side, as in Resolve: the source viewer plays the clip opened from the
//! Media Pool, the timeline viewer the timeline (`monitor.rs`). Playback keys, marks and frame
//! steps act on the active one; clicking a viewer or pressing Q makes it active.

use std::path::PathBuf;
use std::time::Instant;

use egui::{
    Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, UiBuilder,
    ViewportCommand, pos2, vec2,
};
use resolution_engine_api::{Engine as _, Highlight, Page};
use resolution_filmcraft_adapter::FilmCraftEngine;
use resolution_keymap::Keymap;
use resolution_playback::AudioOut;
use serde_json::json;

use crate::chrome::{self, State};
use crate::color_page::{self, ColorPage, ScopeView, Thumbs};
use crate::deliver::{self, Deliver};
use crate::icons::Icon;
use crate::journal::Journal;
use crate::model::{self, GradeView, Kind, MediaItem, SourceClip, Timeline};
use crate::monitor::{Monitor, Shown, Sound};
use crate::theme::{self, Look, Theme};
use crate::timeline_ui::{self, Action};
use crate::waveforms::Waveforms;
use crate::worker::{Reply, Worker};

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "mkv", "mxf", "avi", "webm", "mts", "m2ts", "wav", "mp3", "aac", "png", "jpg", "jpeg", "tif",
    "tiff",
];

/// Where the chosen colour theme is saved between runs.
const THEME_KEY: &str = "theme";

#[derive(Default)]
pub struct Options {
    pub demo: bool,
    pub project: Option<String>,
    pub import: Vec<String>,
    /// Self-test: once a frame is on screen, save a screenshot here and quit.
    pub screenshot: Option<PathBuf>,
    pub seek: Option<f64>,
    /// Play without opening an audio device (tests, machines without one).
    pub mute: bool,
    /// How video is decoded, for the status bar (set by `main`).
    pub decoding: String,
    /// Self-test: once a frame is on screen, play this many seconds, print what was shown and
    /// heard, and quit (after the screenshot, if one is asked for).
    pub play_for: Option<f64>,
    /// A colour theme to start with, overriding the saved one (`--theme`).
    pub theme: Option<String>,
    /// A Media Pool clip (by name) to open in the source viewer once the project is loaded.
    pub source: Option<String>,
    /// The page to start on (`--page deliver`).
    pub page: Option<String>,
    /// Where to write the agent link's file, so `resolution mcp --editor` can reach this editor
    /// (LOG D-026); `None` opens no link (tests).
    pub agent_link: Option<PathBuf>,
    /// Move the window here once it is open: its top left corner, in the screen's own pixels
    /// (`--window-at`, LOG D-049).
    pub window_at: Option<[f32; 2]>,
    /// Start with the Highlight view on, `true` in black and white (`--highlight`, for
    /// self-tests; LOG D-048).
    pub highlight: Option<bool>,
    /// Also write the action journal to this file, a line of JSON per start and end
    /// (`--journal`, LOG D-054).
    pub journal: Option<PathBuf>,
}

/// Commands that run at the frame on screen without stopping playback.
const WHILE_PLAYING: &[&str] = &["mark.in", "mark.out", "mark.clip", "marker.add"];

/// The panels that share the left of the Edit page, one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftPanel {
    MediaPool,
    Effects,
    Index,
    SoundLibrary,
}

/// The panels that share the right of the Edit page, one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanel {
    Inspector,
    Metadata,
}

/// Which panels are showing (Workspace > Show Panel in Workspace).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub left: Option<LeftPanel>,
    pub right: Option<RightPanel>,
    pub mixer: bool,
}

impl Default for Workspace {
    fn default() -> Self {
        Self { left: Some(LeftPanel::MediaPool), right: Some(RightPanel::Inspector), mixer: false }
    }
}

/// The interface toolbar's left group: panel, icon, command.
const LEFT_PANELS: [(LeftPanel, Icon, &str); 4] = [
    (LeftPanel::MediaPool, Icon::MediaPool, "workspace.mediaPool"),
    (LeftPanel::Effects, Icon::Effects, "workspace.effects"),
    (LeftPanel::Index, Icon::Index, "workspace.index"),
    (LeftPanel::SoundLibrary, Icon::SoundLibrary, "workspace.soundLibrary"),
];

/// The timeline's tool, as Resolve's Selection / Trim / Blade modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMode {
    Selection,
    Trim,
    Blade,
}

/// The two viewers: keys that play, step and mark act on the active one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewer {
    Source,
    Timeline,
}

/// What a viewer shows: name, playhead timecode, length, In and Out (frames).
type Showing = (String, String, i64, Option<i64>, Option<i64>);

/// `--play`: when it started (app clock), and the report once done.
struct PlayTest {
    seconds: f64,
    started: Option<f64>,
    done: bool,
}

pub struct ResolutionApp {
    worker: Worker,
    keymap: Keymap,
    look: Look,
    page: Page,
    workspace: Workspace,
    mode: EditMode,
    project_name: Option<String>,
    timeline: Option<Timeline>,
    media: Vec<MediaItem>,
    revision: u64,
    timeline_viewer: Monitor,
    source_viewer: Monitor,
    /// The viewer that playback keys, marks and frame steps act on.
    active: Viewer,
    sound: Sound,
    waveforms: Waveforms,
    deliver: Deliver,
    /// The Color page: the clip being graded, its grade, the current node (LOG D-029).
    color: ColorPage,
    thumbs: Thumbs,
    /// The Color page's Scopes panel (LOG D-030).
    scope_view: ScopeView,
    /// Where the timeline viewer drew its picture last frame (the Color page's window outlines).
    picture_rect: Option<Rect>,
    /// An agent's draft, from the latest batch (LOG D-027).
    draft: Option<crate::worker::Draft>,
    /// Agents reach this editor through it while it is open; dropping it closes the link.
    _agent_link: Option<resolution_agent_link::Listener>,
    clock: Instant,
    play_test: Option<PlayTest>,
    /// Where the timeline is zoomed and scrolled.
    nav: timeline_ui::Nav,
    /// Where each clip was drawn last frame: clip id by screen rectangle.
    clip_rects: Vec<(egui::Rect, u64)>,
    selected_media: Option<(Kind, u64)>,
    /// The clip open in the source viewer, from the latest snapshot.
    source_clip: Option<SourceClip>,
    status: (String, bool),
    /// After `--import`: build a timeline from the first clip once the Media Pool has it.
    auto_timeline: bool,
    seek_after_load: Option<f64>,
    source_after_load: Option<String>,
    screenshot: Option<(PathBuf, u32)>,
    /// Where to move the window on the first frame (`Options::window_at`).
    window_at: Option<[f32; 2]>,
}

impl ResolutionApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        // An editor is judged in the dark: Resolution is always dark, whatever the OS uses.
        Look::install_fonts(&cc.egui_ctx);
        let saved = cc.storage.and_then(|s| s.get_string(THEME_KEY));
        let look = Look {
            theme: theme::theme(options.theme.as_deref().or(saved.as_deref()).unwrap_or("")),
            ..Look::default()
        };
        look.apply(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let engine = FilmCraftEngine::new();
        let scopes = engine.scopes();
        // the action journal (LOG D-054); a file that cannot be made leaves it in memory
        let (journal, journal_problem) = match options.journal.as_deref().map(|p| (p, Journal::new(Some(p)))) {
            None => (Journal::in_memory(), None),
            Some((_, Ok(j))) => (j, None),
            Some((p, Err(e))) => (Journal::in_memory(), Some(format!("No journal file at {}: {e}", p.display()))),
        };
        if let Some(problem) = &journal_problem {
            eprintln!("{problem}");
        }
        let worker = Worker::spawn(Box::new(engine), journal, move || ctx.request_repaint());
        if options.demo {
            worker.run("raw.filmcraft/file.openDemoProject", json!({}));
        }
        if let Some(path) = &options.project {
            worker.run("project.open", json!({ "path": path }));
        }
        if !options.import.is_empty() {
            worker.run("media.import", json!({ "paths": options.import }));
        }
        let agent_link = options.agent_link.as_ref().and_then(|path| {
            match resolution_agent_link::listen(path, worker.agent_handle()) {
                Ok(link) => Some(link),
                Err(e) => {
                    eprintln!("agent link: {e}");
                    None
                }
            }
        });
        Self {
            worker,
            keymap: Keymap::resolve(),
            look,
            page: options
                .page
                .as_deref()
                .and_then(|n| Page::ALL.into_iter().find(|p| p.name() == n))
                .unwrap_or(Page::Edit),
            workspace: Workspace::default(),
            mode: EditMode::Selection,
            project_name: None,
            timeline: None,
            media: Vec::new(),
            revision: 0,
            timeline_viewer: Monitor::new(&cc.egui_ctx, "playhead.set", "timeline viewer"),
            source_viewer: Monitor::new(&cc.egui_ctx, "source.seek", "source viewer"),
            active: Viewer::Timeline,
            sound: Sound::new(options.mute),
            waveforms: {
                let ctx = cc.egui_ctx.clone();
                Waveforms::new(move || ctx.request_repaint())
            },
            _agent_link: agent_link,
            draft: None,
            deliver: Deliver::new(),
            color: {
                let mut c = ColorPage::default();
                c.highlight = options.highlight;
                c
            },
            thumbs: {
                let ctx = cc.egui_ctx.clone();
                Thumbs::new(move || ctx.request_repaint())
            },
            scope_view: {
                let ctx = cc.egui_ctx.clone();
                ScopeView::new(scopes, move || ctx.request_repaint())
            },
            picture_rect: None,
            clock: Instant::now(),
            play_test: options.play_for.map(|seconds| PlayTest { seconds, started: None, done: false }),
            nav: timeline_ui::Nav::default(),
            clip_rects: Vec::new(),
            selected_media: None,
            source_clip: None,
            status: (
                format!(
                    "Import media with Ctrl+I, or drop files on the window.{}",
                    if options.decoding.is_empty() {
                        String::new()
                    } else {
                        format!("   Video: {}.", options.decoding)
                    }
                ),
                false,
            ),
            auto_timeline: !options.import.is_empty(),
            seek_after_load: options.seek,
            source_after_load: options.source,
            screenshot: options.screenshot.map(|p| (p, 0)),
            window_at: options.window_at,
        }
    }

    fn say(&mut self, text: impl Into<String>, error: bool) {
        self.status = (text.into(), error);
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: &'static Theme) {
        self.look.theme = theme;
        self.look.apply(ctx);
    }

    /// A command's name in the vocabulary (Resolve's), for buttons and tooltips.
    fn label(id: &str) -> &'static str {
        resolution_engine_api::commands::find(id).map_or("", |c| c.label)
    }

    /// The keys bound to a command, as this platform writes them ("Ctrl+B").
    fn keys(&self, id: &str) -> Option<String> {
        let keys: Vec<String> = self.keymap.keys_for(id).map(chrome::key_text).collect();
        (!keys.is_empty()).then(|| keys.join(", "))
    }

    /// The timeline's playhead frame: playback's while playing, a seek on its way to the engine,
    /// or the engine's.
    fn playhead_frame(&self) -> i64 {
        self.timeline_viewer.playhead_frame()
    }

    /// A viewer's monitor, with the sound and the worker it needs (disjoint borrows).
    fn viewer_mut(&mut self, v: Viewer) -> (&mut Monitor, &mut Sound, &Worker) {
        match v {
            Viewer::Source => (&mut self.source_viewer, &mut self.sound, &self.worker),
            Viewer::Timeline => (&mut self.timeline_viewer, &mut self.sound, &self.worker),
        }
    }

    fn monitor(&self, v: Viewer) -> &Monitor {
        match v {
            Viewer::Source => &self.source_viewer,
            Viewer::Timeline => &self.timeline_viewer,
        }
    }

    /// The playhead in timeline seconds, for drawing.
    fn playhead(&self) -> f64 {
        self.timeline.as_ref().map_or(0.0, |t| t.rate.seconds_of(self.playhead_frame()))
    }

    fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::Ran { id, result } => {
                // every color.* command answers with the clip's grade
                if id.starts_with("color.")
                    && let Ok(out) = &result
                    && let Some(v) = GradeView::from_json(&out.result)
                {
                    self.color.take(v, out.revision);
                }
                // renders report on the Deliver page, not as "done"
                match id.as_str() {
                    "deliver.render" => {
                        let job = result.and_then(|o| {
                            o.result["job"].as_u64().ok_or_else(|| "the engine started no job".to_string())
                        });
                        if let Err(e) = &job {
                            self.say(format!("Render failed: {e}"), true);
                        }
                        self.deliver.started(job);
                        return;
                    }
                    "deliver.jobs" => {
                        if let Ok(out) = result {
                            for path in self.deliver.update(&out.result) {
                                self.say(format!("Rendered {}", path.display()), false);
                            }
                        }
                        return;
                    }
                    "deliver.cancel" => return,
                    _ => {}
                }
                match id.as_str() {
                    "playhead.set" => self.timeline_viewer.seek_answered(),
                    "source.seek" => self.source_viewer.seek_answered(),
                    _ => {}
                }
                match result {
                    Ok(out) => {
                        if let Some(label) = out.undo_label {
                            self.say(label, false);
                        } else if !id.ends_with(".inspect")
                            && !id.starts_with("playhead.")
                            && !id.starts_with("color.")
                            && !matches!(id.as_str(), "timeline.select" | "source.seek" | "source.open")
                        {
                            self.say(format!("{id}: done"), false);
                        }
                    }
                    Err(e) => self.say(e, true),
                }
            }
            Reply::Snapshot { revision, timeline, project, source, playback, source_playback } => {
                self.revision = revision;
                self.timeline = timeline.as_ref().and_then(Timeline::from_json);
                self.source_clip = source.as_ref().and_then(SourceClip::from_json);
                let frame = self.timeline.as_ref().map_or(0, |t| t.playhead_frame);
                self.timeline_viewer.update_program(playback, frame, &mut self.sound);
                let frame = self.source_clip.as_ref().map_or(0, |s| s.playhead_frame);
                self.source_viewer.update_program(source_playback, frame, &mut self.sound);
                // read the waveforms of the timeline's sound, in the background
                if let (Some(tl), Some(p)) = (&self.timeline, &self.timeline_viewer.program) {
                    let items = tl.audio.iter().flat_map(|t| t.clips.iter().map(|c| c.item));
                    self.waveforms.want(items, p);
                }
                self.media = project.as_ref().map(model::media_items).unwrap_or_default();
                self.project_name = project.as_ref().and_then(model::project_name);
                if self.auto_timeline
                    && self.timeline.is_none()
                    && let Some(first) = self.media.iter().find(|m| m.kind == Kind::Clip)
                {
                    self.auto_timeline = false;
                    self.worker.run("timeline.createFromClips", json!({ "items": [first.id] }));
                }
                if self.timeline.is_some()
                    && let Some(t) = self.seek_after_load.take()
                {
                    self.worker.run("playhead.set", json!({ "seconds": t }));
                }
                // `--source`: open that clip once a timeline exists (before, a clip becomes one)
                if self.timeline.is_some()
                    && let Some(name) = self.source_after_load.take()
                {
                    match self.media.iter().find(|m| m.kind == Kind::Clip && m.name == name).cloned() {
                        Some(m) => self.open_media(&m),
                        None => self.source_after_load = Some(name),
                    }
                }
            }
            Reply::Notice { message, error } => self.say(message, error),
            Reply::Draft(d) => self.draft = d,
        }
    }

    /// A vocabulary command from a key, a button or a menu. Front-end commands (pages, panels,
    /// playback, zoom, edit modes) are handled here; everything else goes to the engine.
    fn command(&mut self, id: &str) {
        // panels, zoom and modes leave playback alone; edits stop it, except marking the viewer
        // that plays, which marks the frame on screen
        let front_end = id.starts_with("workspace.")
            || id.starts_with("view.")
            || id.starts_with("mode.")
            || matches!(id, "color.previousNode" | "color.nextNode" | "color.highlight" | "color.highlightBW");
        let active = self.active;
        if !id.starts_with("playback.") && !front_end {
            for v in [Viewer::Source, Viewer::Timeline] {
                let (m, sound, worker) = self.viewer_mut(v);
                if !m.is_playing() {
                    continue;
                }
                if WHILE_PLAYING.contains(&id) && v == active {
                    m.sync_engine(worker);
                } else {
                    m.stop(worker, sound);
                }
            }
        }
        let speed = self.monitor(active).transport.speed();
        let source = active == Viewer::Source;
        let w = &mut self.workspace;
        let toggle_left = |w: &mut Workspace, p| w.left = if w.left == Some(p) { None } else { Some(p) };
        let toggle_right = |w: &mut Workspace, p| w.right = if w.right == Some(p) { None } else { Some(p) };
        match id {
            _ if id.starts_with("page.") => {
                if let Some(p) = Page::ALL.into_iter().find(|p| format!("page.{}", p.name()) == id) {
                    self.page = p;
                }
            }
            "workspace.mediaPool" => toggle_left(w, LeftPanel::MediaPool),
            "workspace.effects" => toggle_left(w, LeftPanel::Effects),
            "workspace.index" => toggle_left(w, LeftPanel::Index),
            "workspace.soundLibrary" => toggle_left(w, LeftPanel::SoundLibrary),
            "workspace.inspector" => toggle_right(w, RightPanel::Inspector),
            "workspace.metadata" => toggle_right(w, RightPanel::Metadata),
            "workspace.mixer" => w.mixer = !w.mixer,
            "workspace.resetLayout" => *w = Workspace::default(),
            "mode.selection" => self.mode = EditMode::Selection,
            "mode.trim" => {
                self.mode = EditMode::Trim;
                self.say(
                    "Trim Edit Mode: drag an edge to ripple, a clip's upper half to slip, its lower half to slide.",
                    false,
                );
            }
            "mode.blade" => {
                self.mode = EditMode::Blade;
                self.say("Blade Edit Mode: click a clip to cut it there.", false);
            }
            "view.toggleViewer" => {
                self.active = if source { Viewer::Timeline } else { Viewer::Source };
            }
            "playback.playStop" => {
                if self.monitor(active).is_playing() {
                    self.stop_viewer(active);
                } else {
                    self.play(active, 1.0);
                }
            }
            // each press doubles the speed in its direction, up to 8x
            "playback.forward" => self.play(active, speed.filter(|s| *s > 0.0).map_or(1.0, |s| (s * 2.0).min(8.0))),
            "playback.reverse" => self.play(active, speed.filter(|s| *s < 0.0).map_or(-1.0, |s| (s * 2.0).max(-8.0))),
            "playback.stop" => self.stop_viewer(active),
            // the source viewer's playhead and marks: its own commands, or the timeline's with
            // `target: source`
            "playhead.previousFrame" | "playhead.nextFrame" | "playhead.timelineStart" | "playhead.timelineEnd"
                if source =>
            {
                let (m, sound, worker) = self.viewer_mut(Viewer::Source);
                let to = match id {
                    "playhead.previousFrame" => m.playhead_frame() - 1,
                    "playhead.nextFrame" => m.playhead_frame() + 1,
                    "playhead.timelineStart" => 0,
                    _ => m.last_frame(),
                };
                m.seek(worker, to, sound);
            }
            "playhead.previousEdit" | "playhead.nextEdit" | "mark.clip" if source => {}
            "mark.in" | "mark.out" | "mark.clearIn" | "mark.clearOut" | "mark.clearInOut" | "marker.add" if source => {
                if self.source_clip.is_some() {
                    self.worker.run(id, json!({ "target": "source" }));
                }
            }
            "view.zoomIn" => self.nav.zoom_by(1.5),
            "view.zoomOut" => self.nav.zoom_by(1.0 / 1.5),
            "view.zoomToFit" => self.nav.fit(),
            "media.import" => self.import_dialog(),
            // a new, empty timeline in the open timeline's format (the engine's default otherwise)
            "timeline.new" => {
                let n = self.media.iter().filter(|m| m.kind == Kind::Timeline).count() + 1;
                let mut params = json!({ "name": format!("Timeline {n}") });
                if let Some(tl) = &self.timeline {
                    params["width"] = json!(tl.width);
                    params["height"] = json!(tl.height);
                    params["fps"] = json!(tl.fps);
                }
                self.worker.run(id, params);
            }
            "edit.insert" | "edit.overwrite" if self.source_clip.is_none() => {
                self.say("Open a clip in the source viewer first: double-click it in the Media Pool.", true);
            }
            _ if id.starts_with("mode.") || id.starts_with("playback.") => {
                self.say(format!("{id} is not built yet"), false);
            }
            "project.save" | "project.saveAs" => self.save_dialog(),
            // the Color page's node keys act there only
            _ if id.starts_with("color.") => self.color_command(id),
            _ => self.worker.run(id, json!({})),
        }
    }

    /// An edit made with the mouse: stops playback first, as an edit from a key does.
    fn edit(&mut self, id: &str, params: serde_json::Value) {
        self.edit_stop();
        self.worker.run(id, params);
    }

    /// Stop both viewers before an edit.
    fn edit_stop(&mut self) {
        self.stop_viewer(Viewer::Source);
        self.stop_viewer(Viewer::Timeline);
    }

    /// Play `v` at `speed`; the other viewer stops, since they share the sound.
    fn play(&mut self, v: Viewer, speed: f64) {
        self.stop_viewer(if v == Viewer::Source { Viewer::Timeline } else { Viewer::Source });
        let (m, sound, _) = self.viewer_mut(v);
        m.play(speed, sound);
    }

    fn stop_viewer(&mut self, v: Viewer) {
        let (m, sound, worker) = self.viewer_mut(v);
        m.stop(worker, sound);
    }

    /// Move the timeline's playhead to `frame` (ruler clicks and drags), stopping playback.
    fn seek(&mut self, frame: i64) {
        self.stop_viewer(Viewer::Source);
        let (m, sound, worker) = self.viewer_mut(Viewer::Timeline);
        m.seek(worker, frame, sound);
    }

    /// Both viewers: move playing ones on, and put each one's frame on screen.
    fn advance_viewers(&mut self, ctx: &egui::Context) {
        let clock = self.clock;
        let now = move || clock.elapsed().as_secs_f64();
        for v in [Viewer::Source, Viewer::Timeline] {
            let (m, sound, worker) = self.viewer_mut(v);
            let problem = m.advance(ctx, worker, now, sound);
            let error = m.update(ctx, now());
            if let Some(p) = problem {
                self.say(p, true);
            }
            if let Some(e) = error {
                let text = format!("Viewer: {e}");
                if self.status.0 != text {
                    self.say(text, true);
                }
            }
        }
    }

    fn import_dialog(&mut self) {
        if let Some(files) = rfd::FileDialog::new().add_filter("Media", MEDIA_EXTENSIONS).pick_files() {
            let paths: Vec<String> = files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
            self.worker.run("media.import", json!({ "paths": paths }));
        }
    }

    fn save_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Project", &["fcproj"]).save_file() {
            self.worker.run("project.saveAs", json!({ "path": path.to_string_lossy() }));
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Project", &["fcproj"]).pick_file() {
            self.worker.run("project.open", json!({ "path": path.to_string_lossy() }));
        }
    }

    fn read_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = e
                && let Some(chord) = crate::keys::chord(key, modifiers)
                && let Some(id) = self.keymap.command_for(&chord).map(str::to_string)
            {
                self.command(&id);
            }
        }
        let dropped: Vec<String> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_string_lossy().into_owned())
                .filter(|p| !p.is_empty())
                .collect()
        });
        if !dropped.is_empty() {
            self.worker.run("media.import", json!({ "paths": dropped }));
        }
    }

    /// A menu item that runs a command, with its keys.
    fn menu_command(&mut self, ui: &mut Ui, id: &str) {
        let text = match self.keys(id) {
            Some(k) => format!("{}   {k}", Self::label(id)),
            None => Self::label(id).to_string(),
        };
        if ui.button(text).clicked() {
            ui.close();
            self.command(id);
        }
    }

    fn menu(&mut self, ui: &mut Ui) {
        let look = self.look;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(RichText::new("Resolution").font(look.medium()).color(look.theme.text));
            ui.add_space(10.0);
            ui.menu_button("File", |ui| {
                self.menu_command(ui, "timeline.new");
                self.menu_command(ui, "media.import");
                if ui.button("Open Project…").clicked() {
                    ui.close();
                    self.open_dialog();
                }
                if ui.button("Save Project As…").clicked() {
                    ui.close();
                    self.save_dialog();
                }
            });
            ui.menu_button("Edit", |ui| {
                self.menu_command(ui, "edit.undo");
                self.menu_command(ui, "edit.redo");
            });
            ui.menu_button("View", |ui| {
                let lit = self.color.highlight;
                for (id, on) in [("color.highlight", lit == Some(false)), ("color.highlightBW", lit == Some(true))] {
                    let text = match self.keys(id) {
                        Some(k) => format!("{}   {k}", Self::label(id)),
                        None => Self::label(id).to_string(),
                    };
                    if ui.add_enabled(self.page == Page::Color, egui::Button::selectable(on, text)).clicked() {
                        ui.close();
                        self.command(id);
                    }
                }
            });
            ui.menu_button("Timeline", |ui| {
                for id in ["timeline.split", "timeline.rippleDelete", "view.zoomToFit"] {
                    self.menu_command(ui, id);
                }
            });
            ui.menu_button("Workspace", |ui| {
                ui.menu_button("Show Panel in Workspace", |ui| {
                    let w = self.workspace;
                    for (id, on) in [
                        ("workspace.mediaPool", w.left == Some(LeftPanel::MediaPool)),
                        ("workspace.effects", w.left == Some(LeftPanel::Effects)),
                        ("workspace.index", w.left == Some(LeftPanel::Index)),
                        ("workspace.soundLibrary", w.left == Some(LeftPanel::SoundLibrary)),
                        ("workspace.mixer", w.mixer),
                        ("workspace.metadata", w.right == Some(RightPanel::Metadata)),
                        ("workspace.inspector", w.right == Some(RightPanel::Inspector)),
                    ] {
                        if ui.add(egui::Button::selectable(on, Self::label(id))).clicked() {
                            ui.close();
                            self.command(id);
                        }
                    }
                });
                ui.menu_button("Theme", |ui| {
                    for t in theme::THEMES {
                        if ui.add(egui::Button::selectable(self.look.theme.id == t.id, t.name)).clicked() {
                            ui.close();
                            self.set_theme(ui.ctx(), t);
                        }
                    }
                });
                ui.separator();
                self.menu_command(ui, "workspace.resetLayout");
            });
        });
    }

    /// The interface toolbar: panel toggles on either side, the project's name in the middle.
    fn interface_toolbar(&mut self, ui: &mut Ui) {
        let look = self.look;
        let rect = ui.max_rect();
        let name = self.project_name.as_deref().unwrap_or("Untitled Project");
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, name, look.medium(), look.theme.text);
        // the panel buttons belong to the Edit page
        if self.page != Page::Edit {
            return;
        }
        let w = self.workspace;
        let mut run: Option<&'static str> = None;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (panel, icon, id) in LEFT_PANELS {
                let keys = self.keys(id);
                if chrome::icon_button(
                    ui,
                    &look,
                    icon,
                    Self::label(id),
                    keys.as_deref(),
                    State::on(w.left == Some(panel)),
                )
                .clicked()
                {
                    run = Some(id);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for (icon, id, on) in [
                    (Icon::Inspector, "workspace.inspector", w.right == Some(RightPanel::Inspector)),
                    (Icon::Metadata, "workspace.metadata", w.right == Some(RightPanel::Metadata)),
                    (Icon::Mixer, "workspace.mixer", w.mixer),
                ] {
                    let keys = self.keys(id);
                    if chrome::icon_button(ui, &look, icon, Self::label(id), keys.as_deref(), State::on(on)).clicked() {
                        run = Some(id);
                    }
                }
            });
        });
        if let Some(id) = run {
            self.command(id);
        }
    }

    fn page_bar(&mut self, ui: &mut Ui) {
        let look = self.look;
        let pages = [
            (Page::Media, Icon::PageMedia),
            (Page::Cut, Icon::PageCut),
            (Page::Edit, Icon::PageEdit),
            (Page::Fusion, Icon::PageFusion),
            (Page::Color, Icon::PageColor),
            (Page::Fairlight, Icon::PageFairlight),
            (Page::Deliver, Icon::PageDeliver),
        ];
        let label = |p: Page| {
            let name = p.name();
            format!("{}{}", name[..1].to_uppercase(), &name[1..])
        };
        let gap = 6.0;
        let width: f32 = pages
            .iter()
            .map(|(p, _)| ui.painter().layout_no_wrap(label(*p), look.body(), Color32::WHITE).size().x + 53.0 + gap)
            .sum();
        let mut run = None;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            for (p, icon) in pages {
                if chrome::page_button(ui, &look, icon, &label(p), self.page == p).clicked() {
                    run = Some(format!("page.{}", p.name()));
                }
            }
        });
        if let Some(id) = run {
            self.command(&id);
        }
    }

    fn media_pool(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
        let header = chrome::panel_header(ui, &look, "Media Pool", None);
        if chrome::header_button(ui, &look, header, Icon::Import, "Import Media") {
            self.command("media.import");
        }
        let footer = look.skin.button + 16.0;
        let list_h = (ui.available_height() - footer).max(40.0);
        let mut clicked: Option<MediaItem> = None;
        let mut opened: Option<MediaItem> = None;
        ui.allocate_ui(vec2(ui.available_width(), list_h), |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                if self.media.is_empty() {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new("No media yet.\nImport with Ctrl+I or drop files here.").color(t.dim));
                    });
                }
                for m in &self.media {
                    let selected = self.selected_media == Some((m.kind, m.id));
                    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &m.name));
                    let painter = ui.painter();
                    if selected {
                        painter.rect_filled(rect, CornerRadius::ZERO, t.active_bg);
                    } else if r.hovered() {
                        painter.rect_filled(rect, CornerRadius::ZERO, t.hover);
                    }
                    let x = rect.left() + 10.0 + m.depth as f32 * 14.0;
                    let icon = match m.kind {
                        Kind::Bin => Icon::Bin,
                        Kind::Clip => Icon::Clip,
                        Kind::Timeline => Icon::Timeline,
                    };
                    let tint = if m.active { t.accent } else { t.dim };
                    crate::icons::paint(
                        painter,
                        Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0)),
                        icon,
                        Stroke::new(look.skin.icon_stroke, tint),
                    );
                    let duration = match (m.duration, &self.timeline) {
                        (Some(d), Some(tl)) => tl.timecode(d),
                        (Some(d), None) => format!("{d:.1} s"),
                        _ => String::new(),
                    };
                    let right = rect.right() - 10.0;
                    let dur = painter.text(
                        pos2(right, rect.center().y),
                        Align2::RIGHT_CENTER,
                        &duration,
                        look.mono(look.skin.small),
                        t.dim,
                    );
                    let name_clip =
                        Rect::from_min_max(pos2(x + 22.0, rect.top()), pos2(dur.left() - 8.0, rect.bottom()));
                    painter.with_clip_rect(name_clip).text(
                        pos2(x + 24.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &m.name,
                        if m.kind == Kind::Bin { look.medium() } else { look.body() },
                        if selected || m.active { t.text } else { t.dim.lerp_to_gamma(t.text, 0.5) },
                    );
                    if r.clicked() {
                        clicked = Some(m.clone());
                    }
                    if r.double_clicked() {
                        opened = Some(m.clone());
                    }
                }
            });
        });
        if let Some(m) = clicked {
            self.selected_media = Some((m.kind, m.id));
        }
        if let Some(m) = opened {
            self.open_media(&m);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let clip = self.selected_media.filter(|(k, _)| *k == Kind::Clip).map(|(_, id)| id);
            if chrome::text_button(ui, &look, "New timeline from clip", clip.is_some()).clicked()
                && let Some(id) = clip
            {
                self.worker.run("timeline.createFromClips", json!({ "items": [id] }));
            }
        });
    }

    /// Double-click in the Media Pool, as in Resolve: a timeline opens in the timeline panel; a
    /// clip opens in the source viewer (ready for F9 / F10), or becomes the first timeline.
    fn open_media(&mut self, m: &MediaItem) {
        match m.kind {
            Kind::Bin => {}
            Kind::Timeline => self.worker.run("timeline.open", json!({ "item": m.id })),
            Kind::Clip if self.timeline.is_none() => {
                self.worker.run("timeline.createFromClips", json!({ "items": [m.id] }))
            }
            Kind::Clip => {
                self.stop_viewer(Viewer::Source);
                self.worker.run("source.open", json!({ "item": m.id }));
                self.active = Viewer::Source;
                self.say(
                    format!("Source: {}. Mark In and Out with I and O; F9 inserts, F10 overwrites.", m.name),
                    false,
                );
            }
        }
    }

    /// The Index panel's edit list: every clip in record order. Clicking one selects it and
    /// moves the playhead to its start.
    fn index_panel(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
        let Some(tl) = &self.timeline else {
            chrome::panel_header(ui, &look, "Index", None);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("No timeline.").color(t.dim));
            });
            return;
        };
        let mut edits: Vec<(String, &model::Clip)> = tl
            .video
            .iter()
            .enumerate()
            .flat_map(|(i, tr)| tr.clips.iter().map(move |c| (format!("V{}", i + 1), c)))
            .chain(
                tl.audio
                    .iter()
                    .enumerate()
                    .flat_map(|(i, tr)| tr.clips.iter().map(move |c| (format!("A{}", i + 1), c))),
            )
            .collect();
        edits.sort_by(|a, b| a.1.start.total_cmp(&b.1.start).then_with(|| a.0.cmp(&b.0)));
        let detail = format!("{} events", edits.len());
        chrome::panel_header(ui, &look, "Index", Some(&detail));
        let columns = |ui: &mut Ui, rect: Rect, cells: [&str; 5], colour: Color32| {
            let painter = ui.painter();
            let font = look.mono(10.5);
            let xs = [10.0, 34.0, rect.width() - 168.0, rect.width() - 84.0];
            painter.text(
                pos2(rect.left() + xs[0], rect.center().y),
                Align2::LEFT_CENTER,
                cells[0],
                font.clone(),
                colour,
            );
            painter.text(
                pos2(rect.left() + xs[1], rect.center().y),
                Align2::LEFT_CENTER,
                cells[1],
                font.clone(),
                colour,
            );
            let clip = Rect::from_min_max(
                pos2(rect.left() + 64.0, rect.top()),
                pos2(rect.left() + xs[2] - 6.0, rect.bottom()),
            );
            painter.with_clip_rect(clip).text(
                pos2(clip.left(), rect.center().y),
                Align2::LEFT_CENTER,
                cells[2],
                look.small(),
                colour,
            );
            painter.text(
                pos2(rect.left() + xs[2], rect.center().y),
                Align2::LEFT_CENTER,
                cells[3],
                font.clone(),
                colour,
            );
            painter.text(pos2(rect.left() + xs[3], rect.center().y), Align2::LEFT_CENTER, cells[4], font, colour);
        };
        let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
        columns(ui, head, ["#", "Track", "Clip", "Record In", "Record Out"], t.dim);
        ui.painter().line_segment([head.left_bottom(), head.right_bottom()], look.sep_stroke());
        let mut pick: Option<(u64, f64)> = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (n, (track, c)) in edits.iter().enumerate() {
                let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{} {}", track, c.name))
                });
                if c.selected {
                    ui.painter().rect_filled(rect, CornerRadius::ZERO, t.active_bg);
                } else if r.hovered() {
                    ui.painter().rect_filled(rect, CornerRadius::ZERO, t.hover);
                }
                let (num, rin, rout) = ((n + 1).to_string(), tl.timecode(c.start), tl.timecode(c.end));
                columns(ui, rect, [&num, track, &c.name, &rin, &rout], if c.selected { t.text } else { t.dim });
                if r.clicked() {
                    pick = Some((c.id, c.start));
                }
            }
        });
        if let Some((clip, start)) = pick {
            self.worker.run("timeline.select", json!({ "clips": [clip] }));
            if let Some(rate) = self.timeline.as_ref().map(|t| t.rate) {
                self.seek(rate.frame_at(start));
            }
        }
    }

    fn inspector(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
        let Some(tl) = &self.timeline else {
            chrome::panel_header(ui, &look, "Inspector", None);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("No timeline.").color(t.dim));
            });
            return;
        };
        let (title, rows): (&str, Vec<(&str, String)>) = if let Some((track, clip)) = tl.selected() {
            (
                &clip.name,
                vec![
                    ("Track", track.name.clone()),
                    ("Start", tl.timecode(clip.start)),
                    ("End", tl.timecode(clip.end)),
                    ("Duration", tl.timecode(clip.end - clip.start)),
                    ("Enabled", if clip.enabled { "Yes" } else { "No" }.into()),
                ],
            )
        } else {
            (
                &tl.name,
                vec![
                    ("Format", format!("{} × {}", tl.width, tl.height)),
                    ("Frame rate", format!("{:.3}", tl.fps)),
                    ("Duration", tl.timecode(tl.duration)),
                    ("Tracks", format!("{} video, {} audio", tl.video.len(), tl.audio.len())),
                ],
            )
        };
        chrome::panel_header(ui, &look, "Inspector", Some(title));
        ui.add_space(8.0);
        for (k, v) in rows {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
            let painter = ui.painter();
            painter.text(rect.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, k, look.body(), t.dim);
            let field = Rect::from_min_max(
                pos2(rect.left() + 110.0, rect.top() + 2.0),
                pos2(rect.right() - 12.0, rect.bottom() - 2.0),
            );
            painter.rect_filled(field, look.radius(), t.field);
            painter.with_clip_rect(field).text(
                field.left_center() + vec2(8.0, 0.0),
                Align2::LEFT_CENTER,
                v,
                look.mono(look.skin.small),
                t.text,
            );
        }
    }

    /// The source viewer and the timeline viewer, side by side.
    fn viewers(&mut self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        let half = (rect.width() / 2.0).round();
        let left = Rect::from_min_max(rect.min, pos2(rect.left() + half, rect.bottom()));
        let right = Rect::from_min_max(pos2(rect.left() + half + self.look.skin.gap, rect.top()), rect.max);
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.look.theme.sep);
        self.viewer(ui, left, Viewer::Source);
        self.viewer(ui, right, Viewer::Timeline);
        ui.allocate_rect(rect, Sense::hover());
    }

    /// One viewer: name and timecode above the picture (an accent line under them when it is the
    /// active viewer), a scrub bar with its In and Out, then the transport and mark buttons.
    fn viewer(&mut self, ui: &mut Ui, rect: Rect, which: Viewer) {
        let look = self.look;
        let (t, skin) = (look.theme, look.skin);
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), skin.header));
        let transport = Rect::from_min_max(pos2(rect.left(), rect.bottom() - skin.bar), rect.max);
        let scrub = Rect::from_min_max(
            pos2(rect.left() + 12.0, transport.top() - 22.0),
            pos2(rect.right() - 12.0, transport.top()),
        );
        let picture = Rect::from_min_max(pos2(rect.left(), header.bottom()), pos2(rect.right(), scrub.top() - 2.0));
        // what this viewer shows: name, timecode, playhead, length, In and Out (frames)
        let m = self.monitor(which);
        let playhead = m.playhead_frame();
        let info: Option<Showing> = match which {
            Viewer::Source => self
                .source_clip
                .as_ref()
                .map(|s| (s.name.clone(), s.timecode(playhead), s.frames, s.mark_in, s.mark_out)),
            Viewer::Timeline => self.timeline.as_ref().map(|tl| {
                let frames = tl.rate.frame_at(tl.duration).max(1);
                (
                    tl.name.clone(),
                    tl.frame_timecode(playhead),
                    frames,
                    tl.mark_in.map(|s| tl.rate.frame_at(s)),
                    tl.mark_out.map(|s| tl.rate.frame_at(s)),
                )
            }),
        };
        let painter = ui.painter_at(rect);
        let mut picture_rect = None;
        painter.rect_filled(rect, CornerRadius::ZERO, t.panel);
        painter.rect_filled(header, CornerRadius::ZERO, t.head);
        painter.line_segment([header.left_bottom(), header.right_bottom()], look.sep_stroke());
        if self.active == which {
            painter.line_segment(
                [header.left_bottom() + vec2(0.0, -1.0), header.right_bottom() + vec2(0.0, -1.0)],
                Stroke::new(2.0, t.accent),
            );
        }
        painter.rect_filled(picture, CornerRadius::ZERO, Color32::BLACK);
        let mut seek_to = None;
        match &info {
            Some((name, tc, frames, mark_in, mark_out)) => {
                let tc = painter.text(
                    header.right_center() - vec2(10.0, 0.0),
                    Align2::RIGHT_CENTER,
                    tc,
                    look.mono(13.0),
                    t.text,
                );
                // while playing: the speed, and the frames per second actually reaching the screen
                let mut name_end = tc.left() - 12.0;
                if let Some(speed) = m.transport.speed() {
                    let speed = if speed == 1.0 { String::new() } else { format!("{speed}x   ") };
                    let fps = match (m.transport.in_preroll(), m.meter.fps) {
                        (true, _) => "starting".to_string(),
                        (false, Some(fps)) => format!("{fps:.1} fps"),
                        (false, None) => String::new(),
                    };
                    let r = painter.text(
                        tc.left_center() - vec2(14.0, 0.0),
                        Align2::RIGHT_CENTER,
                        format!("{speed}{fps}"),
                        look.mono(11.0),
                        t.dim,
                    );
                    name_end = r.left() - 12.0;
                }
                let label = Rect::from_min_max(header.min, pos2(name_end.max(header.left() + 40.0), header.bottom()));
                painter.with_clip_rect(label).text(
                    header.left_center() + vec2(10.0, 0.0),
                    Align2::LEFT_CENTER,
                    name,
                    look.medium(),
                    t.text,
                );
                if let Some(tex) = &m.frame {
                    let size = tex.size_vec2();
                    let scale = (picture.width() / size.x).min(picture.height() / size.y).max(0.01);
                    let img = Rect::from_center_size(picture.center(), size * scale);
                    painter.image(tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    picture_rect = Some(img);
                }
                // the scrub bar: the whole clip or timeline, its In and Out, the playhead
                let line = Rect::from_center_size(scrub.center(), vec2(scrub.width(), 2.0));
                painter.rect_filled(line, CornerRadius::ZERO, t.line);
                let span = (*frames - 1).max(1) as f32;
                let x_of = |f: i64| scrub.left() + (f as f32 / span).clamp(0.0, 1.0) * scrub.width();
                let y = scrub.center().y;
                if mark_in.is_some() || mark_out.is_some() {
                    let (a, b) = (x_of(mark_in.unwrap_or(0)), x_of(mark_out.unwrap_or(*frames - 1)));
                    painter.rect_filled(
                        Rect::from_x_y_ranges(a..=b.max(a + 1.0), (y - 3.0)..=(y + 3.0)),
                        CornerRadius::ZERO,
                        t.accent.gamma_multiply(0.35),
                    );
                    for (mark, glyph) in [(mark_in, "["), (mark_out, "]")] {
                        if let Some(f) = mark {
                            painter.text(pos2(x_of(*f), y), Align2::CENTER_CENTER, glyph, look.mono(13.0), t.accent);
                        }
                    }
                }
                let x = x_of(playhead);
                painter.line_segment([pos2(x, y - 7.0), pos2(x, y + 7.0)], Stroke::new(2.0, t.accent));
                let r = ui.interact(scrub, ui.id().with(("scrub", which as u8)), Sense::click_and_drag());
                if (r.clicked() || r.dragged())
                    && let Some(p) = r.interact_pointer_pos()
                {
                    seek_to =
                        Some((f64::from((p.x - scrub.left()) / scrub.width()).clamp(0.0, 1.0) * span as f64).round()
                            as i64);
                }
            }
            None => {
                let hint = match which {
                    Viewer::Source => "Double-click a clip in the Media Pool to open it here.",
                    Viewer::Timeline => "Import media, then double-click a clip in the Media Pool to make a timeline.",
                };
                painter.with_clip_rect(picture.shrink(8.0)).text(
                    picture.center(),
                    Align2::CENTER_CENTER,
                    hint,
                    look.body(),
                    t.dim,
                );
            }
        }
        // Frames at the viewer's size on screen, in 240 px steps so resizing does not re-render
        // on every repaint.
        if which == Viewer::Timeline {
            self.picture_rect = picture_rect;
        }
        let px = picture.width().max(picture.height()) * ui.ctx().pixels_per_point();
        let side = ((px / 240.0).ceil() as u32 * 240).clamp(480, 3840);
        let (m, sound, worker) = self.viewer_mut(which);
        m.view_side = side;
        if let Some(frame) = seek_to {
            m.seek(worker, frame, sound);
        }
        // a click anywhere in the viewer makes it the active one
        if ui.input(|i| i.pointer.primary_pressed() && i.pointer.interact_pos().is_some_and(|p| rect.contains(p))) {
            self.active = which;
        }
        // the transport, and the marks, centred under the picture
        let playing = self.monitor(which).transport.speed();
        let has = info.is_some();
        let buttons = [
            (Icon::ToStart, "playhead.timelineStart", State::Off),
            (Icon::Reverse, "playback.reverse", State::on(playing.is_some_and(|s| s < 0.0))),
            (Icon::Stop, "playback.stop", State::Off),
            (Icon::Play, "playback.playStop", State::on(playing.is_some_and(|s| s > 0.0))),
            (Icon::ToEnd, "playhead.timelineEnd", State::Off),
            (Icon::MarkIn, "mark.in", State::Off),
            (Icon::MarkOut, "mark.out", State::Off),
        ];
        let width = buttons.len() as f32 * (skin.button + 4.0) + 13.0;
        let row = Rect::from_center_size(transport.center(), vec2(width, skin.button));
        let mut run = None;
        let prefix = if which == Viewer::Source { "Source" } else { "Timeline" };
        ui.scope_builder(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (n, (icon, id, state)) in buttons.into_iter().enumerate() {
                if n == 5 {
                    chrome::divider(ui, &look);
                }
                let keys = self.keys(id);
                let state = if has { state } else { State::Disabled };
                let label = format!("{prefix} {}", Self::label(id));
                if chrome::icon_button(ui, &look, icon, &label, keys.as_deref(), state).clicked() {
                    run = Some(id);
                }
            }
        });
        if let Some(id) = run {
            self.active = which;
            self.command(id);
        }
    }

    /// Above the timeline: its name and the playhead's timecode, then the tools.
    fn timeline_toolbar(&mut self, ui: &mut Ui) {
        let look = self.look;
        let (t, skin) = (look.theme, look.skin);
        let rect = ui.max_rect();
        let name = self.timeline.as_ref().map_or("No timeline", |tl| tl.name.as_str());
        let tc =
            self.timeline.as_ref().map_or("--:--:--:--".to_string(), |tl| tl.frame_timecode(self.playhead_frame()));
        let painter = ui.painter();
        let name_rect =
            painter.text(rect.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, name, look.medium(), t.text);
        painter.text(name_rect.right_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, tc, look.mono(17.0), t.text);
        let has = self.timeline.is_some();
        let on = |b: bool| if !has { State::Disabled } else { State::on(b) };
        let (snap, linked) = self.timeline.as_ref().map_or((false, false), |tl| (tl.snapping, tl.linked_selection));
        let groups: [&[(Icon, &str, State)]; 5] = [
            &[
                (Icon::Selection, "mode.selection", on(self.mode == EditMode::Selection)),
                (Icon::Trim, "mode.trim", on(self.mode == EditMode::Trim)),
                (Icon::Blade, "mode.blade", on(self.mode == EditMode::Blade)),
            ],
            &[(Icon::Insert, "edit.insert", on(false)), (Icon::Overwrite, "edit.overwrite", on(false))],
            &[(Icon::Snapping, "timeline.snapping", on(snap)), (Icon::Linked, "timeline.linkedSelection", on(linked))],
            &[(Icon::Marker, "marker.add", on(false))],
            &[
                (Icon::ZoomFit, "view.zoomToFit", on(false)),
                (Icon::ZoomOut, "view.zoomOut", on(false)),
                (Icon::ZoomIn, "view.zoomIn", on(false)),
            ],
        ];
        let buttons: usize = groups.iter().map(|g| g.len()).sum();
        let width = buttons as f32 * (skin.button + 2.0) + (groups.len() - 1) as f32 * 15.0;
        let row = Rect::from_center_size(rect.center(), vec2(width, skin.button));
        let mut run = None;
        ui.scope_builder(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (n, group) in groups.iter().enumerate() {
                if n > 0 {
                    chrome::divider(ui, &look);
                }
                for (icon, id, state) in group.iter() {
                    let keys = self.keys(id);
                    if chrome::icon_button(ui, &look, *icon, Self::label(id), keys.as_deref(), *state).clicked() {
                        run = Some(*id);
                    }
                }
            }
        });
        if let Some(id) = run {
            self.command(id);
        }
    }

    /// An agent's draft (LOG D-027): what it changed, and the user's call. Above the timeline on
    /// the Edit page, across the top of the Color page.
    fn draft_banner(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
        if let Some(d) = self.draft.clone() {
            let mut decided = None;
            egui::Panel::top("agent-draft")
                .exact_size(36.0)
                .frame(Frame::new().fill(t.accent.gamma_multiply(0.14)).inner_margin(Margin::symmetric(12, 0)))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        let list = d.edits();
                        let edits = match list.len() {
                            0 => "no edits".to_string(),
                            1 => "1 edit".to_string(),
                            n => format!("{n} edits"),
                        };
                        let latest = list.last().map(|l| format!(", latest: {l}")).unwrap_or_default();
                        let text = format!("Agent draft of \u{201c}{}\u{201d}: {edits}{latest}", d.name);
                        let receipt = list
                            .iter()
                            .enumerate()
                            .map(|(i, e)| format!("{}. {e}", i + 1))
                            .collect::<Vec<_>>()
                            .join("\n");
                        ui.label(RichText::new(text).font(look.medium()).color(t.text)).on_hover_text(receipt);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if chrome::text_button(ui, &look, "Reject Draft", true).clicked() {
                                decided = Some(false);
                            }
                            if chrome::text_button(ui, &look, "Accept Draft", true).clicked() {
                                decided = Some(true);
                            }
                        });
                    });
                });
            if let Some(accept) = decided {
                self.edit_stop();
                self.worker.decide(accept);
            }
        }
    }

    fn timeline_panel(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
        self.draft_banner(ui);
        egui::Panel::top("timeline-toolbar")
            .exact_size(look.skin.bar)
            .frame(Frame::new().fill(t.panel).inner_margin(Margin::symmetric(8, 0)))
            .show(ui, |ui| self.timeline_toolbar(ui));
        if self.workspace.mixer {
            egui::Panel::right("mixer").exact_size(220.0).frame(Frame::new().fill(t.panel)).show(ui, |ui| {
                chrome::panel_header(ui, &look, "Mixer", None);
                chrome::not_built(ui, &look, "The Mixer");
            });
        }
        egui::CentralPanel::default().frame(Frame::new().fill(t.panel)).show(ui, |ui| {
            let playhead = self.playhead();
            match &self.timeline {
                Some(tl) => {
                    let (rate, linked) = (tl.rate, tl.linked_selection);
                    let tool = match self.mode {
                        EditMode::Selection => timeline_ui::Tool::Selection,
                        EditMode::Trim => timeline_ui::Tool::Trim,
                        EditMode::Blade => timeline_ui::Tool::Blade,
                    };
                    let waveforms = self.waveforms.ready();
                    let view = timeline_ui::View { tool, waveforms: &waveforms };
                    let (actions, rects) = timeline_ui::show(ui, &look, tl, playhead, &view, &mut self.nav);
                    self.clip_rects = rects;
                    for a in actions {
                        match a {
                            Action::Seek(s) => self.seek(rate.frame_at(s.max(0.0))),
                            Action::Select { clip, add } => {
                                self.worker.run("timeline.select", json!({ "clips": [clip], "add": add }))
                            }
                            // track switches change the project, not playback: no stop
                            Action::Track { track, command, field, value } => {
                                self.worker.run(command, json!({ "track": track, field: value }))
                            }
                            Action::Move { clip, track, start_frame } => self.edit(
                                "timeline.moveClips",
                                json!({
                                    "moves": [{ "clip": clip, "track": track, "time": model::frame_ticks(rate, start_frame) }],
                                    "insert": false,
                                    "linked": linked,
                                }),
                            ),
                            Action::Trim { clip, edge, ripple, frames } => self.edit(
                                "timeline.trimEdge",
                                json!({
                                    "clip": clip,
                                    "edge": edge.name(),
                                    "mode": if ripple { "ripple" } else { "regular" },
                                    "deltaFrames": frames,
                                }),
                            ),
                            Action::Roll { left, right, frames } => self
                                .edit("timeline.rollEdit", json!({ "left": left, "right": right, "deltaFrames": frames })),
                            Action::Slip { clip, frames } => {
                                self.edit("timeline.slipClip", json!({ "clip": clip, "deltaFrames": frames }))
                            }
                            Action::Slide { clip, frames } => {
                                self.edit("timeline.slideClip", json!({ "clip": clip, "deltaFrames": frames }))
                            }
                            Action::Blade { clip, frame } => self.edit(
                                "timeline.bladeClip",
                                json!({ "clip": clip, "time": model::frame_ticks(rate, frame) }),
                            ),
                        }
                    }
                }
                None => {
                    ui.centered_and_justified(|ui| ui.label(RichText::new("No timeline").color(t.dim)));
                }
            }
        });
    }

    /// Renders: start the next queued one when Render All is on, and follow the running one.
    fn deliver_tick(&mut self, ctx: &egui::Context) {
        if let Some(i) = self.deliver.next() {
            self.deliver.queue[i].status = deliver::Status::Starting;
            self.worker.run("deliver.render", self.deliver.queue[i].params());
        }
        if self.deliver.busy() {
            let now = self.clock.elapsed().as_secs_f64();
            if now - self.deliver.last_poll > 0.4 {
                self.deliver.last_poll = now;
                self.worker.run("deliver.jobs", json!({}));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    /// The Deliver page: render settings on the left, the timeline viewer, the render queue on
    /// the right, the timeline below (for In and Out).
    fn deliver_page(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
        let fill = |c: Color32| Frame::new().fill(c);
        // a shorter timeline than the Edit page's: here it is for In and Out
        egui::Panel::bottom("deliver-timeline")
            .resizable(true)
            .default_size(220.0)
            .min_size(180.0)
            .frame(fill(t.panel))
            .show(ui, |ui| self.timeline_panel(ui));
        let timeline = self.timeline.as_ref().map(|t| t.name.clone());
        let has_marks = self.timeline.as_ref().is_some_and(|t| t.mark_in.is_some() || t.mark_out.is_some());
        let mut asks = Vec::new();
        egui::Panel::left("deliver-settings")
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .frame(fill(t.panel))
            .show(ui, |ui| {
                asks.extend(deliver::settings(ui, &look, &mut self.deliver, timeline.as_deref(), has_marks))
            });
        egui::Panel::right("deliver-queue")
            .resizable(true)
            .default_size(320.0)
            .min_size(240.0)
            .frame(fill(t.panel))
            .show(ui, |ui| asks.extend(deliver::queue(ui, &look, &self.deliver)));
        egui::CentralPanel::default().frame(fill(t.panel)).show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            self.viewer(ui, rect, Viewer::Timeline);
            ui.allocate_rect(rect, Sense::hover());
        });
        for ask in asks {
            match ask {
                deliver::Ask::Browse => {
                    if let Some(dir) = rfd::FileDialog::new().set_directory(&self.deliver.folder).pick_folder() {
                        self.deliver.folder = dir;
                    }
                }
                deliver::Ask::Add => {
                    if let Some(name) = &timeline {
                        let text = format!("Queued {}", self.deliver.add(name).path.display());
                        self.say(text, false);
                    }
                }
                deliver::Ask::RenderAll => self.deliver.running = true,
                deliver::Ask::Stop(i) => {
                    if let Some(job) = self.deliver.queue.get(i).and_then(|j| j.job) {
                        self.worker.run("deliver.cancel", json!({ "job": job }));
                    }
                }
                deliver::Ask::Remove(i) => {
                    if i < self.deliver.queue.len() {
                        self.deliver.queue.remove(i);
                    }
                }
            }
        }
    }

    /// The clip the Color page grades: the one under the playhead on the highest video track
    /// that shows one.
    fn clip_at_playhead(&self) -> Option<u64> {
        let tl = self.timeline.as_ref()?;
        let f = self.playhead_frame();
        tl.video.iter().rev().filter(|tr| tr.enabled).find_map(|tr| {
            tr.clips
                .iter()
                .find(|c| c.enabled && tl.rate.frame_at(c.start) <= f && f < tl.rate.frame_at(c.end))
                .map(|c| c.id)
        })
    }

    /// Follow the playhead onto a clip, and ask for its grade when the clip or the project changes
    /// (not during a drag: the controls show what is being sent).
    fn color_follow(&mut self) {
        let clip = self.clip_at_playhead();
        if clip != self.color.clip {
            self.color.clip = clip;
            self.color.view = None;
            self.color.node = 0;
            self.color.asked = None;
        }
        if let Some(c) = clip
            && !self.color.dragging
            && self.color.asked != Some((c, self.revision))
        {
            self.color.asked = Some((c, self.revision));
            self.worker.run("color.inspect", json!({ "clip": c }));
        }
    }

    /// The timeline's video clips in order, for the Color page's strip.
    fn strip_clips(&self) -> Vec<color_page::StripClip> {
        let Some(tl) = &self.timeline else { return Vec::new() };
        let mut clips: Vec<color_page::StripClip> = tl
            .video
            .iter()
            .flat_map(|tr| {
                tr.clips.iter().map(|c| color_page::StripClip {
                    id: c.id,
                    name: c.name.clone(),
                    track: tr.label.clone(),
                    start_frame: tl.rate.frame_at(c.start),
                    mid_frame: tl.rate.frame_at((c.start + c.end) / 2.0),
                    graded: c.graded,
                })
            })
            .collect();
        clips.sort_by(|a, b| a.start_frame.cmp(&b.start_frame).then_with(|| a.track.cmp(&b.track)));
        clips
    }

    /// A node command from a key, a button or a node's menu, on the clip being graded. Previous
    /// and Next Node only move the selection, looping round as in Resolve.
    fn color_command(&mut self, id: &str) {
        if self.page != Page::Color {
            return;
        }
        // the Highlight view, on and off; each key turns its own kind on, or off again
        if let Some(black_white) = match id {
            "color.highlight" => Some(false),
            "color.highlightBW" => Some(true),
            _ => None,
        } {
            let lit = &mut self.color.highlight;
            *lit = (*lit != Some(black_white)).then_some(black_white);
            return;
        }
        let (Some(clip), Some(count)) = (self.color.clip, self.color.view.as_ref().map(|v| v.grade.nodes.len())) else {
            return;
        };
        let n = self.color.node.min(count.saturating_sub(1));
        let node = n + 1;
        let params = match id {
            "color.previousNode" => {
                self.color.node = (n + count - 1) % count;
                return;
            }
            "color.nextNode" => {
                self.color.node = (n + 1) % count;
                return;
            }
            "color.addSerialNode" => {
                self.color.node = n + 1;
                json!({ "clip": clip, "node": node })
            }
            "color.addNodeBefore" | "color.resetNode" => json!({ "clip": clip, "node": node }),
            "color.appendNode" => {
                self.color.node = count;
                json!({ "clip": clip })
            }
            "color.deleteNode" => {
                self.color.node = n.saturating_sub(1);
                json!({ "clip": clip, "node": node })
            }
            "color.toggleNodes" => json!({ "clip": clip, "nodes": [node] }),
            "color.resetAll" => {
                self.color.node = 0;
                json!({ "clip": clip })
            }
            _ => json!({ "clip": clip }),
        };
        self.edit(id, params);
    }

    /// The Color page (LOG D-029): the timeline viewer with the Node Editor to its right, the
    /// clip strip under them, and the Primaries and Curves palettes at the bottom.
    /// The Highlight view the timeline viewer shows (LOG D-048): on the Color page, of the
    /// current clip's current node, while it is on.
    fn highlight(&self) -> Option<Highlight> {
        let black_white = self.color.highlight.filter(|_| self.page == Page::Color)?;
        Some(Highlight { clip: self.color.clip?, node: self.color.node, black_white })
    }

    fn color_page(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
        let fill = |c: Color32| Frame::new().fill(c);
        self.active = Viewer::Timeline;
        self.color_follow();
        self.scope_view.follow(ui.ctx(), self.timeline_viewer.graded_picture());
        self.draft_banner(ui);
        let mut asks = Vec::new();
        egui::Panel::bottom("color-palettes")
            .resizable(true)
            .default_size(340.0)
            .min_size(260.0)
            .frame(fill(t.panel))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let gap = look.skin.gap;
                let left_w = (rect.width() * 0.5).clamp(520.0, 820.0);
                let mid_w = (rect.width() - left_w).min(460.0);
                let left = Rect::from_min_size(rect.min, vec2(left_w, rect.height()));
                let mid = Rect::from_min_size(pos2(left.right() + gap, rect.top()), vec2(mid_w - gap, rect.height()));
                let right = Rect::from_min_max(pos2(mid.right() + gap, rect.top()), rect.max);
                ui.painter().rect_filled(rect, CornerRadius::ZERO, t.sep);
                for r in [left, mid, right] {
                    ui.painter().rect_filled(r, CornerRadius::ZERO, t.panel);
                }
                asks.extend(color_page::within(ui, left, |ui| color_page::primaries(ui, &look, &mut self.color)));
                asks.extend(color_page::within(ui, mid, |ui| match self.color.palette {
                    color_page::Palette::Curves => color_page::curves(ui, &look, &mut self.color),
                    color_page::Palette::Qualifier => color_page::qualifier(ui, &look, &mut self.color),
                    color_page::Palette::Window => color_page::windows(ui, &look, &mut self.color),
                }));
                color_page::within(ui, right, |ui| color_page::scopes_panel(ui, &look, &mut self.scope_view));
                // the whole panel is used: a resizable panel shrinks to what its contents take
                ui.allocate_rect(rect, Sense::hover());
            });
        let clips = self.strip_clips();
        egui::Panel::bottom("color-clips").exact_size(126.0).frame(fill(t.panel)).show(ui, |ui| {
            let playback = self.timeline_viewer.program.clone();
            asks.extend(color_page::strip(ui, &look, &clips, &self.color, &mut self.thumbs, playback.as_ref()));
        });
        egui::Panel::right("color-nodes")
            .resizable(true)
            .default_size(520.0)
            .min_size(320.0)
            .frame(fill(t.panel))
            .show(ui, |ui| asks.extend(color_page::node_editor(ui, &look, &mut self.color)));
        egui::CentralPanel::default().frame(fill(t.panel)).show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            self.viewer(ui, rect, Viewer::Timeline);
            ui.allocate_rect(rect, Sense::hover());
            // the windows' outlines while the Window palette is open
            if self.color.palette == color_page::Palette::Window
                && let (Some(pic), Some(tl)) = (self.picture_rect, &self.timeline)
            {
                let frame = (tl.width, tl.height);
                asks.extend(color_page::window_overlay(ui, &look, pic, frame, &mut self.color));
            }
            // the qualifier's picker while the Qualifier palette is open
            if self.color.palette == color_page::Palette::Qualifier
                && let Some(pic) = self.picture_rect
            {
                let picture = self.timeline_viewer.graded_picture().cloned();
                asks.extend(color_page::qualifier_picker(ui, pic, picture.as_ref(), &mut self.color));
            }
        });
        for ask in asks {
            match ask {
                color_page::Ask::Adjust(controls, merge) => {
                    if let Some(clip) = self.color.clip {
                        let mut params = json!({ "clip": clip, "node": self.color.node + 1, "controls": controls });
                        if let Some(m) = merge {
                            params["merge"] = json!(m);
                        }
                        self.edit("color.adjust", params);
                    }
                }
                color_page::Ask::Command(id) => self.color_command(id),
                color_page::Ask::Seek(frame) => self.seek(frame),
            }
        }
    }

    /// `--play`: once a frame is on screen, play for the time asked, report, stop, and quit (or
    /// hand over to `--screenshot`).
    fn play_test(&mut self, ctx: &egui::Context) {
        let Some(test) = &self.play_test else { return };
        let (seconds, started, done) = (test.seconds, test.started, test.done);
        let now = self.clock.elapsed().as_secs_f64();
        ctx.request_repaint();
        if done {
            return;
        }
        let Some(started) = started else {
            let m = &self.timeline_viewer;
            if m.shown.is_some() && self.seek_after_load.is_none() && m.pending_seek.is_none() {
                self.active = Viewer::Timeline;
                self.command("playback.playStop");
                if let Some(t) = &mut self.play_test {
                    t.started = Some(now);
                }
            }
            return;
        };
        let m = &self.timeline_viewer;
        let played = now - m.meter.started;
        let reached_end = !m.transport.is_playing();
        if m.transport.in_preroll() || (played < seconds && !reached_end) {
            return;
        }
        let fps = self.timeline.as_ref().map_or(0.0, |t| t.fps);
        let audio = self.sound.out.as_ref();
        let stats = &self.sound.stats;
        let load = |a: &std::sync::atomic::AtomicU64| a.load(std::sync::atomic::Ordering::Relaxed);
        let st = m.frames.stats();
        eprintln!(
            "play test: preroll {:.0} ms, played {played:.1} s{}",
            (m.meter.started - started) * 1e3,
            if reached_end { " (reached the end)" } else { "" }
        );
        eprintln!(
            "  picture: {} frames shown, {:.1} fps (timeline {fps:.3} fps); frame jobs {} done, {} cancelled, {} failed",
            m.meter.shown,
            m.meter.shown as f64 / played.max(1e-9),
            st.jobs,
            st.cancelled,
            st.failed
        );
        let (gpu, cpu) = resolution_filmcraft_adapter::frame_counts();
        eprintln!(
            "  renderer: {gpu} timeline frames drawn on the GPU, {cpu} on the CPU (LOG D-036); {:.1} ms a frame job ({:.1} getting source pictures); the last frame shown {}",
            st.job_ms / st.jobs.max(1) as f64,
            st.decode_ms / st.jobs.max(1) as f64,
            if matches!(m.frame, Some(Shown::OnGpu { .. })) {
                "straight from the GPU (D-039)"
            } else {
                "from its pixels"
            }
        );
        eprintln!(
            "  sound: device {:?}, {} Hz, {} channels; clock from {}; {} device callbacks, {} underruns, {} frames of silence",
            audio.and_then(AudioOut::device),
            audio.map_or(0, AudioOut::sample_rate),
            audio.map_or(0, AudioOut::channels),
            if m.transport.audio_clock() { "the audio device" } else { "the wall clock" },
            load(&stats.callbacks),
            load(&stats.underruns),
            load(&stats.missing_frames)
        );
        eprintln!("  status: {}", self.status.0);
        if let Some(t) = &mut self.play_test {
            t.done = true;
        }
        if self.screenshot.is_some() {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        } else {
            self.stop_viewer(Viewer::Timeline);
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// `--screenshot`: once a frame has been on screen for a few repaints, save the window and quit.
    fn self_test(&mut self, ctx: &egui::Context) {
        self.play_test(ctx);
        let playing_test = self.play_test.as_ref().is_some_and(|t| !t.done);
        let Some((path, frames)) = &mut self.screenshot else { return };
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let [w, h] = image.size;
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let saved = image::RgbaImage::from_raw(w as u32, h as u32, rgba).map(|img| img.save(&*path));
            eprintln!("screenshot {}: {:?}", path.display(), saved.map(|r| r.map_err(|e| e.to_string())));
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        let ready = self.timeline_viewer.frame.is_some() && self.timeline.is_some() && self.seek_after_load.is_none();
        *frames += 1;
        if !playing_test && ((ready && *frames == 30) || *frames == 1800) {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint();
    }
}

impl eframe::App for ResolutionApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // eframe opens the window where it was last (it reads that back even when told not to
        // keep it), so a window put elsewhere on purpose is moved once it is open; egui's
        // positions are points of the screen the window is on, which may be scaled
        if let Some([x, y]) = self.window_at.take() {
            let ppp = ctx.pixels_per_point();
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x / ppp, y / ppp)));
        }
        for reply in self.worker.poll() {
            self.apply(reply);
        }
        self.read_keys(&ctx);
        self.timeline_viewer.highlight = self.highlight();
        self.advance_viewers(&ctx);
        self.deliver_tick(&ctx);

        let look = self.look;
        let t = look.theme;
        let fill = |c: Color32| Frame::new().fill(c);
        egui::Panel::top("menu")
            .frame(fill(t.ground).inner_margin(Margin::symmetric(8, 3)))
            .show(ui, |ui| self.menu(ui));
        egui::Panel::top("interface")
            .exact_size(look.skin.bar)
            .frame(fill(t.panel).inner_margin(Margin::symmetric(8, 0)))
            .show(ui, |ui| self.interface_toolbar(ui));
        egui::Panel::bottom("pages")
            .exact_size(44.0)
            .frame(fill(t.panel).inner_margin(Margin::symmetric(8, 0)))
            .show(ui, |ui| self.page_bar(ui));
        egui::Panel::bottom("status")
            .exact_size(22.0)
            .frame(fill(t.ground).inner_margin(Margin::symmetric(10, 0)))
            .show(ui, |ui| {
                let (text, error) = &self.status;
                ui.centered_and_justified(|ui| {
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.label(RichText::new(text).font(look.small()).color(if *error { t.error } else { t.dim }));
                    });
                });
            });
        if self.page == Page::Deliver {
            self.deliver_page(ui);
            self.self_test(&ctx);
            return;
        }
        if self.page == Page::Color {
            self.color_page(ui);
            self.self_test(&ctx);
            return;
        }
        if self.page != Page::Edit {
            egui::CentralPanel::default().frame(fill(t.panel)).show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "The {} page is not built yet. Shift+4 returns to Edit.",
                            self.page.name()
                        ))
                        .size(16.0)
                        .color(t.dim),
                    );
                });
            });
            self.self_test(&ctx);
            return;
        }
        egui::Panel::bottom("timeline")
            .resizable(true)
            .default_size(330.0)
            .min_size(180.0)
            .frame(fill(t.panel))
            .show(ui, |ui| self.timeline_panel(ui));
        if let Some(left) = self.workspace.left {
            egui::Panel::left("left").resizable(true).default_size(300.0).min_size(220.0).frame(fill(t.panel)).show(
                ui,
                |ui| match left {
                    LeftPanel::MediaPool => self.media_pool(ui),
                    LeftPanel::Index => self.index_panel(ui),
                    LeftPanel::Effects => {
                        chrome::panel_header(ui, &look, "Effects Library", None);
                        chrome::not_built(ui, &look, "The Effects Library");
                    }
                    LeftPanel::SoundLibrary => {
                        chrome::panel_header(ui, &look, "Sound Library", None);
                        chrome::not_built(ui, &look, "The Sound Library");
                    }
                },
            );
        }
        if let Some(right) = self.workspace.right {
            egui::Panel::right("right").resizable(true).default_size(300.0).min_size(220.0).frame(fill(t.panel)).show(
                ui,
                |ui| match right {
                    RightPanel::Inspector => self.inspector(ui),
                    RightPanel::Metadata => {
                        chrome::panel_header(ui, &look, "Metadata", None);
                        chrome::not_built(ui, &look, "The Metadata panel");
                    }
                },
            );
        }
        egui::CentralPanel::default().frame(fill(t.panel)).show(ui, |ui| self.viewers(ui));
        self.self_test(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.look.theme.id.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Key, Modifiers, PointerButton, Pos2};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use std::time::{Duration, Instant};

    fn demo() -> Harness<'static, ResolutionApp> {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1600.0, 920.0))
            .build_eframe(|cc| ResolutionApp::new(cc, Options { demo: true, mute: true, ..Default::default() }));
        pump(&mut h, "the demo timeline", |a| a.timeline.is_some());
        h
    }

    /// Step the UI until `done` holds: the engine answers from its own thread.
    fn pump(h: &mut Harness<'_, ResolutionApp>, what: &str, mut done: impl FnMut(&ResolutionApp) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !done(h.state()) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            h.step();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn v1_clips(a: &ResolutionApp) -> usize {
        a.timeline.as_ref().map_or(0, |t| t.video[0].clips.len())
    }

    #[test]
    fn resolve_keys_edit_the_timeline() {
        let mut h = demo();
        let clips = v1_clips(h.state());
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
        pump(&mut h, "Ctrl+B to split the clip at the playhead", |a| v1_clips(a) == clips + 1);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        pump(&mut h, "Ctrl+Z to undo the split", |a| v1_clips(a) == clips);
        let f0 = h.state().timeline.as_ref().map_or(0, |t| t.playhead_frame);
        h.key_press(Key::ArrowRight);
        pump(&mut h, "Right to step one frame", |a| a.timeline.as_ref().is_some_and(|t| t.playhead_frame == f0 + 1));
    }

    #[test]
    fn space_plays_and_stopping_leaves_the_playhead_where_it_stopped() {
        let mut h = demo();
        pump(&mut h, "the program to play", |a| {
            a.timeline_viewer.program.is_some() && a.timeline_viewer.shown.is_some()
        });
        h.key_press(Key::Space);
        h.step();
        let start = h.state().timeline_viewer.transport.frame().expect("playing");
        pump(&mut h, "playback to move the playhead a few frames", |a| {
            a.timeline_viewer.transport.frame().is_some_and(|f| f >= start + 6)
                && !a.timeline_viewer.transport.in_preroll()
        });
        pump(&mut h, "frames rendered ahead to reach the viewer", |a| {
            a.timeline_viewer.shown.is_some_and(|k| k.frame > start)
        });
        h.key_press(Key::Space);
        h.step();
        let stopped = h.state().playhead_frame();
        assert!(!h.state().timeline_viewer.transport.is_playing());
        assert!(stopped > start);
        pump(&mut h, "the engine's playhead to land where playback stopped", |a| {
            a.timeline_viewer.pending_seek.is_none() && a.timeline.as_ref().is_some_and(|t| t.playhead_frame == stopped)
        });
        pump(&mut h, "the viewer to show the frame it stopped on", |a| {
            a.timeline_viewer.shown.is_some_and(|k| k.frame == stopped)
        });
    }

    #[test]
    fn marking_in_while_playing_keeps_playing() {
        let mut h = demo();
        pump(&mut h, "the program to play", |a| {
            a.timeline_viewer.program.is_some() && a.timeline_viewer.shown.is_some()
        });
        h.key_press(Key::Space);
        pump(&mut h, "the clock to start", |a| {
            a.timeline_viewer.transport.is_playing() && !a.timeline_viewer.transport.in_preroll()
        });
        h.key_press(Key::I);
        h.step();
        let marked = h.state().timeline_viewer.transport.frame().expect("still playing");
        pump(&mut h, "the In point at the frame that was on screen", |a| {
            a.timeline.as_ref().is_some_and(|t| t.mark_in.is_some_and(|s| (s - t.rate.seconds_of(marked)).abs() < 0.1))
        });
        assert!(h.state().timeline_viewer.transport.is_playing(), "marking does not stop playback");
    }

    #[test]
    fn l_doubles_the_speed_and_k_stops() {
        let mut h = demo();
        pump(&mut h, "the program to play", |a| a.timeline_viewer.program.is_some());
        h.key_press(Key::L);
        h.key_press(Key::L);
        h.step();
        assert_eq!(h.state().timeline_viewer.transport.speed(), Some(2.0));
        h.key_press(Key::J);
        h.step();
        assert_eq!(h.state().timeline_viewer.transport.speed(), Some(-1.0), "J reverses at normal speed");
        h.key_press(Key::K);
        h.step();
        assert!(!h.state().timeline_viewer.transport.is_playing());
    }

    #[test]
    fn pages_switch_with_resolve_keys() {
        let mut h = demo();
        h.key_press_modifiers(Modifiers::SHIFT, Key::Num2);
        pump(&mut h, "Shift+2 for the Media page", |a| a.page == Page::Media);
        h.key_press_modifiers(Modifiers::SHIFT, Key::Num4);
        pump(&mut h, "Shift+4 back to the Edit page", |a| a.page == Page::Edit);
    }

    #[test]
    fn clicking_a_clip_selects_it() {
        let mut h = demo();
        pump(&mut h, "clips on screen", |a| !a.clip_rects.is_empty());
        let (rect, clip) = h.state().clip_rects[0];
        let at: Pos2 = rect.center();
        h.event(egui::Event::PointerMoved(at));
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            h.step();
        }
        pump(&mut h, "the clicked clip to be selected", |a| {
            a.timeline.as_ref().and_then(|t| t.selected()).is_some_and(|(_, c)| c.id == clip)
        });
    }

    /// Click the widget named `label` (the demo keeps repainting, so step rather than `run`).
    fn click(h: &mut Harness<'_, ResolutionApp>, label: &str) {
        h.get_by_label(label).click();
        h.step();
        h.step();
    }

    #[test]
    fn the_interface_toolbar_swaps_and_hides_panels() {
        let mut h = demo();
        assert_eq!(h.state().workspace, Workspace::default());
        // the left slot holds one browser at a time
        click(&mut h, "Index");
        assert_eq!(h.state().workspace.left, Some(LeftPanel::Index));
        click(&mut h, "Index");
        assert_eq!(h.state().workspace.left, None, "a second click hides it");
        click(&mut h, "Inspector");
        assert_eq!(h.state().workspace.right, None);
        click(&mut h, "Mixer");
        assert!(h.state().workspace.mixer);
        h.state_mut().command("workspace.resetLayout");
        assert_eq!(h.state().workspace, Workspace::default());
    }

    #[test]
    fn track_header_switches_change_the_track() {
        let mut h = demo();
        let v1 = |a: &ResolutionApp| a.timeline.as_ref().map(|t| t.video[0].clone()).expect("V1");
        assert!(!v1(h.state()).locked);
        click(&mut h, "Lock V1");
        pump(&mut h, "V1 to lock", |a| v1(a).locked);
        click(&mut h, "Lock V1");
        pump(&mut h, "V1 to unlock", |a| !v1(a).locked);
        let targeted = v1(h.state()).targeted;
        click(&mut h, "Auto Select V1");
        pump(&mut h, "V1's auto track selector to flip", |a| v1(a).targeted != targeted);
        click(&mut h, "Show V1");
        pump(&mut h, "V1 to hide", |a| !v1(a).enabled);
    }

    /// Press at `from`, move to `to` in steps, release: a mouse drag.
    fn drag(h: &mut Harness<'_, ResolutionApp>, from: Pos2, to: Pos2) {
        h.event(egui::Event::PointerMoved(from));
        h.step();
        h.event(egui::Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        h.step();
        for k in 1..=6 {
            h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)));
            h.step();
        }
        h.event(egui::Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        h.step();
        h.step();
    }

    /// The demo's clip on V2, and where it is drawn.
    fn v2_clip(a: &ResolutionApp) -> (model::Clip, egui::Rect) {
        let c =
            a.timeline.as_ref().and_then(|t| t.video.get(1)).and_then(|v| v.clips.first().cloned()).expect("V2 clip");
        let rect = a.clip_rects.iter().find(|(_, id)| *id == c.id).map(|(r, _)| *r).expect("V2 clip on screen");
        (c, rect)
    }

    #[test]
    fn dragging_with_the_selection_tool_resizes_and_moves() {
        let mut h = demo();
        pump(&mut h, "clips on screen", |a| !a.clip_rects.is_empty());
        // turn snapping off so the drags land where the pointer does
        if h.state().timeline.as_ref().is_some_and(|t| t.snapping) {
            h.state_mut().command("timeline.snapping");
            pump(&mut h, "snapping off", |a| a.timeline.as_ref().is_some_and(|t| !t.snapping));
        }
        let (c, r) = v2_clip(h.state());
        // the out edge, 60 px left: a shorter clip
        drag(&mut h, pos2(r.right() - 2.0, r.center().y), pos2(r.right() - 62.0, r.center().y));
        pump(&mut h, "the V2 clip to end earlier", |a| v2_clip(a).0.end < c.end - 0.1);
        let (c, r) = v2_clip(h.state());
        assert!((v2_clip(h.state()).0.start - c.start).abs() < 1e-9, "a resize keeps the in point");
        // the middle, 80 px right: the clip moves later, same length
        drag(&mut h, r.center(), r.center() + egui::vec2(80.0, 0.0));
        pump(&mut h, "the V2 clip to move later", |a| v2_clip(a).0.start > c.start + 0.1);
        let moved = v2_clip(h.state()).0;
        assert!(((moved.end - moved.start) - (c.end - c.start)).abs() < 0.05, "a move keeps the length");
    }

    #[test]
    fn the_trim_tool_slips_and_the_blade_tool_cuts() {
        let mut h = demo();
        pump(&mut h, "clips on screen", |a| !a.clip_rects.is_empty());
        let (c, r) = v2_clip(h.state());
        h.key_press(Key::T);
        h.step();
        // the upper half of a clip slips it: same place, other media
        drag(&mut h, r.center() - egui::vec2(0.0, r.height() / 4.0), r.center() + egui::vec2(-50.0, -r.height() / 4.0));
        pump(&mut h, "the V2 clip to slip", |a| (v2_clip(a).0.source_in - c.source_in).abs() > 0.05);
        assert!((v2_clip(h.state()).0.start - c.start).abs() < 1e-9, "a slip keeps the clip in place");
        // the blade cuts the clip it is clicked on
        h.key_press(Key::B);
        h.step();
        let (_, r) = v2_clip(h.state());
        let at = r.center();
        h.event(egui::Event::PointerMoved(at));
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            h.step();
        }
        pump(&mut h, "V2 to hold two clips", |a| a.timeline.as_ref().is_some_and(|t| t.video[1].clips.len() == 2));
    }

    #[test]
    fn the_trim_tool_ripples_once_the_music_track_is_not_sync_locked() {
        let mut h = demo();
        pump(&mut h, "clips on screen", |a| !a.clip_rects.is_empty());
        // the demo's music on A2 spans every cut, and FilmCraft refuses a ripple that would
        // break a sync-locked track's sync (LOG B-023)
        click(&mut h, "Sync Lock A2");
        pump(&mut h, "A2's sync lock off", |a| a.timeline.as_ref().is_some_and(|t| !t.audio[1].sync_lock));
        h.key_press(Key::T);
        h.step();
        let v1 =
            |a: &ResolutionApp, i: usize| a.timeline.as_ref().map(|t| t.video[0].clips[i].clone()).expect("V1 clip");
        let (first, second) = (v1(h.state(), 0), v1(h.state(), 1));
        let r = h.state().clip_rects.iter().find(|(_, id)| *id == first.id).map(|(r, _)| *r).expect("first clip drawn");
        // just inside the out edge, beside the cut rather than on it: a ripple, not a roll
        drag(&mut h, pos2(r.right() - 5.0, r.center().y), pos2(r.right() - 65.0, r.center().y));
        pump(&mut h, "the next clip to ripple earlier", |a| v1(a, 1).start < second.start - 0.1);
    }

    #[test]
    fn snapping_toggles_from_the_toolbar() {
        let mut h = demo();
        let snap = |a: &ResolutionApp| a.timeline.as_ref().map(|t| t.snapping).expect("timeline");
        let before = snap(h.state());
        click(&mut h, "Snapping");
        pump(&mut h, "snapping to flip", |a| snap(a) != before);
    }

    #[test]
    fn showing_a_panel_does_not_stop_playback() {
        let mut h = demo();
        pump(&mut h, "the program to play", |a| {
            a.timeline_viewer.program.is_some() && a.timeline_viewer.shown.is_some()
        });
        h.key_press(Key::Space);
        pump(&mut h, "the clock to start", |a| {
            a.timeline_viewer.transport.is_playing() && !a.timeline_viewer.transport.in_preroll()
        });
        h.state_mut().command("workspace.index");
        h.state_mut().command("view.zoomIn");
        assert!(h.state().timeline_viewer.transport.is_playing());
    }

    #[test]
    fn the_source_viewer_steps_marks_and_inserts_on_its_own() {
        let mut h = demo();
        let item = h.state().media.iter().find(|m| m.name == "Ocean_Sunset.mp4").cloned().expect("demo clip");
        h.state_mut().open_media(&item);
        pump(&mut h, "the clip in the source viewer", |a| {
            a.source_clip.is_some() && a.source_viewer.program.is_some() && a.source_viewer.shown.is_some()
        });
        assert_eq!(h.state().active, Viewer::Source, "opening a clip makes the source viewer active");
        let timeline_frame = h.state().playhead_frame();
        // Right steps the source viewer, not the timeline
        h.key_press(Key::ArrowRight);
        h.key_press(Key::ArrowRight);
        pump(&mut h, "the source playhead two frames on", |a| {
            a.source_clip.as_ref().is_some_and(|s| s.playhead_frame == 2)
        });
        assert_eq!(h.state().playhead_frame(), timeline_frame);
        // I and O mark the source clip
        h.key_press(Key::I);
        pump(&mut h, "the source In", |a| a.source_clip.as_ref().is_some_and(|s| s.mark_in == Some(2)));
        for _ in 0..24 {
            h.key_press(Key::ArrowRight);
        }
        pump(&mut h, "the source playhead a second on", |a| {
            a.source_clip.as_ref().is_some_and(|s| s.playhead_frame == 26)
        });
        h.key_press(Key::O);
        pump(&mut h, "the source Out", |a| a.source_clip.as_ref().is_some_and(|s| s.mark_out == Some(26)));
        // F9 inserts the marked range into the timeline
        let clips = v1_clips(h.state());
        h.key_press(Key::F9);
        pump(&mut h, "an insert on V1", |a| v1_clips(a) > clips);
        // Q hands the keys back to the timeline viewer
        h.key_press(Key::Q);
        h.step();
        assert_eq!(h.state().active, Viewer::Timeline);
    }

    #[test]
    fn the_source_viewer_plays_on_its_own() {
        let mut h = demo();
        let item = h.state().media.iter().find(|m| m.name == "Ocean_Sunset.mp4").cloned().expect("demo clip");
        h.state_mut().open_media(&item);
        pump(&mut h, "the source clip ready", |a| a.source_viewer.shown.is_some());
        h.key_press(Key::Space);
        pump(&mut h, "the source viewer to play", |a| {
            a.source_viewer.transport.frame().is_some_and(|f| f >= 6) && !a.source_viewer.transport.in_preroll()
        });
        assert!(!h.state().timeline_viewer.is_playing(), "the timeline viewer stays still");
        h.key_press(Key::K);
        h.step();
        assert!(!h.state().source_viewer.is_playing());
    }

    #[test]
    fn waveforms_arrive_for_the_timelines_sound() {
        let mut h = demo();
        let items: Vec<u64> = h
            .state()
            .timeline
            .as_ref()
            .map(|t| t.audio.iter().flat_map(|a| a.clips.iter().map(|c| c.item)).collect())
            .unwrap_or_default();
        assert!(!items.is_empty());
        pump(&mut h, "every audio clip's waveform", |a| {
            let ready = a.waveforms.ready();
            items.iter().all(|i| ready.contains_key(i))
        });
        let ready = h.state().waveforms.ready();
        assert!(ready.values().any(|p| !p.buckets.is_empty() && p.loudest > 0.0), "some clip has sound");
    }

    /// Roll the wheel over the timeline, with `modifiers`.
    fn wheel(h: &mut Harness<'_, ResolutionApp>, at: Pos2, delta: egui::Vec2, modifiers: Modifiers) {
        h.event(egui::Event::PointerMoved(at));
        h.step();
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta,
            phase: egui::TouchPhase::Move,
            modifiers,
        });
        // the wheel scrolls smoothly over a few frames
        for _ in 0..30 {
            h.step();
        }
    }

    #[test]
    fn the_wheel_scrolls_the_tracks_and_alt_zooms() {
        let mut h = demo();
        pump(&mut h, "clips on screen", |a| !a.clip_rects.is_empty());
        // over a clip, so the pointer is on the timeline
        let at = h.state().clip_rects[0].0.center();
        assert_eq!(h.state().nav.down, 0.0);
        wheel(&mut h, at, egui::vec2(0.0, -60.0), Modifiers::NONE);
        assert!(h.state().nav.down > 0.0, "the demo's six tracks overflow, so the wheel scrolls down");
        assert!(h.state().nav.zoom.is_none(), "a plain wheel does not zoom");
        let at = h.state().clip_rects[0].0.center();
        h.event(egui::Event::ModifiersChanged(Modifiers::ALT));
        wheel(&mut h, at, egui::vec2(0.0, 120.0), Modifiers::ALT);
        assert!(h.state().nav.zoom.is_some(), "Alt with the wheel zooms in");
    }

    #[test]
    fn zooming_keeps_the_playhead_in_view_and_fit_shows_everything() {
        let mut h = demo();
        pump(&mut h, "clips on screen", |a| !a.clip_rects.is_empty());
        for _ in 0..6 {
            h.state_mut().command("view.zoomIn");
            h.step();
        }
        let zoomed = h.state().nav;
        assert!(zoomed.zoom.is_some());
        // jump to the end: the view pages to follow the playhead
        h.key_press(Key::End);
        pump(&mut h, "the playhead at the end", |a| {
            a.timeline.as_ref().is_some_and(|t| t.playhead_frame >= t.rate.frame_at(t.duration) - 1)
        });
        h.step();
        h.step();
        assert!(h.state().nav.first > zoomed.first, "the view followed the playhead to the end");
        h.state_mut().command("view.zoomToFit");
        h.step();
        assert_eq!((h.state().nav.zoom, h.state().nav.first), (None, 0.0));
    }

    #[test]
    fn ctrl_n_makes_an_empty_timeline_in_the_same_format() {
        let mut h = demo();
        let before = h.state().timeline.clone().expect("demo timeline");
        let count = |a: &ResolutionApp| a.media.iter().filter(|m| m.kind == Kind::Timeline).count();
        let timelines = count(h.state());
        h.key_press_modifiers(Modifiers::COMMAND, Key::N);
        pump(&mut h, "a new timeline open", |a| count(a) == timelines + 1);
        pump(&mut h, "the new timeline in the timeline panel", |a| {
            a.timeline.as_ref().is_some_and(|t| t.name != before.name)
        });
        let t = h.state().timeline.clone().expect("new timeline");
        assert_eq!((t.width, t.height, t.rate), (before.width, before.height, before.rate));
        assert!(t.video.iter().chain(&t.audio).all(|tr| tr.clips.is_empty()), "it starts empty");
    }

    #[test]
    fn the_deliver_page_renders_the_timeline_to_a_file() {
        let mut h = demo();
        let dir = std::env::temp_dir().join(format!("resolution-render-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("render folder");
        // two seconds: In at the first frame, Out at the 48th
        for (id, params) in [
            ("playhead.set", json!({ "frame": 0 })),
            ("mark.in", json!({})),
            ("playhead.set", json!({ "frame": 47 })),
            ("mark.out", json!({})),
        ] {
            h.state().worker.run(id, params);
        }
        pump(&mut h, "In and Out on the timeline", |a| a.timeline.as_ref().is_some_and(|t| t.mark_out.is_some()));
        h.key_press_modifiers(Modifiers::SHIFT, Key::Num8);
        pump(&mut h, "the Deliver page", |a| a.page == Page::Deliver);
        {
            let d = &mut h.state_mut().deliver;
            d.folder = dir.clone();
            d.name = "two seconds".into();
        }
        click(&mut h, "In/Out Range");
        assert!(h.state().deliver.in_out);
        click(&mut h, "Add to Render Queue");
        assert_eq!(h.state().deliver.queue.len(), 1);
        click(&mut h, "Render All");
        let deadline = Instant::now() + Duration::from_secs(120);
        while !matches!(h.state().deliver.queue[0].status, deliver::Status::Done | deliver::Status::Failed(_)) {
            assert!(Instant::now() < deadline, "the render did not finish: {:?}", h.state().deliver.queue[0].status);
            h.step();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(h.state().deliver.queue[0].status, deliver::Status::Done);
        let file = dir.join("two seconds.mp4");
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        assert!(size > 1000, "{} is {size} bytes", file.display());
        assert!(!h.state().deliver.running);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_agent_edits_the_open_timeline_through_the_link() {
        let path = std::env::temp_dir().join(format!("resolution-app-link-{}.json", std::process::id()));
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 920.0)).build_eframe(|cc| {
            ResolutionApp::new(
                cc,
                Options { demo: true, mute: true, agent_link: Some(path.clone()), ..Default::default() },
            )
        });
        pump(&mut h, "the demo timeline", |a| a.timeline.is_some());
        let clips = v1_clips(h.state());
        use resolution_engine_api::Engine as _;
        let mut agent = resolution_agent_link::RemoteEngine::connect(&path).expect("the editor's link");
        agent.execute("playhead.set", json!({ "seconds": 2.0 })).expect("playhead");
        let out = agent.execute("timeline.split", json!({})).expect("split");
        assert!(out.undo_label.is_some());
        // the editor shows the agent's edit, and says it was the agent's
        pump(&mut h, "the agent's split on screen", |a| v1_clips(a) == clips + 1);
        pump(&mut h, "the agent's edit in the status bar", |a| a.status.0.starts_with("Agent:"));
        // one undo history: the user's Ctrl+Z undoes the agent's edit
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        pump(&mut h, "Ctrl+Z to undo the agent's split", |a| v1_clips(a) == clips);
        drop(h);
        assert!(!path.exists(), "closing the editor removes its link file");
    }

    /// The editor with an agent connected, and the original demo timeline's name and V1 clips.
    fn with_agent(name: &str) -> (Harness<'static, ResolutionApp>, resolution_agent_link::RemoteEngine, PathBuf) {
        let path = std::env::temp_dir().join(format!("resolution-app-{name}-{}.json", std::process::id()));
        let link = path.clone();
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 920.0)).build_eframe(move |cc| {
            ResolutionApp::new(cc, Options { demo: true, mute: true, agent_link: Some(link), ..Default::default() })
        });
        pump(&mut h, "the demo timeline", |a| a.timeline.is_some());
        let agent = resolution_agent_link::RemoteEngine::connect(&path).expect("the editor's link");
        (h, agent, path)
    }

    fn timeline_names(a: &ResolutionApp) -> Vec<String> {
        a.media.iter().filter(|m| m.kind == Kind::Timeline).map(|m| m.name.clone()).collect()
    }

    #[test]
    fn an_agents_edits_go_to_a_draft_the_user_accepts() {
        use resolution_engine_api::Engine as _;
        let (mut h, mut agent, _path) = with_agent("accept");
        let original = h.state().timeline.as_ref().map(|t| t.name.clone()).expect("timeline");
        let clips = v1_clips(h.state());
        // looking and moving the playhead make no draft
        agent.execute("timeline.inspect", json!({})).expect("inspect");
        agent.execute("playhead.set", json!({ "seconds": 2.0 })).expect("playhead");
        h.step();
        assert!(h.state().draft.is_none());
        // the first edit does
        agent.execute("timeline.split", json!({})).expect("split");
        pump(&mut h, "the agent's draft open", |a| {
            a.draft.is_some() && a.timeline.as_ref().is_some_and(|t| t.name.ends_with("(agent draft)"))
        });
        assert_eq!(v1_clips(h.state()), clips + 1, "the draft holds the split, made at the agent's playhead");
        assert_eq!(h.state().draft.as_ref().map(|d| d.edits().len()), Some(1));
        assert!(timeline_names(h.state()).contains(&original), "the original is still there, untouched");
        click(&mut h, "Accept Draft");
        pump(&mut h, "the draft accepted", |a| {
            a.draft.is_none() && a.timeline.as_ref().is_some_and(|t| t.name == original)
        });
        assert_eq!(v1_clips(h.state()), clips + 1);
        let names = timeline_names(h.state());
        assert_eq!(names.iter().filter(|n| **n == original).count(), 1, "one timeline of that name: {names:?}");
        assert!(!names.iter().any(|n| n.ends_with("(agent draft)")));
    }

    #[test]
    fn rejecting_a_draft_leaves_the_original_as_it_was() {
        use resolution_engine_api::Engine as _;
        let (mut h, mut agent, _path) = with_agent("reject");
        let original = h.state().timeline.as_ref().map(|t| t.name.clone()).expect("timeline");
        let clips = v1_clips(h.state());
        agent.execute("playhead.set", json!({ "seconds": 2.0 })).expect("playhead");
        agent.execute("timeline.split", json!({})).expect("split");
        agent.execute("playhead.set", json!({ "seconds": 4.0 })).expect("playhead");
        agent.execute("timeline.split", json!({})).expect("split");
        pump(&mut h, "two edits in the draft", |a| {
            a.draft.as_ref().is_some_and(|d| d.edits().len() == 2) && v1_clips(a) == clips + 2
        });
        click(&mut h, "Reject Draft");
        pump(&mut h, "the original back", |a| {
            a.draft.is_none() && a.timeline.as_ref().is_some_and(|t| t.name == original)
        });
        assert_eq!(v1_clips(h.state()), clips, "the original never changed");
        assert!(!timeline_names(h.state()).iter().any(|n| n.ends_with("(agent draft)")), "the draft is deleted");
    }

    #[test]
    fn the_draft_lists_only_the_agents_edits_still_in_it() {
        use resolution_engine_api::Engine as _;
        let (mut h, mut agent, _path) = with_agent("undo");
        let clips = v1_clips(h.state());
        let edits = |a: &ResolutionApp| a.draft.as_ref().map(|d| d.edits().len());
        agent.execute("playhead.set", json!({ "seconds": 2.0 })).expect("playhead");
        agent.execute("timeline.split", json!({})).expect("split");
        agent.execute("playhead.set", json!({ "seconds": 4.0 })).expect("playhead");
        agent.execute("timeline.split", json!({})).expect("split");
        pump(&mut h, "two edits in the draft", |a| edits(a) == Some(2) && v1_clips(a) == clips + 2);
        // the agent's undo (as an MCP dry run or rollback does) takes its edit off the list
        assert_eq!(agent.undo().as_deref(), Some("Add Edit"));
        pump(&mut h, "one edit after the agent's undo", |a| edits(a) == Some(1) && v1_clips(a) == clips + 1);
        // and so does the user's Ctrl+Z: one history
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        pump(&mut h, "no edits after Ctrl+Z", |a| edits(a) == Some(0) && v1_clips(a) == clips);
        assert!(h.state().timeline.as_ref().is_some_and(|t| t.name.ends_with("(agent draft)")), "the draft stays");
        // redo brings an edit back to the list
        assert_eq!(agent.redo().as_deref(), Some("Add Edit"));
        pump(&mut h, "one edit after redo", |a| edits(a) == Some(1) && v1_clips(a) == clips + 1);
    }

    /// The demo on the Color page, with the grade of the clip under the playhead.
    fn on_color_page() -> Harness<'static, ResolutionApp> {
        let mut h = demo();
        h.key_press_modifiers(Modifiers::SHIFT, Key::Num6);
        pump(&mut h, "Shift+6 and the clip's grade", |a| a.page == Page::Color && a.color.view.is_some());
        h
    }

    /// The clip the Color page is grading, as the timeline shows it.
    fn graded_clip(a: &ResolutionApp) -> Option<model::Clip> {
        let id = a.color.clip?;
        a.timeline.as_ref()?.clips().map(|(_, c)| c).find(|c| c.id == id).cloned()
    }

    /// A click at a point: press and release within one step (the harness moves its clock a
    /// quarter second a step, and egui takes a longer press for the start of a drag).
    fn click_at(h: &mut Harness<'_, ResolutionApp>, at: Pos2) {
        h.event(egui::Event::PointerMoved(at));
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        h.step();
        h.step();
    }

    #[test]
    fn a_drag_on_a_master_ring_grades_the_clip_as_one_undo_step() {
        let mut h = on_color_page();
        assert_eq!(h.state().color.view.as_ref().map(|v| v.grade.nodes.len()), Some(1), "a clip starts with one node");
        let ring = h.get_by_label("Gain master").rect();
        drag(&mut h, ring.center(), ring.center() - vec2(60.0, 0.0));
        pump(&mut h, "the clip graded, darker", |a| {
            !a.color.dragging
                && graded_clip(a).is_some_and(|c| c.graded)
                && a.color.current().is_some_and(|n| n.gain.y < 0.95)
        });
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        pump(&mut h, "Ctrl+Z takes the whole drag back", |a| {
            graded_clip(a).is_some_and(|c| !c.graded) && a.color.current().is_some_and(|n| n.gain.y == 1.0)
        });
    }

    #[test]
    fn the_lift_wheel_moves_the_balance_without_changing_brightness() {
        let mut h = on_color_page();
        let wheel = h.get_by_label("Lift wheel").rect();
        drag(&mut h, wheel.center(), wheel.center() + vec2(-24.0, -36.0));
        pump(&mut h, "the balance moved", |a| !a.color.dragging && graded_clip(a).is_some_and(|c| c.graded));
        let lift = h.state().color.current().map(|n| n.lift).expect("a node");
        assert!(lift.r.abs() > 0.005 || lift.b.abs() > 0.005, "{lift:?}");
        let luma = 0.2126 * lift.r + 0.7152 * lift.g + 0.0722 * lift.b;
        assert!(luma.abs() < 1e-3, "the balance keeps luma: {lift:?}");
        assert_eq!(lift.y, 0.0, "the master is untouched");
    }

    #[test]
    fn node_keys_add_select_and_disable_nodes() {
        let mut h = on_color_page();
        h.key_press_modifiers(Modifiers::ALT, Key::S);
        pump(&mut h, "Alt+S adds a serial node and selects it", |a| {
            a.color.view.as_ref().is_some_and(|v| v.grade.nodes.len() == 2) && a.color.node == 1
        });
        h.key_press_modifiers(Modifiers::COMMAND, Key::D);
        pump(&mut h, "Ctrl+D turns node 02 off", |a| {
            a.color.view.as_ref().is_some_and(|v| !v.grade.nodes[1].enabled && v.grade.nodes[0].enabled)
        });
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::Semicolon);
        h.step();
        assert_eq!(h.state().color.node, 0, "Previous Node");
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::Semicolon);
        h.step();
        assert_eq!(h.state().color.node, 1, "and round to the last");
        click(&mut h, "Node 01");
        assert_eq!(h.state().color.node, 0);
    }

    #[test]
    fn node_keys_do_nothing_on_the_edit_page() {
        let mut h = demo();
        // the demo comes with one clip graded in Lumetri already
        let graded = |a: &ResolutionApp| -> Vec<u64> {
            a.timeline
                .as_ref()
                .map(|t| t.clips().filter(|(_, c)| c.graded).map(|(_, c)| c.id).collect())
                .unwrap_or_default()
        };
        let before = graded(h.state());
        let revision = h.state().revision;
        h.key_press_modifiers(Modifiers::ALT, Key::S);
        for _ in 0..5 {
            h.step();
        }
        assert!(h.state().color.view.is_none());
        assert_eq!(graded(h.state()), before);
        assert_eq!(h.state().revision, revision, "no edit was made");
    }

    #[test]
    fn a_custom_curve_takes_a_point_where_clicked_and_bends_where_dragged() {
        let mut h = on_color_page();
        let plot = h.get_by_label("Custom Curves").rect();
        for _ in 0..10 {
            h.step();
        }
        assert_eq!(h.get_by_label("Custom Curves").rect(), plot, "the palettes keep their size from frame to frame");
        click_at(&mut h, plot.center());
        pump(&mut h, "a point in the middle of the Y curve", |a| {
            !a.color.dragging && a.color.current().is_some_and(|n| n.curves.y.0.len() == 3)
        });
        drag(&mut h, plot.center(), plot.center() - vec2(0.0, 30.0));
        pump(&mut h, "the point raised", |a| {
            !a.color.dragging
                && graded_clip(a).is_some_and(|c| c.graded)
                && a.color.current().is_some_and(|n| n.curves.y.eval(0.5) > 0.55)
        });
        // a drag that starts away from every point makes one there and moves it
        let quarter = plot.left_bottom() + vec2(plot.width() * 0.25, -plot.height() * 0.25);
        drag(&mut h, quarter, quarter + vec2(0.0, 20.0));
        pump(&mut h, "a new point at a quarter, lowered", |a| {
            !a.color.dragging
                && a.color.current().is_some_and(|n| n.curves.y.0.len() == 4 && n.curves.y.eval(0.25) < 0.22)
        });
    }

    #[test]
    fn a_hue_curve_is_chosen_from_the_curves_menu_and_bent_by_dragging() {
        let mut h = on_color_page();
        click(&mut h, "Custom ▾");
        click(&mut h, "Hue vs Sat");
        pump(&mut h, "the Hue vs Sat curve", |a| a.color.curve_mode == crate::color_page::CurveMode::HueSat);
        let plot = h.get_by_label("Hue vs Sat curve").rect();
        // a drag down at the red end of the wheel takes the colour out of reds
        let red = pos2(plot.left() + plot.width() * 0.05, plot.center().y);
        drag(&mut h, red, red + vec2(0.0, plot.height() * 0.3));
        pump(&mut h, "reds less saturated, through the engine", |a| {
            !a.color.dragging
                && graded_clip(a).is_some_and(|c| c.graded)
                && a.color.current().is_some_and(|n| n.curves.hue_sat.eval(0.05, true) < 0.35)
        });
        // and the other curves are as they were
        let n = h.state().color.current().cloned().expect("a node");
        assert!(n.curves.is_identity() && n.curves.hue_hue.is_empty());
    }

    #[test]
    fn sat_vs_lum_is_on_the_curves_menu() {
        let mut h = on_color_page();
        click(&mut h, "Custom ▾");
        click(&mut h, "Sat vs Lum");
        pump(&mut h, "the Sat vs Lum curve", |a| a.color.curve_mode == crate::color_page::CurveMode::SatLum);
        let plot = h.get_by_label("Sat vs Lum curve").rect();
        // a drag down at the saturated end darkens the most saturated colours
        let full = pos2(plot.left() + plot.width() * 0.95, plot.center().y);
        drag(&mut h, full, full + vec2(0.0, plot.height() * 0.3));
        pump(&mut h, "saturated colours darker, through the engine", |a| {
            !a.color.dragging
                && graded_clip(a).is_some_and(|c| c.graded)
                && a.color.current().is_some_and(|n| n.curves.sat_lum.eval(0.95, false) < 0.35)
        });
    }

    #[test]
    fn shift_h_highlights_what_the_current_node_keys() {
        let mut h = on_color_page();
        h.state().worker.run("playhead.set", json!({ "seconds": 2.0 }));
        pump(&mut h, "the playhead at 2 s", |a| a.timeline.as_ref().is_some_and(|t| t.playhead_frame > 24));
        let clip = h.state().color.clip.expect("a clip");
        // a key, and no correction yet: the Highlight view shows what it keys all the same
        let q = resolution_grade::Qualifier::picked([0.9, 0.5, 0.2]);
        h.state().worker.run("color.adjust", json!({ "clip": clip, "node": 1, "controls": { "qualifier": q } }));
        pump(&mut h, "the key on the node", |a| a.color.current().is_some_and(|n| n.qualifier.is_some()));
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::H);
        pump(&mut h, "the matte in black and white", |a| {
            a.timeline_viewer.shown.is_some_and(|k| k.highlight.is_some_and(|l| l.black_white))
        });
        let px = h.state().timeline_viewer.picture.as_ref().expect("a picture").pixels().to_vec();
        assert!(px.chunks(4).all(|p| p[0].abs_diff(p[1]) <= 1 && p[1].abs_diff(p[2]) <= 1), "only greys");
        assert!(px.chunks(4).any(|p| p[0] > 200) && px.chunks(4).any(|p| p[0] < 30), "white keyed, black not");
        // the scopes and the picker still read the graded picture
        pump(&mut h, "the graded picture beside it", |a| {
            a.timeline_viewer.graded_picture().is_some_and(|f| f.pixels().chunks(4).any(|p| p[0].abs_diff(p[2]) > 30))
        });
        // Shift+H: flat grey instead; again, off
        h.key_press_modifiers(Modifiers::SHIFT, Key::H);
        pump(&mut h, "the flat-grey highlight", |a| {
            a.timeline_viewer.shown.is_some_and(|k| k.highlight.is_some_and(|l| !l.black_white))
        });
        h.key_press_modifiers(Modifiers::SHIFT, Key::H);
        pump(&mut h, "the picture again", |a| {
            a.color.highlight.is_none() && a.timeline_viewer.shown.is_some_and(|k| k.highlight.is_none())
        });
    }

    #[test]
    fn clicking_a_clip_in_the_strip_grades_that_clip() {
        let mut h = on_color_page();
        let clips = h.state().strip_clips();
        assert!(clips.len() > 1);
        let second = &clips[1];
        let tile = format!("Clip 02 {}", second.name);
        assert!(h.get_by_label(&tile).rect().height() > 100.0, "a tile has room for its picture and name");
        click(&mut h, &tile);
        let id = second.id;
        pump(&mut h, "the second clip under the playhead and its grade", |a| {
            a.color.clip == Some(id) && a.color.view.as_ref().is_some_and(|v| v.clip == id)
        });
    }

    #[test]
    fn an_agents_grade_arrives_as_a_draft_on_the_color_page() {
        use resolution_engine_api::Engine as _;
        let (mut h, mut agent, _path) = with_agent("color");
        h.key_press_modifiers(Modifiers::SHIFT, Key::Num6);
        pump(&mut h, "the Color page", |a| a.page == Page::Color && a.color.view.is_some());
        agent.execute("color.adjust", json!({ "node": 1, "controls": { "saturation": 70.0 } })).expect("grade");
        pump(&mut h, "the draft, graded, on the Color page", |a| {
            a.draft.is_some() && a.color.current().is_some_and(|n| n.saturation == 70.0)
        });
        // the journal has the agent's command, with its undo step (LOG D-054)
        let noted = h.state().worker.journal().entries();
        assert!(
            noted.iter().any(|e| e.who == crate::journal::Who::Agent
                && e.command == "color.adjust"
                && matches!(&e.ended, Some((_, crate::journal::Ended::Ok { undo: Some(_) })))),
            "{noted:?}"
        );
        // the user decides here, without going back to the Edit page
        click(&mut h, "Reject Draft");
        pump(&mut h, "the original back, ungraded", |a| {
            a.draft.is_none() && a.color.current().is_some_and(|n| n.saturation == 50.0)
        });
    }

    #[test]
    fn the_scopes_show_the_viewers_picture_and_switch_by_name() {
        let mut h = on_color_page();
        pump(&mut h, "the waveform of the viewer's picture", |a| {
            a.scope_view.drawn == Some(resolution_engine_api::ScopeKind::Waveform)
        });
        for (button, kind) in [
            ("Vectorscope", resolution_engine_api::ScopeKind::Vectorscope),
            ("Histogram", resolution_engine_api::ScopeKind::Histogram),
            ("Parade", resolution_engine_api::ScopeKind::Parade),
        ] {
            click(&mut h, button);
            pump(&mut h, button, |a| a.scope_view.drawn == Some(kind));
        }
        // a grade changes the picture, and the scope follows it
        let before = h.state().timeline_viewer.picture.as_ref().map(|p| std::sync::Arc::as_ptr(p) as usize);
        let ring = h.get_by_label("Gain master").rect();
        drag(&mut h, ring.center(), ring.center() - vec2(80.0, 0.0));
        pump(&mut h, "a new picture, and its parade", |a| {
            a.timeline_viewer.picture.as_ref().map(|p| std::sync::Arc::as_ptr(p) as usize) != before
                && a.scope_view.drawn == Some(resolution_engine_api::ScopeKind::Parade)
                && !a.color.dragging
        });
    }

    #[test]
    fn a_circle_window_is_added_moved_softened_and_deleted() {
        let mut h = on_color_page();
        click(&mut h, "Window palette");
        click(&mut h, "Add Circle");
        let windows = |a: &ResolutionApp| a.color.current().map(|n| n.windows.clone()).unwrap_or_default();
        pump(&mut h, "a circle window on node 01, from the engine", |a| {
            !a.color.dragging && a.color.asked.is_some() && windows(a).len() == 1
        });
        // the outline in the viewer: drag from the picture's middle (inside the circle) to the right
        pump(&mut h, "a picture in the viewer", |a| a.picture_rect.is_some());
        h.step();
        let osc = h.get_by_label("Window on-screen control").rect();
        drag(&mut h, osc.center(), osc.center() + vec2(60.0, 0.0));
        pump(&mut h, "the circle moved right", |a| {
            !a.color.dragging && windows(a).first().is_some_and(|w| w.center[0] > 0.52)
        });
        let softness = h.get_by_label("Softness").rect();
        drag(&mut h, softness.center(), softness.center() + vec2(40.0, 0.0));
        pump(&mut h, "a softer edge", |a| !a.color.dragging && windows(a).first().is_some_and(|w| w.softness > 0.025));
        click(&mut h, "Window 1 Invert");
        pump(&mut h, "inverted", |a| windows(a).first().is_some_and(|w| w.invert));
        click(&mut h, "Delete Window 1");
        pump(&mut h, "no windows", |a| windows(a).is_empty() && !a.color.dragging);
    }

    #[test]
    fn a_polygons_corners_are_dragged_added_and_removed_on_screen() {
        let mut h = on_color_page();
        click(&mut h, "Window palette");
        click(&mut h, "Add Polygon");
        let points = |a: &ResolutionApp| {
            a.color.current().and_then(|n| n.windows.first().map(|w| w.points.clone())).unwrap_or_default()
        };
        pump(&mut h, "a polygon of four corners, from the engine", |a| !a.color.dragging && points(a).len() == 4);
        pump(&mut h, "a picture in the viewer", |a| a.picture_rect.is_some());
        h.step();
        // a new polygon: 0.4 by 0.25 of the picture's width, in the middle; the demo clip fills it
        let osc = h.get_by_label("Window on-screen control").rect();
        let (c, rx, ry) = (osc.center(), 0.4 * osc.width() / 2.0, 0.25 * osc.width() / 2.0);
        let top_left = c - vec2(rx, ry);
        drag(&mut h, top_left, top_left - vec2(30.0, 20.0));
        pump(&mut h, "the top left corner dragged out", |a| {
            !a.color.dragging && points(a).first().is_some_and(|p| p[0] < -1.05 && p[1] < -1.05)
        });
        // a click on the surface, just under the bottom side's middle: a corner there
        click_at(&mut h, c + vec2(0.0, ry - 4.0));
        pump(&mut h, "a fifth corner on the bottom side", |a| {
            points(a).len() == 5 && points(a).iter().any(|p| p[0].abs() < 0.05 && (p[1] - 1.0).abs() < 0.05)
        });
        // a middle click on it removes it
        let at = c + vec2(0.0, ry);
        h.event(egui::Event::PointerMoved(at));
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: PointerButton::Middle,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        h.step();
        pump(&mut h, "four corners again", |a| points(a).len() == 4);
    }

    #[test]
    fn the_journal_notes_each_command_who_sent_it_and_how_it_ended() {
        use crate::journal::{Ended, Who};
        let mut h = demo();
        h.state().worker.run("playhead.set", json!({ "frame": 12 }));
        h.state().worker.run("timeline.noSuchCommand", json!({}));
        let ended = |a: &ResolutionApp, id: &str| {
            a.worker.journal().entries().into_iter().find(|e| e.command == id).and_then(|e| e.ended)
        };
        pump(&mut h, "both commands ended", |a| {
            ended(a, "playhead.set").is_some() && ended(a, "timeline.noSuchCommand").is_some()
        });
        assert!(matches!(ended(h.state(), "playhead.set"), Some((ms, Ended::Ok { .. })) if ms >= 0.0));
        assert!(matches!(ended(h.state(), "timeline.noSuchCommand"), Some((_, Ended::Err(_)))));
        let entries = h.state().worker.journal().entries();
        assert!(entries.iter().all(|e| e.who == Who::User));
        assert_eq!(entries[0].command, "raw.filmcraft/file.openDemoProject", "the demo opened first, as a command");
    }

    #[test]
    fn shadows_highlights_and_midtone_detail_are_dragged_in_the_primaries() {
        let mut h = on_color_page();
        let node = |a: &ResolutionApp| a.color.current().cloned().unwrap_or_default();
        for (label, dx) in [("Shadows", 60.0), ("Highlights", -60.0), ("Mid/Detail", 60.0)] {
            let field = h.get_by_label(label).rect();
            drag(&mut h, field.center(), field.center() + vec2(dx, 0.0));
        }
        pump(&mut h, "all three set, through the engine", |a| {
            let n = node(a);
            !a.color.dragging && n.shadows > 5.0 && n.highlights < -5.0 && n.midtone_detail > 5.0
        });
        assert!(graded_clip(h.state()).is_some_and(|c| c.graded));
    }

    #[test]
    fn a_windows_handles_change_its_size_softness_and_rotation() {
        let mut h = on_color_page();
        click(&mut h, "Window palette");
        click(&mut h, "Add Circle");
        let first = |a: &ResolutionApp| a.color.current().and_then(|n| n.windows.first().cloned());
        pump(&mut h, "a circle window, from the engine", |a| !a.color.dragging && first(a).is_some());
        pump(&mut h, "a picture in the viewer", |a| a.picture_rect.is_some());
        h.step();
        // a new circle: 0.33 of the picture's width across, in the middle; the demo clip fills it
        let osc = h.get_by_label("Window on-screen control").rect();
        let (c, r) = (osc.center(), 0.33 * osc.width() / 2.0);
        drag(&mut h, c + vec2(r, 0.0), c + vec2(r + 40.0, 0.0));
        pump(&mut h, "wider", |a| !a.color.dragging && first(a).is_some_and(|w| w.size[0] > 0.36 && w.size[1] < 0.34));
        let soft = c + vec2(0.0, r + 14.0 + 0.02 * osc.width() / 2.0);
        drag(&mut h, soft, soft + vec2(0.0, 30.0));
        pump(&mut h, "softer", |a| !a.color.dragging && first(a).is_some_and(|w| w.softness > 0.05));
        let turn = c - vec2(0.0, r + 28.0);
        drag(&mut h, turn, c + vec2(120.0, 0.0));
        pump(&mut h, "a quarter turn", |a| {
            !a.color.dragging && first(a).is_some_and(|w| (w.rotation - 90.0).abs() < 3.0)
        });
    }

    #[test]
    fn a_gradient_window_is_added_without_a_size() {
        let mut h = on_color_page();
        click(&mut h, "Window palette");
        click(&mut h, "Add Gradient");
        let windows = |a: &ResolutionApp| a.color.current().map(|n| n.windows.clone()).unwrap_or_default();
        pump(&mut h, "a gradient window on node 01, from the engine", |a| {
            !a.color.dragging && windows(a).first().is_some_and(|w| w.shape == resolution_grade::Shape::Gradient)
        });
        h.step();
        assert!(h.query_by_label("Size").is_none() && h.query_by_label("Aspect").is_none(), "a gradient has no size");
        assert!(h.query_by_label("Rotate").is_some() && h.query_by_label("Softness").is_some());
    }

    #[test]
    fn the_qualifier_picks_a_colour_from_the_viewer_and_takes_its_ranges() {
        let mut h = on_color_page();
        click(&mut h, "Qualifier palette");
        pump(&mut h, "a picture in the viewer", |a| a.picture_rect.is_some() && a.timeline_viewer.picture.is_some());
        h.step();
        let picker = h.get_by_label("Qualifier picker").rect();
        click_at(&mut h, picker.center());
        let q = |a: &ResolutionApp| a.color.current().and_then(|n| n.qualifier);
        pump(&mut h, "a qualifier from the picked colour, from the engine", |a| {
            !a.color.dragging && a.color.asked.is_some() && q(a).is_some_and(|q| q.enabled)
        });
        let width = q(h.state()).map(|q| q.hue_width).unwrap_or_default();
        let field = h.get_by_label("Hue Width").rect();
        drag(&mut h, field.center(), field.center() + vec2(50.0, 0.0));
        pump(&mut h, "a wider hue range", |a| !a.color.dragging && q(a).is_some_and(|q| q.hue_width > width + 1.0));
        click(&mut h, "Qualifier Invert");
        pump(&mut h, "inverted", |a| q(a).is_some_and(|q| q.invert));
        click(&mut h, "Remove Qualifier");
        pump(&mut h, "no qualifier", |a| q(a).is_none() && !a.color.dragging);
    }

    #[test]
    fn matte_finesse_cleans_the_qualifiers_key() {
        let mut h = on_color_page();
        click(&mut h, "Qualifier palette");
        click(&mut h, "Add Qualifier");
        let q = |a: &ResolutionApp| a.color.current().and_then(|n| n.qualifier);
        pump(&mut h, "a qualifier", |a| !a.color.dragging && q(a).is_some());
        for (field, get) in [
            ("Blur Radius", (|q: resolution_grade::Qualifier| q.blur) as fn(resolution_grade::Qualifier) -> f32),
            ("Clean Black", |q| q.clean_black),
        ] {
            let cell = h.get_by_label(field).rect();
            drag(&mut h, cell.center(), cell.center() + vec2(60.0, 0.0));
            pump(&mut h, field, |a| !a.color.dragging && q(a).is_some_and(|q| get(q) > 3.0));
        }
    }

    #[test]
    fn edit_mode_keys_change_the_tool() {
        let mut h = demo();
        h.key_press(Key::T);
        h.step();
        assert_eq!(h.state().mode, EditMode::Trim);
        h.key_press(Key::B);
        h.step();
        assert_eq!(h.state().mode, EditMode::Blade);
        h.key_press(Key::A);
        h.step();
        assert_eq!(h.state().mode, EditMode::Selection);
    }

    #[test]
    fn the_theme_menu_recolours_the_ui() {
        let mut h = demo();
        let ctx = h.ctx.clone();
        h.state_mut().set_theme(&ctx, &theme::EMBER);
        h.step();
        assert_eq!(h.state().look.theme.id, "ember");
        assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.widgets.active.fg_stroke.color, theme::EMBER.accent);
    }
}
