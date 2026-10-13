//! The engine's timeline and project JSON, read into what the UI draws.
//!
//! This is the only module that knows the JSON's shape, which is FilmCraft's today: time in
//! ticks, `video` / `audio` track lists, a bin tree under `root`. A typed, engine-neutral model in
//! `engine-api` should replace it, so the UI never depends on one engine's output (LOG D-012).

use resolution_engine_api::Rate;
use serde_json::Value;

/// Time unit of the inspect JSON (Premiere's ticks, which FilmCraft uses).
pub const TICKS_PER_SECOND: f64 = 254_016_000_000.0;
const TICKS: i64 = 254_016_000_000;

fn seconds(v: &Value) -> Option<f64> {
    v.as_i64().map(|t| t as f64 / TICKS_PER_SECOND)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: u64,
    pub name: String,
    /// Timeline seconds.
    pub start: f64,
    pub end: f64,
    pub enabled: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub name: String,
    pub clips: Vec<Clip>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub name: String,
    /// Frame rate as a number (23.976…); [`Timeline::nominal_fps`] is the timecode base.
    pub fps: f64,
    /// The same, exactly (24000/1001).
    pub rate: Rate,
    /// The playhead's frame, exactly (not rounded through seconds).
    pub playhead_frame: i64,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub playhead: f64,
    pub mark_in: Option<f64>,
    pub mark_out: Option<f64>,
    /// Bottom-up: `video[0]` is V1.
    pub video: Vec<Track>,
    pub audio: Vec<Track>,
}

impl Timeline {
    /// `None` when there is no timeline (the inspect command fails or returns something else).
    pub fn from_json(v: &Value) -> Option<Self> {
        let settings = &v["settings"];
        let rate = &settings["frame_rate"];
        let rate = match (rate["num"].as_i64(), rate["den"].as_i64()) {
            (Some(n), Some(d)) if n > 0 && d > 0 => Rate::new(n, d),
            _ => Rate::new(24, 1),
        };
        let fps = rate.fps();
        let selection: Vec<u64> =
            v["selection"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        let tracks = |key: &str| -> Vec<Track> {
            v[key]
                .as_array()
                .map(|ts| {
                    ts.iter()
                        .map(|t| Track {
                            name: t["name"].as_str().unwrap_or_default().to_string(),
                            clips: t["items"]
                                .as_array()
                                .map(|items| {
                                    items
                                        .iter()
                                        .filter_map(|c| {
                                            let id = c["clip"].as_u64()?;
                                            Some(Clip {
                                                id,
                                                name: c["name"].as_str().unwrap_or_default().to_string(),
                                                start: seconds(&c["start"])?,
                                                end: seconds(&c["end"])?,
                                                enabled: c["enabled"].as_bool().unwrap_or(true),
                                                selected: selection.contains(&id),
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        Some(Self {
            name: v["name"].as_str()?.to_string(),
            fps,
            rate,
            playhead_frame: v["playhead"].as_i64().map_or(0, |t| rate.frame_at_ticks(t, TICKS)),
            width: settings["width"].as_u64().unwrap_or(0) as u32,
            height: settings["height"].as_u64().unwrap_or(0) as u32,
            duration: seconds(&v["duration"]).unwrap_or(0.0),
            playhead: seconds(&v["playhead"]).unwrap_or(0.0),
            mark_in: seconds(&v["in"]),
            mark_out: seconds(&v["out"]),
            video: tracks("video"),
            audio: tracks("audio"),
        })
    }

    /// Whole frames per second for timecode (24 for 23.976, 30 for 29.97).
    pub fn nominal_fps(&self) -> u32 {
        (self.fps.round() as u32).max(1)
    }

    /// `HH:MM:SS:FF`, non-drop.
    pub fn timecode(&self, seconds: f64) -> String {
        self.frame_timecode(self.rate.frame_at(seconds.max(0.0)))
    }

    /// The timecode of `frame`, non-drop.
    pub fn frame_timecode(&self, frame: i64) -> String {
        let fps = u64::from(self.nominal_fps());
        let frames = frame.max(0) as u64;
        let (f, s) = (frames % fps, frames / fps);
        format!("{:02}:{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60, f)
    }

    pub fn clips(&self) -> impl Iterator<Item = (&Track, &Clip)> {
        self.video.iter().chain(&self.audio).flat_map(|t| t.clips.iter().map(move |c| (t, c)))
    }

    pub fn selected(&self) -> Option<(&Track, &Clip)> {
        self.clips().find(|(_, c)| c.selected)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bin,
    /// Footage, audio or a generator: something to edit with.
    Clip,
    Timeline,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaItem {
    /// The engine's item id (clips and timelines) or bin id; ids are unique within a kind.
    pub id: u64,
    pub name: String,
    pub duration: Option<f64>,
    /// Nesting depth (bins).
    pub depth: usize,
    pub kind: Kind,
    /// The timeline open in the timeline panel.
    pub active: bool,
}

/// The project's name, for the interface toolbar.
pub fn project_name(project: &Value) -> Option<String> {
    project["name"].as_str().filter(|n| !n.is_empty()).map(str::to_string)
}

/// The Media Pool, flattened depth-first from the project's bin tree.
pub fn media_items(project: &Value) -> Vec<MediaItem> {
    fn walk(node: &Value, depth: usize, active: Option<u64>, out: &mut Vec<MediaItem>) {
        for child in node["children"].as_array().into_iter().flatten() {
            let (kind, id) = if child.get("children").is_some() {
                (Kind::Bin, child["id"].as_u64())
            } else if child["type"].as_str() == Some("Sequence") {
                (Kind::Timeline, child["item"].as_u64())
            } else {
                (Kind::Clip, child["item"].as_u64())
            };
            if let Some(id) = id {
                out.push(MediaItem {
                    id,
                    name: child["name"].as_str().unwrap_or("(unnamed)").to_string(),
                    duration: seconds(&child["duration"]),
                    depth,
                    kind,
                    active: kind == Kind::Timeline && active == Some(id),
                });
            }
            if kind == Kind::Bin {
                walk(child, depth + 1, active, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&project["root"], 0, project["activeSequence"].as_u64(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const T: i64 = 254_016_000_000;

    fn timeline_json() -> Value {
        json!({
            "name": "Main Edit", "duration": 10 * T, "playhead": 2 * T, "in": null, "out": 5 * T,
            "selection": [30],
            "settings": {"frame_rate": {"num": 24000, "den": 1001}, "width": 1920, "height": 1080},
            "video": [{"name": "Video 1", "items": [{"clip": 29, "name": "a.mp4", "start": 0, "end": 4 * T, "enabled": true}]}],
            "audio": [{"name": "Audio 1", "items": [{"clip": 30, "name": "a.mp4", "start": 0, "end": 4 * T}]}]
        })
    }

    #[test]
    fn reads_the_timeline() {
        let tl = Timeline::from_json(&timeline_json()).expect("timeline");
        assert_eq!(tl.name, "Main Edit");
        assert!((tl.fps - 23.976).abs() < 0.001);
        assert_eq!(tl.nominal_fps(), 24);
        assert_eq!((tl.duration, tl.playhead, tl.mark_in, tl.mark_out), (10.0, 2.0, None, Some(5.0)));
        // 2 s at 23.976 is 47.95 frames: frame 47, exactly
        assert_eq!((tl.rate, tl.playhead_frame), (Rate::new(24_000, 1001), 47));
        assert_eq!(tl.video[0].clips[0].end, 4.0);
        assert_eq!(tl.selected().map(|(_, c)| c.id), Some(30));
        assert!(Timeline::from_json(&json!({"error": "no timeline"})).is_none());
    }

    #[test]
    fn timecode_counts_whole_frames() {
        let tl = Timeline::from_json(&timeline_json()).expect("timeline");
        assert_eq!(tl.timecode(0.0), "00:00:00:00");
        // frame 48 at 23.976 is 2.002 s; timecode counts 24 frames a second
        assert_eq!(tl.timecode(48.0 / tl.fps), "00:00:02:00");
        assert_eq!(tl.timecode(3661.0 * 1.001), "01:01:01:00");
    }

    #[test]
    fn flattens_the_media_pool() {
        let project = json!({"activeSequence": 9, "root": {"id": 0, "name": "Untitled", "children": [
            {"item": 1, "name": "a.mp4", "duration": 3 * T, "type": "Movie"},
            {"id": 7, "name": "Bin 1", "children": [{"item": 2, "name": "b.mov", "duration": T, "type": "Movie"}]},
            {"item": 9, "name": "a.mp4", "duration": 3 * T, "type": "Sequence"}
        ]}});
        let items = media_items(&project);
        let shape: Vec<_> = items.iter().map(|i| (i.id, i.depth, i.kind, i.active)).collect();
        assert_eq!(
            shape,
            [
                (1, 0, Kind::Clip, false),
                (7, 0, Kind::Bin, false),
                (2, 1, Kind::Clip, false),
                (9, 0, Kind::Timeline, true)
            ]
        );
        assert_eq!(items[0].duration, Some(3.0));
    }
}
