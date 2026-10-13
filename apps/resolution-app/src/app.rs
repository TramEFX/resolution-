//! The editor window: Resolve's page bar and Edit page layout (Media Pool, viewer, inspector,
//! timeline) over the engine's command API. Every edit is a vocabulary command run by the
//! engine thread, the same commands agents use over MCP (ARCHITECTURE invariant 3).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use egui::{
    Align2, Color32, ColorImage, CornerRadius, FontId, RichText, TextureHandle, TextureOptions, Ui, ViewportCommand,
};
use resolution_engine_api::{Page, Playback};
use resolution_filmcraft_adapter::FilmCraftEngine;
use resolution_keymap::Keymap;
use resolution_playback::{AudioLost, AudioOut, AudioStats, FrameKey, FrameServer, Step, Transport, ahead};
use serde_json::json;

use crate::model::{self, Kind, MediaItem, Timeline};
use crate::timeline_ui::{self, Action};
use crate::worker::{Reply, Worker};

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "mkv", "mxf", "avi", "webm", "mts", "m2ts", "wav", "mp3", "aac", "png", "jpg", "jpeg", "tif",
    "tiff",
];

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
}

/// Commands that run at the frame on screen without stopping playback.
const WHILE_PLAYING: &[&str] = &["mark.in", "mark.out", "mark.clip", "marker.add"];

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
    page: Page,
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
        // An editor is judged in the dark: Resolution's theme is always dark, whatever the OS uses.
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
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
            page: Page::Edit,
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

    /// A vocabulary command from a key or a menu. Front-end commands (pages, playback, zoom,
    /// edit modes) are handled here; everything else goes to the engine.
    fn command(&mut self, id: &str) {
        if !id.starts_with("playback.") && self.transport.is_playing() {
            if WHILE_PLAYING.contains(&id) {
                if let Some(frame) = self.transport.frame() {
                    self.worker.run("playhead.set", json!({ "frame": frame }));
                }
            } else {
                self.stop();
            }
        }
        let speed = self.transport.speed();
        match id {
            _ if id.starts_with("page.") => {
                if let Some(p) = Page::ALL.into_iter().find(|p| format!("page.{}", p.name()) == id) {
                    self.page = p;
                }
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

    fn keys(&mut self, ctx: &egui::Context) {
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

    fn menu(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Import Media…   Ctrl+I").clicked() {
                    ui.close();
                    self.command("media.import");
                }
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
                if ui.button("Undo   Ctrl+Z").clicked() {
                    ui.close();
                    self.command("edit.undo");
                }
                if ui.button("Redo   Ctrl+Shift+Z").clicked() {
                    ui.close();
                    self.command("edit.redo");
                }
            });
            ui.menu_button("Timeline", |ui| {
                for (label, id) in [
                    ("Split Clip   Ctrl+B", "timeline.split"),
                    ("Ripple Delete   Delete", "timeline.rippleDelete"),
                    ("Zoom to Fit   Shift+Z", "view.zoomToFit"),
                ] {
                    if ui.button(label).clicked() {
                        ui.close();
                        self.command(id);
                    }
                }
            });
        });
    }

    fn page_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space((ui.available_width() - 7.0 * 92.0).max(0.0) / 2.0);
            for p in Page::ALL.into_iter().filter(|p| *p != Page::Global) {
                let name = p.name();
                let label = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
                if ui.add_sized([86.0, 22.0], egui::Button::selectable(self.page == p, label)).clicked() {
                    self.command(&format!("page.{name}"));
                }
            }
        });
    }

    fn media_pool(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("Media Pool");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Import…").clicked() {
                    self.command("media.import");
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.media.is_empty() {
                ui.label(RichText::new("No media yet.\nImport with Ctrl+I or drop files here.").weak());
            }
            for m in self.media.clone() {
                let length = match (m.duration, &self.timeline) {
                    (Some(d), Some(tl)) => format!("   {}", tl.timecode(d)),
                    (Some(d), None) => format!("   {d:.1} s"),
                    _ => String::new(),
                };
                let text = match m.kind {
                    Kind::Bin => RichText::new(m.name.clone()).strong(),
                    Kind::Timeline => {
                        let open = if m.active { "   (open)" } else { "" };
                        RichText::new(format!("{}   timeline{length}{open}", m.name))
                            .color(Color32::from_rgb(232, 190, 110))
                    }
                    Kind::Clip => RichText::new(format!("{}{length}", m.name)),
                };
                ui.horizontal(|ui| {
                    ui.add_space(m.depth as f32 * 14.0);
                    let r = ui.selectable_label(self.selected_media == Some((m.kind, m.id)), text);
                    if r.clicked() {
                        self.selected_media = Some((m.kind, m.id));
                    }
                    if r.double_clicked() {
                        self.open_media(&m);
                    }
                });
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            let clip = self.selected_media.filter(|(k, _)| *k == Kind::Clip).map(|(_, id)| id);
            if ui.add_enabled(clip.is_some(), egui::Button::new("New timeline from clip")).clicked()
                && let Some(id) = clip
            {
                self.worker.run("timeline.createFromClips", json!({ "items": [id] }));
            }
            if let Some((_, name)) = &self.source {
                ui.label(RichText::new(format!("Source: {name}")).small());
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
                self.worker.run("source.open", json!({ "item": m.id }));
                self.source = Some((m.id, m.name.clone()));
                self.say(format!("Source: {}. F9 inserts it at the playhead, F10 overwrites.", m.name), false);
            }
        }
    }

    fn inspector(&mut self, ui: &mut Ui) {
        ui.heading("Inspector");
        ui.separator();
        let Some(tl) = &self.timeline else {
            ui.label(RichText::new("No timeline.").weak());
            return;
        };
        let grid = |ui: &mut Ui, rows: Vec<(&str, String)>| {
            egui::Grid::new(ui.next_auto_id()).num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                for (k, v) in rows {
                    ui.label(RichText::new(k).weak());
                    ui.label(v);
                    ui.end_row();
                }
            });
        };
        if let Some((track, clip)) = tl.selected() {
            ui.strong(&clip.name);
            grid(
                ui,
                vec![
                    ("Track", track.name.clone()),
                    ("Start", tl.timecode(clip.start)),
                    ("End", tl.timecode(clip.end)),
                    ("Duration", tl.timecode(clip.end - clip.start)),
                    ("Enabled", if clip.enabled { "yes" } else { "no" }.into()),
                ],
            );
        } else {
            ui.strong(&tl.name);
            grid(
                ui,
                vec![
                    ("Format", format!("{}×{}", tl.width, tl.height)),
                    ("Frame rate", format!("{:.3}", tl.fps)),
                    ("Duration", tl.timecode(tl.duration)),
                    ("Tracks", format!("{} video, {} audio", tl.video.len(), tl.audio.len())),
                ],
            );
        }
    }

    fn viewer(&mut self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        // Frames at the viewer's size on screen, in 240 px steps so resizing does not re-render
        // on every repaint.
        let px = rect.width().max(rect.height()) * ui.ctx().pixels_per_point();
        self.view_side = ((px / 240.0).ceil() as u32 * 240).clamp(480, 3840);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(12, 13, 16));
        let playhead = self.playhead_frame();
        match (&self.frame, &self.timeline) {
            (Some(tex), Some(tl)) => {
                // the picture between a strip for the name and frame rate above and one for the
                // timecode below, so no text sits on it
                let area = egui::Rect::from_min_max(rect.min + egui::vec2(8.0, 28.0), rect.max - egui::vec2(8.0, 34.0));
                let size = tex.size_vec2();
                let scale = (area.width() / size.x).min(area.height() / size.y).max(0.01);
                let img = egui::Rect::from_center_size(area.center(), size * scale);
                painter.image(
                    tex.id(),
                    img,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                painter.text(
                    rect.left_top() + egui::vec2(10.0, 8.0),
                    Align2::LEFT_TOP,
                    &tl.name,
                    FontId::proportional(13.0),
                    Color32::LIGHT_GRAY,
                );
                painter.text(
                    rect.center_bottom() - egui::vec2(0.0, 8.0),
                    Align2::CENTER_BOTTOM,
                    tl.frame_timecode(playhead),
                    FontId::monospace(18.0),
                    Color32::WHITE,
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
                        rect.right_top() + egui::vec2(-10.0, 8.0),
                        Align2::RIGHT_TOP,
                        format!("{speed}{fps}"),
                        FontId::monospace(13.0),
                        Color32::GRAY,
                    );
                }
            }
            _ => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Import media, then double-click a clip in the Media Pool to make a timeline.",
                    FontId::proportional(15.0),
                    Color32::GRAY,
                );
            }
        }
        ui.allocate_rect(rect, egui::Sense::hover());
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
        self.keys(&ctx);
        self.advance_playback(&ctx);
        self.update_viewer(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("pages").exact_size(34.0).show(ui, |ui| self.page_bar(ui));
        egui::Panel::bottom("status").exact_size(22.0).show(ui, |ui| {
            let (text, error) = &self.status;
            ui.label(
                RichText::new(text)
                    .color(if *error { Color32::from_rgb(240, 110, 100) } else { Color32::GRAY })
                    .small(),
            );
        });
        if self.page != Page::Edit {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "The {} page is not built yet. Shift+4 returns to Edit.",
                            self.page.name()
                        ))
                        .size(16.0)
                        .weak(),
                    );
                });
            });
            self.self_test(&ctx);
            return;
        }
        egui::Panel::bottom("timeline").resizable(true).default_size(280.0).min_size(140.0).show(ui, |ui| {
            let playhead = self.playhead();
            match &self.timeline {
                Some(tl) => {
                    let rate = tl.rate;
                    let (actions, rects) = timeline_ui::show(ui, tl, playhead, &timeline_ui::View { zoom: self.zoom });
                    self.clip_rects = rects;
                    for a in actions {
                        match a {
                            Action::Seek(t) => self.seek(rate.frame_at(t.max(0.0))),
                            Action::Select { clip, add } => {
                                self.worker.run("timeline.select", json!({ "clips": [clip], "add": add }))
                            }
                        }
                    }
                }
                None => {
                    ui.centered_and_justified(|ui| ui.label(RichText::new("No timeline").weak()));
                }
            }
        });
        egui::Panel::left("media").resizable(true).default_size(260.0).show(ui, |ui| self.media_pool(ui));
        egui::Panel::right("inspector").resizable(true).default_size(250.0).show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default().show(ui, |ui| self.viewer(ui));
        self.self_test(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Key, Modifiers, PointerButton, Pos2};
    use egui_kittest::Harness;
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
}
