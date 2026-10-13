//! The widgets every panel shares, drawn from the [`Look`]: icon buttons, panel title strips,
//! page buttons, toolbar dividers. Each button carries its name for screen readers, tooltips and
//! UI tests.

use egui::{Align2, Color32, Rect, Response, Sense, Stroke, Ui, WidgetInfo, WidgetType, vec2};

use crate::icons::{self, Icon};
use crate::theme::Look;

/// A toolbar button's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Off,
    /// A toggle that is on, or the current mode.
    On,
    Disabled,
}

impl State {
    pub fn on(on: bool) -> Self {
        if on { State::On } else { State::Off }
    }
}

/// A square icon button (with its name beside it when the skin shows labels). `keys`, when
/// given, is added to the tooltip.
pub fn icon_button(ui: &mut Ui, look: &Look, icon: Icon, label: &str, keys: Option<&str>, state: State) -> Response {
    let (skin, t) = (look.skin, look.theme);
    let enabled = state != State::Disabled;
    let galley = skin.labels.then(|| ui.painter().layout_no_wrap(label.to_string(), look.body(), t.text));
    let width = skin.button + galley.as_ref().map_or(0.0, |g| g.size().x + 8.0);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, skin.button), if enabled { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, state == State::On, label));
    if ui.is_rect_visible(rect) {
        let hovered = enabled && response.hovered();
        let fill = match state {
            State::On => t.active_bg,
            _ if hovered => t.hover,
            _ => Color32::TRANSPARENT,
        };
        let painter = ui.painter();
        painter.rect_filled(rect, look.radius(), fill);
        let colour = match state {
            State::Disabled => t.dim.gamma_multiply(0.45),
            State::On => t.accent,
            State::Off if hovered => t.text,
            State::Off => t.dim,
        };
        let icon_rect = Rect::from_center_size(rect.left_center() + vec2(skin.button / 2.0, 0.0), vec2(16.0, 16.0));
        icons::paint(painter, icon_rect, icon, Stroke::new(skin.icon_stroke, colour));
        if let Some(g) = galley {
            painter.galley_with_override_text_color(
                rect.left_center() + vec2(skin.button, -g.size().y / 2.0),
                g,
                colour,
            );
        }
    }
    let tip = match keys {
        Some(k) => format!("{label}   {k}"),
        None => label.to_string(),
    };
    if enabled { response.on_hover_text(tip) } else { response.on_disabled_hover_text(tip) }
}

/// A key chord as the keymap writes it ("Mod+B"), as this platform shows it ("Ctrl+B").
pub fn key_text(chord: &str) -> String {
    chord.replace("Mod", if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" })
}

/// An icon button inside a panel's title strip, at its right end.
pub fn header_button(ui: &mut Ui, look: &Look, header: Rect, icon: Icon, label: &str) -> bool {
    let size = look.skin.button;
    let rect = Rect::from_center_size(header.right_center() - vec2(4.0 + size / 2.0, 0.0), vec2(size, size));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| icon_button(ui, look, icon, label, None, State::Off))
        .inner
        .clicked()
}

/// A page in the page bar: icon and name; the current page in the accent with a line beneath.
pub fn page_button(ui: &mut Ui, look: &Look, icon: Icon, label: &str, current: bool) -> Response {
    let t = look.theme;
    let galley = ui.painter().layout_no_wrap(label.to_string(), look.body(), t.text);
    let size = vec2(18.0 + 7.0 + galley.size().x + 28.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, current, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered();
        if current || hovered {
            painter.rect_filled(rect, look.radius(), if current { t.active_bg } else { t.hover });
        }
        let colour = if current {
            t.accent
        } else if hovered {
            t.text
        } else {
            t.dim
        };
        if current {
            painter.line_segment(
                [rect.left_bottom() + vec2(0.0, -1.0), rect.right_bottom() + vec2(0.0, -1.0)],
                Stroke::new(2.0, t.accent),
            );
        }
        let icon_rect = Rect::from_center_size(rect.left_center() + vec2(14.0 + 9.0, 0.0), vec2(18.0, 18.0));
        icons::paint(painter, icon_rect, icon, Stroke::new(look.skin.icon_stroke, colour));
        painter.galley_with_override_text_color(
            rect.left_center() + vec2(14.0 + 18.0 + 7.0, -galley.size().y / 2.0),
            galley,
            colour,
        );
    }
    response
}

/// A flat text button, for the few actions that read better as words.
pub fn text_button(ui: &mut Ui, look: &Look, label: &str, enabled: bool) -> Response {
    let t = look.theme;
    let galley = ui.painter().layout_no_wrap(label.to_string(), look.body(), t.text);
    let size = vec2(galley.size().x + 20.0, look.skin.button);
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = enabled && response.hovered();
        painter.rect_filled(rect, look.radius(), if hovered { t.active_bg } else { t.field });
        let colour = if !enabled {
            t.dim.gamma_multiply(0.45)
        } else if hovered {
            t.text
        } else {
            t.dim
        };
        painter.galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, colour);
    }
    response
}

/// A short vertical line between groups of toolbar buttons.
pub fn divider(ui: &mut Ui, look: &Look) {
    let (rect, _) = ui.allocate_exact_size(vec2(13.0, look.skin.button), Sense::hover());
    ui.painter().line_segment(
        [rect.center() - vec2(0.0, 9.0), rect.center() + vec2(0.0, 9.0)],
        Stroke::new(1.0, look.theme.line),
    );
}

/// A panel's title strip: its name on the left, a detail (bin, clip, count) on the right.
pub fn panel_header(ui: &mut Ui, look: &Look, title: &str, detail: Option<&str>) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), look.skin.header), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, look.theme.head);
    painter.line_segment([rect.left_bottom(), rect.right_bottom()], look.sep_stroke());
    painter.text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, title, look.medium(), look.theme.text);
    if let Some(detail) = detail {
        painter.text(rect.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, detail, look.small(), look.theme.dim);
    }
    rect
}

/// The body of a panel that is not built yet: says so, rather than looking broken.
pub fn not_built(ui: &mut Ui, look: &Look, what: &str) {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new(format!("{what} is not built yet.")).color(look.theme.dim));
    });
}
