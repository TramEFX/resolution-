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
    /// The media item it plays.
    pub item: u64,
    /// Where in its media the clip starts (seconds).
    pub source_in: f64,
    /// Playback speed (1.0 is 100%); `reverse` plays it backwards.
    pub speed: f64,
    pub reverse: bool,
    pub enabled: bool,
    pub selected: bool,
    /// Its effects start with a Lumetri Color: the Color page's grade (LOG D-028), or a Lumetri
    /// made elsewhere.
    pub graded: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// The engine's track id.
    pub id: u64,
    /// "V1", "A2": how Resolve and the vocabulary name it.
    pub label: String,
    pub name: String,
    pub clips: Vec<Clip>,
    pub locked: bool,
    /// Ripples with edits on other tracks (Resolve's sync lock).
    pub sync_lock: bool,
    /// Video tracks: shown. Audio tracks use `muted`.
    pub enabled: bool,
    pub muted: bool,
    pub solo: bool,
    /// Auto track selector: edits and navigation that work on tracks use this one.
    pub targeted: bool,
    /// Destination control: source edits land here.
    pub destination: bool,
}

/// A timeline marker.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub id: u64,
    /// Timeline seconds.
    pub start: f64,
    pub duration: f64,
    pub name: String,
    pub color: [u8; 3],
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
    pub markers: Vec<Marker>,
    pub snapping: bool,
    pub linked_selection: bool,
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
        let ids = |v: &Value| -> Vec<u64> {
            v.as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default()
        };
        let targeting = &v["targeting"];
        let targeted = ids(&targeting["targeted"]);
        let dests = [targeting["videoDest"].as_u64(), targeting["audioDest"].as_u64()];
        let tracks = |key: &str| -> Vec<Track> {
            let prefix = if key == "video" { "V" } else { "A" };
            v[key]
                .as_array()
                .map(|ts| {
                    ts.iter()
                        .enumerate()
                        .map(|(n, t)| Track {
                            id: t["id"].as_u64().unwrap_or_default(),
                            label: format!("{prefix}{}", n + 1),
                            locked: t["locked"].as_bool().unwrap_or(false),
                            sync_lock: t["syncLock"].as_bool().unwrap_or(true),
                            enabled: t["enabled"].as_bool().unwrap_or(true),
                            muted: t["muted"].as_bool().unwrap_or(false),
                            solo: t["solo"].as_bool().unwrap_or(false),
                            targeted: t["id"].as_u64().is_some_and(|id| targeted.contains(&id)),
                            destination: t["id"].as_u64().is_some_and(|id| dests.contains(&Some(id))),
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
                                                item: c["item"].as_u64().unwrap_or_default(),
                                                source_in: seconds(&c["sourceIn"]).unwrap_or(0.0),
                                                speed: c["speed"].as_f64().unwrap_or(1.0),
                                                reverse: c["reverse"].as_bool().unwrap_or(false),
                                                enabled: c["enabled"].as_bool().unwrap_or(true),
                                                selected: selection.contains(&id),
                                                graded: c["effects"][0]["effect"] == "lumetri",
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
            markers: v["markers"]
                .as_array()
                .map(|ms| {
                    ms.iter()
                        .filter_map(|m| {
                            Some(Marker {
                                id: m["id"].as_u64()?,
                                start: seconds(&m["start"])?,
                                duration: seconds(&m["duration"]).unwrap_or(0.0),
                                name: m["name"].as_str().unwrap_or_default().to_string(),
                                color: label_rgb(m["color"].as_str().unwrap_or_default()),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            snapping: v["snapping"].as_bool().unwrap_or(true),
            linked_selection: v["linkedSelection"].as_bool().unwrap_or(true),
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
        timecode(frame, self.nominal_fps())
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

/// A label colour by its engine name (FilmCraft's labels, used for markers), brightened to read
/// on a dark ruler; unknown names are a neutral light grey.
pub fn label_rgb(name: &str) -> [u8; 3] {
    let rgb: [u8; 3] = match name {
        "Violet" => [0x38, 0x0e, 0xa7],
        "Iris" => [0x1d, 0x4a, 0x64],
        "Caribbean" => [0x35, 0x54, 0x18],
        "Lavender" => [0x6b, 0x1c, 0x82],
        "Cerulean" => [0x23, 0x53, 0x5a],
        "Forest" => [0x40, 0x4a, 0x11],
        "Rose" => [0x80, 0x18, 0x36],
        "Mango" => [0x80, 0x3f, 0x17],
        "Purple" => [0x59, 0x0d, 0xb0],
        "Blue" => [0x19, 0x2d, 0x94],
        "Teal" => [0x1f, 0x4d, 0x45],
        "Magenta" => [0x79, 0x1c, 0x56],
        "Tan" => [0x6c, 0x5b, 0x47],
        "Green" => [0x29, 0x5c, 0x2d],
        "Brown" => [0x58, 0x3d, 0x14],
        "Yellow" => [0x6e, 0x66, 0x28],
        _ => return [0xc8, 0xc8, 0xc8],
    };
    // halfway to white
    rgb.map(|c| ((u16::from(c) + 255) / 2) as u8)
}

/// The clip open in the source viewer, from `source.inspect`. Frames count from the first of
/// its range (a subclip's start), in its own frame rate.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceClip {
    pub item: u64,
    pub name: String,
    pub rate: Rate,
    /// Frames in its range.
    pub frames: i64,
    pub playhead_frame: i64,
    pub mark_in: Option<i64>,
    /// The last frame in (inclusive), as Resolve's Out mark.
    pub mark_out: Option<i64>,
    /// The media's own timecode at frame 0 (in frames), when the file carries one.
    pub start_timecode: i64,
}

impl SourceClip {
    /// `None` when nothing is open (`{"item": null}`) or the JSON is something else.
    pub fn from_json(v: &Value) -> Option<Self> {
        let item = v["item"].as_u64()?;
        let rate = match (v["frameRate"]["num"].as_i64(), v["frameRate"]["den"].as_i64()) {
            (Some(n), Some(d)) if n > 0 && d > 0 => Rate::new(n, d),
            _ => Rate::new(24, 1),
        };
        let start = v["start"].as_i64().unwrap_or(0);
        let frame = |key: &str| v[key].as_i64().map(|t| rate.frame_at_ticks(t - start, TICKS));
        let span = v["end"].as_i64().unwrap_or(start) - start;
        let last = rate.frame_at_ticks(span, TICKS);
        // frames that start before the end
        let frames = if frame_ticks(rate, last) < span { last + 1 } else { last };
        Some(Self {
            item,
            name: v["name"].as_str().unwrap_or_default().to_string(),
            rate,
            frames: frames.max(1),
            playhead_frame: frame("playhead").unwrap_or(0),
            mark_in: frame("markIn"),
            mark_out: frame("markOut"),
            start_timecode: v["startTimecodeFrames"].as_i64().unwrap_or(0),
        })
    }

    /// `HH:MM:SS:FF` in the media's own timecode, non-drop.
    pub fn timecode(&self, frame: i64) -> String {
        timecode(self.start_timecode + frame, (self.rate.fps().round() as u32).max(1))
    }
}

/// `HH:MM:SS:FF` for `frame` at `fps` whole frames a second, non-drop.
pub fn timecode(frame: i64, fps: u32) -> String {
    let fps = u64::from(fps.max(1));
    let frames = frame.max(0) as u64;
    let (f, s) = (frames % fps, frames / fps);
    format!("{:02}:{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60, f)
}

/// The engine time (ticks) at which timeline frame `frame` starts, exactly.
pub fn frame_ticks(rate: Rate, frame: i64) -> i64 {
    (i128::from(frame) * i128::from(TICKS) * i128::from(rate.den) / i128::from(rate.num.max(1))) as i64
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
            "video": [{"id": 1, "name": "Video 1", "locked": true, "items": [{"clip": 29, "name": "a.mp4", "start": 0, "end": 4 * T, "enabled": true}]}],
            "audio": [{"id": 2, "name": "Audio 1", "muted": true, "items": [{"clip": 30, "name": "a.mp4", "start": 0, "end": 4 * T}]}],
            "markers": [{"id": 5, "start": 3 * T, "duration": 0, "name": "beat", "color": "Blue"}],
            "targeting": {"targeted": [1], "videoDest": 1, "audioDest": null},
            "snapping": false
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
        let (v1, a1) = (&tl.video[0], &tl.audio[0]);
        assert_eq!((v1.label.as_str(), v1.locked, v1.targeted, v1.destination), ("V1", true, true, true));
        assert_eq!((a1.label.as_str(), a1.muted, a1.targeted, a1.destination), ("A1", true, false, false));
        assert_eq!((tl.markers[0].start, tl.markers[0].name.as_str()), (3.0, "beat"));
        assert_eq!((tl.snapping, tl.linked_selection), (false, true));
        assert!(Timeline::from_json(&json!({"error": "no timeline"})).is_none());
    }

    #[test]
    fn reads_the_source_clip() {
        assert_eq!(SourceClip::from_json(&json!({"item": null})), None);
        let s = SourceClip::from_json(&json!({
            "item": 4, "name": "a.mp4", "start": 0, "end": 10 * T, "playhead": 2 * T, "markIn": T, "markOut": null,
            "frameRate": {"num": 25, "den": 1}, "startTimecodeFrames": 90_000
        }))
        .expect("source clip");
        assert_eq!((s.frames, s.playhead_frame, s.mark_in, s.mark_out), (250, 50, Some(25), None));
        // its own timecode: 01:00:00:00 at frame 0
        assert_eq!(s.timecode(51), "01:00:02:01");
    }

    #[test]
    fn frames_convert_to_ticks_exactly() {
        // frame 48 at 23.976 starts at 2.002 s
        assert_eq!(frame_ticks(Rate::new(24_000, 1001), 48), 2_002 * T / 1000);
        assert_eq!(frame_ticks(Rate::new(25, 1), 50), 2 * T);
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

/// A clip's grade, as `color.inspect` and the other `color.*` commands answer.
#[derive(Clone, Debug, PartialEq)]
pub struct GradeView {
    pub clip: u64,
    pub grade: resolution_grade::Grade,
    /// The clip has Lumetri effects made outside the Color page; a grade edit replaces them.
    pub foreign: bool,
    /// The clip's picture size, which windows are measured on.
    pub size: (u32, u32),
}

impl GradeView {
    pub fn from_json(v: &Value) -> Option<Self> {
        let nodes = v["nodes"]
            .as_array()?
            .iter()
            .map(|n| {
                // the node's own fields, without what `color.inspect` adds for reading
                let mut n = n.clone();
                if let Some(o) = n.as_object_mut() {
                    o.remove("number");
                    o.remove("neutral");
                }
                serde_json::from_value(n).ok()
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            clip: v["clip"].as_u64()?,
            grade: resolution_grade::Grade { nodes },
            foreign: v["foreign"].as_bool().unwrap_or(false),
            size: (v["width"].as_u64().unwrap_or(1920) as u32, v["height"].as_u64().unwrap_or(1080) as u32),
        })
    }
}
