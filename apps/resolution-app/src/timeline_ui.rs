//! The timeline: a ruler with markers, track headers, video tracks above audio tracks, clips, marks
//! and the playhead. It reports what the user did; the app turns that into engine commands.
//! Colours come from the theme (LOG D-018); the workflow is Resolve's, the pixels are our own
//! (ARCHITECTURE invariant 7).
//!
//! Editing with the mouse follows Resolve's tools (Reference Manual, "Trimming", pp. 1061-1066):
//! - Selection: drag a clip to move it (up or down to another track of its kind), an edge to
//!   resize it, the cut between two clips to roll it.
//! - Trim: drag an edge to ripple it, the cut's centre to roll it, a clip's upper half to slip
//!   it and its lower half to slide it.
//! - Blade: click a clip to cut it there.
//!
//! A drag shows where the edit will land and becomes one command, one undo step, when it ends.
//! With snapping on, moving edges snap to the playhead, marks, markers and other clips' edges.

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::icons::{self, Icon};
use crate::model::{Clip, Timeline, Track};
use crate::theme::Look;
use crate::waveforms::{Peaks, Ready, column_height};
use resolution_engine_api::{PEAK_RATE, PEAK_SAMPLES};

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
    /// Move a clip so it starts at `start_frame` on track `track` (an engine track id).
    Move {
        clip: u64,
        track: u64,
        start_frame: i64,
    },
    /// Resize (Selection) or ripple (Trim) one edge of a clip by `frames`.
    Trim {
        clip: u64,
        edge: Edge,
        ripple: bool,
        frames: i64,
    },
    Roll {
        left: u64,
        right: u64,
        frames: i64,
    },
    Slip {
        clip: u64,
        frames: i64,
    },
    Slide {
        clip: u64,
        frames: i64,
    },
    /// Cut `clip` at `frame` (timeline frames).
    Blade {
        clip: u64,
        frame: i64,
    },
}

/// The timeline tool: Resolve's Selection, Trim and Blade modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Selection,
    Trim,
    Blade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    In,
    Out,
}

impl Edge {
    pub fn name(self) -> &'static str {
        match self {
            Edge::In => "in",
            Edge::Out => "out",
        }
    }
}

/// What a drag on a clip does, decided by the tool and where it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Move { clip: u64 },
    Resize { clip: u64, edge: Edge },
    Ripple { clip: u64, edge: Edge },
    Roll { left: u64, right: u64 },
    Slip { clip: u64 },
    Slide { clip: u64 },
}

impl Gesture {
    fn cursor(self) -> CursorIcon {
        match self {
            Gesture::Move { .. } => CursorIcon::Grab,
            Gesture::Slip { .. } | Gesture::Slide { .. } => CursorIcon::ResizeHorizontal,
            _ => CursorIcon::ResizeColumn,
        }
    }
}

/// A drag in progress, kept in egui's memory between frames.
#[derive(Clone, Copy, Debug)]
struct Drag {
    gesture: Gesture,
    /// Where the pointer was pressed.
    from: Pos2,
    /// The dragged clip: its row, and its times when the drag started.
    row: usize,
    start: f64,
    end: f64,
}

/// Width of the track headers.
pub const HEADER: f32 = 220.0;
const RULER: f32 = 26.0;
const SWITCH: f32 = 22.0;
/// How close to an edge (px) grabs it rather than the clip.
const EDGE: f32 = 7.0;
/// How close to the cut (px) a Trim-tool drag rolls rather than ripples.
const ROLL: f32 = 3.0;
/// How close (px) a moving edge snaps.
const SNAP: f32 = 8.0;

/// Pixels per second (`None` fits the whole timeline), the tool, and the waveforms read so far.
pub struct View<'a> {
    pub zoom: Option<f32>,
    pub tool: Tool,
    pub waveforms: &'a Ready,
}

/// A clip as drawn this frame.
struct Placed<'a> {
    row: usize,
    track: &'a Track,
    clip: &'a Clip,
    rect: Rect,
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
    let frames_of = |seconds: f64| (seconds * tl.fps).round() as i64;

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
    let rows: Vec<(&Track, bool, Rect)> = {
        let mut y = ruler.bottom();
        tl.video
            .iter()
            .rev()
            .map(|tr| (tr, look.skin.video_track, true))
            .chain(tl.audio.iter().map(|tr| (tr, look.skin.audio_track, false)))
            .map(|(tr, h, video)| {
                let lane = Rect::from_min_size(pos2(lane_x, y), vec2(lane_w, h));
                y += h;
                (tr, video, lane)
            })
            .collect()
    };
    let any_solo = tl.audio.iter().any(|a| a.solo);
    let mut placed: Vec<Placed> = Vec::new();
    for (n, (track, video, lane)) in rows.iter().enumerate() {
        painter.rect_filled(*lane, CornerRadius::ZERO, if n % 2 == 0 { t.track } else { t.track_alt });
        let head = Rect::from_min_max(pos2(rect.left(), lane.top()), pos2(lane_x, lane.bottom()));
        actions.extend(track_header(ui, look, head, track, *video));
        painter.line_segment([pos2(rect.left(), lane.bottom()), pos2(rect.right(), lane.bottom())], look.sep_stroke());
        painter.line_segment([pos2(lane_x - 0.5, lane.top()), pos2(lane_x - 0.5, lane.bottom())], look.sep_stroke());
        // a hidden video track, or an audio track that is muted or silenced by another's solo
        let silent = if *video { !track.enabled } else { track.muted || (any_solo && !track.solo) };
        let fill = if *video { t.video } else { t.audio };
        for c in track.clips.iter() {
            let (x0, x1) = (x_of(c.start).max(lane_x), x_of(c.end));
            if x1 <= lane_x || x0 >= lane.right() {
                continue;
            }
            let r =
                Rect::from_x_y_ranges(x0..=(x1 - 1.0).min(lane.right()), (lane.top() + 3.0)..=(lane.bottom() - 3.0));
            draw_clip(&painter, look, r, c, fill, *video, silent);
            if !*video && let Some(peaks) = view.waveforms.get(&c.item) {
                draw_waveform(&painter, look, r, x_of(c.start), pps, c, peaks, c.enabled && !silent);
            }
            placed.push(Placed { row: n, track, clip: c, rect: r });
        }
        if track.locked {
            hatch(&painter.with_clip_rect(*lane), *lane, t.line);
        }
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

    // ---- editing with the mouse ----
    let drag_id: Id = ui.id().with("timeline-drag");
    let mut drag: Option<Drag> = ui.data(|d| d.get_temp::<Option<Drag>>(drag_id)).flatten();
    let pointer = response.hover_pos().or(response.interact_pointer_pos());
    let in_lanes = |p: Pos2| p.x >= lane_x && p.y > ruler.bottom();
    // where moving edges may snap to
    let snap_targets = |exclude: &[u64]| -> Vec<f64> {
        let mut v = vec![0.0, playhead];
        v.extend(tl.mark_in);
        v.extend(tl.mark_out);
        v.extend(tl.markers.iter().map(|m| m.start));
        for p in placed.iter().filter(|p| !exclude.contains(&p.clip.id)) {
            v.extend([p.clip.start, p.clip.end]);
        }
        v
    };
    let tolerance = f64::from(SNAP / pps);

    if response.drag_started()
        && view.tool != Tool::Blade
        && let Some(from) = ui.input(|i| i.pointer.press_origin()).filter(|p| in_lanes(*p))
        && let Some((gesture, i)) = gesture_at(from, view.tool, &placed, tl.fps)
    {
        let c = placed[i].clip;
        drag = Some(Drag { gesture, from, row: placed[i].row, start: c.start, end: c.end });
    }

    if let Some(d) = drag {
        let p = pointer.unwrap_or(d.from);
        let raw = f64::from((p.x - d.from.x) / pps);
        // edges that move with the drag, and clips whose edges are not snap targets
        let (moving, exclude): (Vec<f64>, Vec<u64>) = match d.gesture {
            Gesture::Move { clip } | Gesture::Slide { clip } => (vec![d.start, d.end], linked(&placed, clip)),
            Gesture::Resize { clip, edge } | Gesture::Ripple { clip, edge } => {
                (vec![if edge == Edge::In { d.start } else { d.end }], linked(&placed, clip))
            }
            Gesture::Roll { left, right } => {
                let mut ex = linked(&placed, left);
                ex.extend(linked(&placed, right));
                (vec![d.end], ex)
            }
            Gesture::Slip { .. } => (Vec::new(), Vec::new()),
        };
        let delta = if tl.snapping { snap(raw, &moving, &snap_targets(&exclude), tolerance) } else { raw };
        let frames = frames_of(delta);
        let shown = frames as f64 / tl.fps;
        ui.ctx().set_cursor_icon(if matches!(d.gesture, Gesture::Move { .. }) {
            CursorIcon::Grabbing
        } else {
            d.gesture.cursor()
        });
        // the row a moved clip lands on: the one under the pointer, if it holds the same kind
        let target_row = match d.gesture {
            Gesture::Move { .. } => rows
                .iter()
                .position(|(_, _, lane)| (lane.top()..lane.bottom()).contains(&p.y))
                .filter(|r| rows[*r].1 == rows[d.row].1)
                .unwrap_or(d.row),
            _ => d.row,
        };
        draw_preview(&painter, look, &rows, d, target_row, shown, x_of);
        let label = format!("{}{}", if frames < 0 { "-" } else { "+" }, tl.frame_timecode(frames.abs()));
        painter.text(p + vec2(12.0, -14.0), Align2::LEFT_BOTTOM, label, look.mono(11.0), t.text);

        if response.drag_stopped() {
            let start_frame = frames_of(d.start) + frames;
            let action = match d.gesture {
                Gesture::Move { clip } if frames != 0 || target_row != d.row => {
                    Some(Action::Move { clip, track: rows[target_row].0.id, start_frame })
                }
                Gesture::Resize { clip, edge } if frames != 0 => {
                    Some(Action::Trim { clip, edge, ripple: false, frames })
                }
                Gesture::Ripple { clip, edge } if frames != 0 => {
                    Some(Action::Trim { clip, edge, ripple: true, frames })
                }
                Gesture::Roll { left, right } if frames != 0 => Some(Action::Roll { left, right, frames }),
                // the media moves with the pointer: dragging right shows earlier frames (LOG B-022)
                Gesture::Slip { clip } if frames != 0 => Some(Action::Slip { clip, frames: -frames }),
                Gesture::Slide { clip } if frames != 0 => Some(Action::Slide { clip, frames }),
                _ => None,
            };
            actions.extend(action);
            drag = None;
        }
    } else if let Some(p) = pointer.filter(|p| in_lanes(*p)) {
        // hovering: show what a drag here would do
        match view.tool {
            Tool::Blade => {
                if let Some(c) = placed.iter().find(|c| c.rect.contains(p) && !c.track.locked) {
                    let raw = t_of(p.x);
                    let at = raw + if tl.snapping { snap(0.0, &[raw], &[playhead], tolerance) } else { 0.0 };
                    let frame = frames_of(at);
                    let x = x_of(frame as f64 / tl.fps);
                    painter.line_segment(
                        [pos2(x, c.rect.top() - 2.0), pos2(x, c.rect.bottom() + 2.0)],
                        Stroke::new(1.5, t.accent),
                    );
                    ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                    if response.clicked() {
                        actions.push(Action::Blade { clip: c.clip.id, frame });
                    }
                }
            }
            tool => {
                if let Some((g, _)) = gesture_at(p, tool, &placed, tl.fps) {
                    ui.ctx().set_cursor_icon(g.cursor());
                }
            }
        }
    }
    ui.data_mut(|d| d.insert_temp(drag_id, drag));

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

    // clicks: a clip selects it (Shift adds); the ruler or an empty lane moves the playhead.
    // A drag that is not editing a clip scrubs.
    let editing = drag.is_some() || response.drag_stopped();
    if let Some(p) = response.interact_pointer_pos().filter(|p| p.x >= lane_x) {
        let on_clip = placed.iter().find(|c| c.rect.contains(p)).map(|c| c.clip.id);
        if response.clicked() && view.tool != Tool::Blade {
            match on_clip {
                Some(clip) if p.y > ruler.bottom() => {
                    actions.push(Action::Select { clip, add: ui.input(|i| i.modifiers.shift) })
                }
                _ => actions.push(Action::Seek(t_of(p.x))),
            }
        } else if (response.clicked() && on_clip.is_none())
            || (response.dragged() && !editing && (on_clip.is_none() || p.y <= ruler.bottom()))
        {
            // the Blade tool's clicks off a clip, and drags that are not edits, scrub
            actions.push(Action::Seek(t_of(p.x)));
        }
    }
    let hits = placed.iter().map(|c| (c.rect, c.clip.id)).collect();
    (actions, hits)
}

/// The gesture a drag starting at `p` would make, and the clip it grabs.
fn gesture_at(p: Pos2, tool: Tool, placed: &[Placed], fps: f64) -> Option<(Gesture, usize)> {
    let i = placed.iter().position(|c| c.rect.contains(p))?;
    let c = &placed[i];
    if c.track.locked {
        return None;
    }
    let half_frame = 0.5 / fps.max(1.0);
    // the clip meeting this one at its in or out point, on the same track
    let neighbour = |at: f64, before: bool| {
        c.track
            .clips
            .iter()
            .find(|o| o.id != c.clip.id && (if before { o.end } else { o.start } - at).abs() < half_frame)
            .map(|o| o.id)
    };
    let edge = EDGE.min(c.rect.width() / 4.0);
    let (to_in, to_out) = (p.x - c.rect.left(), c.rect.right() - p.x);
    let gesture = if to_in < edge {
        match (tool, neighbour(c.clip.start, true)) {
            (Tool::Selection, Some(left)) => Gesture::Roll { left, right: c.clip.id },
            (Tool::Trim, Some(left)) if to_in < ROLL => Gesture::Roll { left, right: c.clip.id },
            (Tool::Selection, None) => Gesture::Resize { clip: c.clip.id, edge: Edge::In },
            _ => Gesture::Ripple { clip: c.clip.id, edge: Edge::In },
        }
    } else if to_out < edge {
        match (tool, neighbour(c.clip.end, false)) {
            (Tool::Selection, Some(right)) => Gesture::Roll { left: c.clip.id, right },
            (Tool::Trim, Some(right)) if to_out < ROLL => Gesture::Roll { left: c.clip.id, right },
            (Tool::Selection, None) => Gesture::Resize { clip: c.clip.id, edge: Edge::Out },
            _ => Gesture::Ripple { clip: c.clip.id, edge: Edge::Out },
        }
    } else {
        match tool {
            Tool::Trim if p.y < c.rect.center().y => Gesture::Slip { clip: c.clip.id },
            Tool::Trim => Gesture::Slide { clip: c.clip.id },
            _ => Gesture::Move { clip: c.clip.id },
        }
    };
    // a roll grabs the outgoing clip, so the preview starts from its out point
    let i = match gesture {
        Gesture::Roll { left, .. } => placed.iter().position(|o| o.clip.id == left).unwrap_or(i),
        _ => i,
    };
    Some((gesture, i))
}

/// `clip` and the clips that move with it: on other tracks with exactly its times (its linked
/// audio or video, as the engine keeps them).
fn linked(placed: &[Placed], clip: u64) -> Vec<u64> {
    let Some(c) = placed.iter().find(|p| p.clip.id == clip) else { return vec![clip] };
    placed.iter().filter(|p| p.clip.start == c.clip.start && p.clip.end == c.clip.end).map(|p| p.clip.id).collect()
}

/// `delta` (s) nudged so that one of the `moving` edges lands on a target, if one is within
/// `tolerance`.
fn snap(delta: f64, moving: &[f64], targets: &[f64], tolerance: f64) -> f64 {
    let mut best: Option<f64> = None;
    for m in moving {
        for t in targets {
            let adjust = t - (m + delta);
            if adjust.abs() <= tolerance && best.is_none_or(|b| adjust.abs() < b.abs()) {
                best = Some(adjust);
            }
        }
    }
    delta + best.unwrap_or(0.0)
}

/// Where the dragged clip (or cut) will land: an accent outline over the timeline.
fn draw_preview(
    painter: &egui::Painter,
    look: &Look,
    rows: &[(&Track, bool, Rect)],
    d: Drag,
    target_row: usize,
    shown: f64,
    x_of: impl Fn(f64) -> f32,
) {
    let t = look.theme;
    let lane = rows[d.row].2;
    let y = |row: usize| (rows[row].2.top() + 3.0)..=(rows[row].2.bottom() - 3.0);
    let outline = |r: Rect| {
        painter.rect_filled(r, look.radius(), t.accent.gamma_multiply(0.15));
        painter.rect_stroke(r, look.radius(), Stroke::new(2.0, t.accent), StrokeKind::Outside);
    };
    let (a, b) = (x_of(d.start), x_of(d.end));
    let dx = x_of(d.start + shown) - a;
    match d.gesture {
        Gesture::Move { .. } | Gesture::Slide { .. } => {
            outline(Rect::from_x_y_ranges((a + dx)..=(b + dx), y(target_row)))
        }
        Gesture::Resize { edge, .. } | Gesture::Ripple { edge, .. } => {
            let (l, r) = if edge == Edge::In { (a + dx, b) } else { (a, b + dx) };
            outline(Rect::from_x_y_ranges(l.min(r - 1.0)..=r.max(l + 1.0), y(d.row)));
        }
        Gesture::Roll { .. } => {
            painter.line_segment(
                [pos2(b + dx, lane.top() - 4.0), pos2(b + dx, lane.bottom() + 4.0)],
                Stroke::new(2.0, t.accent),
            );
        }
        Gesture::Slip { .. } => outline(Rect::from_x_y_ranges(a..=b, y(d.row))),
    }
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

/// The clip's sound as a waveform under its name, from its media's peaks: one column per pixel,
/// mirrored about the middle.
#[allow(clippy::too_many_arguments)]
fn draw_waveform(
    painter: &egui::Painter,
    look: &Look,
    r: Rect,
    clip_x: f32,
    pps: f32,
    c: &Clip,
    peaks: &Peaks,
    live: bool,
) {
    if peaks.buckets.is_empty() || c.speed <= 0.0 || c.reverse {
        return;
    }
    let area = Rect::from_min_max(pos2(r.left() + 1.0, r.top() + 18.0), pos2(r.right() - 1.0, r.bottom() - 2.0));
    if area.height() < 4.0 {
        return;
    }
    let (mid, half) = (area.center().y, area.height() / 2.0);
    let per_bucket = PEAK_SAMPLES as f64 / f64::from(PEAK_RATE);
    let colour = if live { look.theme.wave.gamma_multiply(0.8) } else { look.theme.dim.gamma_multiply(0.4) };
    let source_at = |x: f32| c.source_in + f64::from((x - clip_x) / pps) * c.speed;
    let visible = painter.clip_rect().intersect(area);
    let mut mesh = egui::Mesh::default();
    for x in (visible.left().floor() as i32)..(visible.right().ceil() as i32) {
        let x = x as f32;
        let b0 = (source_at(x) / per_bucket).floor().max(0.0) as usize;
        let b1 = ((source_at(x + 1.0) / per_bucket).ceil().max(0.0) as usize).max(b0 + 1).min(peaks.buckets.len());
        if b0 >= b1 {
            continue;
        }
        let m = peaks.buckets[b0..b1].iter().fold(0f32, |m, (lo, hi)| m.max(lo.abs()).max(hi.abs()));
        let h = column_height(m, peaks.loudest) * half;
        if h > 0.3 {
            mesh.add_colored_rect(Rect::from_min_max(pos2(x, mid - h), pos2(x + 1.0, mid + h)), colour);
        }
    }
    painter.add(mesh);
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
        &[
            ("Lock", track.locked),
            ("Sync Lock", track.sync_lock),
            ("Auto Select", track.targeted),
            ("Show", track.enabled),
        ]
    } else {
        &[
            ("Lock", track.locked),
            ("Sync Lock", track.sync_lock),
            ("Auto Select", track.targeted),
            ("Solo", track.solo),
            ("Mute", track.muted),
        ]
    };
    let mut x = head.right() - 6.0 - kinds.len() as f32 * (SWITCH + 1.0);
    let switches_left = x;
    for (kind, on) in kinds {
        let rect = Rect::from_min_size(pos2(x, head.center().y - SWITCH / 2.0), vec2(SWITCH, SWITCH));
        x += SWITCH + 1.0;
        let r = switch(ui, head, rect, &format!("{kind} {}", track.label), *on);
        let lit = *kind != "Show" && *on;
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
            "Sync Lock" => icons::paint(&painter, icon_rect, Icon::SyncLock, stroke),
            "Auto Select" => icons::paint(&painter, icon_rect, Icon::AutoSelect, stroke),
            "Show" => {
                icons::paint(&painter, icon_rect, Icon::Eye, stroke);
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
                "Sync Lock" => flip("track.set", "syncLock", !on),
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
    use super::{nice_step, snap};

    #[test]
    fn ruler_steps_are_round_numbers() {
        assert_eq!(nice_step(0.3), 1.0);
        assert_eq!(nice_step(3.0), 5.0);
        assert_eq!(nice_step(40.0), 60.0);
        assert_eq!(nice_step(1e9), 3600.0);
    }

    #[test]
    fn edges_snap_to_the_nearest_target_within_reach() {
        // an edge at 10 s dragged 1.9 s lands on the playhead at 12 s, 0.1 s away
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(near(snap(1.9, &[10.0], &[12.0, 20.0], 0.2), 2.0));
        // either edge of a moved clip may snap; the closer wins
        assert!(near(snap(1.0, &[10.0, 14.0], &[11.15, 15.05], 0.2), 1.05));
        // nothing within reach: the drag is kept as is
        assert!(near(snap(1.0, &[10.0], &[30.0], 0.2), 1.0));
    }
}
