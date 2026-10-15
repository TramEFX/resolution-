use resolution_engine_api::Engine;
use serde_json::{Value, json};

use crate::FilmCraftEngine;

/// The demo timeline with `name` open in the source viewer, marked from frame 24 to frame 71: two
/// seconds of it, 48 frames.
fn with_source(name: &str) -> (FilmCraftEngine, u64) {
    let mut e = FilmCraftEngine::new();
    e.execute("raw.filmcraft/file.openDemoProject", json!({})).unwrap();
    let project = e.execute("project.inspect", json!({})).unwrap().result;
    fn find(node: &Value, name: &str) -> Option<u64> {
        node["children"]
            .as_array()?
            .iter()
            .find_map(|c| if c["name"] == name { c["item"].as_u64() } else { find(c, name) })
    }
    let item = find(&project["root"], name).unwrap();
    e.execute("source.open", json!({ "item": item })).unwrap();
    e.execute("source.seek", json!({ "frame": 24 })).unwrap();
    e.execute("mark.in", json!({ "target": "source" })).unwrap();
    e.execute("source.seek", json!({ "frame": 71 })).unwrap();
    e.execute("mark.out", json!({ "target": "source" })).unwrap();
    (e, item)
}

/// Seconds of a tick count.
fn secs(t: filmcraft_engine::time::Tick) -> f64 {
    t.seconds()
}

/// The clips of a video track (`v`, from 0) as (media item, start s, length s), in order.
fn track(e: &FilmCraftEngine, v: usize) -> Vec<(u64, f64, f64)> {
    let seq = e.session.active_sequence().unwrap();
    seq.video_tracks
        .get(v)
        .map_or(Vec::new(), |t| t.items.iter().map(|it| (it.item.0, secs(it.start), secs(it.duration))).collect())
}

/// The clips of an audio track (`a`, from 0), as [`track`] gives a video track's.
fn audio(e: &FilmCraftEngine, a: usize) -> Vec<(u64, f64, f64)> {
    let seq = e.session.active_sequence().unwrap();
    seq.audio_tracks
        .get(a)
        .map_or(Vec::new(), |t| t.items.iter().map(|it| (it.item.0, secs(it.start), secs(it.duration))).collect())
}

fn seek(e: &mut FilmCraftEngine, seconds: f64) {
    e.execute("playhead.set", json!({ "seconds": seconds })).unwrap();
}

/// Two seconds of the source, at 23.976: 48 frames.
const TWO_S: f64 = 48.0 * 1001.0 / 24000.0;

#[test]
fn append_at_end_puts_the_source_after_the_last_clip_whatever_the_marks() {
    let (mut e, item) = with_source("City_Night_Drive.mp4");
    // a timeline In that an ordinary overwrite would use
    seek(&mut e, 3.0);
    e.execute("mark.in", json!({})).unwrap();
    let mark = e.session.active_sequence().unwrap().mark_in;
    let end = {
        let seq = e.session.active_sequence().unwrap();
        seq.all_tracks().flat_map(|t| &t.items).map(|it| secs(it.end())).fold(0.0, f64::max)
    };
    let v1 = track(&e, 0);
    let out = e.execute("edit.appendAtEnd", json!({})).unwrap();
    assert_eq!(out.undo_label.as_deref(), Some("Append at End of Timeline"));
    let after = track(&e, 0);
    let last = *after.last().unwrap();
    assert_eq!(last.0, item);
    assert!((last.1 - end).abs() < 0.05 && (last.2 - TWO_S).abs() < 0.05, "{last:?} after {end}");
    assert_eq!(after.len(), v1.len() + 1, "nothing else moved");
    assert_eq!(&after[..v1.len()], &v1[..]);
    assert_eq!(e.session.active_sequence().unwrap().mark_in, mark, "the timeline's In kept");
    // one undo step
    e.execute("edit.undo", json!({})).unwrap();
    assert_eq!(track(&e, 0), v1);
}

#[test]
fn replace_keeps_the_clips_place_and_length() {
    let (mut e, item) = with_source("City_Night_Drive.mp4");
    seek(&mut e, 2.0);
    let before = track(&e, 0);
    let out = e.execute("edit.replace", json!({})).unwrap();
    assert!(out.undo_label.is_some());
    let after = track(&e, 0);
    assert_eq!(after.len(), before.len());
    assert_eq!(after[0].0, item, "the clip under the playhead takes the source");
    assert_eq!((after[0].1, after[0].2), (before[0].1, before[0].2), "in its place, at its length");
    assert_eq!(&after[1..], &before[1..]);
}

#[test]
fn ripple_overwrite_replaces_the_clip_and_moves_what_follows() {
    let (mut e, item) = with_source("City_Night_Drive.mp4");
    // FilmCraft will not ripple past the demo's music while its track is sync-locked (LOG B-023)
    e.execute("track.set", json!({ "track": "A2", "syncLock": false })).unwrap();
    let before = track(&e, 0);
    // Misty Forest, the fifth clip on V1
    let (k, misty) = before.iter().enumerate().find(|(_, c)| c.1 > 15.0).map(|(k, c)| (k, *c)).unwrap();
    seek(&mut e, misty.1 + misty.2 / 2.0);
    let out = e.execute("edit.rippleOverwrite", json!({})).unwrap();
    assert_eq!(out.undo_label.as_deref(), Some("Ripple Overwrite"));
    let after = track(&e, 0);
    assert_eq!(after.len(), before.len(), "{after:?}");
    assert_eq!(after[k].0, item);
    assert!((after[k].1 - misty.1).abs() < 0.05 && (after[k].2 - TWO_S).abs() < 0.05, "{:?}", after[k]);
    // the next clip follows straight on, moved by the difference
    let moved = misty.2 - TWO_S;
    assert!((after[k + 1].1 - (before[k + 1].1 - moved)).abs() < 0.05, "{:?} from {:?}", after[k + 1], before[k + 1]);
    assert!((after[k + 1].2 - before[k + 1].2).abs() < 0.01, "and whole: {:?} from {:?}", after[k + 1], before[k + 1]);
    assert_eq!(&after[..k], &before[..k]);
    // its sound went with it: on A1 the source's sound in its place, the old clip's gone
    let a1 = audio(&e, 0);
    assert!(a1.iter().any(|c| c.0 == item && (c.1 - misty.1).abs() < 0.05 && (c.2 - TWO_S).abs() < 0.05), "{a1:?}");
    assert!(!a1.iter().any(|c| c.0 == misty.0), "the replaced clip's sound is gone: {a1:?}");
    e.execute("edit.undo", json!({})).unwrap();
    assert_eq!(track(&e, 0), before, "one undo step");
}

#[test]
fn place_on_top_finds_a_free_track_above_and_adds_one_when_none_is() {
    let (mut e, item) = with_source("City_Night_Drive.mp4");
    seek(&mut e, 1.5);
    let (v1, v2) = (track(&e, 0), track(&e, 1));
    let patched = e.session.targeting();
    e.execute("edit.placeOnTop", json!({})).unwrap();
    assert_eq!(track(&e, 0), v1, "V1 untouched");
    let on_v2: Vec<_> = track(&e, 1).into_iter().filter(|c| !v2.contains(c)).collect();
    assert_eq!(on_v2.len(), 1);
    assert!(on_v2[0].0 == item && (on_v2[0].1 - 1.5).abs() < 0.05, "{on_v2:?}");
    assert_eq!(e.session.targeting(), patched, "the destinations put back");
    // again at the same place: V2 is taken there now, so V3
    let tracks = e.session.active_sequence().unwrap().video_tracks.len();
    seek(&mut e, 1.5);
    e.execute("edit.placeOnTop", json!({})).unwrap();
    let on_v3 = track(&e, 2);
    assert!(on_v3.iter().any(|c| c.0 == item && (c.1 - 1.5).abs() < 0.05), "{on_v3:?}");
    assert_eq!(e.session.active_sequence().unwrap().video_tracks.len(), tracks, "V3 was free");
    // and a third time: no track is free above, so one is added
    seek(&mut e, 1.5);
    e.execute("edit.placeOnTop", json!({})).unwrap();
    assert_eq!(e.session.active_sequence().unwrap().video_tracks.len(), tracks + 1);
    assert!(track(&e, tracks).iter().any(|c| c.0 == item));
}

/// Fit to Fill (LOG D-060): the source's 48 frames retimed to fill the timeline's In to Out on the
/// destinations, slower and faster, the rest of the timeline as it was.
#[test]
fn fit_to_fill_retimes_the_source_into_the_timelines_in_and_out() {
    let frame = 1001.0 / 24000.0;
    for (out, speed) in [(95, 48.0 / 84.0), (35, 2.0)] {
        let (mut e, item) = with_source("City_Night_Drive.mp4");
        e.execute("playhead.set", json!({ "frame": 12 })).unwrap();
        e.execute("mark.in", json!({})).unwrap();
        e.execute("playhead.set", json!({ "frame": out })).unwrap();
        e.execute("mark.out", json!({})).unwrap();
        let span = (out + 1 - 12) as f64 * frame;
        let (v1, a2) = (track(&e, 0), audio(&e, 1));
        let end = {
            let seq = e.session.active_sequence().unwrap();
            seq.all_tracks().flat_map(|t| &t.items).map(|it| secs(it.end())).fold(0.0, f64::max)
        };
        let done = e.execute("edit.fitToFill", json!({})).unwrap();
        assert_eq!(done.undo_label.as_deref(), Some("Fit to Fill"));
        let after = track(&e, 0);
        let fitted = after.iter().find(|c| c.0 == item).copied().unwrap();
        assert!((fitted.1 - 12.0 * frame).abs() < 0.01 && (fitted.2 - span).abs() < frame, "{fitted:?}, {span}");
        let seq = e.session.active_sequence().unwrap();
        let clip = seq.video_tracks[0].items.iter().find(|c| c.item.0 == item).unwrap();
        assert!((clip.speed - speed).abs() < 0.01, "speed {} for {speed}", clip.speed);
        // around it: the clip it was cut into, before and after; everything later where it was
        assert!(after.iter().any(|c| c.0 == v1[0].0 && c.1 < 0.01 && (c.2 - 12.0 * frame).abs() < 0.01), "{after:?}");
        assert_eq!(&after[after.len() - (v1.len() - 1)..], &v1[1..]);
        assert_eq!(audio(&e, 1), a2, "the music on A2 untouched");
        let end_now = seq.all_tracks().flat_map(|t| &t.items).map(|it| secs(it.end())).fold(0.0, f64::max);
        assert!((end_now - end).abs() < 0.01, "nothing left at the end: {end_now} against {end}");
        e.execute("edit.undo", json!({})).unwrap();
        assert_eq!(track(&e, 0), v1, "one undo step");
    }
}
