//! The editor window: Resolve's page bar and Edit page layout over the engine's command API.
//! Every edit is a vocabulary command run by the engine thread, the same commands agents use over
//! MCP (ARCHITECTURE invariant 3).
//!
//! The Edit page has a fixed layout, as in Resolve (LOG D-016): the interface toolbar shows and
//! hides panels in fixed places (a browser on the left, the Inspector or Metadata on the right,
//! the Mixer beside the timeline) rather than letting them dock anywhere. The look is the Black
//! Box skin in a choice of colour themes (LOG D-018, `theme.rs`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use egui::{
    Align, Align2, Color32, ColorImage, CornerRadius, Frame, Layout, Margin, Rect, RichText, Sense, Stroke,
    TextureHandle, TextureOptions, Ui, UiBuilder, ViewportCommand, pos2, vec2,
};
use resolution_engine_api::{Page, Playback};
use resolution_filmcraft_adapter::FilmCraftEngine;
use resolution_keymap::Keymap;
use resolution_playback::{AudioLost, AudioOut, AudioStats, FrameKey, FrameServer, Step, Transport, ahead};
use serde_json::json;

use crate::chrome::{self, State};
use crate::icons::Icon;
use crate::model::{self, Kind, MediaItem, Timeline};
use crate::theme::{self, Look, Theme};
use crate::timeline_ui::{self, Action};
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

/// Frames shown while playing: the viewer's frame-rate readout and the `--play` report.
#[derive(Clone, Copy, Debug, Default)]
struct Meter {
    /// When the clock started (s, app clock).
    started: f64,
    shown: u64,
    window: (f64, u32),
    fps: Option<f64>,
}

impl Meter {
    fn new(now: f64) -> Self {
        Self { started: now, window: (now, 0), ..Self::default() }
    }

    fn frame_shown(&mut self, now: f64) {
        self.shown += 1;
        self.window.1 += 1;
        let dt = now - self.window.0;
        if dt >= 1.0 {
            self.fps = Some(f64::from(self.window.1) / dt);
            self.window = (now, 0);
        }
    }
}

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
    /// The open timeline as the viewer renders and plays it, from the latest snapshot.
    program: Option<Arc<dyn Playback>>,
    frames: FrameServer,
    frame: Option<TextureHandle>,
    /// The frame in `frame`.
    shown: Option<FrameKey>,
    /// Longest side, in pixels, of the frames the viewer asks for: its size on screen.
    view_side: u32,
    transport: Transport,
    /// Opened at the first play.
    audio: Option<AudioOut>,
    audio_stats: Arc<AudioStats>,
    mute: bool,
    clock: Instant,
    /// A seek sent to the engine and not in a snapshot yet, shown meanwhile so the playhead does
    /// not jump back while the engine catches up; and how many seeks are in flight.
    pending_seek: Option<i64>,
    seeks_in_flight: u32,
    meter: Meter,
    play_test: Option<PlayTest>,
    zoom: Option<f32>,
    /// Where each clip was drawn last frame: clip id by screen rectangle.
    clip_rects: Vec<(egui::Rect, u64)>,
    selected_media: Option<(Kind, u64)>,
    source: Option<(u64, String)>,
    status: (String, bool),
    /// After `--import`: build a timeline from the first clip once the Media Pool has it.
    auto_timeline: bool,
    seek_after_load: Option<f64>,
    screenshot: Option<(PathBuf, u32)>,
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
        let worker = Worker::spawn(Box::new(FilmCraftEngine::new()), move || ctx.request_repaint());
        let frames = FrameServer::new(FrameServer::default_workers());
        let ctx = cc.egui_ctx.clone();
        frames.on_ready(move || ctx.request_repaint());
        if options.demo {
            worker.run("raw.filmcraft/file.openDemoProject", json!({}));
        }
        if let Some(path) = &options.project {
            worker.run("project.open", json!({ "path": path }));
        }
        if !options.import.is_empty() {
            worker.run("media.import", json!({ "paths": options.import }));
        }
        Self {
            worker,
            keymap: Keymap::resolve(),
            look,
            page: Page::Edit,
            workspace: Workspace::default(),
            mode: EditMode::Selection,
            project_name: None,
            timeline: None,
            media: Vec::new(),
            revision: 0,
            program: None,
            frames,
            frame: None,
            shown: None,
            view_side: 960,
            transport: Transport::default(),
            audio: None,
            audio_stats: Arc::default(),
            mute: options.mute,
            clock: Instant::now(),
            pending_seek: None,
            seeks_in_flight: 0,
            meter: Meter::default(),
            play_test: options.play_for.map(|seconds| PlayTest { seconds, started: None, done: false }),
            zoom: None,
            clip_rects: Vec::new(),
            selected_media: None,
            source: None,
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
            screenshot: options.screenshot.map(|p| (p, 0)),
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

    /// The playhead's frame: playback's while playing, a seek on its way to the engine, or the
    /// engine's.
    fn playhead_frame(&self) -> i64 {
        self.transport
            .frame()
            .or(self.pending_seek)
            .unwrap_or_else(|| self.timeline.as_ref().map_or(0, |t| t.playhead_frame))
    }

    /// The playhead in timeline seconds, for drawing.
    fn playhead(&self) -> f64 {
        self.timeline.as_ref().map_or(0.0, |t| t.rate.seconds_of(self.playhead_frame()))
    }

    fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::Ran { id, result } => {
                if id == "playhead.set" {
                    self.seeks_in_flight = self.seeks_in_flight.saturating_sub(1);
                }
                match result {
                    Ok(out) => {
                        if let Some(label) = out.undo_label {
                            self.say(label, false);
                        } else if !id.ends_with(".inspect") && !id.starts_with("playhead.") && id != "timeline.select" {
                            self.say(format!("{id}: done"), false);
                        }
                    }
                    Err(e) => self.say(e, true),
                }
            }
            Reply::Snapshot { revision, timeline, project, playback } => {
                self.revision = revision;
                self.timeline = timeline.as_ref().and_then(Timeline::from_json);
                self.program = playback;
                if self.seeks_in_flight == 0 {
                    self.pending_seek = None;
                }
                if self.program.is_none() {
                    self.transport.stop();
                    self.stop_audio();
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
            }
            Reply::Notice { message, error } => self.say(message, error),
        }
    }

    /// A vocabulary command from a key, a button or a menu. Front-end commands (pages, panels,
    /// playback, zoom, edit modes) are handled here; everything else goes to the engine.
    fn command(&mut self, id: &str) {
        // panels, zoom and modes leave playback alone; edits stop it, except marking
        let front_end = id.starts_with("workspace.") || id.starts_with("view.") || id.starts_with("mode.");
        if !id.starts_with("playback.") && !front_end && self.transport.is_playing() {
            if WHILE_PLAYING.contains(&id) {
                if let Some(frame) = self.transport.frame() {
                    self.worker.run("playhead.set", json!({ "frame": frame }));
                }
            } else {
                self.stop();
            }
        }
        let speed = self.transport.speed();
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
                self.say("Trim Edit Mode. Trimming by dragging is not built yet.", false);
            }
            "mode.blade" => {
                self.mode = EditMode::Blade;
                self.say(
                    "Blade Edit Mode. Cutting by clicking is not built yet; Ctrl+B splits at the playhead.",
                    false,
                );
            }
            "playback.playStop" => {
                if self.transport.is_playing() {
                    self.stop();
                } else {
                    self.play(1.0);
                }
            }
            // each press doubles the speed in its direction, up to 8x
            "playback.forward" => self.play(speed.filter(|s| *s > 0.0).map_or(1.0, |s| (s * 2.0).min(8.0))),
            "playback.reverse" => self.play(speed.filter(|s| *s < 0.0).map_or(-1.0, |s| (s * 2.0).max(-8.0))),
            "playback.stop" => self.stop(),
            "view.zoomIn" | "view.zoomOut" => {
                let base = self.zoom.unwrap_or(40.0);
                self.zoom = Some(if id == "view.zoomIn" { base * 1.5 } else { base / 1.5 }.clamp(2.0, 2000.0));
            }
            "view.zoomToFit" => self.zoom = None,
            "media.import" => self.import_dialog(),
            "edit.insert" | "edit.overwrite" if self.source.is_none() => {
                self.say("Open a clip in the source viewer first: double-click it in the Media Pool.", true);
            }
            _ if id.starts_with("mode.") || id.starts_with("playback.") => {
                self.say(format!("{id} is not built yet"), false);
            }
            "project.save" | "project.saveAs" => self.save_dialog(),
            _ => self.worker.run(id, json!({})),
        }
    }

    /// An edit made with the mouse: stops playback first, as an edit from a key does.
    fn edit(&mut self, id: &str, params: serde_json::Value) {
        self.stop();
        self.worker.run(id, params);
    }

    /// Play from the playhead at `speed` (from the start when it is on the last frame). Starts
    /// with the preroll; [`ResolutionApp::advance_playback`] starts the clock and the sound.
    fn play(&mut self, speed: f64) {
        let Some(p) = self.program.clone() else { return };
        let mut from = self.playhead_frame();
        if speed > 0.0 && from >= p.frames() - 1 {
            from = 0;
        }
        self.stop_audio();
        self.transport.play(from, speed, p.rate());
    }

    /// Stop playback and leave the engine's playhead where it stopped.
    fn stop(&mut self) {
        if let Some(frame) = self.transport.stop() {
            self.stop_audio();
            self.frames.stop_prefetch();
            self.send_seek(frame);
        }
    }

    fn stop_audio(&mut self) {
        if let Some(a) = &mut self.audio {
            a.stop();
        }
    }

    /// Move the playhead to `frame` (ruler clicks and drags), stopping playback.
    fn seek(&mut self, frame: i64) {
        self.stop();
        let last = self.program.as_ref().map_or(0, |p| p.frames() - 1).max(0);
        self.send_seek(frame.clamp(0, last));
    }

    fn send_seek(&mut self, frame: i64) {
        if self.playhead_frame() == frame {
            return;
        }
        self.pending_seek = Some(frame);
        self.seeks_in_flight += 1;
        self.worker.run("playhead.set", json!({ "frame": frame }));
    }

    fn frame_key(&self, p: &Arc<dyn Playback>, frame: i64) -> FrameKey {
        FrameKey { revision: p.revision(), frame, side: self.view_side, draft: false }
    }

    /// Playing: wait for the first frames, then start the sound and the clock; afterwards move
    /// the playhead with the clock and keep the frames ahead of it rendering.
    fn advance_playback(&mut self, ctx: &egui::Context) {
        if !self.transport.is_playing() {
            return;
        }
        let Some(p) = self.program.clone() else { return };
        ctx.request_repaint();
        let speed = self.transport.speed().unwrap_or(1.0);
        let last = p.frames() - 1;
        let now = self.clock.elapsed().as_secs_f64();
        if self.transport.in_preroll() {
            let key = self.frame_key(&p, self.transport.frame().unwrap_or(0));
            self.frames.schedule_playback(&p, key, speed, true);
            if self.transport.preroll_done(now, self.frames.preroll_ready(key, speed, last)) {
                let audio = self.start_audio(&p, key.frame, speed);
                let now = self.clock.elapsed().as_secs_f64();
                self.transport.start_clock(now, audio);
                self.meter = Meter::new(now);
            }
            return;
        }
        let played = self
            .audio
            .as_ref()
            .filter(|_| self.transport.audio_clock())
            .and_then(|a| a.played_frames().map(|f| (f, a.sample_rate())));
        let Some((step, lost)) = self.transport.advance(now, played, p.rate(), last) else { return };
        if let Some(lost) = lost {
            self.stop_audio();
            let why = match lost {
                AudioLost::Failed => "Audio output failed",
                AudioLost::Stalled => "Audio output is not responding",
            };
            self.say(format!("{why}: playing without sound."), true);
        }
        match step {
            Step::Play(f) => self.frames.schedule_playback(&p, self.frame_key(&p, f), speed, false),
            Step::End(f) => {
                self.stop_audio();
                self.frames.stop_prefetch();
                self.send_seek(f);
            }
            Step::Preroll(_) => {}
        }
    }

    /// Start the sound from `from`; whether it started (it then drives the clock). Reverse and
    /// shuttle faster than 4x play silent.
    fn start_audio(&mut self, p: &Arc<dyn Playback>, from: i64, speed: f64) -> bool {
        if self.mute || resolution_playback::shuttle_mix_rate(48_000, speed).is_none() {
            return false;
        }
        let (p, stats) = (p.clone(), self.audio_stats.clone());
        let started = self.audio.get_or_insert_with(AudioOut::new).start(move |device_rate, channels| {
            let mix_rate = resolution_playback::shuttle_mix_rate(device_rate, speed).unwrap_or(device_rate);
            ahead::spawn(p.mixer(mix_rate), p.rate().sample_of(from, mix_rate), device_rate, channels, stats)
        });
        match started {
            Ok(_) => true,
            Err(e) => {
                self.say(format!("No sound ({e}): playing without it."), true);
                false
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
        let footer = look.skin.button + 16.0 + if self.source.is_some() { 18.0 } else { 0.0 };
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
        if let Some((_, name)) = &self.source {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("Source: {name}")).font(look.small()).color(t.dim));
            });
        }
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
                self.worker.run("source.open", json!({ "item": m.id }));
                self.source = Some((m.id, m.name.clone()));
                self.say(format!("Source: {}. F9 inserts it at the playhead, F10 overwrites.", m.name), false);
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

    /// The timeline viewer: name and timecode above the picture, a scrub bar and the transport
    /// controls below it.
    fn viewer(&mut self, ui: &mut Ui) {
        let look = self.look;
        let (t, skin) = (look.theme, look.skin);
        let rect = ui.available_rect_before_wrap();
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), skin.header));
        let transport = Rect::from_min_max(pos2(rect.left(), rect.bottom() - skin.bar), rect.max);
        let scrub = Rect::from_min_max(
            pos2(rect.left() + 12.0, transport.top() - 22.0),
            pos2(rect.right() - 12.0, transport.top()),
        );
        let picture = Rect::from_min_max(pos2(rect.left(), header.bottom()), pos2(rect.right(), scrub.top() - 2.0));
        // Frames at the viewer's size on screen, in 240 px steps so resizing does not re-render
        // on every repaint.
        let px = picture.width().max(picture.height()) * ui.ctx().pixels_per_point();
        self.view_side = ((px / 240.0).ceil() as u32 * 240).clamp(480, 3840);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, t.panel);
        painter.rect_filled(header, CornerRadius::ZERO, t.head);
        painter.line_segment([header.left_bottom(), header.right_bottom()], look.sep_stroke());
        painter.rect_filled(picture, CornerRadius::ZERO, Color32::BLACK);
        let playhead = self.playhead_frame();
        let mut seek_to = None;
        match &self.timeline {
            Some(tl) => {
                painter.text(
                    header.left_center() + vec2(10.0, 0.0),
                    Align2::LEFT_CENTER,
                    &tl.name,
                    look.medium(),
                    t.text,
                );
                let tc = painter.text(
                    header.right_center() - vec2(10.0, 0.0),
                    Align2::RIGHT_CENTER,
                    tl.frame_timecode(playhead),
                    look.mono(13.0),
                    t.text,
                );
                // while playing: the speed, and the frames per second actually reaching the screen
                if let Some(speed) = self.transport.speed() {
                    let speed = if speed == 1.0 { String::new() } else { format!("{speed}x   ") };
                    let fps = match (self.transport.in_preroll(), self.meter.fps) {
                        (true, _) => "starting".to_string(),
                        (false, Some(fps)) => format!("{fps:.1} fps"),
                        (false, None) => String::new(),
                    };
                    painter.text(
                        tc.left_center() - vec2(14.0, 0.0),
                        Align2::RIGHT_CENTER,
                        format!("{speed}{fps}"),
                        look.mono(11.0),
                        t.dim,
                    );
                }
                if let Some(tex) = &self.frame {
                    let size = tex.size_vec2();
                    let scale = (picture.width() / size.x).min(picture.height() / size.y).max(0.01);
                    let img = Rect::from_center_size(picture.center(), size * scale);
                    painter.image(tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                // the scrub bar: the whole timeline, its marks and the playhead
                let line = Rect::from_center_size(scrub.center(), vec2(scrub.width(), 2.0));
                painter.rect_filled(line, CornerRadius::ZERO, t.line);
                let x_of = |s: f64| scrub.left() + (s / tl.duration.max(1e-9)).clamp(0.0, 1.0) as f32 * scrub.width();
                if let (Some(a), Some(b)) = (tl.mark_in, tl.mark_out) {
                    painter.rect_filled(
                        Rect::from_x_y_ranges(x_of(a)..=x_of(b), (scrub.center().y - 3.0)..=(scrub.center().y + 3.0)),
                        CornerRadius::ZERO,
                        t.accent.gamma_multiply(0.35),
                    );
                }
                let x = x_of(tl.rate.seconds_of(playhead));
                painter.line_segment(
                    [pos2(x, scrub.center().y - 7.0), pos2(x, scrub.center().y + 7.0)],
                    Stroke::new(2.0, t.accent),
                );
                let r = ui.interact(scrub, ui.id().with("scrub"), Sense::click_and_drag());
                if (r.clicked() || r.dragged())
                    && let Some(p) = r.interact_pointer_pos()
                {
                    let s = f64::from((p.x - scrub.left()) / scrub.width()).clamp(0.0, 1.0) * tl.duration;
                    seek_to = Some(tl.rate.frame_at(s));
                }
            }
            None => {
                painter.text(
                    picture.center(),
                    Align2::CENTER_CENTER,
                    "Import media, then double-click a clip in the Media Pool to make a timeline.",
                    look.body(),
                    t.dim,
                );
            }
        }
        if let Some(frame) = seek_to {
            self.seek(frame);
        }
        // the transport, centred under the picture
        let playing = self.transport.speed();
        let buttons = [
            (Icon::ToStart, "playhead.timelineStart", State::Off),
            (Icon::Reverse, "playback.reverse", State::on(playing.is_some_and(|s| s < 0.0))),
            (Icon::Stop, "playback.stop", State::Off),
            (Icon::Play, "playback.playStop", State::on(playing.is_some_and(|s| s > 0.0))),
            (Icon::ToEnd, "playhead.timelineEnd", State::Off),
        ];
        let width = buttons.len() as f32 * (skin.button + 4.0);
        let row = Rect::from_center_size(transport.center(), vec2(width, skin.button));
        let mut run = None;
        ui.scope_builder(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (icon, id, state) in buttons {
                let keys = self.keys(id);
                let state = if self.timeline.is_none() { State::Disabled } else { state };
                if chrome::icon_button(ui, &look, icon, Self::label(id), keys.as_deref(), state).clicked() {
                    run = Some(id);
                }
            }
        });
        if let Some(id) = run {
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

    fn timeline_panel(&mut self, ui: &mut Ui) {
        let look = self.look;
        let t = look.theme;
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
                    let (actions, rects) =
                        timeline_ui::show(ui, &look, tl, playhead, &timeline_ui::View { zoom: self.zoom, tool });
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

    /// Put the frame at the playhead on screen: the exact frame when it is rendered; until then
    /// the one already there (while playing, the newest rendered frame before the playhead).
    fn update_viewer(&mut self, ctx: &egui::Context) {
        let Some(p) = self.program.clone() else { return };
        let frame = self.playhead_frame().clamp(0, (p.frames() - 1).max(0));
        let key = self.frame_key(&p, frame);
        let found = if self.transport.is_playing() {
            let dir = if self.transport.speed().unwrap_or(1.0) < 0.0 { -1 } else { 1 };
            self.frames.nearest(key, 48, dir)
        } else {
            self.frames.request(&p, key);
            self.frames.get(&key).map(|f| (key, f))
        };
        match found {
            Some((k, f)) if self.shown != Some(k) => {
                let image = ColorImage::from_rgba_unmultiplied([f.width as usize, f.height as usize], &f.rgba);
                match &mut self.frame {
                    Some(t) => t.set(image, TextureOptions::LINEAR),
                    None => self.frame = Some(ctx.load_texture("viewer", image, TextureOptions::LINEAR)),
                }
                self.shown = Some(k);
                if self.transport.is_playing() && !self.transport.in_preroll() {
                    self.meter.frame_shown(self.clock.elapsed().as_secs_f64());
                }
            }
            None if !self.transport.is_playing() => {
                if let Some(e) = self.frames.error(&key) {
                    let text = format!("Viewer: {e}");
                    if self.status.0 != text {
                        self.say(text, true);
                    }
                }
            }
            _ => {}
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
            if self.shown.is_some() && self.seek_after_load.is_none() && self.pending_seek.is_none() {
                self.command("playback.playStop");
                if let Some(t) = &mut self.play_test {
                    t.started = Some(now);
                }
            }
            return;
        };
        let played = now - self.meter.started;
        let reached_end = !self.transport.is_playing();
        if self.transport.in_preroll() || (played < seconds && !reached_end) {
            return;
        }
        let fps = self.timeline.as_ref().map_or(0.0, |t| t.fps);
        let audio = self.audio.as_ref();
        let stats = &self.audio_stats;
        let load = |a: &std::sync::atomic::AtomicU64| a.load(std::sync::atomic::Ordering::Relaxed);
        let st = self.frames.stats();
        eprintln!(
            "play test: preroll {:.0} ms, played {played:.1} s{}",
            (self.meter.started - started) * 1e3,
            if reached_end { " (reached the end)" } else { "" }
        );
        eprintln!(
            "  picture: {} frames shown, {:.1} fps (timeline {fps:.3} fps); frame jobs {} done, {} cancelled, {} failed",
            self.meter.shown,
            self.meter.shown as f64 / played.max(1e-9),
            st.jobs,
            st.cancelled,
            st.failed
        );
        eprintln!(
            "  sound: device {:?}, {} Hz, {} channels; clock from {}; {} device callbacks, {} underruns, {} frames of silence",
            audio.and_then(AudioOut::device),
            audio.map_or(0, AudioOut::sample_rate),
            audio.map_or(0, AudioOut::channels),
            if self.transport.audio_clock() { "the audio device" } else { "the wall clock" },
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
            self.stop();
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
        let ready = self.frame.is_some() && self.timeline.is_some() && self.seek_after_load.is_none();
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
        for reply in self.worker.poll() {
            self.apply(reply);
        }
        self.read_keys(&ctx);
        self.advance_playback(&ctx);
        self.update_viewer(&ctx);

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
        egui::CentralPanel::default().frame(fill(t.panel)).show(ui, |ui| self.viewer(ui));
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
        pump(&mut h, "the program to play", |a| a.program.is_some() && a.shown.is_some());
        h.key_press(Key::Space);
        h.step();
        let start = h.state().transport.frame().expect("playing");
        pump(&mut h, "playback to move the playhead a few frames", |a| {
            a.transport.frame().is_some_and(|f| f >= start + 6) && !a.transport.in_preroll()
        });
        pump(&mut h, "frames rendered ahead to reach the viewer", |a| a.shown.is_some_and(|k| k.frame > start));
        h.key_press(Key::Space);
        h.step();
        let stopped = h.state().playhead_frame();
        assert!(!h.state().transport.is_playing());
        assert!(stopped > start);
        pump(&mut h, "the engine's playhead to land where playback stopped", |a| {
            a.pending_seek.is_none() && a.timeline.as_ref().is_some_and(|t| t.playhead_frame == stopped)
        });
        pump(&mut h, "the viewer to show the frame it stopped on", |a| a.shown.is_some_and(|k| k.frame == stopped));
    }

    #[test]
    fn marking_in_while_playing_keeps_playing() {
        let mut h = demo();
        pump(&mut h, "the program to play", |a| a.program.is_some() && a.shown.is_some());
        h.key_press(Key::Space);
        pump(&mut h, "the clock to start", |a| a.transport.is_playing() && !a.transport.in_preroll());
        h.key_press(Key::I);
        h.step();
        let marked = h.state().transport.frame().expect("still playing");
        pump(&mut h, "the In point at the frame that was on screen", |a| {
            a.timeline.as_ref().is_some_and(|t| t.mark_in.is_some_and(|s| (s - t.rate.seconds_of(marked)).abs() < 0.1))
        });
        assert!(h.state().transport.is_playing(), "marking does not stop playback");
    }

    #[test]
    fn l_doubles_the_speed_and_k_stops() {
        let mut h = demo();
        pump(&mut h, "the program to play", |a| a.program.is_some());
        h.key_press(Key::L);
        h.key_press(Key::L);
        h.step();
        assert_eq!(h.state().transport.speed(), Some(2.0));
        h.key_press(Key::J);
        h.step();
        assert_eq!(h.state().transport.speed(), Some(-1.0), "J reverses at normal speed");
        h.key_press(Key::K);
        h.step();
        assert!(!h.state().transport.is_playing());
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
        pump(&mut h, "the program to play", |a| a.program.is_some() && a.shown.is_some());
        h.key_press(Key::Space);
        pump(&mut h, "the clock to start", |a| a.transport.is_playing() && !a.transport.in_preroll());
        h.state_mut().command("workspace.index");
        h.state_mut().command("view.zoomIn");
        assert!(h.state().transport.is_playing());
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
