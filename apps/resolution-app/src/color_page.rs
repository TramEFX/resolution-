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
use resolution_engine_api::{Playback, RenderHints};
use resolution_grade::{Curve, Node, Wheel};
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
    /// The custom curve being edited: 0 Y, 1 R, 2 G, 3 B.
    pub channel: usize,
    /// The clip and revision of the last `color.inspect` asked for.
    pub asked: Option<(u64, u64)>,
    /// A control is being dragged.
    pub dragging: bool,
    drags: u64,
    /// The curve point being dragged (an index into the curve's knots).
    curve_point: Option<usize>,
    /// Per clip, the revision at which its grade last changed: when its thumbnail is due again.
    pub stamps: HashMap<u64, u64>,
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
        self.node = self.node.min(v.grade.nodes.len().saturating_sub(1));
        self.view = Some(v);
    }

    fn begin_drag(&mut self) {
        self.drags += 1;
        self.dragging = true;
    }

    /// The drag is over: ask the engine for the grade again.
    fn end_drag(&mut self) {
        self.dragging = false;
        self.curve_point = None;
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

const TOP_BAR: [FieldSpec; 4] = [
    FieldSpec { name: "Temp", key: "temp", step: 10.0, decimals: 1 },
    FieldSpec { name: "Tint", key: "tint", step: 0.5, decimals: 2 },
    FieldSpec { name: "Contrast", key: "contrast", step: 0.005, decimals: 3 },
    FieldSpec { name: "Pivot", key: "pivot", step: 0.002, decimals: 3 },
];

const BOTTOM_BAR: [FieldSpec; 3] = [
    FieldSpec { name: "Color Boost", key: "color_boost", step: 0.5, decimals: 2 },
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
        "saturation" => n.saturation,
        _ => n.hue,
    }
}

/// The Primaries palette in Wheels mode: Temp, Tint, Contrast and Pivot above; the Lift, Gamma,
/// Gain and Offset wheels, each with its master ring and YRGB numbers; Color Boost, Saturation
/// and Hue below.
pub fn primaries(ui: &mut Ui, look: &Look, page: &mut ColorPage) -> Vec<Ask> {
    let mut asks = Vec::new();
    let header = chrome::panel_header(ui, look, "Primaries - Color Wheels", None);
    let Some(node) = page.current().cloned() else {
        hint(ui, look, "Move the playhead onto a clip to grade it.");
        return asks;
    };
    if chrome::header_button(ui, look, header, crate::icons::Icon::Reset, "Reset Primaries") {
        let d = serde_json::to_value(Node::default()).unwrap_or_default();
        let mut controls = Map::new();
        for k in
            ["lift", "gamma", "gain", "offset", "contrast", "pivot", "saturation", "hue", "color_boost", "temp", "tint"]
        {
            controls.insert(k.to_string(), d[k].clone());
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
        wheel(ui, look, col, spec, wheel_of(&node, spec.field), page, &mut asks);
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
    let t = look.theme;
    let w = row.width() / specs.len() as f32;
    let default = Node::default();
    for (k, spec) in specs.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(row.left() + w * k as f32, row.top()), vec2(w, row.height()))
            .shrink2(vec2(6.0, 2.0));
        let value = field_value(node, spec.key);
        let r = ui.interact(cell, ui.id().with(("field", spec.key)), Sense::click_and_drag());
        name(&r, WidgetType::DragValue, spec.name);
        let painter = ui.painter();
        painter.rect_filled(cell, look.radius(), if r.hovered() || r.dragged() { t.hover } else { t.field });
        painter.text(cell.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, spec.name, look.small(), t.dim);
        let changed = (value - field_value(&default, spec.key)).abs() > 1e-6;
        painter.text(
            cell.right_center() - vec2(8.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("{value:.*}", spec.decimals),
            look.mono(12.0),
            if changed { t.accent } else { t.text },
        );
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

// ---- Curves ----

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
    let header = chrome::panel_header(ui, look, "Curves - Custom", None);
    let Some(node) = page.current().cloned() else { return asks };
    // the channel buttons, in the title strip
    let mut x = header.right() - 10.0;
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
    let (w, h) = (96.0, 60.0);
    let gap = ((area.width() - 80.0 - w * count as f32) / (count as f32 + 1.0)).clamp(12.0, 40.0);
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
        painter.text(rect.center() + vec2(0.0, 6.0), Align2::CENTER_CENTER, label, look.small(), t.dim);
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
