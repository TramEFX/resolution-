//! The Color page (LOG D-029): the timeline viewer, the Node Editor beside it, the clip strip
//! under them, and the Primaries and Curves palettes at the bottom, in Resolve's places.
//!
//! The page grades the clip under the playhead. Every change is a `color.*` command
//! (`color.adjust` for the controls, LOG D-028). A drag sends one command per frame under one
//! merge key, so it is one undo step, and the viewer shows each step as the engine renders it.
//! While a drag lasts, the controls show the values being sent, not the engine's answers, which
//! trail them.

use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use egui::{
    Align2, Color32, ColorImage, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, TextureHandle,
    TextureOptions, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};
use resolution_engine_api::{Frame, PARADE_GAP, Playback, RenderHints, ScopeKind, Scopes, VECTORSCOPE_EXTENT};
use resolution_grade::{
    Curve, HslCurve, Morph, Node, Qualifier, Shape as WindowShape, Wheel, Window, hsl_to_rgb, rgb_to_hsl,
};
use serde_json::{Map, Value, json};

use crate::chrome;
use crate::model::GradeView;
use crate::theme::Look;

/// What the page asks the app to do.
pub enum Ask {
    /// Set controls of the current node; the merge key names the drag they belong to.
    Adjust(Value, Option<String>),
    /// A `color.*` command on the current clip and node.
    Command(&'static str),
    /// Move the playhead to a timeline frame (a clip in the strip).
    Seek(i64),
}

#[derive(Default)]
pub struct ColorPage {
    /// The clip being graded, and its grade as last known.
    pub clip: Option<u64>,
    pub view: Option<GradeView>,
    /// The current node (0-based).
    pub node: usize,
    /// A node just asked for and made current: kept current while grades from before it still
    /// arrive, until the add's own answer (LOG B-055).
    pub new_node: Option<usize>,
    /// The custom curve being edited: 0 Y, 1 R, 2 G, 3 B.
    pub channel: usize,
    /// What the Curves palette shows: the Custom Curves or a hue curve.
    pub curve_mode: CurveMode,
    /// The Primaries palette shows its wheels, or its bars (LOG D-067).
    pub bars: bool,
    /// The Mini-Timeline is open above the clips (LOG D-071).
    pub mini_timeline: bool,
    /// The clip and revision of the last `color.inspect` asked for.
    pub asked: Option<(u64, u64)>,
    /// A control is being dragged.
    pub dragging: bool,
    drags: u64,
    /// The curve point being dragged (an index into the curve's knots).
    curve_point: Option<usize>,
    /// The centre palette showing.
    pub palette: Palette,
    /// The window chosen in the Window palette (an index into the current node's windows).
    pub window: usize,
    /// A window just added and chosen, on that node: kept chosen while grades from before it still
    /// arrive (LOG B-056).
    pub new_window: Option<(usize, usize)>,
    /// The on-screen control is moving the chosen window.
    moving_window: bool,
    /// The on-screen control is moving this corner of the chosen polygon window.
    moving_point: Option<usize>,
    /// The on-screen control is dragging this handle of the chosen window (LOG D-057).
    moving_handle: Option<Handle>,
    /// Per clip, the revision at which its grade last changed: when its thumbnail is due again.
    pub stamps: HashMap<u64, u64>,
    /// The Highlight view of the current node, when on (LOG D-048): `true` in black and white.
    pub highlight: Option<bool>,
    /// That view as Highlight Difference (LOG D-070); only `--highlight diff` sets it so far.
    pub highlight_difference: bool,
}

impl ColorPage {
    /// The current node, once the grade is known.
    pub fn current(&self) -> Option<&Node> {
        self.view.as_ref()?.grade.nodes.get(self.node)
    }

    /// Take a grade from the engine: ignored during a drag (it trails what is being sent) and for
    /// another clip.
    pub fn take(&mut self, v: GradeView, revision: u64) {
        if self.dragging || Some(v.clip) != self.clip {
            return;
        }
        if self.view.as_ref().is_none_or(|old| old.grade != v.grade) {
            self.stamps.insert(v.clip, revision);
        }
        let len = v.grade.nodes.len();
        match self.new_node {
            // the new node is here (and current, unless another was chosen meanwhile)
            Some(n) if n < len => self.new_node = None,
            // a grade from before it: the current node is left as it is (the add's own answer
            // settles it)
            Some(_) => {}
            None => self.node = self.node.min(len.saturating_sub(1)),
        }
        // the window added is in the engine's grade: it need be held no longer (LOG B-056)
        if let Some((node, w)) = self.new_window
            && v.grade.nodes.get(node).is_some_and(|n| n.windows.len() > w)
        {
            self.new_window = None;
        }
        self.view = Some(v);
    }

    /// The add's own answer has come: if the node it asked for is still missing, it was not added,
    /// and the current node is one the grade has again.
    pub fn added(&mut self) {
        if self.new_node.is_none() {
            return;
        }
        self.new_node = None;
        let len = self.view.as_ref().map_or(1, |v| v.grade.nodes.len());
        self.node = self.node.min(len.saturating_sub(1));
    }

    fn begin_drag(&mut self) {
        self.drags += 1;
        self.dragging = true;
    }

    /// The drag is over: ask the engine for the grade again.
    fn end_drag(&mut self) {
        self.dragging = false;
        self.curve_point = None;
        self.moving_window = false;
        self.moving_point = None;
        self.moving_handle = None;
        self.asked = None;
    }

    /// Set controls of the current node: here at once, and in the engine.
    fn adjust(&mut self, asks: &mut Vec<Ask>, controls: Value) {
        if let Some(n) = self.view.as_mut().and_then(|v| v.grade.nodes.get_mut(self.node)) {
            let _ = n.patch(&controls);
        }
        asks.push(Ask::Adjust(controls, self.dragging.then(|| format!("drag{}", self.drags))));
    }
}

/// `{key: value}`.
fn obj(key: &str, value: Value) -> Value {
    let mut m = Map::new();
    m.insert(key.to_string(), value);
    Value::Object(m)
}

/// A line of guidance where a panel has nothing to show.
fn hint(ui: &mut Ui, look: &Look, text: &str) {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new(text).color(look.theme.dim));
    });
}

/// A widget's name for screen readers and UI tests.
fn name(r: &Response, kind: WidgetType, label: &str) {
    r.widget_info(|| WidgetInfo::labeled(kind, true, label));
}

/// Functional colours for the R, G and B channels.
const RED: Color32 = Color32::from_rgb(222, 96, 96);
const GREEN: Color32 = Color32::from_rgb(96, 196, 116);
const BLUE: Color32 = Color32::from_rgb(98, 146, 236);

// ---- Primaries ----

struct WheelSpec {
    name: &'static str,
    field: &'static str,
    /// The value of R, G and B at the wheel's centre.
    neutral: f32,
    /// How far the balance goes at the wheel's edge.
    scale: f32,
    /// The master's change per point dragged.
    ring_step: f32,
}

const WHEELS: [WheelSpec; 4] = [
    WheelSpec { name: "Lift", field: "lift", neutral: 0.0, scale: 0.25, ring_step: 0.002 },
    WheelSpec { name: "Gamma", field: "gamma", neutral: 0.0, scale: 0.25, ring_step: 0.002 },
    WheelSpec { name: "Gain", field: "gain", neutral: 1.0, scale: 0.5, ring_step: 0.004 },
    WheelSpec { name: "Offset", field: "offset", neutral: 25.0, scale: 12.5, ring_step: 0.1 },
];

fn wheel_of<'a>(n: &'a Node, field: &str) -> &'a Wheel {
    match field {
        "lift" => &n.lift,
        "gamma" => &n.gamma,
        "gain" => &n.gain,
        _ => &n.offset,
    }
}

/// A number control in an adjustment bar.
struct FieldSpec {
    name: &'static str,
    key: &'static str,
    step: f32,
    decimals: usize,
}

const TOP_BAR: [FieldSpec; 5] = [
    FieldSpec { name: "Temp", key: "temp", step: 10.0, decimals: 1 },
    FieldSpec { name: "Tint", key: "tint", step: 0.5, decimals: 2 },
    FieldSpec { name: "Contrast", key: "contrast", step: 0.005, decimals: 3 },
    FieldSpec { name: "Pivot", key: "pivot", step: 0.002, decimals: 3 },
    // Midtone Detail, as Resolve's bar names it (LOG D-055)
    FieldSpec { name: "Mid/Detail", key: "midtone_detail", step: 0.5, decimals: 2 },
];

const BOTTOM_BAR: [FieldSpec; 5] = [
    FieldSpec { name: "Color Boost", key: "color_boost", step: 0.5, decimals: 2 },
    FieldSpec { name: "Shadows", key: "shadows", step: 0.5, decimals: 2 },
    FieldSpec { name: "Highlights", key: "highlights", step: 0.5, decimals: 2 },
    FieldSpec { name: "Saturation", key: "saturation", step: 0.25, decimals: 2 },
    FieldSpec { name: "Hue", key: "hue", step: 0.25, decimals: 2 },
];

fn field_value(n: &Node, key: &str) -> f32 {
    match key {
        "temp" => n.temp,
        "tint" => n.tint,
        "contrast" => n.contrast,
        "pivot" => n.pivot,
        "color_boost" => n.color_boost,
        "shadows" => n.shadows,
        "highlights" => n.highlights,
        "midtone_detail" => n.midtone_detail,
        "saturation" => n.saturation,
        _ => n.hue,
    }
}

/// The Primaries palette: Temp, Tint, Contrast, Pivot and Midtone Detail above; Color Boost,
/// Shadows, Highlights, Saturation and Hue below; between them Lift, Gamma, Gain and Offset, as
/// wheels (each with its master ring and YRGB numbers) or as bars (LOG D-067), from the mode menu
/// in the title strip.
pub fn primaries(ui: &mut Ui, look: &Look, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let title = if page.bars { "Primaries - Color Bars" } else { "Primaries - Color Wheels" };
    let header = chrome::panel_header(ui, look, title, None);
    // the mode, from a menu left of the reset button
    let chooser = Rect::from_min_max(
        pos2(header.right() - 16.0 - look.skin.button - 96.0, header.top()),
        pos2(header.right() - 12.0 - look.skin.button, header.bottom()),
    );
    within(ui, chooser, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mode = if page.bars { "Bars" } else { "Wheels" };
            ui.menu_button(format!("{mode} {}", crate::theme::MENU_MARK), |ui| {
                for (bars, label) in [(false, "Color Wheels"), (true, "Color Bars")] {
                    if ui.selectable_label(page.bars == bars, label).clicked() {
                        page.bars = bars;
                        ui.close();
                    }
                }
            });
        });
    });
    let Some(node) = page.current().cloned() else {
        hint(ui, look, "Move the playhead onto a clip to grade it.");
        return asks;
    };
    if chrome::header_button(ui, look, header, crate::icons::Icon::Reset, "Reset Primaries") {
        let d = serde_json::to_value(Node::default()).unwrap_or_default();
        let mut controls = Map::new();
        for k in [
            "lift",
            "gamma",
            "gain",
            "offset",
            "contrast",
            "pivot",
            "saturation",
            "hue",
            "color_boost",
            "temp",
            "tint",
            "shadows",
            "highlights",
            "midtone_detail",
        ] {
            // (the controls added later are not written while neutral: 0)
            controls.insert(k.to_string(), d.get(k).cloned().unwrap_or(json!(0.0)));
        }
        page.adjust(&mut asks, Value::Object(controls));
    }
    let area = ui.available_rect_before_wrap().shrink2(vec2(8.0, 4.0));
    let bar = 26.0;
    let top = Rect::from_min_size(area.min, vec2(area.width(), bar));
    let bottom = Rect::from_min_max(pos2(area.left(), area.bottom() - bar), area.max);
    let wheels = Rect::from_min_max(pos2(area.left(), top.bottom() + 4.0), pos2(area.right(), bottom.top() - 4.0));
    fields(ui, look, top, &TOP_BAR, &node, page, &mut asks);
    let w = wheels.width() / 4.0;
    for (k, spec) in WHEELS.iter().enumerate() {
        let col = Rect::from_min_size(pos2(wheels.left() + w * k as f32, wheels.top()), vec2(w, wheels.height()));
        if page.bars {
            bars(ui, look, col, spec, wheel_of(&node, spec.field), page, &mut asks);
        } else {
            wheel(ui, look, col, spec, wheel_of(&node, spec.field), page, &mut asks);
        }
    }
    fields(ui, look, bottom, &BOTTOM_BAR, &node, page, &mut asks);
    ui.allocate_rect(area, Sense::hover());
    asks
}

/// A row of number controls: drag one sideways to change it, double-click to reset it.
fn fields(
    ui: &mut Ui,
    look: &Look,
    row: Rect,
    specs: &[FieldSpec],
    node: &Node,
    page: &mut ColorPage,
    asks: &mut Vec<Ask>,
) {
    let w = row.width() / specs.len() as f32;
    let default = Node::default();
    for (k, spec) in specs.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(row.left() + w * k as f32, row.top()), vec2(w, row.height()))
            .shrink2(vec2(6.0, 2.0));
        let value = field_value(node, spec.key);
        let changed = (value - field_value(&default, spec.key)).abs() > 1e-6;
        let r = number(ui, look, cell, spec.name, &format!("{value:.*}", spec.decimals), changed);
        if r.drag_started() {
            page.begin_drag();
        }
        if r.double_clicked() {
            page.adjust(asks, obj(spec.key, json!(field_value(&default, spec.key))));
        } else if r.dragged() && r.drag_delta().x != 0.0 {
            page.adjust(asks, obj(spec.key, json!(value + r.drag_delta().x * spec.step)));
        }
        if r.drag_stopped() {
            page.end_drag();
        }
    }
}

/// A number control: its name and value in a field. Drag it sideways to change it; double-click
/// it to reset it (the caller does both).
fn number(ui: &mut Ui, look: &Look, cell: Rect, label: &str, value: &str, changed: bool) -> Response {
    number_shown(ui, look, cell, label, label, value, changed)
}

/// A number control whose cell shows `shown` for its name, `label` staying its name for
/// accessibility (the qualifier's columns, whose title already says Hue, Sat or Lum; LOG B-051).
fn number_shown(
    ui: &mut Ui,
    look: &Look,
    cell: Rect,
    label: &str,
    shown: &str,
    value: &str,
    changed: bool,
) -> Response {
    let t = look.theme;
    let r = ui.interact(cell, ui.id().with(("number", label)), Sense::click_and_drag());
    name(&r, WidgetType::DragValue, label);
    let painter = ui.painter();
    painter.rect_filled(cell, look.radius(), if r.hovered() || r.dragged() { t.hover } else { t.field });
    painter.text(cell.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, shown, look.small(), t.dim);
    painter.text(
        cell.right_center() - vec2(8.0, 0.0),
        Align2::RIGHT_CENTER,
        value,
        look.mono(12.0),
        if changed { t.accent } else { t.text },
    );
    r
}

/// The colour a point on a wheel moves the balance towards (direction only).
fn wheel_colour(angle: f32) -> Color32 {
    let mut w = Wheel::splat(0.5);
    w.set_balance([angle.cos(), angle.sin()], 0.5, 0.35);
    let c = [w.r, w.g, w.b].map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
    Color32::from_rgb(c[0], c[1], c[2])
}

/// One Primaries wheel: its name, the balance wheel (drag to move the balance, Shift to jump
/// to the pointer, double-click to reset), the master ring under it (drag sideways), and its
/// YRGB numbers.
fn wheel(ui: &mut Ui, look: &Look, col: Rect, spec: &WheelSpec, w: &Wheel, page: &mut ColorPage, asks: &mut Vec<Ask>) {
    let t = look.theme;
    let id = ui.id().with(("wheel", spec.field));
    let d = (col.width() - 28.0).min(col.height() - 84.0).max(40.0);
    let c = pos2(col.center().x, col.top() + 20.0 + d / 2.0);
    let r = d / 2.0;
    let wheel_rect = Rect::from_center_size(c, vec2(d, d));
    let ring = Rect::from_center_size(pos2(c.x, wheel_rect.bottom() + 16.0), vec2(d, 12.0));
    let painter = ui.painter_at(col);
    painter.text(pos2(c.x, col.top() + 8.0), Align2::CENTER_CENTER, spec.name, look.small(), t.dim);
    // the wheel: a ring of the colours the balance moves towards, round a dark field
    painter.circle_filled(c, r, t.field);
    let n = 72;
    for i in 0..n {
        let (a0, a1) = (i as f32 / n as f32 * TAU, (i + 1) as f32 / n as f32 * TAU);
        let at = |a: f32, rr: f32| c + vec2(a.cos(), -a.sin()) * rr;
        let colour = wheel_colour((a0 + a1) / 2.0).gamma_multiply(0.85);
        painter.add(Shape::convex_polygon(
            vec![at(a0, r), at(a1, r), at(a1, r - 5.0), at(a0, r - 5.0)],
            colour,
            Stroke::NONE,
        ));
    }
    painter.line_segment([c - vec2(r - 8.0, 0.0), c + vec2(r - 8.0, 0.0)], Stroke::new(1.0, t.line));
    painter.line_segment([c - vec2(0.0, r - 8.0), c + vec2(0.0, r - 8.0)], Stroke::new(1.0, t.line));
    let bal = w.balance(spec.neutral, spec.scale);
    let puck = c + vec2(bal[0].clamp(-1.0, 1.0), -bal[1].clamp(-1.0, 1.0)) * (r - 6.0);
    painter.circle_stroke(puck, 5.0, Stroke::new(1.5, t.text));
    let resp = ui.interact(wheel_rect, id, Sense::click_and_drag());
    name(&resp, WidgetType::Slider, &format!("{} wheel", spec.name));
    let shift = ui.input(|i| i.modifiers.shift);
    if resp.drag_started() {
        page.begin_drag();
    }
    let set = |page: &mut ColorPage, asks: &mut Vec<Ask>, mut b: [f32; 2]| {
        let len = (b[0] * b[0] + b[1] * b[1]).sqrt();
        if len > 1.0 {
            b = [b[0] / len, b[1] / len];
        }
        let mut nw = *w;
        nw.set_balance(b, spec.neutral, spec.scale);
        page.adjust(asks, obj(spec.field, json!({ "r": nw.r, "g": nw.g, "b": nw.b })));
    };
    if resp.double_clicked() {
        let n = spec.neutral;
        page.adjust(asks, obj(spec.field, json!({ "r": n, "g": n, "b": n })));
    } else if let Some(p) = resp.interact_pointer_pos().filter(|_| shift && (resp.dragged() || resp.clicked())) {
        let v = (p - c) / (r - 6.0);
        set(page, asks, [v.x, -v.y]);
    } else if resp.dragged() && resp.drag_delta() != egui::Vec2::ZERO {
        // relative, as a trackball: half the pointer's movement, for fine work
        let dl = resp.drag_delta() / (r - 6.0) * 0.5;
        set(page, asks, [bal[0] + dl.x, bal[1] - dl.y]);
    }
    if resp.drag_stopped() {
        page.end_drag();
    }
    // the master ring: ticks that slide as it turns
    let rr = ui.interact(ring, id.with("ring"), Sense::click_and_drag());
    name(&rr, WidgetType::Slider, &format!("{} master", spec.name));
    painter.rect_filled(ring, CornerRadius::same(6), if rr.hovered() || rr.dragged() { t.hover } else { t.field });
    let turn = ((w.y - spec.neutral) / spec.ring_step).rem_euclid(8.0);
    let mut x = ring.left() + turn;
    while x < ring.right() {
        painter.line_segment([pos2(x, ring.top() + 3.0), pos2(x, ring.bottom() - 3.0)], Stroke::new(1.0, t.line));
        x += 8.0;
    }
    if rr.drag_started() {
        page.begin_drag();
    }
    if rr.double_clicked() {
        page.adjust(asks, obj(spec.field, json!({ "y": spec.neutral })));
    } else if rr.dragged() && rr.drag_delta().x != 0.0 {
        page.adjust(asks, obj(spec.field, json!({ "y": w.y + rr.drag_delta().x * spec.ring_step })));
    }
    if rr.drag_stopped() {
        page.end_drag();
    }
    // Y, R, G and B
    let top = ring.bottom() + 8.0;
    for (k, (letter, v, colour)) in
        [("Y", w.y, t.text), ("R", w.r, RED), ("G", w.g, GREEN), ("B", w.b, BLUE)].into_iter().enumerate()
    {
        let x = col.left() + 10.0 + (col.width() - 20.0) * (k as f32 + 0.5) / 4.0;
        painter.text(pos2(x, top + 6.0), Align2::CENTER_CENTER, letter, look.small(), colour);
        let changed = (v - spec.neutral).abs() > 1e-4;
        painter.text(
            pos2(x, top + 21.0),
            Align2::CENTER_CENTER,
            format!("{v:.2}"),
            look.mono(11.0),
            if changed { t.accent } else { t.dim },
        );
    }
}

/// How far a Primaries bar reaches each side of neutral: twice its wheel's reach (LOG D-067).
fn bar_reach(spec: &WheelSpec) -> f32 {
    2.0 * spec.scale
}

/// One group of Primaries bars (LOG D-067): Lift, Gamma, Gain or Offset as four vertical sliders,
/// Y, R, G and B, neutral at the middle. Drag one up or down (by the bar's height for its whole
/// reach, Shift for a tenth of that), double-click it to reset it; its value under it.
fn bars(ui: &mut Ui, look: &Look, col: Rect, spec: &WheelSpec, w: &Wheel, page: &mut ColorPage, asks: &mut Vec<Ask>) {
    let t = look.theme;
    let painter = ui.painter_at(col);
    painter.text(pos2(col.center().x, col.top() + 8.0), Align2::CENTER_CENTER, spec.name, look.small(), t.dim);
    let (top, bottom) = (col.top() + 22.0, col.bottom() - 34.0);
    let reach = bar_reach(spec);
    let fine = ui.input(|i| i.modifiers.shift);
    for (k, (letter, key, v, colour)) in
        [("Y", "y", w.y, t.text), ("R", "r", w.r, RED), ("G", "g", w.g, GREEN), ("B", "b", w.b, BLUE)]
            .into_iter()
            .enumerate()
    {
        let x = col.left() + 10.0 + (col.width() - 20.0) * (k as f32 + 0.5) / 4.0;
        let bar = Rect::from_min_max(pos2(x - 7.0, top), pos2(x + 7.0, bottom.max(top + 20.0)));
        let r = ui.interact(bar.expand2(vec2(4.0, 0.0)), ui.id().with(("bar", spec.field, k)), Sense::click_and_drag());
        name(&r, WidgetType::Slider, &format!("{} {letter} bar", spec.name));
        painter.rect_filled(bar, look.radius(), if r.hovered() || r.dragged() { t.hover } else { t.field });
        // from neutral, at the middle, to the value
        let y_of = |v: f32| bar.center().y - ((v - spec.neutral) / reach).clamp(-1.0, 1.0) * bar.height() / 2.0;
        let (a, b) = (y_of(spec.neutral), y_of(v));
        painter.rect_filled(
            Rect::from_min_max(pos2(bar.left() + 2.0, a.min(b)), pos2(bar.right() - 2.0, a.max(b).max(a.min(b) + 1.0))),
            CornerRadius::ZERO,
            colour.gamma_multiply(0.8),
        );
        painter.line_segment([pos2(bar.left(), a), pos2(bar.right(), a)], Stroke::new(1.0, t.line));
        let changed = (v - spec.neutral).abs() > 1e-4;
        painter.text(pos2(x, bottom + 10.0), Align2::CENTER_CENTER, letter, look.small(), colour);
        painter.text(
            pos2(x, bottom + 25.0),
            Align2::CENTER_CENTER,
            format!("{v:.2}"),
            look.mono(11.0),
            if changed { t.accent } else { t.dim },
        );
        if r.drag_started() {
            page.begin_drag();
        }
        if r.double_clicked() {
            page.adjust(asks, obj(spec.field, obj(key, json!(spec.neutral))));
        } else if r.dragged() && r.drag_delta().y != 0.0 {
            // up raises it
            let per_point = 2.0 * reach / bar.height().max(1.0) * if fine { 0.1 } else { 1.0 };
            page.adjust(asks, obj(spec.field, obj(key, json!(v - r.drag_delta().y * per_point))));
        }
        if r.drag_stopped() {
            page.end_drag();
        }
    }
}

// ---- Curves ----

/// The Curves palette's modes, chosen from its title strip as in Resolve (LOG D-043). Resolve's
/// Sat vs Lum is not here: the engine's Lumetri, which frames off the GPU render with, has no
/// such curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CurveMode {
    #[default]
    Custom,
    HueHue,
    HueSat,
    HueLum,
    LumSat,
    SatSat,
    SatLum,
}

impl CurveMode {
    pub const ALL: [CurveMode; 7] = [
        CurveMode::Custom,
        CurveMode::HueHue,
        CurveMode::HueSat,
        CurveMode::HueLum,
        CurveMode::LumSat,
        CurveMode::SatSat,
        CurveMode::SatLum,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CurveMode::Custom => "Custom",
            CurveMode::HueHue => "Hue vs Hue",
            CurveMode::HueSat => "Hue vs Sat",
            CurveMode::HueLum => "Hue vs Lum",
            CurveMode::LumSat => "Lum vs Sat",
            CurveMode::SatSat => "Sat vs Sat",
            CurveMode::SatLum => "Sat vs Lum",
        }
    }

    /// The hue curve's name in the grade, and whether it runs round the colour wheel; `None` for
    /// the Custom Curves.
    fn curve(self) -> Option<(&'static str, bool)> {
        match self {
            CurveMode::Custom => None,
            CurveMode::HueHue => Some(("hue_hue", true)),
            CurveMode::HueSat => Some(("hue_sat", true)),
            CurveMode::HueLum => Some(("hue_lum", true)),
            CurveMode::LumSat => Some(("lum_sat", false)),
            CurveMode::SatSat => Some(("sat_sat", false)),
            CurveMode::SatLum => Some(("sat_lum", false)),
        }
    }
}

/// A Color page palette by name, for `--palette` (LOG D-069): `curves`, `qualifier`, `window`, or
/// the Curves palette on a hue curve, `curves:` and its name in the grade (`curves:hue_sat`).
pub fn palette_of(name: &str) -> Option<(Palette, CurveMode)> {
    match name {
        "curves" => Some((Palette::Curves, CurveMode::Custom)),
        "qualifier" => Some((Palette::Qualifier, CurveMode::Custom)),
        "window" => Some((Palette::Window, CurveMode::Custom)),
        _ => {
            let key = name.strip_prefix("curves:")?;
            CurveMode::ALL.into_iter().find(|m| m.curve().is_some_and(|c| c.0 == key)).map(|m| (Palette::Curves, m))
        }
    }
}

fn hsl_curve_of<'a>(n: &'a Node, key: &str) -> &'a HslCurve {
    match key {
        "hue_hue" => &n.curves.hue_hue,
        "hue_sat" => &n.curves.hue_sat,
        "hue_lum" => &n.curves.hue_lum,
        "lum_sat" => &n.curves.lum_sat,
        "sat_sat" => &n.curves.sat_sat,
        _ => &n.curves.sat_lum,
    }
}

/// The six vectors round the colour wheel, from red, as the hue curves' patches name them.
const VECTORS: [&str; 6] = ["Red", "Yellow", "Green", "Cyan", "Blue", "Magenta"];

/// What hue curve `key` is over, for a colour: its hue (turns), its saturation or its lightness,
/// as the grade reads them (`Node::color`).
fn curve_input(key: &str, rgb: [f32; 3]) -> f32 {
    let [h, s, l] = rgb_to_hsl(rgb);
    match key {
        "lum_sat" => l,
        "sat_sat" | "sat_lum" => s,
        _ => h,
    }
}

/// How far a sampled range's outer points sit beyond it (LOG D-066, provisional): a 24th of the
/// wheel over hue, 0.05 of the range otherwise.
const SAMPLE_MARGIN: (f32, f32) = (1.0 / 24.0, 0.05);

/// The curve with three more points (LOG D-066): either side of `lo` to `hi`, `margin` beyond it,
/// and in its middle. Each sits on the curve as it is, so nothing changes until one is moved.
fn with_three(curve: &HslCurve, periodic: bool, (lo, hi): (f32, f32), margin: f32) -> Vec<[f32; 2]> {
    let place = |x: f32| if periodic { x.rem_euclid(1.0) } else { x.clamp(0.0, 1.0) };
    let mut pts = curve.0.clone();
    for x in [lo - margin, (lo + hi) / 2.0, hi + margin].map(place) {
        pts.push([x, curve.eval(x, periodic)]);
    }
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-3);
    pts
}

/// The curve with three points for what `inputs` (hues, saturations or lightnesses) cover, as
/// Resolve's image sampling adds them (LOG D-066). Over hue the range runs round the wheel: the
/// shortest arc that holds every sample.
fn sampled(curve: &HslCurve, periodic: bool, inputs: &[f32]) -> Vec<[f32; 2]> {
    let mut v: Vec<f32> = inputs.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return curve.0.clone();
    }
    v.sort_by(f32::total_cmp);
    let range = if periodic {
        // the arc starts after the widest gap between neighbouring hues, the one past red included
        let n = v.len();
        let gap = |i: usize| if i + 1 < n { v[i + 1] - v[i] } else { v[0] + 1.0 - v[i] };
        let widest = (0..n).max_by(|a, b| gap(*a).total_cmp(&gap(*b))).unwrap_or(0);
        let lo = v[(widest + 1) % n];
        (lo, lo + 1.0 - gap(widest))
    } else {
        (v[0], v[v.len() - 1])
    };
    with_three(curve, periodic, range, if periodic { SAMPLE_MARGIN.0 } else { SAMPLE_MARGIN.1 })
}

/// The curve with a six-vector patch's three points (LOG D-066): the middle on vector `k` (from
/// red), the outer two half way to its neighbours.
fn patched(curve: &HslCurve, k: usize) -> Vec<[f32; 2]> {
    let at = k as f32 / 6.0;
    with_three(curve, true, (at, at), 1.0 / 12.0)
}

fn curve_of(n: &Node, channel: usize) -> &Curve {
    match channel {
        1 => &n.curves.r,
        2 => &n.curves.g,
        3 => &n.curves.b,
        _ => &n.curves.y,
    }
}

const CHANNELS: [(&str, &str); 4] = [("Y", "y"), ("R", "r"), ("G", "g"), ("B", "b")];

/// The Custom Curves: click to add a point, drag one to move it (a drag elsewhere makes one and
/// moves it), right-click one to remove it.
/// Y moves all three channels; R, G and B their own.
pub fn curves(ui: &mut Ui, look: &Look, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let t = look.theme;
    let header = palette_header(ui, look, page);
    let Some(node) = page.current().cloned() else { return asks };
    // the mode, from a menu at the strip's right, as Resolve's Curves palette chooses it
    let chooser =
        Rect::from_min_max(pos2(header.right() - 118.0, header.top()), pos2(header.right() - 6.0, header.bottom()));
    within(ui, chooser, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button(format!("{} {}", page.curve_mode.name(), crate::theme::MENU_MARK), |ui| {
                for m in CurveMode::ALL {
                    if ui.selectable_label(page.curve_mode == m, m.name()).clicked() {
                        page.curve_mode = m;
                        ui.close();
                    }
                }
            });
        });
    });
    if let Some((key, periodic)) = page.curve_mode.curve() {
        // over hue, Resolve's six-vector patches in the title strip (LOG D-066)
        if periodic {
            let mut x = chooser.left() - 6.0;
            for k in (0..6).rev() {
                let b = Rect::from_min_max(pos2(x - 18.0, header.top() + 6.0), pos2(x, header.bottom() - 6.0));
                x -= 22.0;
                let r = ui.interact(b, ui.id().with(("patch", k)), Sense::click());
                name(&r, WidgetType::Button, VECTORS[k]);
                let c = hsl_to_rgb([k as f32 / 6.0, 1.0, 0.5]).map(|v| (v * 255.0) as u8);
                ui.painter().rect_filled(
                    b.shrink(if r.hovered() { 0.0 } else { 2.0 }),
                    look.radius(),
                    Color32::from_rgb(c[0], c[1], c[2]),
                );
                if r.clicked() {
                    let pts = patched(hsl_curve_of(&node, key), k);
                    page.adjust(&mut asks, json!({ "curves": obj(key, json!(pts)) }));
                }
            }
        }
        hsl_curve(ui, look, page, &node, key, periodic, &mut asks);
        return asks;
    }
    // the channel buttons, in the title strip
    let mut x = chooser.left() - 6.0;
    for (k, (letter, _)) in CHANNELS.iter().enumerate().rev() {
        let b = Rect::from_min_max(pos2(x - 24.0, header.top() + 4.0), pos2(x, header.bottom() - 4.0));
        x -= 28.0;
        let r = ui.interact(b, ui.id().with(("channel", k)), Sense::click());
        name(&r, WidgetType::Button, &format!("{letter} curve"));
        let on = page.channel == k;
        ui.painter().rect_filled(
            b,
            look.radius(),
            if on {
                t.active_bg
            } else if r.hovered() {
                t.hover
            } else {
                t.field
            },
        );
        let colour = [t.text, RED, GREEN, BLUE][k];
        ui.painter().text(b.center(), Align2::CENTER_CENTER, *letter, look.small(), if on { colour } else { t.dim });
        if r.clicked() {
            page.channel = k;
        }
    }
    let area = ui.available_rect_before_wrap().shrink(12.0);
    let side = area.width().min(area.height()).max(40.0);
    let plot = Rect::from_center_size(area.center(), vec2(side, side));
    let painter = ui.painter_at(area.expand(6.0));
    painter.rect_filled(plot, CornerRadius::ZERO, t.field);
    for k in 1..4 {
        let f = k as f32 / 4.0;
        painter.line_segment(
            [pos2(plot.left() + plot.width() * f, plot.top()), pos2(plot.left() + plot.width() * f, plot.bottom())],
            Stroke::new(1.0, t.line),
        );
        painter.line_segment(
            [pos2(plot.left(), plot.top() + plot.height() * f), pos2(plot.right(), plot.top() + plot.height() * f)],
            Stroke::new(1.0, t.line),
        );
    }
    let to_screen = |p: [f32; 2]| pos2(plot.left() + p[0] * plot.width(), plot.bottom() - p[1] * plot.height());
    let from_screen = |p: Pos2| {
        [((p.x - plot.left()) / plot.width()).clamp(0.0, 1.0), ((plot.bottom() - p.y) / plot.height()).clamp(0.0, 1.0)]
    };
    // the other channels faint, the one being edited bright
    for k in (0..4).filter(|k| *k != page.channel).chain([page.channel]) {
        let c = curve_of(&node, k);
        let colour = [t.text, RED, GREEN, BLUE][k];
        let colour = if k == page.channel { colour } else { colour.gamma_multiply(0.3) };
        let pts: Vec<Pos2> = (0..=96).map(|i| i as f32 / 96.0).map(|x| to_screen([x, c.eval(x)])).collect();
        painter.add(Shape::line(pts, Stroke::new(if k == page.channel { 1.5 } else { 1.0 }, colour)));
    }
    let curve = curve_of(&node, page.channel).clone();
    let knots = curve.knots();
    for p in &knots {
        painter.circle_filled(to_screen(*p), 4.0, t.text);
    }
    let r = ui.interact(plot.expand(6.0), ui.id().with("curve plot"), Sense::click_and_drag());
    name(&r, WidgetType::Other, "Custom Curves");
    let key = CHANNELS[page.channel].1;
    let near = |p: Pos2| knots.iter().position(|k| to_screen(*k).distance(p) <= 8.0);
    let send = |page: &mut ColorPage, asks: &mut Vec<Ask>, pts: Vec<[f32; 2]>| {
        page.adjust(asks, json!({ "curves": obj(key, json!(pts)) }));
    };
    let mut started = false;
    if r.drag_started()
        && let Some(p) = r.interact_pointer_pos()
    {
        page.begin_drag();
        started = true;
        page.curve_point = near(p);
        // a drag away from every point makes one there to move, as in Resolve
        if page.curve_point.is_none() {
            let x = from_screen(p)[0];
            let mut pts = knots.clone();
            pts.push([x, curve.eval(x)]);
            pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
            page.curve_point = pts.iter().position(|k| k[0] == x);
            send(page, &mut asks, pts);
        }
    }
    if r.dragged()
        && !started
        && let (Some(i), Some(p)) = (page.curve_point, r.interact_pointer_pos())
    {
        let mut pts = knots.clone();
        let [mut x, y] = from_screen(p);
        // ends stay at the ends; a point stays between its neighbours
        x = if i == 0 {
            0.0
        } else if i == pts.len() - 1 {
            1.0
        } else {
            x.clamp(pts[i - 1][0] + 0.01, pts[i + 1][0] - 0.01)
        };
        pts[i] = [x, y];
        send(page, &mut asks, pts);
    }
    if r.drag_stopped() {
        page.end_drag();
    }
    if r.clicked()
        && let Some(p) = r.interact_pointer_pos()
        && near(p).is_none()
    {
        let x = from_screen(p)[0];
        let mut pts = knots.clone();
        pts.push([x, curve.eval(x)]);
        pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        send(page, &mut asks, pts);
    }
    if r.secondary_clicked()
        && let Some(i) = r.interact_pointer_pos().and_then(near)
        && i > 0
        && i < knots.len() - 1
    {
        let mut pts = knots.clone();
        pts.remove(i);
        send(page, &mut asks, pts);
    }
    ui.allocate_rect(area, Sense::hover());
    asks
}

/// A hue curve (Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat or Sat vs Lum): the
/// middle line changes nothing; above it more (hue turned one way, more saturation or lightness),
/// below it less. Click to add a point on the curve, drag one to move it (a drag elsewhere makes
/// one and moves it), right-click one to remove it. Along the bottom, what the curve is over: the
/// colour wheel from red, black to white, or grey to full colour.
fn hsl_curve(
    ui: &mut Ui,
    look: &Look,
    page: &mut ColorPage,
    node: &Node,
    key: &'static str,
    periodic: bool,
    asks: &mut Vec<Ask>,
) {
    let t = look.theme;
    let area = ui.available_rect_before_wrap().shrink(12.0);
    let band = 10.0;
    let plot = Rect::from_min_max(area.min, pos2(area.right(), area.bottom() - band - 4.0));
    let painter = ui.painter_at(area.expand(6.0));
    painter.rect_filled(plot, CornerRadius::ZERO, t.field);
    for k in 1..6 {
        let x = plot.left() + plot.width() * k as f32 / 6.0;
        painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0, t.line));
    }
    painter.line_segment(
        [pos2(plot.left(), plot.center().y), pos2(plot.right(), plot.center().y)],
        Stroke::new(1.0, t.dim),
    );
    // what the curve is over
    let strip = Rect::from_min_max(pos2(plot.left(), plot.bottom() + 4.0), pos2(plot.right(), area.bottom()));
    let steps = 48;
    for i in 0..steps {
        let u = (i as f32 + 0.5) / steps as f32;
        let c = match key {
            "lum_sat" => [u, u, u],
            "sat_sat" | "sat_lum" => hsl_to_rgb([0.0, u, 0.5]),
            _ => hsl_to_rgb([u, 1.0, 0.5]),
        };
        let x0 = strip.left() + strip.width() * i as f32 / steps as f32;
        let x1 = strip.left() + strip.width() * (i + 1) as f32 / steps as f32;
        let colour = Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);
        painter.rect_filled(
            Rect::from_min_max(pos2(x0, strip.top()), pos2(x1 + 0.5, strip.bottom())),
            CornerRadius::ZERO,
            colour,
        );
    }
    let to_screen = |p: [f32; 2]| pos2(plot.left() + p[0] * plot.width(), plot.bottom() - p[1] * plot.height());
    let from_screen = |p: Pos2| {
        [((p.x - plot.left()) / plot.width()).clamp(0.0, 1.0), ((plot.bottom() - p.y) / plot.height()).clamp(0.0, 1.0)]
    };
    let curve = hsl_curve_of(node, key).clone();
    let line: Vec<Pos2> =
        (0..=192).map(|i| i as f32 / 192.0).map(|x| to_screen([x, curve.eval(x, periodic)])).collect();
    painter.add(Shape::line(line, Stroke::new(1.5, t.text)));
    let mut points = curve.0.clone();
    points.sort_by(|a, b| a[0].total_cmp(&b[0]));
    for p in &points {
        painter.circle_filled(to_screen(*p), 4.0, t.text);
    }
    if points.is_empty() {
        painter.text(
            plot.center() + vec2(0.0, -18.0),
            Align2::CENTER_CENTER,
            "Click the line to add a point; drag it up or down",
            look.small(),
            t.dim,
        );
    }
    let r = ui.interact(plot.expand(6.0), ui.id().with(("hsl curve", key)), Sense::click_and_drag());
    name(&r, WidgetType::Other, &format!("{} curve", page.curve_mode.name()));
    let near = |p: Pos2| points.iter().position(|k| to_screen(*k).distance(p) <= 8.0);
    let send = |page: &mut ColorPage, asks: &mut Vec<Ask>, pts: Vec<[f32; 2]>| {
        page.adjust(asks, json!({ "curves": obj(key, json!(pts)) }));
    };
    let mut started = false;
    if r.drag_started()
        && let Some(p) = r.interact_pointer_pos()
    {
        page.begin_drag();
        started = true;
        page.curve_point = near(p);
        // a drag away from every point makes one there to move, as in Resolve
        if page.curve_point.is_none() {
            let x = from_screen(p)[0];
            let mut pts = points.clone();
            pts.push([x, curve.eval(x, periodic)]);
            pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
            page.curve_point = pts.iter().position(|k| k[0] == x);
            send(page, asks, pts);
        }
    }
    if r.dragged()
        && !started
        && let (Some(i), Some(p)) = (page.curve_point, r.interact_pointer_pos())
        && i < points.len()
    {
        let mut pts = points.clone();
        let [x, y] = from_screen(p);
        // a point stays between its neighbours
        let lo = if i == 0 { 0.0 } else { pts[i - 1][0] + 0.005 };
        let hi = if i + 1 == pts.len() { 1.0 } else { pts[i + 1][0] - 0.005 };
        pts[i] = [x.clamp(lo, hi.max(lo)), y];
        send(page, asks, pts);
    }
    if r.drag_stopped() {
        page.end_drag();
    }
    if r.clicked()
        && let Some(p) = r.interact_pointer_pos()
        && near(p).is_none()
    {
        let x = from_screen(p)[0];
        let mut pts = points.clone();
        pts.push([x, curve.eval(x, periodic)]);
        pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        send(page, asks, pts);
    }
    if r.secondary_clicked()
        && let Some(i) = r.interact_pointer_pos().and_then(near)
    {
        let mut pts = points.clone();
        pts.remove(i);
        send(page, asks, pts);
    }
    ui.allocate_rect(area, Sense::hover());
}

// ---- the Node Editor ----

/// The commands on a node's right-click menu and the Node Editor's buttons.
const NODE_COMMANDS: [(&str, &str); 5] = [
    ("color.addSerialNode", "Add Serial Node"),
    ("color.addNodeBefore", "Add Before Current"),
    ("color.toggleNodes", "Enable/Disable Node"),
    ("color.resetNode", "Reset Node Grade"),
    ("color.deleteNode", "Delete Node"),
];

/// The Node Editor: the clip's serial nodes from its input to its output. Click a node to make it
/// current; right-click for its commands.
pub fn node_editor(ui: &mut Ui, look: &Look, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let t = look.theme;
    let detail = page.view.as_ref().map(|v| format!("{} nodes", v.grade.nodes.len()));
    chrome::panel_header(ui, look, "Nodes", detail.as_deref());
    let Some(view) = page.view.clone() else {
        hint(ui, look, "Move the playhead onto a clip to grade it.");
        return asks;
    };
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        for (id, label) in [NODE_COMMANDS[0], NODE_COMMANDS[1], NODE_COMMANDS[4]] {
            if chrome::text_button(ui, look, label, true).clicked() {
                asks.push(Ask::Command(id));
            }
        }
    });
    let area = ui.available_rect_before_wrap();
    let painter = ui.painter_at(area);
    let count = view.grade.nodes.len();
    // the chain scaled down to fit the panel, as Resolve's Node Editor zooms out (LOG B-054)
    let room = area.width() - 80.0;
    let scale = (room / ((96.0 + 12.0) * count as f32 + 12.0)).min(1.0);
    let (w, h) = (96.0 * scale, 60.0 * scale);
    let gap = ((room - w * count as f32) / (count as f32 + 1.0)).clamp(12.0 * scale, 40.0);
    let y = area.top() + (area.height() - 30.0).max(h) / 2.0;
    let input = pos2(area.left() + 16.0, y);
    let output = pos2(area.right() - 16.0, y);
    let mut last = input;
    for (i, n) in view.grade.nodes.iter().enumerate() {
        let rect = Rect::from_min_size(pos2(area.left() + 40.0 + gap + (w + gap) * i as f32, y - h / 2.0), vec2(w, h));
        painter.line_segment([last, rect.left_center()], Stroke::new(1.5, t.dim));
        last = rect.right_center();
        let r = ui.interact(rect, ui.id().with(("node", i)), Sense::click());
        name(&r, WidgetType::Button, &format!("Node {:02}", i + 1));
        let current = page.node == i;
        painter.rect_filled(rect, CornerRadius::same(4), if n.enabled { t.field } else { t.ground });
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            if current { Stroke::new(2.0, t.accent) } else { Stroke::new(1.0, t.line) },
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.left_top() + vec2(8.0, 9.0),
            Align2::LEFT_CENTER,
            format!("{:02}", i + 1),
            look.mono(11.0),
            if n.enabled { t.text } else { t.dim },
        );
        let label = if !n.enabled {
            "Off"
        } else if n.label.is_empty() {
            "Corrector"
        } else {
            n.label.as_str()
        };
        // a node too small for its label shows its number alone
        let text = painter.layout_no_wrap(label.to_string(), look.small(), t.dim);
        if text.size().x + 8.0 <= w {
            painter.galley(rect.center() + vec2(0.0, 6.0) - text.size() / 2.0, text, t.dim);
        }
        if !n.is_neutral() {
            painter.circle_filled(rect.right_top() + vec2(-9.0, 9.0), 3.0, t.accent);
        }
        if r.clicked() {
            page.node = i;
        }
        r.context_menu(|ui| {
            for (id, label) in NODE_COMMANDS {
                if ui.button(label).clicked() {
                    page.node = i;
                    asks.push(Ask::Command(id));
                    ui.close();
                }
            }
        });
    }
    painter.line_segment([last, output], Stroke::new(1.5, t.dim));
    for p in [input, output] {
        painter.circle_filled(p, 5.0, t.dim);
    }
    if view.foreign {
        painter.text(
            pos2(area.left() + 12.0, area.bottom() - 14.0),
            Align2::LEFT_CENTER,
            "This clip has Lumetri effects made outside the Color page; grading it replaces them.",
            look.small(),
            t.error,
        );
    }
    ui.allocate_rect(area, Sense::hover());
    asks
}

// ---- the clip strip ----

/// A clip in the strip.
pub struct StripClip {
    pub id: u64,
    pub name: String,
    pub track: String,
    /// Where it starts on the timeline, and the frame its thumbnail shows.
    pub start_frame: i64,
    pub mid_frame: i64,
    pub graded: bool,
}

/// A clip as the Mini-Timeline draws it (LOG D-071): its video track (0 is V1) and its span in
/// seconds.
pub struct MiniClip {
    pub id: u64,
    pub track: usize,
    pub start: f64,
    pub end: f64,
}

/// The height of a Mini-Timeline track's row.
const MINI_ROW: f32 = 12.0;

/// The Mini-Timeline's height for `tracks` video tracks (LOG D-071).
pub fn mini_timeline_height(tracks: usize) -> f32 {
    12.0 + MINI_ROW * tracks.max(1) as f32
}

/// The Mini-Timeline (LOG D-071), as Resolve's Color page has it above the clips: the timeline's
/// video tracks in `rect`, V1 at the bottom, each clip as long as it is, the clip being graded
/// bright and the playhead a line. A click or a drag moves the playhead there.
#[allow(clippy::too_many_arguments)]
pub fn mini_timeline(
    ui: &mut Ui,
    look: &Look,
    rect: Rect,
    clips: &[MiniClip],
    tracks: usize,
    duration: f64,
    playhead: f64,
    rate: resolution_engine_api::Rate,
    page: &ColorPage,
) -> Vec<Ask> {
    let mut asks = Vec::new();
    let t = look.theme;
    let area = rect.shrink2(vec2(8.0, 4.0));
    let lanes = Rect::from_min_max(pos2(area.left() + 28.0, area.top()), area.max);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, t.panel);
    let length = duration.max(1e-6);
    let x_of = |s: f64| lanes.left() + lanes.width() * (s / length).clamp(0.0, 1.0) as f32;
    let row = |track: usize| {
        let bottom = lanes.bottom() - MINI_ROW * track as f32;
        Rect::from_min_max(pos2(lanes.left(), bottom - MINI_ROW + 1.0), pos2(lanes.right(), bottom - 1.0))
    };
    for k in 0..tracks.max(1) {
        let lane = row(k);
        painter.rect_filled(lane, CornerRadius::ZERO, t.field);
        painter.text(
            pos2(area.left(), lane.center().y),
            Align2::LEFT_CENTER,
            format!("V{}", k + 1),
            look.small(),
            t.dim,
        );
    }
    for c in clips {
        let lane = row(c.track);
        // the Edit page's video colour, a point's gap at each cut
        let (x0, x1) = (x_of(c.start), x_of(c.end));
        let r = Rect::from_min_max(pos2(x0, lane.top()), pos2((x1 - 1.0).max(x0 + 1.0), lane.bottom()));
        let current = page.clip == Some(c.id);
        painter.rect_filled(r, CornerRadius::ZERO, if current { t.accent } else { t.video });
    }
    let x = x_of(playhead);
    painter.line_segment([pos2(x, lanes.top() - 2.0), pos2(x, lanes.bottom())], Stroke::new(1.5, t.accent));
    let r = ui.interact(lanes, ui.id().with("mini timeline"), Sense::click_and_drag());
    name(&r, WidgetType::Slider, "Mini-Timeline");
    if (r.clicked() || r.dragged())
        && let Some(p) = r.interact_pointer_pos()
    {
        let s = f64::from(((p.x - lanes.left()) / lanes.width().max(1.0)).clamp(0.0, 1.0)) * length;
        let frame = rate.frame_at(s);
        if frame != rate.frame_at(playhead) {
            asks.push(Ask::Seek(frame));
        }
    }
    asks
}

/// The buttons beside the clips: the Mini-Timeline on and off (LOG D-071).
pub fn strip_buttons(ui: &mut Ui, look: &Look, rect: Rect, page: &mut ColorPage) {
    let b = Rect::from_min_size(rect.left_top() + vec2(6.0, 8.0), vec2(rect.width() - 14.0, look.skin.button));
    if toggle(ui, look, b, "Timeline", "Mini-Timeline on", page.mini_timeline) {
        page.mini_timeline = !page.mini_timeline;
    }
}

/// The clips of the timeline in order, each with a picture from its middle: click one to grade it.
pub fn strip(
    ui: &mut Ui,
    look: &Look,
    clips: &[StripClip],
    page: &ColorPage,
    thumbs: &mut Thumbs,
    playback: Option<&Arc<dyn Playback>>,
) -> Vec<Ask> {
    let mut asks = Vec::new();
    let t = look.theme;
    let ctx = ui.ctx().clone();
    // measured before the scroll area, whose rows are only as tall as a line of text
    let tile_h = (ui.available_height() - 6.0).max(40.0);
    egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            for (n, c) in clips.iter().enumerate() {
                let (rect, r) = ui.allocate_exact_size(vec2(140.0, tile_h), Sense::click());
                let label = format!("Clip {:02} {}", n + 1, c.name);
                name(&r, WidgetType::Button, &label);
                let current = page.clip == Some(c.id);
                let painter = ui.painter_at(rect);
                if current || r.hovered() {
                    painter.rect_filled(rect, look.radius(), if current { t.active_bg } else { t.hover });
                }
                painter.text(
                    rect.left_top() + vec2(8.0, 10.0),
                    Align2::LEFT_CENTER,
                    format!("{:02}", n + 1),
                    look.mono(11.0),
                    if c.graded { t.accent } else { t.text },
                );
                painter.text(rect.right_top() + vec2(-8.0, 10.0), Align2::RIGHT_CENTER, &c.track, look.small(), t.dim);
                let img = Rect::from_min_size(rect.left_top() + vec2(6.0, 20.0), vec2(128.0, 72.0));
                painter.rect_filled(img, CornerRadius::ZERO, Color32::BLACK);
                let stamp = page.stamps.get(&c.id).copied().unwrap_or(0);
                if let Some(p) = playback {
                    thumbs.want((c.id, stamp), c.mid_frame, p);
                }
                if let Some(tex) = thumbs.texture(&ctx, c.id) {
                    let size = tex.size_vec2();
                    let scale = (img.width() / size.x).min(img.height() / size.y);
                    painter.image(
                        tex.id(),
                        Rect::from_center_size(img.center(), size * scale),
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if current {
                    painter.rect_stroke(img, CornerRadius::ZERO, Stroke::new(2.0, t.accent), egui::StrokeKind::Outside);
                }
                painter.with_clip_rect(rect.shrink2(vec2(6.0, 0.0))).text(
                    pos2(rect.left() + 8.0, img.bottom() + 11.0),
                    Align2::LEFT_CENTER,
                    &c.name,
                    look.small(),
                    t.dim,
                );
                if r.clicked() {
                    asks.push(Ask::Seek(c.start_frame));
                }
            }
        });
    });
    asks
}

/// A clip's thumbnail is keyed by the clip and the revision its grade last changed at.
type Key = (u64, u64);

/// A thumbnail to render: its key, the timeline frame, and the timeline to render it from.
type Job = (Key, i64, Arc<dyn Playback>);

/// Thumbnails for the strip: one background thread renders timeline frames small
/// (`Playback::render`); the newest of each clip becomes a texture.
pub struct Thumbs {
    ready: Arc<Mutex<Vec<(Key, ColorImage)>>>,
    asked: HashSet<Key>,
    jobs: Option<Sender<Job>>,
    textures: HashMap<u64, (u64, TextureHandle)>,
}

impl Thumbs {
    /// Start the rendering thread; `repaint` wakes the UI when a thumbnail arrives.
    pub fn new(repaint: impl Fn() + Send + 'static) -> Self {
        let ready: Arc<Mutex<Vec<(Key, ColorImage)>>> = Arc::default();
        let (tx, rx) = mpsc::channel::<Job>();
        let results = ready.clone();
        let thread = std::thread::Builder::new().name("thumbnails".into()).spawn(move || {
            for (key, frame, playback) in rx {
                if let Ok(f) = playback.render(frame, 160, &RenderHints::default()) {
                    let img = ColorImage::from_rgba_unmultiplied([f.width as usize, f.height as usize], &f.rgba);
                    results.lock().unwrap_or_else(|e| e.into_inner()).push((key, img));
                    repaint();
                }
            }
        });
        Self { ready, asked: HashSet::new(), jobs: thread.ok().map(|_| tx), textures: HashMap::new() }
    }

    /// Render `key`'s thumbnail at timeline `frame`, once.
    pub fn want(&mut self, key: Key, frame: i64, playback: &Arc<dyn Playback>) {
        if let Some(jobs) = &self.jobs
            && self.asked.insert(key)
        {
            let _ = jobs.send((key, frame, playback.clone()));
        }
    }

    /// How many clips have a thumbnail on screen.
    #[cfg(test)]
    pub fn shown(&self) -> usize {
        self.textures.len()
    }

    /// The newest thumbnail of `clip` that has arrived.
    pub fn texture(&mut self, ctx: &egui::Context, clip: u64) -> Option<&TextureHandle> {
        let arrived: Vec<(Key, ColorImage)> =
            std::mem::take(&mut *self.ready.lock().unwrap_or_else(|e| e.into_inner()));
        for ((id, stamp), img) in arrived {
            if self.textures.get(&id).is_none_or(|(s, _)| *s <= stamp) {
                let tex = ctx.load_texture(format!("thumb {id}"), img, TextureOptions::LINEAR);
                self.textures.insert(id, (stamp, tex));
            }
        }
        self.textures.get(&clip).map(|(_, t)| t)
    }
}

/// Lay out `f` in `rect`.
pub fn within<R>(ui: &mut Ui, rect: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(UiBuilder::new().max_rect(rect), f).inner
}

// ---- the Scopes ----

/// A scope drawing to make: its number, the picture and the scope.
type ScopeJob = (u64, Arc<Frame>, ScopeKind);

/// The Scopes panel's state: one scope at a time, drawn on a background thread from the frame the
/// timeline viewer shows, so it follows scrubbing, playback and every step of a grade drag. Only
/// the newest picture is drawn; older requests are dropped.
pub struct ScopeView {
    pub kind: ScopeKind,
    jobs: Option<Sender<ScopeJob>>,
    ready: Arc<Mutex<Option<(u64, ScopeKind, ColorImage)>>>,
    asked: u64,
    /// The picture (by address) and scope last asked for.
    last: Option<(usize, ScopeKind)>,
    pub texture: Option<TextureHandle>,
    /// The scope in `texture`.
    pub drawn: Option<ScopeKind>,
}

impl ScopeView {
    /// Start the drawing thread over the engine's scopes (none: the panel stays empty).
    pub fn new(scopes: Option<Arc<dyn Scopes>>, repaint: impl Fn() + Send + 'static) -> Self {
        let ready: Arc<Mutex<Option<(u64, ScopeKind, ColorImage)>>> = Arc::default();
        let jobs = scopes.and_then(|scopes| {
            let (tx, rx) = mpsc::channel::<ScopeJob>();
            let results = ready.clone();
            std::thread::Builder::new()
                .name("scopes".into())
                .spawn(move || {
                    while let Ok(mut job) = rx.recv() {
                        // only the newest picture matters
                        while let Ok(newer) = rx.try_recv() {
                            job = newer;
                        }
                        let (n, frame, kind) = job;
                        let f = scopes.draw(&frame, kind);
                        if f.width > 0 && f.height > 0 {
                            let img =
                                ColorImage::from_rgba_premultiplied([f.width as usize, f.height as usize], &f.rgba);
                            *results.lock().unwrap_or_else(|e| e.into_inner()) = Some((n, kind, img));
                            repaint();
                        }
                    }
                })
                .ok()
                .map(|_| tx)
        });
        Self { kind: ScopeKind::default(), jobs, ready, asked: 0, last: None, texture: None, drawn: None }
    }

    /// Draw the scope of `picture` when it or the chosen scope has changed; put a finished
    /// drawing on screen.
    pub fn follow(&mut self, ctx: &egui::Context, picture: Option<&Arc<Frame>>) {
        if let (Some(jobs), Some(p)) = (&self.jobs, picture) {
            let key = (Arc::as_ptr(p) as usize, self.kind);
            if self.last != Some(key) {
                self.last = Some(key);
                self.asked += 1;
                let _ = jobs.send((self.asked, p.clone(), self.kind));
            }
        }
        if let Some((_, kind, img)) = self.ready.lock().unwrap_or_else(|e| e.into_inner()).take() {
            match &mut self.texture {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => self.texture = Some(ctx.load_texture("scope", img, TextureOptions::LINEAR)),
            }
            self.drawn = Some(kind);
        }
    }
}

/// Rec. 709 Cb and Cr of an RGB colour.
fn cbcr(rgb: [f32; 3]) -> [f32; 2] {
    let y = resolution_grade::luma(rgb);
    [(rgb[2] - y) / 1.8556, (rgb[0] - y) / 1.5748]
}

/// The Scopes panel: the scope buttons in the title strip, the trace, and its graticule (levels
/// in 10-bit code values, 0 to 1023, as Resolve labels them; the vectorscope's 75 % colour-bar
/// targets and skin-tone line).
pub fn scopes_panel(ui: &mut Ui, look: &Look, view: &mut ScopeView) {
    let t = look.theme;
    let measure = ui.painter().clone();
    let width = |text: &str, font| measure.layout_no_wrap(text.into(), font, t.text).size().x;
    let buttons: f32 = ScopeKind::ALL.iter().map(|k| width(k.label(), look.small()) + 12.0 + 3.0).sum();
    // the title gives way to the buttons when the panel is narrow
    let room = ui.available_width() - 20.0 - buttons;
    let header =
        chrome::panel_header(ui, look, if room > width("Scopes", look.medium()) + 12.0 { "Scopes" } else { "" }, None);
    let mut x = header.right() - 10.0;
    for kind in ScopeKind::ALL.into_iter().rev() {
        let w = width(kind.label(), look.small()) + 12.0;
        let b = Rect::from_min_max(pos2(x - w, header.top() + 4.0), pos2(x, header.bottom() - 4.0));
        x -= w + 3.0;
        let r = ui.interact(b, ui.id().with(("scope", kind.label())), Sense::click());
        name(&r, WidgetType::Button, kind.label());
        let on = view.kind == kind;
        ui.painter().rect_filled(
            b,
            look.radius(),
            if on {
                t.active_bg
            } else if r.hovered() {
                t.hover
            } else {
                t.field
            },
        );
        ui.painter().text(
            b.center(),
            Align2::CENTER_CENTER,
            kind.label(),
            look.small(),
            if on { t.accent } else { t.dim },
        );
        if r.clicked() {
            view.kind = kind;
        }
    }
    let area = ui.available_rect_before_wrap().shrink(10.0);
    ui.allocate_rect(area, Sense::hover());
    let painter = ui.painter_at(area.expand(2.0));
    painter.rect_filled(area, CornerRadius::ZERO, Color32::BLACK);
    let line = Stroke::new(1.0, t.line);
    let label = |p: Pos2, a: Align2, s: &str| {
        painter.text(p, a, s, look.mono(10.0), t.dim);
    };
    let kind = view.kind;
    // levels scopes leave room on the left for the numbers
    let plot = match kind {
        ScopeKind::Vectorscope => {
            let side = area.width().min(area.height());
            Rect::from_center_size(area.center(), vec2(side, side))
        }
        ScopeKind::Histogram => Rect::from_min_max(area.min + vec2(4.0, 4.0), area.max - vec2(4.0, 18.0)),
        _ => Rect::from_min_max(area.min + vec2(36.0, 6.0), area.max - vec2(6.0, 18.0)),
    };
    // the trace, under the graticule
    if view.drawn == Some(kind)
        && let Some(tex) = &view.texture
    {
        painter.image(tex.id(), plot, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    match kind {
        ScopeKind::Waveform | ScopeKind::Parade => {
            for (f, code) in [(0.0, "0"), (0.25, "256"), (0.5, "512"), (0.75, "768"), (1.0, "1023")] {
                let y = plot.bottom() - plot.height() * f;
                painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], line);
                label(pos2(plot.left() - 6.0, y), Align2::RIGHT_CENTER, code);
            }
            if kind == ScopeKind::Parade {
                // the panes as the engine lays them out: three equal, PARADE_GAP apart
                let gap = PARADE_GAP as f32 * plot.width() / (3.0 * 480.0 + 2.0 * PARADE_GAP as f32);
                let pane = (plot.width() - 2.0 * gap) / 3.0;
                for (k, (letter, colour)) in [("R", RED), ("G", GREEN), ("B", BLUE)].into_iter().enumerate() {
                    let cx = plot.left() + pane / 2.0 + k as f32 * (pane + gap);
                    painter.text(pos2(cx, plot.bottom() + 9.0), Align2::CENTER_CENTER, letter, look.small(), colour);
                }
            }
        }
        ScopeKind::Histogram => {
            for (f, code) in [(0.0, "0"), (0.25, "256"), (0.5, "512"), (0.75, "768"), (1.0, "1023")] {
                let x = plot.left() + plot.width() * f;
                painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], line);
                let a = if f == 0.0 {
                    Align2::LEFT_CENTER
                } else if f == 1.0 {
                    Align2::RIGHT_CENTER
                } else {
                    Align2::CENTER_CENTER
                };
                label(pos2(x, plot.bottom() + 9.0), a, code);
            }
        }
        ScopeKind::Vectorscope => {
            let c = plot.center();
            let half = plot.width() / 2.0;
            let at = |cb: f32, cr: f32| c + vec2(cb, -cr) / VECTORSCOPE_EXTENT * half;
            painter.circle_stroke(c, half * 0.5 / VECTORSCOPE_EXTENT, line);
            painter.line_segment([c - vec2(half, 0.0), c + vec2(half, 0.0)], line);
            painter.line_segment([c - vec2(0.0, half), c + vec2(0.0, half)], line);
            // skin tone: 123° counter-clockwise from +Cb
            let a = 123f32.to_radians();
            painter.line_segment([c, at(a.cos() * 0.5, a.sin() * 0.5)], Stroke::new(1.0, t.dim));
            for (name, rgb) in [
                ("R", [0.75, 0.0, 0.0]),
                ("Mg", [0.75, 0.0, 0.75]),
                ("B", [0.0, 0.0, 0.75]),
                ("Cy", [0.0, 0.75, 0.75]),
                ("G", [0.0, 0.75, 0.0]),
                ("Yl", [0.75, 0.75, 0.0]),
            ] {
                let [cb, cr] = cbcr(rgb);
                let p = at(cb, cr);
                painter.rect_stroke(
                    Rect::from_center_size(p, vec2(9.0, 9.0)),
                    CornerRadius::ZERO,
                    line,
                    egui::StrokeKind::Middle,
                );
                let out = (p - c).normalized() * 14.0;
                label(p + out, Align2::CENTER_CENTER, name);
            }
        }
    }
    if view.texture.is_none() {
        painter.text(
            area.center(),
            Align2::CENTER_CENTER,
            "Scopes appear with a picture in the viewer.",
            look.small(),
            t.dim,
        );
    }
}

// ---- the centre palettes and the Window palette ----

/// The centre palettes, chosen in their title strip as Resolve's palette bar chooses them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Palette {
    #[default]
    Curves,
    Qualifier,
    Window,
}

/// The centre palettes' title strip: the palettes' names, the one showing bright.
fn palette_header(ui: &mut Ui, look: &Look, page: &mut ColorPage) -> Rect {
    let t = look.theme;
    let header = chrome::panel_header(ui, look, "", None);
    let mut x = header.left() + 4.0;
    for (p, label) in [(Palette::Curves, "Curves"), (Palette::Qualifier, "Qualifier"), (Palette::Window, "Window")] {
        let w = ui.painter().layout_no_wrap(label.into(), look.medium(), t.text).size().x + 16.0;
        let b = Rect::from_min_max(pos2(x, header.top()), pos2(x + w, header.bottom()));
        x += w;
        let r = ui.interact(b, ui.id().with(("palette", label)), Sense::click());
        name(&r, WidgetType::Button, &format!("{label} palette"));
        let on = page.palette == p;
        ui.painter().text(b.center(), Align2::CENTER_CENTER, label, look.medium(), if on { t.text } else { t.dim });
        if on {
            ui.painter().line_segment(
                [pos2(b.left() + 6.0, b.bottom() - 2.0), pos2(b.right() - 6.0, b.bottom() - 2.0)],
                Stroke::new(2.0, t.accent),
            );
        }
        if r.clicked() {
            page.palette = p;
        }
    }
    header
}

/// A window number in the Window palette.
struct WindowField {
    name: &'static str,
    get: fn(&Window) -> f32,
    set: fn(&mut Window, f32),
    step: f32,
    decimals: usize,
}

/// Pan and Tilt in percent of the picture (50 is the middle; Tilt rises upwards); Size is the
/// width in percent of the picture's width, Aspect the height to the width (50 is round or
/// square); Rotate in degrees clockwise; Softness in percent of the picture's width, and in its
/// place a Polygon's or a Curve's In Soft and Out Soft (LOG D-063, D-069) and a Linear window's
/// Soft 1 to 4, its top, bottom, left and right (LOG D-065).
const WINDOW_FIELDS: [WindowField; 13] = [
    WindowField {
        name: "Pan",
        get: |w| w.center[0] * 100.0,
        set: |w, v| w.center[0] = v / 100.0,
        step: 0.1,
        decimals: 1,
    },
    WindowField {
        name: "Tilt",
        get: |w| (1.0 - w.center[1]) * 100.0,
        set: |w, v| w.center[1] = 1.0 - v / 100.0,
        step: 0.1,
        decimals: 1,
    },
    WindowField {
        name: "Size",
        get: |w| w.size[0] * 100.0,
        set: |w, v| {
            let aspect = w.size[1] / w.size[0].max(1e-4);
            w.size[0] = (v / 100.0).max(0.002);
            w.size[1] = w.size[0] * aspect;
        },
        step: 0.2,
        decimals: 1,
    },
    WindowField {
        name: "Aspect",
        get: |w| 50.0 * w.size[1] / w.size[0].max(1e-4),
        set: |w, v| w.size[1] = w.size[0] * (v / 50.0).max(0.01),
        step: 0.2,
        decimals: 1,
    },
    WindowField { name: "Rotate", get: |w| w.rotation, set: |w, v| w.rotation = v, step: 0.5, decimals: 1 },
    WindowField {
        name: "Softness",
        get: |w| w.softness * 100.0,
        set: |w, v| w.softness = v / 100.0,
        step: 0.05,
        decimals: 2,
    },
    WindowField {
        name: "In Soft",
        get: |w| w.edge_softness().0 * 100.0,
        set: |w, v| w.inside_softness = Some((v / 100.0).clamp(0.0, 1.0)),
        step: 0.05,
        decimals: 2,
    },
    WindowField {
        name: "Out Soft",
        get: |w| w.edge_softness().1 * 100.0,
        set: |w, v| w.outside_softness = Some((v / 100.0).clamp(0.0, 1.0)),
        step: 0.05,
        decimals: 2,
    },
    WindowField { name: "Opacity", get: |w| w.opacity, set: |w, v| w.opacity = v, step: 0.5, decimals: 1 },
    WindowField {
        name: "Soft 1",
        get: |w| w.side_softnesses()[0] * 100.0,
        set: |w, v| set_side(w, 0, v),
        step: 0.05,
        decimals: 2,
    },
    WindowField {
        name: "Soft 2",
        get: |w| w.side_softnesses()[1] * 100.0,
        set: |w, v| set_side(w, 1, v),
        step: 0.05,
        decimals: 2,
    },
    WindowField {
        name: "Soft 3",
        get: |w| w.side_softnesses()[2] * 100.0,
        set: |w, v| set_side(w, 2, v),
        step: 0.05,
        decimals: 2,
    },
    WindowField {
        name: "Soft 4",
        get: |w| w.side_softnesses()[3] * 100.0,
        set: |w, v| set_side(w, 3, v),
        step: 0.05,
        decimals: 2,
    },
];

/// A Linear window with side `k`'s softness (top, bottom, left, right) at `percent` of the
/// picture's width, the others as they were.
fn set_side(w: &mut Window, k: usize, percent: f32) {
    let mut s = w.side_softnesses();
    s[k] = (percent / 100.0).clamp(0.0, 1.0);
    w.side_softness = Some(s);
}

fn shape_name(s: WindowShape) -> &'static str {
    match s {
        WindowShape::Linear => "Linear",
        WindowShape::Circle => "Circle",
        WindowShape::Gradient => "Gradient",
        WindowShape::Polygon => "Polygon",
        WindowShape::Curve => "Curve",
    }
}

/// The Window palette: add a Linear, Circle, Polygon or Gradient window to the current node; each window's row
/// chooses it and turns it on or off, inverts or deletes it; the chosen window's numbers below.
/// The viewer shows the windows' outlines while this palette is open.
pub fn windows(ui: &mut Ui, look: &Look, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let t = look.theme;
    palette_header(ui, look, page);
    let Some(node) = page.current().cloned() else { return asks };
    let mut wins = node.windows.clone();
    match page.new_window {
        // the window added stays chosen until a grade from the engine has it (`take`): the grade
        // shown here has it at once, patched locally, and answers from before it can still come
        Some((node, _)) if node == page.node => {}
        _ => {
            page.new_window = None;
            page.window = page.window.min(wins.len().saturating_sub(1));
        }
    }
    let set = |page: &mut ColorPage, asks: &mut Vec<Ask>, wins: &[Window]| {
        page.adjust(asks, json!({ "windows": wins }));
    };
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        for shape in
            [WindowShape::Linear, WindowShape::Circle, WindowShape::Polygon, WindowShape::Curve, WindowShape::Gradient]
        {
            if chrome::text_button(ui, look, &format!("Add {}", shape_name(shape)), true).clicked() {
                wins.push(Window::new(shape));
                page.window = wins.len() - 1;
                page.new_window = Some((page.node, page.window));
                set(page, &mut asks, &wins);
            }
        }
    });
    ui.add_space(4.0);
    let mut changed = false;
    let mut delete = None;
    // the list scrolls past four windows, so the palette keeps its height and the viewer its size
    // (LOG B-052)
    let row = look.skin.button + ui.spacing().item_spacing.y;
    egui::ScrollArea::vertical().id_salt("window list").max_height(4.0 * row).auto_shrink([false, true]).show(
        ui,
        |ui| {
            for (i, w) in wins.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let label = format!("Window {}", i + 1);
                    let (rect, r) = ui.allocate_exact_size(vec2(110.0, look.skin.button), Sense::click());
                    name(&r, WidgetType::Button, &label);
                    let chosen = page.window == i;
                    ui.painter().rect_filled(
                        rect,
                        look.radius(),
                        if chosen {
                            t.active_bg
                        } else if r.hovered() {
                            t.hover
                        } else {
                            t.field
                        },
                    );
                    ui.painter().text(
                        rect.left_center() + vec2(8.0, 0.0),
                        Align2::LEFT_CENTER,
                        format!("{} {}", shape_name(w.shape), i + 1),
                        look.small(),
                        if chosen { t.accent } else { t.text },
                    );
                    if r.clicked() {
                        page.window = i;
                    }
                    for (text, on, what) in [("On", w.enabled, 0), ("Invert", w.invert, 1)] {
                        let (b, r) = ui.allocate_exact_size(vec2(52.0, look.skin.button), Sense::click());
                        name(&r, WidgetType::Button, &format!("{label} {text}"));
                        ui.painter().rect_filled(
                            b,
                            look.radius(),
                            if on {
                                t.active_bg
                            } else if r.hovered() {
                                t.hover
                            } else {
                                t.field
                            },
                        );
                        ui.painter().text(
                            b.center(),
                            Align2::CENTER_CENTER,
                            text,
                            look.small(),
                            if on { t.accent } else { t.dim },
                        );
                        if r.clicked() {
                            if what == 0 {
                                w.enabled = !w.enabled;
                            } else {
                                w.invert = !w.invert;
                            }
                            changed = true;
                        }
                    }
                    if chrome::text_button(ui, look, &format!("Delete {label}"), true).clicked() {
                        delete = Some(i);
                    }
                });
            }
        },
    );
    if let Some(i) = delete {
        wins.remove(i);
        page.window = page.window.min(wins.len().saturating_sub(1));
        changed = true;
    }
    if changed {
        set(page, &mut asks, &wins);
    }
    // the chosen window's numbers, in rows of four
    if let Some(w) = wins.get(page.window).cloned() {
        ui.add_space(6.0);
        let area = ui.available_rect_before_wrap();
        let row_h = 26.0;
        let cell_w = (area.width() - 16.0) / 4.0;
        let fresh = Window::new(w.shape);
        let cells = field_cells(w.shape);
        let rows = cells.iter().map(|c| c.1 + 1).max().unwrap_or(0);
        for (f, row, col) in cells {
            let cell = Rect::from_min_size(
                pos2(area.left() + 8.0 + cell_w * col as f32, area.top() + row_h * row as f32),
                vec2(cell_w, row_h),
            )
            .shrink2(vec2(3.0, 2.0));
            let v = (f.get)(&w);
            let r = number(ui, look, cell, f.name, &format!("{v:.*}", f.decimals), (v - (f.get)(&fresh)).abs() > 1e-4);
            if r.drag_started() {
                page.begin_drag();
            }
            let mut nw = w.clone();
            if r.double_clicked() {
                (f.set)(&mut nw, (f.get)(&fresh));
            } else if r.dragged() && r.drag_delta().x != 0.0 {
                (f.set)(&mut nw, v + r.drag_delta().x * f.step);
            }
            if nw != w {
                wins[page.window] = nw;
                set(page, &mut asks, &wins);
            }
            if r.drag_stopped() {
                page.end_drag();
            }
        }
        ui.allocate_rect(Rect::from_min_size(area.min, vec2(area.width(), row_h * rows as f32)), Sense::hover());
    }
    asks
}

/// Where a clip's picture sits on screen: the timeline's frame drawn in `screen`, and a clip of
/// `clip` pixels fitted into the frame and centred, as clips are placed by default.
fn clip_on_screen(screen: Rect, frame: (u32, u32), clip: (u32, u32)) -> Rect {
    let (fw, fh) = (frame.0.max(1) as f32, frame.1.max(1) as f32);
    let (cw, ch) = (clip.0.max(1) as f32, clip.1.max(1) as f32);
    let fit = (fw / cw).min(fh / ch);
    let k = screen.width() / fw;
    Rect::from_center_size(screen.center(), vec2(cw * fit * k, ch * fit * k))
}

/// A window's outline on screen, for a clip picture drawn in `pic`.
fn outline(w: &Window, pic: Rect) -> Vec<Pos2> {
    let c = pic.min + vec2(w.center[0] * pic.width(), w.center[1] * pic.height());
    let (rx, ry) = (w.size[0] * pic.width() / 2.0, w.size[1] * pic.width() / 2.0);
    let (sin, cos) = w.rotation.to_radians().sin_cos();
    let turn = |x: f32, y: f32| c + vec2(x * cos - y * sin, x * sin + y * cos);
    match w.shape {
        WindowShape::Circle => {
            (0..72).map(|i| i as f32 / 72.0 * TAU).map(|a| turn(rx * a.cos(), ry * a.sin())).collect()
        }
        WindowShape::Linear => vec![turn(-rx, -ry), turn(rx, -ry), turn(rx, ry), turn(-rx, ry)],
        WindowShape::Polygon => w.points.iter().map(|p| turn(p[0] * rx, p[1] * ry)).collect(),
        WindowShape::Curve => {
            resolution_grade::curve_corners(&w.points).iter().map(|p| turn(p[0] * rx, p[1] * ry)).collect()
        }
        // the keyed side, reaching well past the picture: on screen, its line across it
        WindowShape::Gradient => {
            let far = 4.0 * pic.width().max(pic.height());
            vec![turn(-far, -far), turn(far, -far), turn(far, 0.0), turn(-far, 0.0)]
        }
    }
}

/// A handle of the chosen window on screen (LOG D-057).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    /// The right side: the width.
    Width,
    /// The top: the height.
    Height,
    /// A corner: both, keeping the aspect.
    Scale,
    /// Above the window: the rotation.
    Rotate,
    /// Below it: the softness.
    Soft,
    /// Beside a Linear window's side (0 to 3: top, bottom, left, right): that side's softness
    /// (LOG D-065).
    Side(usize),
}

/// The numbers a window of shape `s` shows: a gradient has no size, and a Polygon's or a Curve's
/// softness is its In Soft and Out Soft (LOG D-063, D-069).
fn window_fields(s: WindowShape) -> impl Iterator<Item = &'static WindowField> {
    let points = s.has_points();
    WINDOW_FIELDS.iter().filter(move |f| match f.name {
        "Size" | "Aspect" => s != WindowShape::Gradient,
        "Softness" => !points && s != WindowShape::Linear,
        "In Soft" | "Out Soft" => points,
        "Soft 1" | "Soft 2" | "Soft 3" | "Soft 4" => s == WindowShape::Linear,
        _ => true,
    })
}

/// Where each of a window's numbers sits in the palette, as (field, row, column) in rows of four:
/// the softness after the rest, starting a row of its own when it would not fit on the last one
/// (a Linear window's four, LOG D-065).
fn field_cells(s: WindowShape) -> Vec<(&'static WindowField, usize, usize)> {
    let (soft, rest): (Vec<_>, Vec<_>) =
        window_fields(s).partition(|f| f.name.starts_with("Soft") || f.name.ends_with("Soft"));
    let mut k = rest.len();
    if k % 4 + soft.len() > 4 {
        k = k.div_ceil(4) * 4;
    }
    let at = |i: usize| (i / 4, i % 4);
    rest.into_iter()
        .enumerate()
        .map(|(i, f)| (f, at(i).0, at(i).1))
        .chain(soft.into_iter().enumerate().map(|(i, f)| (f, at(k + i).0, at(k + i).1)))
        .collect()
}

/// Pixels between a window's edge and its rotate and softness handles.
const HANDLE_GAP: f32 = 28.0;

/// The chosen window's handles on screen, for a clip picture drawn in `pic`: a Gradient has no
/// size, and its rotate handle is at the tip of its tick.
fn handles(w: &Window, pic: Rect) -> Vec<(Handle, Pos2)> {
    let c = pic.min + vec2(w.center[0] * pic.width(), w.center[1] * pic.height());
    let (sin, cos) = w.rotation.to_radians().sin_cos();
    let at = |x: f32, y: f32| c + vec2(x * cos - y * sin, x * sin + y * cos);
    // the soft edge's reach out of the shape: half the softness, or a Polygon's Outside Soft
    let soft = w.edge_softness().1 * pic.width();
    if w.shape == WindowShape::Gradient {
        return vec![(Handle::Rotate, at(0.0, -HANDLE_GAP)), (Handle::Soft, at(0.0, HANDLE_GAP / 2.0 + soft))];
    }
    let (rx, ry) = (w.size[0] * pic.width() / 2.0, w.size[1] * pic.width() / 2.0);
    let mut grips = vec![
        (Handle::Width, at(rx, 0.0)),
        (Handle::Height, at(0.0, -ry)),
        (Handle::Scale, at(rx, ry)),
        (Handle::Rotate, at(0.0, -ry - HANDLE_GAP)),
    ];
    if w.shape == WindowShape::Linear {
        // each side's softness beside it, as far out as its soft edge reaches; the top one aside,
        // clear of the rotate handle (LOG D-065)
        let s = w.side_softnesses().map(|v| v * pic.width() / 2.0);
        let g = HANDLE_GAP / 2.0;
        grips.extend([
            (Handle::Side(0), at(rx / 2.0, -ry - g - s[0])),
            (Handle::Side(1), at(0.0, ry + g + s[1])),
            (Handle::Side(2), at(-rx - g - s[2], 0.0)),
            (Handle::Side(3), at(rx + g + s[3], 0.0)),
        ]);
    } else {
        grips.push((Handle::Soft, at(0.0, ry + HANDLE_GAP / 2.0 + soft)));
    }
    grips
}

/// The window `w` with handle `h` dragged to `p` on screen.
fn dragged(w: &Window, pic: Rect, h: Handle, p: Pos2) -> Window {
    let mut w = w.clone();
    let c = pic.min + vec2(w.center[0] * pic.width(), w.center[1] * pic.height());
    let d = p - c;
    let (sin, cos) = w.rotation.to_radians().sin_cos();
    // in the window's own frame, in pixels
    let (lx, ly) = (d.x * cos + d.y * sin, -d.x * sin + d.y * cos);
    let pw = pic.width().max(1.0);
    let ry = if w.shape == WindowShape::Gradient { 0.0 } else { w.size[1] * pw / 2.0 };
    match h {
        Handle::Width => w.size[0] = (2.0 * lx.abs() / pw).max(0.002),
        Handle::Height => w.size[1] = (2.0 * ly.abs() / pw).max(0.002),
        Handle::Scale => {
            let now = (w.size[0].hypot(w.size[1]) * pw / 2.0).max(1e-3);
            let k = lx.hypot(ly) / now;
            w.size = w.size.map(|s| (s * k).max(0.002));
        }
        // 0 straight up, clockwise
        Handle::Rotate => w.rotation = d.x.atan2(-d.y).to_degrees(),
        Handle::Side(k) => {
            let rx = w.size[0] * pw / 2.0;
            // how far out from that side, past the gap: half its softness
            let out = match k {
                0 => -ly - ry,
                1 => ly - ry,
                2 => -lx - rx,
                _ => lx - rx,
            };
            set_side(&mut w, k, 200.0 * (out - HANDLE_GAP / 2.0) / pw);
        }
        Handle::Soft => {
            let out = ((ly - ry - HANDLE_GAP / 2.0) / pw).clamp(0.0, 1.0);
            if w.shape.has_points() {
                w.outside_softness = Some(out);
            } else {
                w.softness = (out * 2.0).min(1.0);
            }
        }
    }
    w
}

/// A point in a window's own space (see `Window::points`) on screen, for a clip picture drawn in
/// `pic`; and back.
fn window_to_screen(w: &Window, pic: Rect, p: [f32; 2]) -> Pos2 {
    let c = pic.min + vec2(w.center[0] * pic.width(), w.center[1] * pic.height());
    let (x, y) = (p[0] * w.size[0] * pic.width() / 2.0, p[1] * w.size[1] * pic.width() / 2.0);
    let (sin, cos) = w.rotation.to_radians().sin_cos();
    c + vec2(x * cos - y * sin, x * sin + y * cos)
}

fn screen_to_window(w: &Window, pic: Rect, s: Pos2) -> [f32; 2] {
    let d = s - (pic.min + vec2(w.center[0] * pic.width(), w.center[1] * pic.height()));
    let (sin, cos) = w.rotation.to_radians().sin_cos();
    let (x, y) = (d.x * cos + d.y * sin, -d.x * sin + d.y * cos);
    let (rx, ry) = ((w.size[0] * pic.width() / 2.0).max(1e-3), (w.size[1] * pic.width() / 2.0).max(1e-3));
    [x / rx, y / ry]
}

/// Whether `p` is inside the closed outline `poly`.
fn inside(poly: &[Pos2], p: Pos2) -> bool {
    let mut odd = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}

/// The windows' on-screen control over the viewer's picture `screen` (the timeline's frame of
/// `frame` pixels): each window's outline, the chosen one bright. Drag inside a window to move it
/// (and choose it). A chosen polygon shows its corners, as Resolve's does: drag one to move it,
/// click the polygon's surface to add a corner on its nearest side, middle-click a corner to
/// remove it (LOG D-053). The chosen window's handles change its size (right side, top, a corner),
/// its rotation (above it) and its softness (below it) (LOG D-057).
pub fn window_overlay(ui: &mut Ui, look: &Look, screen: Rect, frame: (u32, u32), page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let Some(view) = page.view.clone() else { return asks };
    let Some(node) = view.grade.nodes.get(page.node) else { return asks };
    let pic = clip_on_screen(screen, frame, view.size);
    let painter = ui.painter_at(screen);
    let outlines: Vec<Vec<Pos2>> = node.windows.iter().map(|w| outline(w, pic)).collect();
    for (i, o) in outlines.iter().enumerate() {
        let chosen = i == page.window;
        let colour = if chosen { look.theme.accent } else { Color32::from_white_alpha(150) };
        let stroke = Stroke::new(if chosen { 2.0 } else { 1.0 }, colour);
        painter.add(Shape::closed_line(o.clone(), stroke));
        // a gradient's centre, and a tick towards the side it keys
        if let Some(w) = node.windows.get(i).filter(|w| w.shape == WindowShape::Gradient) {
            let c = pic.min + vec2(w.center[0] * pic.width(), w.center[1] * pic.height());
            let (sin, cos) = w.rotation.to_radians().sin_cos();
            painter.circle_stroke(c, 5.0, stroke);
            painter.line_segment([c, c + vec2(sin, -cos) * 28.0], stroke);
        }
    }
    // the chosen polygon's corners, as handles
    // a polygon's or a curve's points (LOG D-053, D-061)
    let polygon = node.windows.get(page.window).filter(|w| w.shape.has_points());
    let corners: Vec<Pos2> =
        polygon.map(|w| w.points.iter().map(|p| window_to_screen(w, pic, *p)).collect()).unwrap_or_default();
    for c in &corners {
        let handle = Rect::from_center_size(*c, vec2(7.0, 7.0));
        painter.rect_filled(handle, CornerRadius::ZERO, look.theme.accent);
    }
    let near = |p: Pos2| corners.iter().position(|c| c.distance(p) <= 8.0);
    // the chosen window's handles: squares for its size, a circle to turn it, a ring for softness
    let grips = node.windows.get(page.window).map(|w| handles(w, pic)).unwrap_or_default();
    for (h, at) in &grips {
        let _ = match h {
            Handle::Rotate => painter.circle_filled(*at, 4.5, look.theme.text),
            Handle::Soft | Handle::Side(_) => painter.circle_stroke(*at, 5.0, Stroke::new(1.5, look.theme.text)),
            _ => painter.rect_filled(Rect::from_center_size(*at, vec2(7.0, 7.0)), CornerRadius::ZERO, look.theme.text),
        };
    }
    let grip = |p: Pos2| grips.iter().find(|(_, at)| at.distance(p) <= 8.0).map(|(h, _)| *h);
    let r = ui.interact(screen, ui.id().with("window osc"), Sense::click_and_drag());
    name(&r, WidgetType::Other, "Window on-screen control");
    // what a drag takes hold of is where it was pressed: egui starts it a few pixels on (LOG D-057)
    let pressed = ui.input(|i| i.pointer.press_origin()).or(r.interact_pointer_pos());
    if r.drag_started()
        && let Some(p) = pressed
        && let Some(k) = near(p)
    {
        page.begin_drag();
        page.moving_point = Some(k);
    } else if r.drag_started()
        && let Some(p) = pressed
        && let Some(h) = grip(p)
    {
        page.begin_drag();
        page.moving_handle = Some(h);
    } else if r.drag_started()
        && let Some(p) = pressed
    {
        // the chosen window first, then the others, top one last drawn
        let hit = std::iter::once(page.window)
            .chain((0..outlines.len()).rev())
            .find(|&i| outlines.get(i).is_some_and(|o| inside(o, p)));
        if let Some(i) = hit {
            page.window = i;
            page.begin_drag();
            page.moving_window = true;
        }
    }
    if page.moving_window && r.dragged() && r.drag_delta() != egui::Vec2::ZERO {
        let mut wins = node.windows.clone();
        if let Some(w) = wins.get_mut(page.window) {
            w.center[0] += r.drag_delta().x / pic.width();
            w.center[1] += r.drag_delta().y / pic.height();
            page.adjust(&mut asks, json!({ "windows": wins }));
        }
    }
    if let (Some(k), Some(w), true) = (page.moving_point, polygon, r.dragged())
        && let Some(p) = r.interact_pointer_pos()
    {
        let mut wins = node.windows.clone();
        if let Some(corner) = wins.get_mut(page.window).and_then(|w| w.points.get_mut(k)) {
            *corner = screen_to_window(w, pic, p);
            page.adjust(&mut asks, json!({ "windows": wins }));
        }
    }
    if let (Some(w), Some(p)) = (polygon, r.interact_pointer_pos()) {
        let mut points = w.points.clone();
        if r.clicked()
            && near(p).is_none()
            && grip(p).is_none()
            && outlines.get(page.window).is_some_and(|o| inside(o, p))
        {
            // a corner on the nearest side, where the click falls across it
            let n = corners.len();
            let side = |i: usize| {
                let (a, b) = (corners[i], corners[(i + 1) % n]);
                let e = b - a;
                let t = ((p - a).dot(e) / e.length_sq().max(1e-6)).clamp(0.0, 1.0);
                (a + e * t, (a + e * t).distance(p))
            };
            if let Some(i) = (0..n).min_by(|&i, &j| side(i).1.total_cmp(&side(j).1)) {
                points.insert(i + 1, screen_to_window(w, pic, side(i).0));
            }
        } else if r.middle_clicked()
            && let Some(k) = near(p)
            && points.len() > 3
        {
            points.remove(k);
        }
        if points != w.points {
            let mut wins = node.windows.clone();
            wins[page.window].points = points;
            page.adjust(&mut asks, json!({ "windows": wins }));
        }
    }
    if let (Some(h), true) = (page.moving_handle, r.dragged())
        && let Some(p) = r.interact_pointer_pos()
        && let Some(w) = node.windows.get(page.window)
    {
        let moved = dragged(w, pic, h, p);
        if moved != *w {
            let mut wins = node.windows.clone();
            wins[page.window] = moved;
            page.adjust(&mut asks, json!({ "windows": wins }));
        }
    }
    if r.drag_stopped() && (page.moving_window || page.moving_point.is_some() || page.moving_handle.is_some()) {
        page.end_drag();
    }
    asks
}

// ---- the Qualifier palette ----

/// A qualifier number: its name in the palette, and where it lives.
struct QualifierField {
    name: &'static str,
    get: fn(&Qualifier) -> f32,
    set: fn(&mut Qualifier, f32),
}

/// The three columns of the Qualifier palette: each section's switch, then its numbers.
type Section = (&'static str, fn(&Qualifier) -> bool, fn(&mut Qualifier), &'static [QualifierField]);

const SECTIONS: [Section; 3] = [
    (
        "Hue",
        |q| q.hue_on,
        |q| q.hue_on = !q.hue_on,
        &[
            QualifierField { name: "Hue Center", get: |q| q.hue_center, set: |q, v| q.hue_center = v },
            QualifierField { name: "Hue Width", get: |q| q.hue_width, set: |q, v| q.hue_width = v },
            QualifierField { name: "Hue Soft", get: |q| q.hue_soft, set: |q, v| q.hue_soft = v },
        ],
    ),
    (
        "Sat",
        |q| q.sat_on,
        |q| q.sat_on = !q.sat_on,
        &[
            QualifierField { name: "Sat Low", get: |q| q.sat_low, set: |q, v| q.sat_low = v },
            QualifierField { name: "Sat High", get: |q| q.sat_high, set: |q, v| q.sat_high = v },
            QualifierField { name: "Sat Low Soft", get: |q| q.sat_low_soft, set: |q, v| q.sat_low_soft = v },
            QualifierField { name: "Sat High Soft", get: |q| q.sat_high_soft, set: |q, v| q.sat_high_soft = v },
        ],
    ),
    (
        "Lum",
        |q| q.lum_on,
        |q| q.lum_on = !q.lum_on,
        &[
            QualifierField { name: "Lum Low", get: |q| q.lum_low, set: |q, v| q.lum_low = v },
            QualifierField { name: "Lum High", get: |q| q.lum_high, set: |q, v| q.lum_high = v },
            QualifierField { name: "Lum Low Soft", get: |q| q.lum_low_soft, set: |q, v| q.lum_low_soft = v },
            QualifierField { name: "Lum High Soft", get: |q| q.lum_high_soft, set: |q, v| q.lum_high_soft = v },
        ],
    ),
];

/// Matte Finesse (LOG D-046, D-058): the key cleaned, in this order; Morph Operation's button
/// follows them.
const MATTE: [QualifierField; 7] = [
    QualifierField { name: "Denoise", get: |q| q.denoise, set: |q, v| q.denoise = v },
    QualifierField { name: "Clean Black", get: |q| q.clean_black, set: |q, v| q.clean_black = v },
    QualifierField { name: "Clean White", get: |q| q.clean_white, set: |q, v| q.clean_white = v },
    QualifierField { name: "Morph Radius", get: |q| q.morph_radius, set: |q, v| q.morph_radius = v },
    QualifierField { name: "Blur Radius", get: |q| q.blur, set: |q, v| q.blur = v },
    QualifierField { name: "Black Clip", get: |q| q.black_clip, set: |q, v| q.black_clip = v },
    QualifierField { name: "White Clip", get: |q| q.white_clip, set: |q, v| q.white_clip = v },
];

/// Morph Operation's choices, in the order its button steps through them (LOG D-058).
const MORPHS: [(Morph, &str); 5] = [
    (Morph::None, "None"),
    (Morph::Shrink, "Shrink"),
    (Morph::Grow, "Grow"),
    (Morph::Opening, "Opening"),
    (Morph::Closing, "Closing"),
];

/// A small on/off button.
fn toggle(ui: &mut Ui, look: &Look, rect: Rect, label: &str, accessible: &str, on: bool) -> bool {
    let t = look.theme;
    let r = ui.interact(rect, ui.id().with(("toggle", accessible)), Sense::click());
    name(&r, WidgetType::Button, accessible);
    ui.painter().rect_filled(
        rect,
        look.radius(),
        if on {
            t.active_bg
        } else if r.hovered() {
            t.hover
        } else {
            t.field
        },
    );
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, look.small(), if on { t.accent } else { t.dim });
    r.clicked()
}

/// The Qualifier palette: the current node's HSL key. With none, Add Qualifier (or click the
/// viewer to pick a colour); with one, On, Invert and Remove, then the Hue, Saturation and
/// Luminance ranges, each section switched on or off, and Matte Finesse.
pub fn qualifier(ui: &mut Ui, look: &Look, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let t = look.theme;
    palette_header(ui, look, page);
    // the palette's own width: a row that runs over it must not widen the columns (LOG B-051)
    let width = ui.max_rect().width();
    let Some(node) = page.current().cloned() else { return asks };
    ui.add_space(6.0);
    let Some(q) = node.qualifier else {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if chrome::text_button(ui, look, "Add Qualifier", true).clicked() {
                page.adjust(&mut asks, json!({ "qualifier": Qualifier::default() }));
            }
            ui.label(egui::RichText::new("or click the viewer to pick a colour.").color(t.dim));
        });
        return asks;
    };
    let mut nq = q;
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        for (text, on, flip) in [("On", q.enabled, 0), ("Invert", q.invert, 1)] {
            let (b, _) = ui.allocate_exact_size(vec2(60.0, look.skin.button), Sense::hover());
            if toggle(ui, look, b, text, &format!("Qualifier {text}"), on) {
                if flip == 0 {
                    nq.enabled = !nq.enabled;
                } else {
                    nq.invert = !nq.invert;
                }
            }
        }
        // the qualifier's mode, as Resolve's palette chooses it (LOG D-072): Luma is HSL with Hue
        // and Sat off, so the mode shown is what the switches say
        let luma = !q.hue_on && !q.sat_on && q.lum_on;
        for (text, on, set_luma) in [("HSL", !luma, false), ("Luma", luma, true)] {
            let (b, _) = ui.allocate_exact_size(vec2(52.0, look.skin.button), Sense::hover());
            if toggle(ui, look, b, text, &format!("Qualifier {text}"), on) {
                (nq.hue_on, nq.sat_on, nq.lum_on) = (!set_luma, !set_luma, true);
            }
        }
        if chrome::text_button(ui, look, "Remove Qualifier", true).clicked() {
            remove = true;
        }
        ui.add(egui::Label::new(egui::RichText::new("Click the viewer to pick a colour.").color(t.dim)).truncate());
    });
    ui.add_space(6.0);
    let area = ui.available_rect_before_wrap();
    let area = Rect::from_min_size(area.min, vec2(width.min(area.width()), area.height()));
    let col_w = (area.width() - 16.0) / 4.0;
    let row_h = 24.0;
    let fresh = Qualifier::default();
    let matte = ("Matte Finesse", &MATTE[..]);
    let columns = SECTIONS.iter().map(|(title, _, _, fields)| (*title, &fields[..])).chain([matte]);
    for (c, (title, fields)) in columns.enumerate() {
        // Matte Finesse's names are the longest: its column a third wider than the others
        let (x, col_w) = if c < 3 {
            (area.left() + 8.0 + col_w * 0.9 * c as f32, col_w * 0.9)
        } else {
            (area.left() + 8.0 + col_w * 2.7, col_w * 1.3)
        };
        let head = Rect::from_min_size(pos2(x, area.top()), vec2(col_w, row_h)).shrink2(vec2(3.0, 2.0));
        match SECTIONS.get(c) {
            Some((_, is_on, flip, _)) => {
                if toggle(ui, look, head, title, &format!("{title} Range"), is_on(&q)) {
                    flip(&mut nq);
                }
            }
            None => {
                ui.painter().text(head.center(), Align2::CENTER_CENTER, title, look.small(), t.dim);
            }
        }
        for (k, f) in fields.iter().enumerate() {
            let cell = Rect::from_min_size(pos2(x, area.top() + row_h * (k + 1) as f32), vec2(col_w, row_h))
                .shrink2(vec2(3.0, 2.0));
            let v = (f.get)(&q);
            let shown = f.name.strip_prefix(title).map_or(f.name, str::trim_start);
            let r = number_shown(ui, look, cell, f.name, shown, &format!("{v:.1}"), (v - (f.get)(&fresh)).abs() > 1e-3);
            if r.drag_started() {
                page.begin_drag();
            }
            if r.double_clicked() {
                (f.set)(&mut nq, (f.get)(&fresh));
            } else if r.dragged() && r.drag_delta().x != 0.0 {
                (f.set)(&mut nq, v + r.drag_delta().x * 0.1);
            }
            if r.drag_stopped() {
                page.end_drag();
            }
        }
        // after Matte Finesse's numbers, Morph Operation: a click steps to the next
        if SECTIONS.get(c).is_none() {
            let cell = Rect::from_min_size(pos2(x, area.top() + row_h * (fields.len() + 1) as f32), vec2(col_w, row_h))
                .shrink2(vec2(3.0, 2.0));
            let k = MORPHS.iter().position(|(m, _)| *m == q.morph).unwrap_or(0);
            let label = format!("Morph: {}", MORPHS[k].1);
            if toggle(ui, look, cell, &label, "Morph Operation", !q.morph.is_none()) {
                nq.morph = MORPHS[(k + 1) % MORPHS.len()].0;
            }
        }
    }
    ui.allocate_rect(Rect::from_min_size(area.min, vec2(area.width(), row_h * 9.0)), Sense::hover());
    if remove {
        page.adjust(&mut asks, json!({ "qualifier": Value::Null }));
    } else if nq != q {
        page.adjust(&mut asks, json!({ "qualifier": nq }));
    }
    asks
}

/// The qualifier picker over the viewer's picture `screen` (showing `picture`): a click takes the
/// colour under the pointer as the node's key. It reads the picture as the viewer shows it, after
/// the grade.
pub fn qualifier_picker(ui: &mut Ui, screen: Rect, picture: Option<&Arc<Frame>>, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let r = ui.interact(screen, ui.id().with("qualifier picker"), Sense::click());
    name(&r, WidgetType::Other, "Qualifier picker");
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if r.clicked()
        && page.current().is_some()
        && let (Some(p), Some(f)) = (r.interact_pointer_pos(), picture)
    {
        let u = ((p.x - screen.left()) / screen.width()).clamp(0.0, 0.999);
        let v = ((p.y - screen.top()) / screen.height()).clamp(0.0, 0.999);
        let (x, y) = ((u * f.width as f32) as usize, (v * f.height as f32) as usize);
        let i = (y * f.width as usize + x) * 4;
        if let Some(px) = f.pixels().get(i..i + 3) {
            let rgb = [px[0], px[1], px[2]].map(|c| f32::from(c) / 255.0);
            page.adjust(&mut asks, json!({ "qualifier": Qualifier::picked(rgb) }));
        }
    }
    asks
}

/// The hue curves' sampler over the viewer's picture `screen` (showing `picture`), while one is
/// open (LOG D-066): a click, or a drag over a region, adds three points on the curve for the hue
/// (or saturation, or lightness) under it, as Resolve's image sampling does. It reads the picture
/// as the viewer shows it, after the grade, as the qualifier's picker does.
pub fn curve_sampler(ui: &mut Ui, screen: Rect, picture: Option<&Arc<Frame>>, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let Some((key, periodic)) = page.curve_mode.curve() else { return asks };
    let r = ui.interact(screen, ui.id().with("curve sampler"), Sense::click_and_drag());
    name(&r, WidgetType::Other, "Curve sampler");
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let from = ui.input(|i| i.pointer.press_origin());
    let region = match (from, r.interact_pointer_pos()) {
        (Some(a), Some(b)) if r.dragged() || r.drag_stopped() => Some(Rect::from_two_pos(a, b)),
        (_, Some(p)) if r.clicked() => Some(Rect::from_center_size(p, vec2(1.0, 1.0))),
        _ => None,
    };
    if r.dragged()
        && let Some(region) = region
    {
        ui.painter().rect_stroke(
            region,
            CornerRadius::ZERO,
            Stroke::new(1.0, Color32::WHITE),
            egui::StrokeKind::Middle,
        );
    }
    if (r.clicked() || r.drag_stopped())
        && let (Some(region), Some(f), Some(node)) = (region, picture, page.current().cloned())
    {
        let inputs: Vec<f32> = samples(f, screen, region).into_iter().map(|c| curve_input(key, c)).collect();
        let pts = sampled(hsl_curve_of(&node, key), periodic, &inputs);
        page.adjust(&mut asks, json!({ "curves": obj(key, json!(pts)) }));
    }
    asks
}

/// The colours of `picture`, drawn in `screen`, under `region` of the screen: every pixel there,
/// or an even spread of 4096 of them.
fn samples(picture: &Frame, screen: Rect, region: Rect) -> Vec<[f32; 3]> {
    let (w, h) = (picture.width as usize, picture.height as usize);
    if w == 0 || h == 0 || screen.width() <= 0.0 || screen.height() <= 0.0 {
        return Vec::new();
    }
    let px = |x: f32, lo: f32, size: f32, n: usize| ((((x - lo) / size).clamp(0.0, 0.9999)) * n as f32) as usize;
    let (x0, x1) =
        (px(region.left(), screen.left(), screen.width(), w), px(region.right(), screen.left(), screen.width(), w));
    let (y0, y1) =
        (px(region.top(), screen.top(), screen.height(), h), px(region.bottom(), screen.top(), screen.height(), h));
    let step = (((x1 - x0 + 1) * (y1 - y0 + 1)) as f32 / 4096.0).sqrt().ceil().max(1.0) as usize;
    let pixels = picture.pixels();
    (y0..=y1)
        .step_by(step)
        .flat_map(|y| (x0..=x1).step_by(step).map(move |x| (y * w + x) * 4))
        .filter_map(|i| pixels.get(i..i + 3))
        .map(|p| [p[0], p[1], p[2]].map(|c| f32::from(c) / 255.0))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod handle_tests {
    use super::*;

    /// Each handle, dragged to a point, gives the window that puts it there (LOG D-057).
    #[test]
    fn a_dragged_handle_lands_under_the_pointer() {
        let pic = Rect::from_min_size(pos2(100.0, 50.0), vec2(1000.0, 500.0));
        let mut w = Window::new(WindowShape::Circle);
        w.rotation = 30.0;
        // (the rotate handle turns about the centre: tested on its own below)
        for h in [Handle::Width, Handle::Height, Handle::Scale, Handle::Soft] {
            let (_, at) = handles(&w, pic).into_iter().find(|(k, _)| *k == h).unwrap();
            // moved out from the centre by a third
            let c = pic.min + vec2(w.center[0] * pic.width(), w.center[1] * pic.height());
            let to = c + (at - c) * 1.3;
            let moved = dragged(&w, pic, h, to);
            let (_, now) = handles(&moved, pic).into_iter().find(|(k, _)| *k == h).unwrap();
            if h != Handle::Soft {
                assert!(now.distance(to) < 0.5, "{h:?}: {now:?} against {to:?}");
            }
            assert_ne!(moved, w, "{h:?} changed nothing");
        }
        // the rotate handle straight to the right of the centre: a quarter turn
        let c = pic.center();
        assert!((dragged(&w, pic, Handle::Rotate, c + vec2(80.0, 0.0)).rotation - 90.0).abs() < 1e-3);
        // softness below the window: dragged further down, softer
        let (_, soft) = handles(&w, pic).into_iter().find(|(k, _)| *k == Handle::Soft).unwrap();
        let (sin, cos) = w.rotation.to_radians().sin_cos();
        let further = soft + vec2(-sin, cos) * 20.0;
        assert!(dragged(&w, pic, Handle::Soft, further).softness > w.softness + 0.03);
    }

    /// A Linear window's Soft 1 to 4 (LOG D-065): four numbers on a row of their own in place of
    /// Softness, the other shapes' rows as they were; a handle beside each side sets its softness.
    #[test]
    fn a_linear_windows_sides_have_their_own_fields_and_handles() {
        let cells = |s: WindowShape| field_cells(s).into_iter().map(|(f, r, c)| (f.name, r, c)).collect::<Vec<_>>();
        let linear = cells(WindowShape::Linear);
        assert_eq!(
            &linear[6..],
            &[("Soft 1", 2, 0), ("Soft 2", 2, 1), ("Soft 3", 2, 2), ("Soft 4", 2, 3)],
            "{linear:?}"
        );
        assert!(!linear.iter().any(|c| c.0 == "Softness"));
        assert_eq!(linear.iter().find(|c| c.0 == "Opacity").map(|c| (c.1, c.2)), Some((1, 1)));
        for s in [WindowShape::Circle, WindowShape::Polygon, WindowShape::Gradient] {
            let c = cells(s);
            assert!(c.iter().all(|c| c.1 < 2 && !c.0.starts_with("Soft ")), "two rows: {c:?}");
        }
        // a handle beside each side, none below for the whole
        let pic = Rect::from_min_size(pos2(100.0, 50.0), vec2(1000.0, 500.0));
        let mut w = Window::new(WindowShape::Linear);
        w.rotation = 30.0;
        let grips = handles(&w, pic);
        assert!(!grips.iter().any(|g| g.0 == Handle::Soft));
        for k in 0..4 {
            let (_, at) = grips.iter().find(|g| g.0 == Handle::Side(k)).copied().unwrap();
            // dragged 20 pixels further out from its side: that side softer, the others kept
            let (sin, cos) = w.rotation.to_radians().sin_cos();
            let out = match k {
                0 => vec2(sin, -cos),
                1 => vec2(-sin, cos),
                2 => vec2(-cos, -sin),
                _ => vec2(cos, sin),
            };
            let moved = dragged(&w, pic, Handle::Side(k), at + out * 20.0);
            let (before, after) = (w.side_softnesses(), moved.side_softnesses());
            assert!((after[k] - before[k] - 0.04).abs() < 1e-3, "side {k}: {before:?} to {after:?}");
            assert!((0..4).filter(|j| *j != k).all(|j| after[j] == before[j]), "side {k}: {after:?}");
            // and the handle follows: it is where it was dragged to
            let now = handles(&moved, pic).into_iter().find(|g| g.0 == Handle::Side(k)).unwrap().1;
            assert!(now.distance(at + out * 20.0) < 0.5, "side {k}: {now:?}");
        }
    }

    /// A Polygon's or a Curve's softness is its In Soft and Out Soft (LOG D-063, D-069): the
    /// palette shows those in place of Softness, and the soft handle sets how far out it reaches.
    #[test]
    fn a_polygons_soft_handle_and_fields_are_its_inside_and_outside_softness() {
        let names = |s: WindowShape| window_fields(s).map(|f| f.name).collect::<Vec<_>>();
        for s in [WindowShape::Polygon, WindowShape::Curve] {
            let n = names(s);
            assert!(n.contains(&"In Soft") && n.contains(&"Out Soft") && !n.contains(&"Softness"), "{n:?}");
            assert_eq!(n.len(), 8, "two rows of four: {n:?}");
        }
        for s in [WindowShape::Circle, WindowShape::Gradient] {
            let n = names(s);
            assert!(n.contains(&"Softness") && !n.contains(&"In Soft"), "{n:?}");
        }
        assert!(!names(WindowShape::Linear).contains(&"In Soft"), "a Linear window has Soft 1 to 4 (LOG D-065)");
        // the fields set the window's own, one leaving the other
        let field = |name: &str| WINDOW_FIELDS.iter().find(|f| f.name == name).unwrap();
        let mut w = Window::new(WindowShape::Polygon);
        (field("Out Soft").set)(&mut w, 4.0);
        assert_eq!(w.edge_softness(), (w.softness / 2.0, 0.04));
        assert!(((field("In Soft").get)(&w) - 1.0).abs() < 1e-4 && ((field("Out Soft").get)(&w) - 4.0).abs() < 1e-4);
        // the soft handle sits that far out below the window, and dragged lower reaches further
        let pic = Rect::from_min_size(pos2(100.0, 50.0), vec2(1000.0, 500.0));
        let (_, soft) = handles(&w, pic).into_iter().find(|(k, _)| *k == Handle::Soft).unwrap();
        let bottom = pic.min.y + w.center[1] * pic.height() + w.size[1] * pic.width() / 2.0;
        assert!((soft.y - (bottom + HANDLE_GAP / 2.0 + 40.0)).abs() < 0.01, "{soft:?} below {bottom}");
        let moved = dragged(&w, pic, Handle::Soft, soft + vec2(0.0, 20.0));
        assert!((moved.edge_softness().1 - 0.06).abs() < 1e-4, "{:?}", moved.edge_softness());
        assert_eq!((moved.softness, moved.inside_softness), (w.softness, w.inside_softness), "the rest kept");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod sampling_tests {
    use super::*;

    fn xs(pts: &[[f32; 2]]) -> Vec<f32> {
        pts.iter().map(|p| (p[0] * 1e4).round() / 1e4).collect()
    }

    /// Sampled hues either side of red: the arc through red, not the long way round (LOG D-066).
    #[test]
    fn sampled_hues_span_the_shortest_arc_and_sit_on_the_curve() {
        let empty = HslCurve::default();
        let pts = sampled(&empty, true, &[0.98, 0.99, 0.01, 0.02]);
        let m = SAMPLE_MARGIN.0;
        let mut want = [0.0, 0.02 + m, 0.98 - m];
        want.sort_by(f32::total_cmp);
        assert_eq!(xs(&pts), xs(&want.iter().map(|x| [*x, 0.0]).collect::<Vec<_>>()), "{pts:?}");
        assert!(pts.iter().all(|p| p[1] == 0.5), "on the middle line: nothing changes yet");
        // a range that does not cross red, on a bent curve: the curve's points kept, the new
        // ones on the curve as it is
        let bent = HslCurve(vec![[0.1, 0.8], [0.6, 0.3]]);
        let pts = sampled(&bent, true, &[0.3, 0.4, 0.35]);
        assert_eq!(xs(&pts), xs(&[[0.1, 0.0], [0.3 - m, 0.0], [0.35, 0.0], [0.4 + m, 0.0], [0.6, 0.0]]));
        for p in &pts {
            assert!((p[1] - bent.eval(p[0], true)).abs() < 1e-5, "{p:?} is on the curve");
        }
        // lightness, saturation: a plain range, held inside 0 to 1
        let m = SAMPLE_MARGIN.1;
        assert_eq!(xs(&sampled(&empty, false, &[0.5, 0.3])), xs(&[[0.3 - m, 0.0], [0.4, 0.0], [0.5 + m, 0.0]]));
        assert_eq!(xs(&sampled(&empty, false, &[0.0, 0.02])), xs(&[[0.0, 0.0], [0.01, 0.0], [0.02 + m, 0.0]]));
        assert!(sampled(&bent, true, &[]) == bent.0, "nothing sampled, nothing added");
    }

    /// A node just added stays current while grades from before it arrive; its add's own answer
    /// settles it (LOG B-055).
    #[test]
    fn a_new_node_stays_current_until_its_add_answers() {
        let view = |nodes: usize| GradeView {
            clip: 7,
            grade: resolution_grade::Grade { nodes: vec![Node::default(); nodes] },
            foreign: false,
            size: (1920, 1080),
        };
        let mut page = ColorPage { clip: Some(7), view: Some(view(1)), ..Default::default() };
        // Alt+S: node 2 current before the engine has it
        (page.node, page.new_node) = (1, Some(1));
        page.take(view(1), 3);
        assert_eq!(page.node, 1, "a grade from before the add keeps the new node current");
        page.take(view(2), 4);
        page.added();
        assert_eq!((page.node, page.new_node), (1, None));
        // another node chosen before the add answers stays chosen
        (page.node, page.new_node) = (2, Some(2));
        page.node = 0;
        page.take(view(3), 5);
        page.added();
        assert_eq!((page.node, page.new_node), (0, None));
        page.view = Some(view(2));
        // an add that fails: its answer brings no grade, and node 1 is current again
        (page.node, page.new_node) = (2, Some(2));
        page.added();
        assert_eq!((page.node, page.new_node), (1, None));
        // without an add, a grade with fewer nodes moves the current node in range
        page.node = 5;
        page.take(view(2), 6);
        assert_eq!(page.node, 1);
    }

    /// `--palette`'s names (LOG D-069).
    #[test]
    fn palettes_are_named_for_the_command_line() {
        assert_eq!(palette_of("window"), Some((Palette::Window, CurveMode::Custom)));
        assert_eq!(palette_of("qualifier"), Some((Palette::Qualifier, CurveMode::Custom)));
        assert_eq!(palette_of("curves"), Some((Palette::Curves, CurveMode::Custom)));
        assert_eq!(palette_of("curves:hue_sat"), Some((Palette::Curves, CurveMode::HueSat)));
        assert_eq!(palette_of("curves:sat_lum"), Some((Palette::Curves, CurveMode::SatLum)));
        assert_eq!(palette_of("curves:custom"), None);
        assert_eq!(palette_of("wheels"), None);
    }

    /// A six-vector patch: its middle on the vector, the outer two half way to its neighbours.
    #[test]
    fn a_patch_puts_three_points_round_its_vector() {
        let pts = patched(&HslCurve::default(), 1);
        assert_eq!(xs(&pts), xs(&[[1.0 / 12.0, 0.0], [1.0 / 6.0, 0.0], [0.25, 0.0]]), "yellow");
        let red = patched(&HslCurve::default(), 0);
        assert_eq!(xs(&red), xs(&[[0.0, 0.0], [1.0 / 12.0, 0.0], [11.0 / 12.0, 0.0]]), "red, round the wheel");
    }

    /// Each curve samples what it is over; the pixels under a region of the viewer.
    #[test]
    fn the_sampler_reads_what_each_curve_is_over_under_the_region() {
        let c = [0.8, 0.4, 0.2];
        let [h, s, l] = rgb_to_hsl(c);
        assert_eq!(curve_input("hue_sat", c), h);
        assert_eq!(curve_input("lum_sat", c), l);
        assert_eq!((curve_input("sat_sat", c), curve_input("sat_lum", c)), (s, s));
        // a 4 x 2 picture, red on the left half, blue on the right, drawn 40 x 20 on screen
        let px: Vec<u8> = (0..8).flat_map(|i| if i % 4 < 2 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
        let f = Frame::new(4, 2, px);
        let screen = Rect::from_min_size(pos2(100.0, 100.0), vec2(40.0, 20.0));
        let left = samples(&f, screen, Rect::from_min_max(pos2(101.0, 101.0), pos2(118.0, 119.0)));
        assert_eq!(left, vec![[1.0, 0.0, 0.0]; 4]);
        let all = samples(&f, screen, screen);
        assert_eq!(all.len(), 8);
        assert_eq!(all.iter().filter(|c| c[2] == 1.0).count(), 4);
        // a click: the one pixel under it
        assert_eq!(
            samples(&f, screen, Rect::from_center_size(pos2(135.0, 105.0), vec2(1.0, 1.0))),
            vec![[0.0, 0.0, 1.0]]
        );
    }
}
