//! The Deliver page, minimal (ROADMAP step 3): render settings on the left, the render queue on
//! the right, as in Resolve. A queued render is the engine's export job (`deliver.render`);
//! Render All starts them one after another and follows each through `deliver.jobs`.

use std::path::{Path, PathBuf};

use egui::{Align2, CornerRadius, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::chrome;
use crate::theme::Look;

/// The presets the page offers: our name, the engine's preset, and the file's extension.
pub const PRESETS: [(&str, &str, &str); 6] = [
    ("Match Source (H.264)", "Match Source – Adaptive High Bitrate", "mp4"),
    ("H.264 1080p", "High Quality 1080p HD", "mp4"),
    ("H.264 2160p 4K", "High Quality 2160p 4K", "mp4"),
    ("YouTube 1080p", "YouTube 1080p Full HD", "mp4"),
    ("Vertical 1080 × 1920", "Social Vertical 1080×1920", "mp4"),
    ("ProRes 422 HQ", "Apple ProRes 422 HQ", "mov"),
];

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Ready,
    /// Sent to the engine; waiting for its job id.
    Starting,
    Rendering {
        progress: f64,
        eta: Option<f64>,
    },
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderJob {
    pub path: PathBuf,
    /// Index into [`PRESETS`].
    pub preset: usize,
    /// Only the timeline's In to Out.
    pub in_out: bool,
    pub status: Status,
    pub job: Option<u64>,
}

impl RenderJob {
    /// The `deliver.render` parameters.
    pub fn params(&self) -> Value {
        json!({
            "path": self.path.to_string_lossy(),
            "preset": PRESETS[self.preset].1,
            "range": if self.in_out { "inOut" } else { "entire" },
        })
    }
}

/// What the page asks the app to do.
pub enum Ask {
    Browse,
    Add,
    RenderAll,
    Stop(usize),
    Remove(usize),
}

pub struct Deliver {
    pub preset: usize,
    pub name: String,
    pub folder: PathBuf,
    pub in_out: bool,
    pub queue: Vec<RenderJob>,
    /// Render All is working through the queue.
    pub running: bool,
    /// When `deliver.jobs` was last asked (app clock, s).
    pub last_poll: f64,
}

impl Deliver {
    pub fn new() -> Self {
        Self {
            preset: 0,
            name: String::new(),
            folder: default_folder(),
            in_out: false,
            queue: Vec::new(),
            running: false,
            last_poll: 0.0,
        }
    }

    /// Queue a render with the current settings, named `name` (the timeline's name when the
    /// field is empty). A file of that name already on disk, or queued, gets a number.
    pub fn add(&mut self, timeline: &str) -> &RenderJob {
        let name = if self.name.trim().is_empty() { timeline.to_string() } else { self.name.trim().to_string() };
        let ext = PRESETS[self.preset].2;
        let taken = |p: &Path| p.exists() || self.queue.iter().any(|j| j.path == p);
        let mut path = self.folder.join(format!("{name}.{ext}"));
        let mut n = 2;
        while taken(&path) {
            path = self.folder.join(format!("{name} ({n}).{ext}"));
            n += 1;
        }
        self.queue.push(RenderJob { path, preset: self.preset, in_out: self.in_out, status: Status::Ready, job: None });
        self.queue.last().expect("just pushed")
    }

    /// The next render Render All should start, if none is under way.
    pub fn next(&self) -> Option<usize> {
        if !self.running || self.busy() {
            return None;
        }
        self.queue.iter().position(|j| j.status == Status::Ready)
    }

    pub fn busy(&self) -> bool {
        self.queue.iter().any(|j| matches!(j.status, Status::Starting | Status::Rendering { .. }))
    }

    /// The engine answered `deliver.render` for the render that was starting.
    pub fn started(&mut self, result: Result<u64, String>) {
        if let Some(j) = self.queue.iter_mut().find(|j| j.status == Status::Starting) {
            match result {
                Ok(job) => {
                    j.job = Some(job);
                    j.status = Status::Rendering { progress: 0.0, eta: None };
                }
                Err(e) => j.status = Status::Failed(e),
            }
        }
    }

    /// Progress from `deliver.jobs` (the engine's job list). Returns the paths finished now.
    pub fn update(&mut self, jobs: &Value) -> Vec<PathBuf> {
        let mut finished = Vec::new();
        for j in self.queue.iter_mut().filter(|j| matches!(j.status, Status::Rendering { .. })) {
            let Some(v) = jobs.as_array().and_then(|a| a.iter().find(|v| v["id"].as_u64() == j.job)) else { continue };
            if v["finished"].as_bool() == Some(true) {
                j.status = match v["result"]["error"].as_str() {
                    Some(e) if e.contains("cancel") => Status::Cancelled,
                    Some(e) => Status::Failed(e.to_string()),
                    None => {
                        finished.push(j.path.clone());
                        Status::Done
                    }
                };
            } else {
                j.status = Status::Rendering {
                    progress: v["progress"].as_f64().unwrap_or(0.0),
                    eta: v["etaSeconds"].as_f64(),
                };
            }
        }
        if self.running && !self.busy() && !self.queue.iter().any(|j| j.status == Status::Ready) {
            self.running = false;
        }
        finished
    }
}

/// Where renders go unless chosen: the user's Videos (Movies on macOS) folder.
fn default_folder() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    let videos = home.map(|h| h.join(if cfg!(target_os = "macos") { "Movies" } else { "Videos" }));
    videos.filter(|v| v.is_dir()).unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// The Render Settings panel.
pub fn settings(ui: &mut Ui, look: &Look, d: &mut Deliver, timeline: Option<&str>, has_marks: bool) -> Vec<Ask> {
    let t = look.theme;
    let mut asks = Vec::new();
    chrome::panel_header(ui, look, "Render Settings", None);
    let section = |ui: &mut Ui, title: &str| {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new(title).font(look.small()).color(t.dim));
        });
    };
    // the settings scroll when the panel is short, so the Add button stays reachable
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        section(ui, "Preset");
        for (i, (label, _, ext)) in PRESETS.iter().enumerate() {
            let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
            r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, d.preset == i, *label));
            let on = d.preset == i;
            if on || r.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::ZERO, if on { t.active_bg } else { t.hover });
            }
            ui.painter().text(
                rect.left_center() + vec2(14.0, 0.0),
                Align2::LEFT_CENTER,
                label,
                look.body(),
                if on { t.accent } else { t.text },
            );
            ui.painter().text(
                rect.right_center() - vec2(12.0, 0.0),
                Align2::RIGHT_CENTER,
                ext,
                look.mono(look.skin.small),
                t.dim,
            );
            if r.clicked() {
                d.preset = i;
            }
        }
        section(ui, "File name");
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let hint = timeline.unwrap_or("Timeline");
            ui.add(egui::TextEdit::singleline(&mut d.name).hint_text(hint).desired_width(ui.available_width() - 12.0));
        });
        section(ui, "Location");
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let shown = d.folder.to_string_lossy();
            let label = ui.label(RichText::new(shown.as_ref()).font(look.small()).color(t.text));
            label.on_hover_text(shown.as_ref());
        });
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            if chrome::text_button(ui, look, "Browse…", true).clicked() {
                asks.push(Ask::Browse);
            }
        });
        section(ui, "Render");
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            for (label, in_out) in [("Entire Timeline", false), ("In/Out Range", true)] {
                let enabled = !in_out || has_marks;
                let on = d.in_out == in_out;
                let (rect, r) = ui.allocate_exact_size(
                    vec2(118.0, look.skin.button),
                    if enabled { Sense::click() } else { Sense::hover() },
                );
                r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, on, label));
                ui.painter().rect_filled(rect, look.radius(), if on { t.active_bg } else { t.field });
                if on {
                    ui.painter().rect_stroke(rect, look.radius(), Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
                }
                let colour = if !enabled {
                    t.dim.gamma_multiply(0.45)
                } else if on {
                    t.accent
                } else {
                    t.dim
                };
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, look.small(), colour);
                if r.clicked() {
                    d.in_out = in_out;
                }
            }
        });
        if d.in_out && !has_marks {
            d.in_out = false;
        }
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            if chrome::text_button(ui, look, "Add to Render Queue", timeline.is_some()).clicked() {
                asks.push(Ask::Add);
            }
        });
    });
    asks
}

/// The Render Queue panel.
pub fn queue(ui: &mut Ui, look: &Look, d: &Deliver) -> Vec<Ask> {
    let t = look.theme;
    let mut asks = Vec::new();
    let detail = match d.queue.len() {
        1 => "1 job".to_string(),
        n => format!("{n} jobs"),
    };
    chrome::panel_header(ui, look, "Render Queue", Some(&detail));
    let footer = look.skin.button + 20.0;
    let list_h = (ui.available_height() - footer).max(40.0);
    ui.allocate_ui(vec2(ui.available_width(), list_h), |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if d.queue.is_empty() {
                ui.add_space(12.0);
                egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 0)).show(ui, |ui| {
                    ui.label(RichText::new("Nothing queued. Choose a preset and Add to Render Queue.").color(t.dim));
                });
            }
            for (i, j) in d.queue.iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::hover());
                let painter = ui.painter();
                painter.line_segment([rect.left_bottom(), rect.right_bottom()], look.sep_stroke());
                let name = j.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let text = Rect::from_min_max(rect.min + vec2(12.0, 0.0), pos2(rect.right() - 40.0, rect.bottom()));
                painter.with_clip_rect(text).text(
                    rect.left_top() + vec2(12.0, 8.0),
                    Align2::LEFT_TOP,
                    format!("Job {}  {name}", i + 1),
                    look.body(),
                    t.text,
                );
                let range = if j.in_out { "In/Out" } else { "Entire timeline" };
                painter.with_clip_rect(text).text(
                    rect.left_top() + vec2(12.0, 25.0),
                    Align2::LEFT_TOP,
                    format!("{} · {range}", PRESETS[j.preset].0),
                    look.small(),
                    t.dim,
                );
                let (status, colour, progress) = match &j.status {
                    Status::Ready => ("Ready".to_string(), t.dim, None),
                    Status::Starting => ("Starting…".to_string(), t.dim, Some(0.0)),
                    Status::Rendering { progress, eta } => (
                        match eta {
                            Some(s) => format!("{:.0}%  ·  {:.0} s left", progress * 100.0, s.max(0.0)),
                            None => format!("{:.0}%", progress * 100.0),
                        },
                        t.text,
                        Some(*progress),
                    ),
                    Status::Done => ("Done".to_string(), t.accent, Some(1.0)),
                    Status::Failed(e) => (format!("Failed: {e}"), t.error, None),
                    Status::Cancelled => ("Stopped".to_string(), t.dim, None),
                };
                if let Some(p) = progress {
                    let bar =
                        Rect::from_min_size(rect.left_bottom() + vec2(12.0, -14.0), vec2(rect.width() - 24.0, 4.0));
                    painter.rect_filled(bar, CornerRadius::same(2), t.field);
                    let fill = Rect::from_min_size(bar.min, vec2(bar.width() * p.clamp(0.0, 1.0) as f32, bar.height()));
                    painter.rect_filled(fill, CornerRadius::same(2), t.accent);
                }
                painter.with_clip_rect(text).text(
                    rect.left_top() + vec2(12.0, 40.0) - vec2(0.0, if progress.is_some() { 1.0 } else { 0.0 }),
                    Align2::LEFT_TOP,
                    if progress.is_some() { String::new() } else { status.clone() },
                    look.small(),
                    colour,
                );
                if progress.is_some() {
                    painter.text(
                        rect.right_top() + vec2(-12.0, 25.0),
                        Align2::RIGHT_TOP,
                        &status,
                        look.mono(look.skin.small),
                        colour,
                    );
                }
                // stop a running job; remove one that is not
                let button = Rect::from_center_size(rect.right_top() + vec2(-22.0, 18.0), vec2(22.0, 22.0));
                let running = matches!(j.status, Status::Starting | Status::Rendering { .. });
                let label = if running { format!("Stop job {}", i + 1) } else { format!("Remove job {}", i + 1) };
                let r = ui.interact(button, ui.id().with(("render-job", i)), Sense::click());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
                let colour = if r.hovered() { t.text } else { t.dim };
                if r.hovered() {
                    ui.painter().rect_filled(button, look.radius(), t.hover);
                }
                if running {
                    ui.painter().rect_filled(
                        Rect::from_center_size(button.center(), vec2(8.0, 8.0)),
                        CornerRadius::same(1),
                        colour,
                    );
                } else {
                    let c = button.center();
                    let s = Stroke::new(1.5, colour);
                    ui.painter().line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], s);
                    ui.painter().line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], s);
                }
                if r.on_hover_text(&label).clicked() {
                    asks.push(if running { Ask::Stop(i) } else { Ask::Remove(i) });
                }
            }
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let ready = d.queue.iter().any(|j| j.status == Status::Ready);
        let label = if d.running { "Rendering…" } else { "Render All" };
        if chrome::text_button(ui, look, label, ready && !d.running).clicked() {
            asks.push(Ask::RenderAll);
        }
    });
    asks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deliver(dir: &Path) -> Deliver {
        Deliver { folder: dir.to_path_buf(), ..Deliver::new() }
    }

    #[test]
    fn queued_names_do_not_collide() {
        let dir = std::env::temp_dir().join(format!("resolution-deliver-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let mut d = deliver(&dir);
        let first = d.add("Main Edit").path.clone();
        let second = d.add("Main Edit").path.clone();
        assert_eq!(first.file_name().and_then(|n| n.to_str()), Some("Main Edit.mp4"));
        assert_eq!(second.file_name().and_then(|n| n.to_str()), Some("Main Edit (2).mp4"));
        d.preset = 5;
        d.name = "Master".into();
        assert_eq!(d.add("Main Edit").path.file_name().and_then(|n| n.to_str()), Some("Master.mov"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_all_works_through_the_queue_in_order() {
        let mut d = deliver(&std::env::temp_dir());
        d.add("a");
        d.add("b");
        assert_eq!(d.next(), None, "nothing starts until Render All");
        d.running = true;
        assert_eq!(d.next(), Some(0));
        d.queue[0].status = Status::Starting;
        assert_eq!(d.next(), None, "one at a time");
        d.started(Ok(7));
        assert_eq!(d.queue[0].status, Status::Rendering { progress: 0.0, eta: None });
        d.update(&json!([{"id": 7, "finished": false, "progress": 0.5, "etaSeconds": 3.0}]));
        assert_eq!(d.queue[0].status, Status::Rendering { progress: 0.5, eta: Some(3.0) });
        let done = d.update(&json!([{"id": 7, "finished": true, "result": {"frames": 48}}]));
        assert_eq!((d.queue[0].status.clone(), done.len()), (Status::Done, 1));
        assert_eq!(d.next(), Some(1));
        d.queue[1].status = Status::Starting;
        d.started(Err("no timeline".into()));
        d.update(&json!([]));
        assert_eq!(d.queue[1].status, Status::Failed("no timeline".into()));
        assert!(!d.running, "Render All stops when the queue is worked through");
    }
}
