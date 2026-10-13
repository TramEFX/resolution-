//! The timeline: a ruler with markers, track headers, video tracks above audio tracks, clips, marks
//! and the playhead. It reports what the user did; the app turns that into engine commands.
//! Colours come from the theme (LOG D-018); the workflow is Resolve's, the pixels are our own
//! (ARCHITECTURE invariant 7).

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::icons::{self, Icon};
use crate::model::{Clip, Timeline, Track};
use crate::theme::Look;

pub enum Action {
    Seek(f64),
    Select {
        clip: u64,
        add: bool,
    },
    /// Flip one of a track's switches: `track.set` (locked, enabled, muted, solo) or
    /// `track.target` (targeted, sourcePatch).
    Track {
        track: String,
        command: &'static str,
        field: &'static str,
        value: bool,
    },
}

/// Width of the track headers.
pub const HEADER: f32 = 190.0;
const RULER: f32 = 26.0;
const SWITCH: f32 = 22.0;

/// Pixels per second, and the first second shown. `zoom` of `None` fits the whole timeline.
pub struct View {
    pub zoom: Option<f32>,
}

/// Draws the timeline and returns what the user did, plus where each clip was drawn (for
/// hit-testing and UI tests).
pub fn show(ui: &mut Ui, look: &Look, tl: &Timeline, playhead: f64, view: &View) -> (Vec<Action>, Vec<(Rect, u64)>) {
    let t = look.theme;
    let mut actions = Vec::new();
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, t.panel);

    let lane_x = rect.left() + HEADER;
    let lane_w = (rect.width() - HEADER).max(1.0);
    let span = tl.duration.max(1.0) * 1.02;
    let pps = view.zoom.unwrap_or(lane_w / span as f32).max(1.0);
    // keep the playhead in view when zoomed in
    let visible = (lane_w / pps) as f64;
    let first = if visible >= span { 0.0 } else { (playhead - visible / 2.0).clamp(0.0, span - visible) };
    let x_of = |s: f64| lane_x + ((s - first) as f32) * pps;
    let t_of = |x: f32| (first + ((x - lane_x) / pps) as f64).clamp(0.0, tl.duration.max(0.0));

    // ruler with timecode labels
    let ruler = Rect::from_min_size(pos2(lane_x, rect.top()), vec2(lane_w, RULER));
    painter.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), RULER)), CornerRadius::ZERO, t.head);
    painter.line_segment([pos2(rect.left(), ruler.bottom()), pos2(rect.right(), ruler.bottom())], look.sep_stroke());
    // ticks on whole timecode seconds (24 frames at 23.976), so labels read 00:00:02:00, not 00:00:01:23
    let step = nice_step(90.0 / pps as f64) * tl.nominal_fps() as f64;
    let mut f = (first * tl.fps / step).floor() * step;
    while f / tl.fps <= first + visible {
        let s = f / tl.fps;
        let x = x_of(s);
        if x >= lane_x {
            painter.line_segment([pos2(x, ruler.bottom() - 8.0), pos2(x, ruler.bottom())], Stroke::new(1.0, t.dim));
            painter.text(pos2(x + 4.0, ruler.top() + 5.0), Align2::LEFT_TOP, tl.timecode(s), look.mono(10.0), t.dim);
            // a minor tick halfway
            let half = x_of(s + step / tl.fps / 2.0);
            painter
                .line_segment([pos2(half, ruler.bottom() - 4.0), pos2(half, ruler.bottom())], Stroke::new(1.0, t.line));
        }
        f += step;
    }

    // tracks: V(n)..V1 on top, then A1..A(n)
    let rows: Vec<(&Track, f32, bool)> = tl
        .video
        .iter()
        .rev()
        .map(|tr| (tr, look.skin.video_track, true))
        .chain(tl.audio.iter().map(|tr| (tr, look.skin.audio_track, false)))
        .collect();
    let any_solo = tl.audio.iter().any(|a| a.solo);
    let mut y = ruler.bottom();
    let mut hits: Vec<(Rect, u64)> = Vec::new();
    for (n, (track, h, video)) in rows.iter().enumerate() {
        let lane = Rect::from_min_size(pos2(lane_x, y), vec2(lane_w, *h));
        painter.rect_filled(lane, CornerRadius::ZERO, if n % 2 == 0 { t.track } else { t.track_alt });
        let head = Rect::from_min_size(pos2(rect.left(), y), vec2(HEADER, *h));
        actions.extend(track_header(ui, look, head, track, *video));
        painter.line_segment([pos2(rect.left(), lane.bottom()), pos2(rect.right(), lane.bottom())], look.sep_stroke());
        painter.line_segment([pos2(lane_x - 0.5, y), pos2(lane_x - 0.5, lane.bottom())], look.sep_stroke());
        // a hidden video track, or an audio track that is muted or silenced by another's solo
        let silent = if *video { !track.enabled } else { track.muted || (any_solo && !track.solo) };
        let fill = if *video { t.video } else { t.audio };
        for c in track.clips.iter() {
            let (x0, x1) = (x_of(c.start).max(lane_x), x_of(c.end));
            if x1 <= lane_x || x0 >= lane.right() {
                continue;
            }
            let r = Rect::from_x_y_ranges(x0..=(x1 - 1.0).min(lane.right()), (y + 3.0)..=(y + h - 3.0));
            draw_clip(&painter, look, r, c, fill, *video, silent);
            hits.push((r, c.id));
        }
        if track.locked {
            hatch(&painter.with_clip_rect(lane), lane, t.line);
        }
        y += h;
    }

    // in/out range, markers and the playhead
    if let (Some(a), Some(b)) = (tl.mark_in, tl.mark_out) {
        painter.rect_filled(
            Rect::from_x_y_ranges(x_of(a).max(lane_x)..=x_of(b).max(lane_x), ruler.top()..=ruler.bottom()),
            CornerRadius::ZERO,
            t.accent.gamma_multiply(0.22),
        );
    }
    for (mark, glyph) in [(tl.mark_in, "["), (tl.mark_out, "]")] {
        if let Some(m) = mark.filter(|m| x_of(*m) >= lane_x) {
            painter.text(pos2(x_of(m), ruler.bottom() - 1.0), Align2::CENTER_BOTTOM, glyph, look.mono(14.0), t.accent);
        }
    }
    for m in &tl.markers {
        let x = x_of(m.start);
        if x < lane_x || x > ruler.right() {
            continue;
        }
        let colour = Color32::from_rgb(m.color[0], m.color[1], m.color[2]);
        if m.duration > 0.0 {
            let x1 = x_of(m.start + m.duration).min(ruler.right());
            painter.rect_filled(
                Rect::from_x_y_ranges(x..=x1, (ruler.bottom() - 4.0)..=ruler.bottom()),
                CornerRadius::ZERO,
                colour.gamma_multiply(0.6),
            );
        }
        let top = ruler.top() + 13.0;
        painter.add(egui::Shape::convex_polygon(
            vec![
                pos2(x - 4.5, top),
                pos2(x + 4.5, top),
                pos2(x + 4.5, top + 6.0),
                pos2(x, top + 10.0),
                pos2(x - 4.5, top + 6.0),
            ],
            colour,
            Stroke::NONE,
        ));
        let hit = Rect::from_center_size(pos2(x, top + 5.0), vec2(12.0, 12.0));
        let name = if m.name.is_empty() { "Marker".to_string() } else { format!("Marker: {}", m.name) };
        let r = ui.interact(hit, ui.id().with(("marker", m.id)), Sense::click());
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        if r.on_hover_text(&name).clicked() {
            actions.push(Action::Seek(m.start));
        }
    }
    let px = x_of(playhead);
    if px >= lane_x {
        painter.line_segment([pos2(px, rect.top()), pos2(px, rect.bottom())], Stroke::new(1.5, t.accent));
        painter.add(egui::Shape::convex_polygon(
            vec![
                pos2(px - 6.0, rect.top()),
                pos2(px + 6.0, rect.top()),
                pos2(px + 6.0, rect.top() + 7.0),
                pos2(px, rect.top() + 12.0),
                pos2(px - 6.0, rect.top() + 7.0),
            ],
            t.accent,
            Stroke::NONE,
        ));
    }
    if rows.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER, "No tracks", look.body(), t.dim);
    }

    // clicks: a clip selects it (Shift adds); the ruler or an empty lane moves the playhead
    let pointer: Option<Pos2> = response.interact_pointer_pos();
    if let Some(p) = pointer.filter(|p| p.x >= lane_x) {
        let on_clip = hits.iter().find(|(r, _)| r.contains(p)).map(|(_, id)| *id);
        if response.clicked() {
            match on_clip {
                Some(clip) if p.y > ruler.bottom() => {
                    actions.push(Action::Select { clip, add: ui.input(|i| i.modifiers.shift) })
                }
                _ => actions.push(Action::Seek(t_of(p.x))),
            }
        } else if response.dragged() && (on_clip.is_none() || p.y <= ruler.bottom()) {
            actions.push(Action::Seek(t_of(p.x)));
        }
    }
    (actions, hits)
}

fn draw_clip(painter: &egui::Painter, look: &Look, r: Rect, c: &Clip, fill: Color32, video: bool, silent: bool) {
    let t = look.theme;
    let live = c.enabled && !silent;
    painter.rect_filled(r, look.radius(), if live { fill } else { t.disabled_clip });
    if look.skin.clip_edge && live {
        let edge = Rect::from_min_size(r.min, vec2(r.width(), 2.0));
        painter.rect_filled(edge, CornerRadius::ZERO, if video { t.accent } else { t.wave });
    }
    if c.selected {
        painter.rect_stroke(r, look.radius(), Stroke::new(2.0, t.accent), StrokeKind::Inside);
    }
    painter.with_clip_rect(r.shrink(3.0)).text(
        r.left_top() + vec2(6.0, 5.0),
        Align2::LEFT_TOP,
        &c.name,
        look.small(),
        if live { t.clip_text } else { t.dim },
    );
}

/// Diagonal lines over a locked track.
fn hatch(painter: &egui::Painter, r: Rect, colour: Color32) {
    let mut x = r.left() - r.height();
    while x < r.right() {
        painter.line_segment([pos2(x, r.bottom()), pos2(x + r.height(), r.top())], Stroke::new(1.0, colour));
        x += 7.0;
    }
}

/// A track header: the destination control with the track's name ("V1"), its name and clip
/// count, then its switches (lock, auto track selector, and show for video or solo and mute for
/// audio). Each switch is a widget with a name, for screen readers and UI tests.
fn track_header(ui: &mut Ui, look: &Look, head: Rect, track: &Track, video: bool) -> Vec<Action> {
    let t = look.theme;
    let painter = ui.painter_at(head);
    painter.rect_filled(head, CornerRadius::ZERO, t.head);
    let mut out = Vec::new();
    let mut flip = |command: &'static str, field: &'static str, value: bool| {
        out.push(Action::Track { track: track.label.clone(), command, field, value })
    };

    // destination control: source edits land on the patched track
    let badge = Rect::from_min_size(head.left_center() + vec2(8.0, -10.0), vec2(28.0, 20.0));
    let r = switch(ui, head, badge, &format!("Destination {}", track.label), track.destination);
    painter.rect_filled(badge, look.radius(), if track.destination { t.accent } else { t.field });
    if r.hovered() && !track.destination {
        painter.rect_stroke(badge, look.radius(), Stroke::new(1.0, t.line), StrokeKind::Inside);
    }
    painter.text(
        badge.center(),
        Align2::CENTER_CENTER,
        &track.label,
        look.mono(look.skin.small),
        if track.destination { t.on_accent } else { t.dim },
    );
    if r.clicked() && !track.destination {
        flip("track.target", "sourcePatch", true);
    }

    // the switches, right-aligned
    let kinds: &[(&str, bool)] = if video {
        &[("Lock", track.locked), ("Auto Select", track.targeted), ("Show", track.enabled)]
    } else {
        &[("Lock", track.locked), ("Auto Select", track.targeted), ("Solo", track.solo), ("Mute", track.muted)]
    };
    let mut x = head.right() - 6.0 - kinds.len() as f32 * (SWITCH + 1.0);
    let switches_left = x;
    for (kind, on) in kinds {
        let rect = Rect::from_min_size(pos2(x, head.center().y - SWITCH / 2.0), vec2(SWITCH, SWITCH));
        x += SWITCH + 1.0;
        let r = switch(ui, head, rect, &format!("{kind} {}", track.label), *on);
        let lit = match *kind {
            "Show" => false,
            _ => *on,
        };
        if lit {
            painter.rect_filled(rect, look.radius(), t.active_bg);
        } else if r.hovered() {
            painter.rect_filled(rect, look.radius(), t.hover);
        }
        let colour = if lit {
            t.accent
        } else if r.hovered() {
            t.text
        } else {
            t.dim
        };
        let stroke = Stroke::new(look.skin.icon_stroke, colour);
        let icon_rect = rect.shrink(4.0);
        match *kind {
            "Lock" => icons::paint(&painter, icon_rect, Icon::Lock, stroke),
            "Auto Select" => icons::paint(&painter, icon_rect, Icon::AutoSelect, stroke),
            "Show" => {
                icons::paint(
                    &painter,
                    icon_rect,
                    Icon::Eye,
                    Stroke::new(look.skin.icon_stroke, if *on { colour } else { t.dim }),
                );
                if !on {
                    painter.line_segment([icon_rect.left_bottom(), icon_rect.right_top()], Stroke::new(1.5, t.error));
                }
            }
            letter => {
                painter.text(rect.center(), Align2::CENTER_CENTER, &letter[..1], look.mono(look.skin.small), colour);
            }
        }
        if r.clicked() {
            match *kind {
                "Lock" => flip("track.set", "locked", !on),
                "Auto Select" => flip("track.target", "targeted", !on),
                "Show" => flip("track.set", "enabled", !on),
                "Solo" => flip("track.set", "solo", !on),
                _ => flip("track.set", "muted", !on),
            }
        }
    }

    // name and clip count between the badge and the switches
    let count = match track.clips.len() {
        0 => "Empty".to_string(),
        1 => "1 clip".to_string(),
        c => format!("{c} clips"),
    };
    let text = Rect::from_min_max(pos2(badge.right() + 8.0, head.top()), pos2(switches_left - 4.0, head.bottom()));
    let clipped = painter.with_clip_rect(text);
    clipped.text(pos2(text.left(), head.center().y - 1.0), Align2::LEFT_BOTTOM, &track.name, look.small(), t.text);
    clipped.text(pos2(text.left(), head.center().y + 1.0), Align2::LEFT_TOP, count, look.mono(10.0), t.dim);
    out
}

/// A clickable switch in a track header, named for screen readers and UI tests.
fn switch(ui: &mut Ui, head: Rect, rect: Rect, label: &str, on: bool) -> Response {
    let r = ui.interact(rect, ui.id().with(("track-switch", label, head.top() as i32)), Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, label));
    r.on_hover_text(label)
}

/// A ruler step (seconds) of at least `min`: 1, 2, 5, 10, 15, 30 s, minutes, …
fn nice_step(min: f64) -> f64 {
    [1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0]
        .into_iter()
        .find(|s| *s >= min)
        .unwrap_or(3600.0)
}

#[cfg(test)]
mod tests {
    use super::nice_step;

    #[test]
    fn ruler_steps_are_round_numbers() {
        assert_eq!(nice_step(0.3), 1.0);
        assert_eq!(nice_step(3.0), 5.0);
        assert_eq!(nice_step(40.0), 60.0);
        assert_eq!(nice_step(1e9), 3600.0);
    }
}
