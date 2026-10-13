//! The timeline: a ruler, video tracks above audio tracks, clips, marks and the playhead.
//! It reports what the user did; the app turns that into engine commands.
//! Colours come from the theme (LOG D-018); the workflow is Resolve's, the pixels are our own
//! (ARCHITECTURE invariant 7).

use egui::{Align2, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::model::Timeline;
use crate::theme::Look;

pub enum Action {
    Seek(f64),
    Select { clip: u64, add: bool },
}

/// Width of the track headers.
pub const HEADER: f32 = 150.0;
const RULER: f32 = 26.0;

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
    let mut rows: Vec<(String, &str, &[crate::model::Clip], f32, bool)> = Vec::new();
    for (i, track) in tl.video.iter().enumerate().rev() {
        rows.push((format!("V{}", i + 1), track.name.as_str(), &track.clips, look.skin.video_track, true));
    }
    for (i, track) in tl.audio.iter().enumerate() {
        rows.push((format!("A{}", i + 1), track.name.as_str(), &track.clips, look.skin.audio_track, false));
    }
    let mut y = ruler.bottom();
    let mut hits: Vec<(Rect, u64)> = Vec::new();
    for (n, (label, name, clips, h, video)) in rows.iter().enumerate() {
        let lane = Rect::from_min_size(pos2(lane_x, y), vec2(lane_w, *h));
        painter.rect_filled(lane, CornerRadius::ZERO, if n % 2 == 0 { t.track } else { t.track_alt });
        let head = Rect::from_min_size(pos2(rect.left(), y), vec2(HEADER, *h));
        painter.rect_filled(head, CornerRadius::ZERO, t.head);
        painter.line_segment([pos2(rect.left(), lane.bottom()), pos2(rect.right(), lane.bottom())], look.sep_stroke());
        painter.line_segment([pos2(lane_x - 0.5, y), pos2(lane_x - 0.5, lane.bottom())], look.sep_stroke());
        let badge = Rect::from_min_size(head.left_center() + vec2(8.0, -10.0), vec2(28.0, 20.0));
        painter.rect_filled(badge, look.radius(), t.field);
        painter.text(badge.center(), Align2::CENTER_CENTER, label, look.mono(look.skin.small), t.dim);
        let count = match clips.len() {
            0 => "Empty".to_string(),
            1 => "1 clip".to_string(),
            c => format!("{c} clips"),
        };
        let text_x = badge.right() + 8.0;
        painter.with_clip_rect(head.shrink(2.0)).text(
            pos2(text_x, head.center().y - 1.0),
            Align2::LEFT_BOTTOM,
            name,
            look.small(),
            t.text,
        );
        painter.text(pos2(text_x, head.center().y + 1.0), Align2::LEFT_TOP, count, look.mono(10.0), t.dim);
        let fill = if *video { t.video } else { t.audio };
        for c in clips.iter() {
            let (x0, x1) = (x_of(c.start).max(lane_x), x_of(c.end));
            if x1 <= lane_x || x0 >= lane.right() {
                continue;
            }
            let r = Rect::from_x_y_ranges(x0..=(x1 - 1.0).min(lane.right()), (y + 3.0)..=(y + h - 3.0));
            painter.rect_filled(r, look.radius(), if c.enabled { fill } else { t.disabled_clip });
            if look.skin.clip_edge && c.enabled {
                let edge = Rect::from_min_size(r.min, vec2(r.width(), 2.0));
                painter.rect_filled(edge, CornerRadius::ZERO, if *video { t.accent } else { t.wave });
            }
            if c.selected {
                painter.rect_stroke(r, look.radius(), Stroke::new(2.0, t.accent), StrokeKind::Inside);
            }
            painter.with_clip_rect(r.shrink(3.0)).text(
                r.left_top() + vec2(6.0, 5.0),
                Align2::LEFT_TOP,
                &c.name,
                look.small(),
                if c.enabled { t.clip_text } else { t.dim },
            );
            hits.push((r, c.id));
        }
        y += h;
    }

    // in/out range and the playhead
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
