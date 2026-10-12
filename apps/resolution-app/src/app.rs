//! The editor window: Resolve's page bar and Edit page layout (Media Pool, viewer, inspector,
//! timeline) over the engine's command API. Every edit is a vocabulary command run by the
//! engine thread, the same commands agents use over MCP (ARCHITECTURE invariant 3).

use std::path::PathBuf;
use std::time::Instant;

use egui::{
    Align2, Color32, ColorImage, CornerRadius, FontId, RichText, TextureHandle, TextureOptions, Ui, ViewportCommand,
};
use resolution_engine_api::Page;
use resolution_filmcraft_adapter::FilmCraftEngine;
use resolution_keymap::Keymap;
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
}

struct Playback {
    origin: Instant,
    from: f64,
    rate: f64,
}

pub struct ResolutionApp {
    worker: Worker,
    keymap: Keymap,
    page: Page,
    timeline: Option<Timeline>,
    media: Vec<MediaItem>,
    revision: u64,
    frame: Option<TextureHandle>,
    frame_at: f64,
    requested: Option<(u64, i64, u32)>,
    /// Longest side, in pixels, of the frames the viewer asks for: its size on screen.
    view_side: u32,
    playing: Option<Playback>,
    last_seek: Option<f64>,
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
            frame: None,
            frame_at: 0.0,
            requested: None,
            view_side: 960,
            playing: None,
            last_seek: None,
            zoom: None,
            clip_rects: Vec::new(),
            selected_media: None,
            source: None,
            status: ("Import media with Ctrl+I, or drop files on the window.".into(), false),
            auto_timeline: !options.import.is_empty(),
            seek_after_load: options.seek,
            screenshot: options.screenshot.map(|p| (p, 0)),
        }
    }

    fn say(&mut self, text: impl Into<String>, error: bool) {
        self.status = (text.into(), error);
    }

    /// Where the viewer and timeline put the playhead: the engine's, or the playback clock's.
    fn playhead(&self) -> f64 {
        match (&self.playing, &self.timeline) {
            (Some(p), Some(tl)) => (p.from + p.origin.elapsed().as_secs_f64() * p.rate).clamp(0.0, tl.duration),
            (None, Some(tl)) => tl.playhead,
            _ => 0.0,
        }
    }

    fn apply(&mut self, ctx: &egui::Context, reply: Reply) {
        match reply {
            Reply::Ran { id, result } => match result {
                Ok(out) => {
                    if let Some(label) = out.undo_label {
                        self.say(label, false);
                    } else if !id.ends_with(".inspect") && !id.starts_with("playhead.") && id != "timeline.select" {
                        self.say(format!("{id}: done"), false);
                    }
                }
                Err(e) => self.say(e, true),
            },
            Reply::Snapshot { revision, timeline, project } => {
                self.revision = revision;
                self.timeline = timeline.as_ref().and_then(Timeline::from_json);
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
            Reply::Frame { seconds, frame } => match frame {
                Ok(f) => {
                    let image = ColorImage::from_rgba_unmultiplied([f.width as usize, f.height as usize], &f.rgba);
                    match &mut self.frame {
                        Some(t) => t.set(image, TextureOptions::LINEAR),
                        None => self.frame = Some(ctx.load_texture("viewer", image, TextureOptions::LINEAR)),
                    }
                    self.frame_at = seconds;
                }
                Err(e) => self.say(format!("viewer: {e}"), true),
            },
            Reply::Notice { message, error } => self.say(message, error),
        }
    }

    /// A vocabulary command from a key or a menu. Front-end commands (pages, playback, zoom,
    /// edit modes) are handled here; everything else goes to the engine.
    fn command(&mut self, id: &str) {
        let is_transport = id.starts_with("playback.");
        if !is_transport {
            self.stop();
        }
        match id {
            _ if id.starts_with("page.") => {
                if let Some(p) = Page::ALL.into_iter().find(|p| format!("page.{}", p.name()) == id) {
                    self.page = p;
                }
            }
            "playback.playStop" => {
                if self.playing.is_some() {
                    self.stop();
                } else {
                    self.play(1.0);
                }
            }
            "playback.forward" => self
                .play(self.playing.as_ref().map_or(1.0, |p| if p.rate > 0.0 { (p.rate * 2.0).min(8.0) } else { 1.0 })),
            "playback.reverse" => self.play(
                self.playing.as_ref().map_or(-1.0, |p| if p.rate < 0.0 { (p.rate * 2.0).max(-8.0) } else { -1.0 }),
            ),
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

    fn play(&mut self, rate: f64) {
        let from = self.playhead();
        if self.timeline.is_some() {
            self.playing = Some(Playback { origin: Instant::now(), from, rate });
        }
    }

    /// Stop playback and leave the engine's playhead where it stopped.
    fn stop(&mut self) {
        if self.playing.is_some() {
            let at = self.playhead();
            self.playing = None;
            self.worker.run("playhead.set", json!({ "seconds": at }));
        }
    }

    fn seek(&mut self, t: f64) {
        let half = self.timeline.as_ref().map_or(0.02, |tl| tl.frame_seconds() / 2.0);
        if self.last_seek.is_some_and(|l| (l - t).abs() < half) {
            return;
        }
        self.last_seek = Some(t);
        self.stop();
        self.worker.run("playhead.set", json!({ "seconds": t }));
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
        let playhead = self.playhead();
        match (&self.frame, &self.timeline) {
            (Some(tex), Some(tl)) => {
                let size = tex.size_vec2();
                let scale = ((rect.width() - 16.0) / size.x).min((rect.height() - 40.0) / size.y).max(0.01);
                let img = egui::Rect::from_center_size(rect.center() - egui::vec2(0.0, 10.0), size * scale);
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
                    tl.timecode(playhead),
                    FontId::monospace(18.0),
                    Color32::WHITE,
                );
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

    /// Ask for the frame at the playhead when it, or the project, changed.
    fn request_frame(&mut self) {
        let Some(tl) = &self.timeline else { return };
        let at = self.playhead();
        let key = (self.revision, (at / tl.frame_seconds()).floor() as i64, self.view_side);
        if self.requested != Some(key) {
            self.requested = Some(key);
            self.worker.render(at, self.view_side);
        }
    }

    /// `--screenshot`: once a frame has been on screen for a few repaints, save the window and quit.
    fn self_test(&mut self, ctx: &egui::Context) {
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
        if (ready && *frames == 30) || *frames == 1800 {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint();
    }
}

impl eframe::App for ResolutionApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for reply in self.worker.poll() {
            self.apply(&ctx, reply);
        }
        self.keys(&ctx);
        if let (Some(p), Some(tl)) = (&self.playing, &self.timeline) {
            let at = p.from + p.origin.elapsed().as_secs_f64() * p.rate;
            if at <= 0.0 || at >= tl.duration {
                self.stop();
            }
            ctx.request_repaint();
        }
        self.request_frame();

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
                    let (actions, rects) = timeline_ui::show(ui, tl, playhead, &timeline_ui::View { zoom: self.zoom });
                    self.clip_rects = rects;
                    for a in actions {
                        match a {
                            Action::Seek(t) => self.seek(t),
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
            .build_eframe(|cc| ResolutionApp::new(cc, Options { demo: true, ..Default::default() }));
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
        let t0 = h.state().timeline.as_ref().map_or(0.0, |t| t.playhead);
        h.key_press(Key::ArrowRight);
        pump(&mut h, "Right to step one frame", |a| {
            a.timeline.as_ref().is_some_and(|t| (t.playhead - t0 - t.frame_seconds()).abs() < 1e-3)
        });
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
