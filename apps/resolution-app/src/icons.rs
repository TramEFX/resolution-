//! Resolution's own icons (LOG D-018): 16×16 line drawings, stroked in the theme's colours at the
//! skin's weight, never Resolve's artwork (ARCHITECTURE invariant 7). Each is SVG path data,
//! so it can be drawn in any SVG editor and pasted here; [`outline`] reads the subset used
//! (M L H V C S Q A Z, absolute and relative).

use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

use egui::{Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

/// A drawing's polylines, each with whether it is closed.
type Outline = Vec<(Vec<Pos2>, bool)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    MediaPool,
    Effects,
    Index,
    SoundLibrary,
    Mixer,
    Metadata,
    Inspector,
    Selection,
    Trim,
    Blade,
    Insert,
    Overwrite,
    Marker,
    ZoomFit,
    ZoomIn,
    ZoomOut,
    ToStart,
    Reverse,
    Stop,
    Play,
    ToEnd,
    Import,
    Clip,
    Bin,
    Timeline,
    Search,
    PageMedia,
    PageCut,
    PageEdit,
    PageFusion,
    PageColor,
    PageFairlight,
    PageDeliver,
    Lock,
    Eye,
    AutoSelect,
    Snapping,
    Linked,
    SyncLock,
}

impl Icon {
    pub const ALL: [Icon; 39] = [
        Icon::MediaPool,
        Icon::Effects,
        Icon::Index,
        Icon::SoundLibrary,
        Icon::Mixer,
        Icon::Metadata,
        Icon::Inspector,
        Icon::Selection,
        Icon::Trim,
        Icon::Blade,
        Icon::Insert,
        Icon::Overwrite,
        Icon::Marker,
        Icon::ZoomFit,
        Icon::ZoomIn,
        Icon::ZoomOut,
        Icon::ToStart,
        Icon::Reverse,
        Icon::Stop,
        Icon::Play,
        Icon::ToEnd,
        Icon::Import,
        Icon::Clip,
        Icon::Bin,
        Icon::Timeline,
        Icon::Search,
        Icon::PageMedia,
        Icon::PageCut,
        Icon::PageEdit,
        Icon::PageFusion,
        Icon::PageColor,
        Icon::PageFairlight,
        Icon::PageDeliver,
        Icon::Lock,
        Icon::Eye,
        Icon::AutoSelect,
        Icon::Snapping,
        Icon::Linked,
        Icon::SyncLock,
    ];

    /// The drawing, in a 16×16 box.
    pub fn path(self) -> &'static str {
        match self {
            Icon::MediaPool => "M2.5 3.5h4v4h-4zM9.5 3.5h4v4h-4zM2.5 9.5h4v4h-4zM9.5 9.5h4v4h-4z",
            Icon::Effects => "M8 2v3.5M8 10.5V14M2 8h3.5M10.5 8H14M4 4l2 2M10 10l2 2M12 4l-2 2M6 10l-2 2",
            Icon::Index => "M5.5 4h8M5.5 8h8M5.5 12h8M2.5 4h.5M2.5 8h.5M2.5 12h.5",
            Icon::SoundLibrary => {
                "M6 12.5a1.75 1.75 0 1 1-3.5 0a1.75 1.75 0 1 1 3.5 0zM6 12.5V3.5l7-1.5v8.5M13 10a1.75 1.75 0 1 1-3.5 0a1.75 1.75 0 1 1 3.5 0z"
            }
            Icon::Mixer => "M4 2.5v11M8 2.5v11M12 2.5v11M2.5 10h3M6.5 5h3M10.5 8.5h3",
            Icon::Metadata => "M2.5 2.5h5.5l5.5 5.5-5.5 5.5-5.5-5.5zM5.5 5.5h.01",
            Icon::Inspector => "M2.5 4.5h11M2.5 8h11M2.5 11.5h11M5 3v3M10.5 6.5v3M7 10v3",
            Icon::Selection => "M4 2.5l8 5.5-3.5.8 2 4-1.8.9-2-4L4 12z",
            Icon::Trim => "M3.5 2.5h3v11h-3M12.5 2.5h-3v11h3",
            Icon::Blade => "M2.5 13.5l6-6M8.5 7.5l4-4.5 1 1-4.5 4z",
            Icon::Insert => "M8 2.5v6M5.5 6l2.5 2.5L10.5 6M2.5 11h4M9.5 11h4M2.5 13.5h11",
            Icon::Overwrite => "M8 2.5v6M5.5 6l2.5 2.5L10.5 6M2.5 11.5h11v2h-11z",
            Icon::Marker => "M8 13.5l-4-4.5V2.5h8V9z",
            Icon::ZoomFit => "M2.5 3v10M13.5 3v10M5 8h6M6.5 6.5L5 8l1.5 1.5M9.5 6.5L11 8l-1.5 1.5",
            Icon::ZoomIn => "M7 11.5a4.5 4.5 0 1 1 0-9a4.5 4.5 0 1 1 0 9zM10.5 10.5l3 3M5 7h4M7 5v4",
            Icon::ZoomOut => "M7 11.5a4.5 4.5 0 1 1 0-9a4.5 4.5 0 1 1 0 9zM10.5 10.5l3 3M5 7h4",
            Icon::ToStart => "M3.5 3v10M12.5 3.5L6 8l6.5 4.5z",
            Icon::Reverse => "M12 3.5L4.5 8l7.5 4.5z",
            Icon::Stop => "M4 4h8v8H4z",
            Icon::Play => "M4.5 3.5L12 8l-7.5 4.5z",
            Icon::ToEnd => "M12.5 3v10M3.5 3.5L10 8l-6.5 4.5z",
            Icon::Import => "M8 2.5v8M5 7.5l3 3 3-3M2.5 10.5v3h11v-3",
            Icon::Clip => "M2.5 3.5h11v9h-11zM6.5 6l3.5 2-3.5 2z",
            Icon::Bin => "M2.5 4.5h4l1.5 1.5h5.5v7.5h-11z",
            Icon::Timeline => "M2.5 4h7M5.5 8h8M2.5 12h5",
            Icon::Search => "M7 11.5a4.5 4.5 0 1 1 0-9a4.5 4.5 0 1 1 0 9zM10.5 10.5l3 3",
            Icon::PageMedia => "M2.5 4.5h4l1.5 1.5h5.5v7.5h-11z",
            Icon::PageCut => {
                "M5 12.5a2 2 0 1 1-4 0a2 2 0 1 1 4 0zM15 12.5a2 2 0 1 1-4 0a2 2 0 1 1 4 0zM4.3 11L11 2.5M11.7 11L5 2.5"
            }
            Icon::PageEdit => "M2.5 4h7M5.5 8h8M2.5 12h5",
            Icon::PageFusion => {
                "M4 5.5a2 2 0 1 1 0-4a2 2 0 1 1 0 4zM12 9.5a2 2 0 1 1 0-4a2 2 0 1 1 0 4zM6 14.5a2 2 0 1 1 0-4a2 2 0 1 1 0 4zM5.8 4.4l4.4 2.2M10.4 8.8L7.4 11"
            }
            Icon::PageColor => "M8 14a6 6 0 1 1 0-12a6 6 0 1 1 0 12zM8 9a1 1 0 1 1 0-2a1 1 0 1 1 0 2z",
            Icon::PageFairlight => "M2 8h1M4.5 5v6M7 3v10M9.5 6v4M12 4.5v7M14 8h.5",
            Icon::PageDeliver => "M8 10V2.5M5 5.5l3-3 3 3M2.5 9.5v4h11v-4",
            Icon::Lock => "M4 7.5h8v6H4zM5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5",
            Icon::Eye => {
                "M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM8 10a2 2 0 1 1 0-4a2 2 0 1 1 0 4z"
            }
            Icon::AutoSelect => "M2.5 2.5h11v11h-11zM5.5 8l2 2 3.5-4",
            Icon::SyncLock => "M2.5 4.5h4v7h-4zM9.5 4.5h4v7h-4zM6.5 8h3",
            Icon::Snapping => "M3.5 2.5h3v5a1.5 1.5 0 0 0 3 0v-5h3v5a4.5 4.5 0 0 1-9 0zM3.5 5.5h3M9.5 5.5h3",
            Icon::Linked => {
                "M7 9a2.5 2.5 0 0 0 3.5 0l2-2a2.5 2.5 0 0 0-3.5-3.5l-.75.75M9 7a2.5 2.5 0 0 0-3.5 0l-2 2A2.5 2.5 0 0 0 7 12.5l.75-.75"
            }
        }
    }

    /// The drawing as polylines, read once.
    fn outline(self) -> &'static [(Vec<Pos2>, bool)] {
        static CACHE: OnceLock<Vec<Outline>> = OnceLock::new();
        let all = CACHE.get_or_init(|| Icon::ALL.iter().map(|i| outline(i.path())).collect());
        let i = Icon::ALL.iter().position(|i| *i == self).unwrap_or(0);
        &all[i]
    }
}

/// Paint `icon` centred in `rect`, scaled to fit, with `stroke`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, stroke: Stroke) {
    let scale = rect.width().min(rect.height()) / 16.0;
    let origin = rect.center() - vec2(8.0, 8.0) * scale;
    for (points, closed) in icon.outline() {
        let points: Vec<Pos2> = points.iter().map(|p| origin + p.to_vec2() * scale).collect();
        painter.add(if *closed { Shape::closed_line(points, stroke) } else { Shape::line(points, stroke) });
    }
}

/// SVG path data as polylines (each with whether it is closed); curves become short segments.
/// Unknown commands end the path.
pub fn outline(d: &str) -> Outline {
    let tokens = tokenize(d);
    let mut out: Outline = Vec::new();
    let mut current: Vec<Pos2> = Vec::new();
    let (mut pen, mut start) = (pos2(0.0, 0.0), pos2(0.0, 0.0));
    // the last cubic's second control point, for S
    let mut last_control: Option<Pos2> = None;
    let mut i = 0;
    let mut command = 'M';
    let finish = |current: &mut Vec<Pos2>, out: &mut Outline, closed: bool| {
        if current.len() > 1 {
            out.push((std::mem::take(current), closed));
        } else {
            current.clear();
        }
    };
    while i < tokens.len() {
        if let Token::Command(c) = tokens[i] {
            command = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                finish(&mut current, &mut out, true);
                pen = start;
                last_control = None;
                continue;
            }
        }
        let relative = command.is_ascii_lowercase();
        let base = if relative { pen.to_vec2() } else { vec2(0.0, 0.0) };
        let mut take = |n: usize| -> Option<Vec<f32>> {
            let nums: Option<Vec<f32>> = (0..n)
                .map(|k| match tokens.get(i + k) {
                    Some(Token::Number(v)) => Some(*v),
                    _ => None,
                })
                .collect();
            if nums.is_some() {
                i += n;
            }
            nums
        };
        match command.to_ascii_uppercase() {
            'M' => {
                let Some(v) = take(2) else { break };
                finish(&mut current, &mut out, false);
                pen = pos2(v[0], v[1]) + base;
                start = pen;
                current.push(pen);
                // further pairs after a move are lines
                command = if relative { 'l' } else { 'L' };
                last_control = None;
            }
            'L' => {
                let Some(v) = take(2) else { break };
                pen = pos2(v[0], v[1]) + base;
                current.push(pen);
                last_control = None;
            }
            'H' => {
                let Some(v) = take(1) else { break };
                pen.x = if relative { pen.x + v[0] } else { v[0] };
                current.push(pen);
                last_control = None;
            }
            'V' => {
                let Some(v) = take(1) else { break };
                pen.y = if relative { pen.y + v[0] } else { v[0] };
                current.push(pen);
                last_control = None;
            }
            'C' | 'S' => {
                let smooth = command.eq_ignore_ascii_case(&'S');
                let Some(v) = take(if smooth { 4 } else { 6 }) else { break };
                let (c1, rest) = if smooth {
                    (last_control.map_or(pen, |c| pen + (pen - c)), &v[..])
                } else {
                    (pos2(v[0], v[1]) + base, &v[2..])
                };
                let c2 = pos2(rest[0], rest[1]) + base;
                let end = pos2(rest[2], rest[3]) + base;
                for k in 1..=8 {
                    let t = k as f32 / 8.0;
                    let u = 1.0 - t;
                    let p = pen.to_vec2() * (u * u * u)
                        + c1.to_vec2() * (3.0 * u * u * t)
                        + c2.to_vec2() * (3.0 * u * t * t)
                        + end.to_vec2() * (t * t * t);
                    current.push(p.to_pos2());
                }
                last_control = Some(c2);
                pen = end;
            }
            'Q' => {
                let Some(v) = take(4) else { break };
                let (c, end) = (pos2(v[0], v[1]) + base, pos2(v[2], v[3]) + base);
                for k in 1..=8 {
                    let t = k as f32 / 8.0;
                    let u = 1.0 - t;
                    let p = pen.to_vec2() * (u * u) + c.to_vec2() * (2.0 * u * t) + end.to_vec2() * (t * t);
                    current.push(p.to_pos2());
                }
                last_control = None;
                pen = end;
            }
            'A' => {
                let Some(v) = take(7) else { break };
                let end = pos2(v[5], v[6]) + base;
                current.extend(arc(pen, v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0, end));
                last_control = None;
                pen = end;
            }
            _ => break,
        }
    }
    finish(&mut current, &mut out, false);
    out
}

/// An elliptical arc from `from` to `to` as points (excluding `from`), per the SVG spec's
/// endpoint-to-centre conversion (SVG 1.1, appendix F.6.5).
fn arc(from: Pos2, rx: f32, ry: f32, rotation_deg: f32, large: bool, sweep: bool, to: Pos2) -> Vec<Pos2> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || from == to {
        return vec![to];
    }
    let phi = rotation_deg.to_radians();
    let (sin, cos) = phi.sin_cos();
    let d = (from - to) / 2.0;
    let (x1, y1) = (cos * d.x + sin * d.y, -sin * d.x + cos * d.y);
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = (num / den).sqrt();
    if large == sweep {
        k = -k;
    }
    let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let mid = (from.to_vec2() + to.to_vec2()) / 2.0;
    let centre = pos2(cos * cx1 - sin * cy1 + mid.x, sin * cx1 + cos * cy1 + mid.y);
    let angle = |ux: f32, uy: f32| uy.atan2(ux);
    let theta1 = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry) - theta1;
    if sweep && delta < 0.0 {
        delta += TAU;
    } else if !sweep && delta > 0.0 {
        delta -= TAU;
    }
    let steps = ((delta.abs() / (PI / 12.0)).ceil() as usize).max(2);
    (1..=steps)
        .map(|s| {
            let a = theta1 + delta * s as f32 / steps as f32;
            let (x, y) = (rx * a.cos(), ry * a.sin());
            pos2(cos * x - sin * y + centre.x, sin * x + cos * y + centre.y)
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Token {
    Command(char),
    Number(f32),
}

/// Commands and numbers; numbers may run together as SVG allows ("1-3.5", "1.5.5").
fn tokenize(d: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let chars: Vec<char> = d.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            out.push(Token::Command(c));
            i += 1;
        } else if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
            let begin = i;
            i += 1;
            let mut dot = c == '.';
            while i < chars.len() {
                let n = chars[i];
                if n.is_ascii_digit() {
                    i += 1;
                } else if n == '.' && !dot {
                    dot = true;
                    i += 1;
                } else if (n == 'e' || n == 'E') && chars.get(i + 1).is_some_and(|x| x.is_ascii_digit() || *x == '-') {
                    i += 2;
                } else {
                    break;
                }
            }
            if let Ok(v) = chars[begin..i].iter().collect::<String>().parse() {
                out.push(Token::Number(v));
            }
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_inside_its_box() {
        for icon in Icon::ALL {
            let lines = outline(icon.path());
            assert!(!lines.is_empty(), "{icon:?} draws nothing");
            for (points, _) in &lines {
                for p in points {
                    assert!(
                        (-0.5..=16.5).contains(&p.x) && (-0.5..=16.5).contains(&p.y),
                        "{icon:?} leaves its box at {p:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_icon_is_listed_once() {
        for (i, icon) in Icon::ALL.iter().enumerate() {
            assert_eq!(Icon::ALL.iter().position(|x| x == icon), Some(i), "{icon:?} listed twice");
        }
    }

    #[test]
    fn reads_run_together_numbers_and_relative_moves() {
        assert_eq!(
            tokenize("M1-3.5.5h.01"),
            vec![
                Token::Command('M'),
                Token::Number(1.0),
                Token::Number(-3.5),
                Token::Number(0.5),
                Token::Command('h'),
                Token::Number(0.01)
            ]
        );
        let lines = outline("M2 2h4v4h-4z");
        assert_eq!(lines, vec![(vec![pos2(2.0, 2.0), pos2(6.0, 2.0), pos2(6.0, 6.0), pos2(2.0, 6.0)], true)]);
    }

    #[test]
    fn a_full_circle_from_two_arcs_stays_on_its_radius() {
        let lines = outline("M8 14a6 6 0 1 1 0-12a6 6 0 1 1 0 12z");
        let (points, closed) = &lines[0];
        assert!(*closed);
        for p in points {
            let r = (*p - pos2(8.0, 8.0)).length();
            assert!((r - 6.0).abs() < 0.01, "point {p:?} is {r} from the centre");
        }
    }
}
