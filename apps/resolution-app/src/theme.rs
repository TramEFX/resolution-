//! Resolution's look (LOG D-018), in two layers:
//! - a [`Skin`] is the chrome: type sizes, corners, gaps, icon weight, toolbar labels;
//! - a [`Theme`] is the colours.
//!
//! There is one skin today, Black Box, and a few colour themes. Grounds and panel greys are
//! colourless (R = G = B) in every theme: a tinted UI around the viewer biases colour judgement.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke, TextStyle, vec2};

/// The font family for headings and active labels (Geist Medium, falling back like the body).
pub const MEDIUM: &str = "Geist Medium";

/// The chrome.
#[derive(Debug, PartialEq)]
pub struct Skin {
    pub name: &'static str,
    /// Body text, in points.
    pub text: f32,
    pub small: f32,
    pub radius: u8,
    /// Width of the line between panels, drawn in [`Theme::sep`].
    pub gap: f32,
    pub icon_stroke: f32,
    /// Height of the interface toolbar, the timeline toolbar and the viewer's transport.
    pub bar: f32,
    /// Toolbar buttons are this square.
    pub button: f32,
    /// Height of a panel's title strip.
    pub header: f32,
    /// Toolbar buttons show their names next to their icons.
    pub labels: bool,
    pub video_track: f32,
    pub audio_track: f32,
    /// Clips get a strip of colour along their top edge.
    pub clip_edge: bool,
}

pub const BLACK_BOX: Skin = Skin {
    name: "Black Box",
    text: 12.5,
    small: 11.0,
    radius: 2,
    gap: 1.0,
    icon_stroke: 1.25,
    bar: 40.0,
    button: 26.0,
    header: 30.0,
    labels: false,
    video_track: 48.0,
    audio_track: 40.0,
    clip_edge: true,
};

/// The colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Saved in the preferences; never renamed.
    pub id: &'static str,
    pub name: &'static str,
    pub ground: Color32,
    /// Lines between panels.
    pub sep: Color32,
    pub panel: Color32,
    /// Panel title strips, the ruler, track headers.
    pub head: Color32,
    pub hover: Color32,
    pub active_bg: Color32,
    /// Text fields and value boxes.
    pub field: Color32,
    /// Dividers inside a toolbar, ticks, slider rails.
    pub line: Color32,
    pub track: Color32,
    pub track_alt: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    /// Text on an accent fill.
    pub on_accent: Color32,
    pub video: Color32,
    pub audio: Color32,
    pub title: Color32,
    pub wave: Color32,
    pub clip_text: Color32,
    pub disabled_clip: Color32,
    pub error: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const MIDNIGHT: Theme = Theme {
    id: "midnight",
    name: "Midnight",
    ground: hex(0x0b0b0b),
    sep: hex(0x262626),
    panel: hex(0x0b0b0b),
    head: hex(0x0b0b0b),
    hover: hex(0x161616),
    active_bg: hex(0x1a1a1a),
    field: hex(0x151515),
    line: hex(0x2d2d2d),
    track: hex(0x0e0e0e),
    track_alt: hex(0x0b0b0b),
    text: hex(0xececec),
    dim: hex(0xa0a0a0),
    accent: hex(0x4c8dff),
    on_accent: hex(0x0b0b0b),
    video: hex(0x1e2a40),
    audio: hex(0x1a2f28),
    title: hex(0x32244a),
    wave: hex(0x6fd0b4),
    clip_text: hex(0xececec),
    disabled_clip: hex(0x262626),
    error: hex(0xff7a6b),
};

pub const EMBER: Theme = Theme { id: "ember", name: "Ember", accent: hex(0xf2a43a), ..MIDNIGHT };

pub const MINT: Theme = Theme { id: "mint", name: "Mint", accent: hex(0x3dd6a3), wave: hex(0xa9c4f0), ..MIDNIGHT };

pub const ROSE: Theme = Theme { id: "rose", name: "Rose", accent: hex(0xff7896), ..MIDNIGHT };

/// The same accent family on a lifted ground, for rooms that are not dark.
pub const CHARCOAL: Theme = Theme {
    id: "charcoal",
    name: "Charcoal",
    ground: hex(0x181818),
    sep: hex(0x303030),
    panel: hex(0x181818),
    head: hex(0x181818),
    hover: hex(0x212121),
    active_bg: hex(0x262626),
    field: hex(0x202020),
    line: hex(0x3a3a3a),
    track: hex(0x1b1b1b),
    track_alt: hex(0x181818),
    accent: hex(0x7cc4ff),
    on_accent: hex(0x121212),
    ..MIDNIGHT
};

pub const THEMES: [&Theme; 5] = [&MIDNIGHT, &EMBER, &MINT, &ROSE, &CHARCOAL];

/// The theme saved as `id`, or the default for an unknown id.
pub fn theme(id: &str) -> &'static Theme {
    THEMES.into_iter().find(|t| t.id == id).unwrap_or(&MIDNIGHT)
}

/// What the UI is drawn with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub skin: &'static Skin,
    pub theme: &'static Theme,
}

impl Default for Look {
    fn default() -> Self {
        Self { skin: &BLACK_BOX, theme: &MIDNIGHT }
    }
}

impl Look {
    pub fn body(&self) -> FontId {
        FontId::proportional(self.skin.text)
    }

    pub fn small(&self) -> FontId {
        FontId::proportional(self.skin.small)
    }

    pub fn medium(&self) -> FontId {
        FontId::new(self.skin.text, FontFamily::Name(MEDIUM.into()))
    }

    pub fn mono(&self, size: f32) -> FontId {
        FontId::monospace(size)
    }

    pub fn radius(&self) -> CornerRadius {
        CornerRadius::same(self.skin.radius)
    }

    /// The line between panels.
    pub fn sep_stroke(&self) -> Stroke {
        Stroke::new(self.skin.gap, self.theme.sep)
    }

    /// Install the fonts once, at start-up.
    pub fn install_fonts(ctx: &egui::Context) {
        ctx.set_fonts(fonts());
    }

    /// Make egui's own widgets (menus, text fields, scroll bars, tooltips) match.
    pub fn apply(&self, ctx: &egui::Context) {
        let (skin, t) = (self.skin, self.theme);
        ctx.set_theme(egui::ThemePreference::Dark);
        ctx.style_mut_of(egui::Theme::Dark, |s| {
            s.text_styles = [
                (TextStyle::Small, FontId::proportional(skin.small)),
                (TextStyle::Body, FontId::proportional(skin.text)),
                (TextStyle::Button, FontId::proportional(skin.text)),
                (TextStyle::Heading, FontId::new(skin.text + 1.0, FontFamily::Name(MEDIUM.into()))),
                (TextStyle::Monospace, FontId::monospace(skin.text)),
            ]
            .into();
            s.spacing.item_spacing = vec2(6.0, 4.0);
            s.spacing.button_padding = vec2(8.0, 3.0);
            s.spacing.menu_margin = egui::Margin::same(6);
            let v = &mut s.visuals;
            v.dark_mode = true;
            v.override_text_color = None;
            v.weak_text_color = Some(t.dim);
            v.panel_fill = t.panel;
            v.window_fill = t.panel;
            v.window_stroke = Stroke::new(1.0, t.sep);
            v.window_corner_radius = CornerRadius::same(skin.radius + 2);
            v.menu_corner_radius = CornerRadius::same(skin.radius + 2);
            v.window_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(160) };
            v.popup_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(160) };
            v.extreme_bg_color = t.field;
            v.text_edit_bg_color = Some(t.field);
            v.faint_bg_color = t.track;
            v.code_bg_color = t.field;
            v.hyperlink_color = t.accent;
            v.warn_fg_color = t.accent;
            v.error_fg_color = t.error;
            v.selection.bg_fill = t.accent.gamma_multiply(0.35);
            v.selection.stroke = Stroke::new(1.0, t.accent);
            let radius = CornerRadius::same(skin.radius);
            let w = &mut v.widgets;
            w.noninteractive.bg_fill = t.panel;
            w.noninteractive.weak_bg_fill = t.panel;
            w.noninteractive.bg_stroke = Stroke::new(skin.gap, t.sep);
            w.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
            w.inactive.bg_fill = t.field;
            w.inactive.weak_bg_fill = t.field;
            w.inactive.bg_stroke = Stroke::NONE;
            w.inactive.fg_stroke = Stroke::new(1.0, t.text);
            w.hovered.bg_fill = t.active_bg;
            w.hovered.weak_bg_fill = t.active_bg;
            w.hovered.bg_stroke = Stroke::new(1.0, t.line);
            w.hovered.fg_stroke = Stroke::new(1.0, t.text);
            w.active.bg_fill = t.active_bg;
            w.active.weak_bg_fill = t.active_bg;
            w.active.bg_stroke = Stroke::new(1.0, t.accent);
            w.active.fg_stroke = Stroke::new(1.0, t.accent);
            w.open.bg_fill = t.active_bg;
            w.open.weak_bg_fill = t.active_bg;
            w.open.bg_stroke = Stroke::new(1.0, t.line);
            w.open.fg_stroke = Stroke::new(1.0, t.text);
            for wv in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
                wv.corner_radius = radius;
                wv.expansion = 0.0;
            }
        });
    }
}

/// Geist and Geist Mono (SIL OFL 1.1, `assets/fonts/OFL.txt`), with egui's own fonts behind them
/// for the glyphs Geist lacks (Greek, symbols, emoji).
fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    for (name, bytes) in [
        ("Geist", &include_bytes!("../assets/fonts/Geist-Regular.ttf")[..]),
        (MEDIUM, &include_bytes!("../assets/fonts/Geist-Medium.ttf")[..]),
        ("Geist Mono", &include_bytes!("../assets/fonts/GeistMono-Regular.ttf")[..]),
    ] {
        f.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let fallback = |f: &FontDefinitions, family: FontFamily| f.families.get(&family).cloned().unwrap_or_default();
    let proportional = fallback(&f, FontFamily::Proportional);
    let monospace = fallback(&f, FontFamily::Monospace);
    f.families
        .insert(FontFamily::Proportional, ["Geist".to_string()].into_iter().chain(proportional.clone()).collect());
    f.families.insert(FontFamily::Monospace, ["Geist Mono".to_string()].into_iter().chain(monospace).collect());
    f.families.insert(FontFamily::Name(MEDIUM.into()), [MEDIUM.to_string()].into_iter().chain(proportional).collect());
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(c: Color32) -> bool {
        c.r() == c.g() && c.g() == c.b()
    }

    /// Relative luminance (WCAG 2), for contrast ratios.
    fn luminance(c: Color32) -> f64 {
        let lin = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn grounds_and_panels_are_colourless() {
        for t in THEMES {
            for (what, c) in [
                ("ground", t.ground),
                ("sep", t.sep),
                ("panel", t.panel),
                ("head", t.head),
                ("hover", t.hover),
                ("active_bg", t.active_bg),
                ("field", t.field),
                ("line", t.line),
                ("track", t.track),
                ("track_alt", t.track_alt),
            ] {
                assert!(grey(c), "{} {what} {c:?} is tinted", t.name);
            }
        }
    }

    #[test]
    fn text_is_readable() {
        for t in THEMES {
            for (what, fg, bg, min) in [
                ("text on panel", t.text, t.panel, 4.5),
                ("dim on panel", t.dim, t.panel, 4.5),
                ("dim on head", t.dim, t.head, 4.5),
                ("accent on active", t.accent, t.active_bg, 4.5),
                ("on_accent on accent", t.on_accent, t.accent, 4.5),
                ("clip text on video", t.clip_text, t.video, 4.5),
                ("clip text on audio", t.clip_text, t.audio, 4.5),
                ("clip text on title", t.clip_text, t.title, 4.5),
            ] {
                let c = contrast(fg, bg);
                assert!(c >= min, "{} {what}: {c:.2} < {min}", t.name);
            }
        }
    }

    #[test]
    fn ids_are_unique_and_unknown_ids_fall_back() {
        let mut ids: Vec<_> = THEMES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), THEMES.len());
        assert_eq!(theme("ember").name, "Ember");
        assert_eq!(theme("no such theme").id, "midnight");
    }
}
