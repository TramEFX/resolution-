//! The timeline: a ruler, video tracks above audio tracks, clips, marks and the playhead.
//! It reports what the user did; the app turns that into engine commands.
//! Colours are Resolution's own (ARCHITECTURE invariant 7): workflow like Resolve, not its pixels.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::model::Timeline;

pub enum Action {
    Seek(f64),
    Select { clip: u64, add: bool },
}

const HEADER: f32 = 64.0;
const RULER: f32 = 24.0;
const VIDEO_H: f32 = 46.0;
const AUDIO_H: f32 = 34.0;

const BG: Color32 = Color32::from_rgb(24, 27, 33);
const TRACK_BG: Color32 = Color32::from_rgb(31, 35, 43);
const HEADER_BG: Color32 = Color32::from_rgb(38, 43, 52);
const VIDEO_CLIP: Color32 = Color32::from_rgb(62, 104, 150);
const AUDIO_CLIP: Color32 = Color32::from_rgb(58, 128, 98);
const DISABLED_CLIP: Color32 = Color32::from_rgb(70, 74, 82);
const PLAYHEAD: Color32 = Color32::from_rgb(255, 176, 32);
/// (90, 120, 170) at alpha 60, premultiplied.
const MARKS: Color32 = Color32::from_rgba_premultiplied(21, 28, 40, 60);

/// Pixels per second, and the first second shown. `zoom` of `None` fits the whole timeline.
pub struct View {
    pub zoom: Option<f32>,
}

/// Draws the timeline and returns what the user did, plus where each clip was drawn (for
/// hit-testing and UI tests).
pub fn show(ui: &mut Ui, tl: &Timeline, playhead: f64, view: &View) -> (Vec<Action>, Vec<(Rect, u64)>) {
    let mut actions = Vec::new();
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, BG);

    let lane_x = rect.left() + HEADER;
    let lane_w = (rect.width() - HEADER).max(1.0);
    let span = tl.duration.max(1.0) * 1.02;
    let pps = view.zoom.unwrap_or(lane_w / span as f32).max(1.0);
    // keep the playhead in view when zoomed in
    let visible = (lane_w / pps) as f64;
    let first = if visible >= span { 0.0 } else { (playhead - visible / 2.0).clamp(0.0, span - visible) };
    let x_of = |t: f64| lane_x + ((t - first) as f32) * pps;
    let t_of = |x: f32| (first + ((x - lane_x) / pps) as f64).clamp(0.0, tl.duration.max(0.0));

    // ruler with timecode labels
    let ruler = Rect::from_min_size(pos2(lane_x, rect.top()), vec2(lane_w, RULER));
    painter.rect_filled(ruler, CornerRadius::ZERO, HEADER_BG);
    // ticks on whole timecode seconds (24 frames at 23.976), so labels read 00:00:02:00, not 00:00:01:23
    let step = nice_step(80.0 / pps as f64) * tl.nominal_fps() as f64;
    let mut f = (first * tl.fps / step).floor() * step;
    while f / tl.fps <= first + visible {
        let t = f / tl.fps;
        let x = x_of(t);
        if x >= lane_x {
            painter.line_segment(
                [pos2(x, ruler.bottom() - 6.0), pos2(x, ruler.bottom())],
                Stroke::new(1.0, Color32::GRAY),
            );
            painter.text(
                pos2(x + 3.0, ruler.top() + 3.0),
                Align2::LEFT_TOP,
                tl.timecode(t),
                FontId::monospace(10.0),
                Color32::LIGHT_GRAY,
            );
        }
        f += step;
    }

    // tracks: V(n)..V1 on top, then A1..A(n)
    let mut rows: Vec<(String, &[crate::model::Clip], f32, Color32)> = Vec::new();
    for (i, track) in tl.video.iter().enumerate().rev() {
        rows.push((format!("V{}", i + 1), &track.clips, VIDEO_H, VIDEO_CLIP));
    }
    for (i, track) in tl.audio.iter().enumerate() {
        rows.push((format!("A{}", i + 1), &track.clips, AUDIO_H, AUDIO_CLIP));
    }
    let mut y = ruler.bottom() + 2.0;
    let mut hits: Vec<(Rect, u64)> = Vec::new();
    for (label, clips, h, colour) in &rows {
        let lane = Rect::from_min_size(pos2(lane_x, y), vec2(lane_w, *h));
        painter.rect_filled(lane, CornerRadius::ZERO, TRACK_BG);
        let head = Rect::from_min_size(pos2(rect.left(), y), vec2(HEADER - 2.0, *h));
        painter.rect_filled(head, CornerRadius::same(2), HEADER_BG);
        painter.text(
            head.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            Color32::LIGHT_GRAY,
        );
        for c in clips.iter() {
            let (x0, x1) = (x_of(c.start).max(lane_x), x_of(c.end));
            if x1 <= lane_x || x0 >= lane.right() {
                continue;
            }
            let r = Rect::from_x_y_ranges(x0..=x1.min(lane.right()), (y + 2.0)..=(y + h - 2.0));
            painter.rect_filled(r, CornerRadius::same(3), if c.enabled { *colour } else { DISABLED_CLIP });
            if c.selected {
                painter.rect_stroke(r, CornerRadius::same(3), Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
            }
            painter.with_clip_rect(r.shrink(4.0)).text(
                r.left_top() + vec2(5.0, 4.0),
                Align2::LEFT_TOP,
                &c.name,
                FontId::proportional(11.0),
                Color32::WHITE,
            );
            hits.push((r, c.id));
        }
        y += h + 2.0;
    }

    // in/out range and the playhead
    if let (Some(a), Some(b)) = (tl.mark_in, tl.mark_out) {
        painter.rect_filled(
            Rect::from_x_y_ranges(x_of(a)..=x_of(b), ruler.top()..=ruler.bottom()),
            CornerRadius::ZERO,
            MARKS,
        );
    }
    for (mark, glyph) in [(tl.mark_in, "["), (tl.mark_out, "]")] {
        if let Some(m) = mark {
            painter.text(
                pos2(x_of(m), ruler.bottom() - 2.0),
                Align2::CENTER_BOTTOM,
                glyph,
                FontId::monospace(14.0),
                Color32::WHITE,
            );
        }
    }
    let px = x_of(playhead);
    if px >= lane_x {
        painter.line_segment([pos2(px, rect.top()), pos2(px, rect.bottom())], Stroke::new(2.0, PLAYHEAD));
        painter.add(egui::Shape::convex_polygon(
            vec![pos2(px - 6.0, rect.top()), pos2(px + 6.0, rect.top()), pos2(px, rect.top() + 9.0)],
            PLAYHEAD,
            Stroke::NONE,
        ));
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
