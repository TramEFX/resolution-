//! Resolve's source edits that FilmCraft has no one command for (LOG D-059), each composed from
//! FilmCraft's commands and run as one undo step (`Session::edit_group`), so a failure leaves the
//! timeline as it was:
//!
//! - **Replace** (F11): the clip under the playhead on the video destination track takes the
//!   source clip, keeping its place and length, with the source playhead's frame lined up with
//!   the timeline playhead's (FilmCraft's `clip.replaceFromSourceMatchFrame`).
//! - **Place on Top** (F12): the source's In to Out on the first video track above the
//!   destination that is free over it, and its sound on the first audio track after the audio
//!   destination that is free; at the timeline's In, or the playhead. A track is added where none
//!   is free.
//! - **Ripple Overwrite** (Shift+F10): the clip under the playhead gives way to the source's In to
//!   Out, and what follows moves up or back by the difference.
//! - **Append at End** (Shift+F12): the source's In to Out after the last clip on the timeline,
//!   whatever the timeline's In and Out.
//! - **Fit to Fill** (Shift+F11): the source's In to Out, its speed changed so it fills the
//!   timeline's In to Out on the destination tracks (LOG D-060).

use filmcraft_engine::project::{ClipId, ItemKind, Track, TrackId};
use filmcraft_engine::time::Tick;
use filmcraft_engine::{EngineError, Session};
use serde_json::{Value, json};

/// Runs composed edit `id`; `None` for an id that is not one of them.
pub(crate) fn run(s: &mut Session, id: &str) -> Option<Result<Value, EngineError>> {
    Some(match id {
        "edit.replace" => replace(s),
        "edit.placeOnTop" => place_on_top(s),
        "edit.rippleOverwrite" => ripple_overwrite(s),
        "edit.appendAtEnd" => append_at_end(s),
        "edit.fitToFill" => fit_to_fill(s),
        _ => return None,
    })
}

fn refuse(why: &str) -> EngineError {
    EngineError::Other(why.into())
}

/// The clip under the playhead on the video destination track (on any video track, lowest first,
/// when no video destination is patched).
fn clip_at_playhead(s: &Session) -> Option<(ClipId, Tick)> {
    let seq = s.active_sequence()?;
    let t = s.playhead();
    let dest = s.targeting().video_dest;
    seq.video_tracks
        .iter()
        .filter(|tr| dest.is_none_or(|d| tr.id == d))
        .flat_map(|tr| &tr.items)
        .find(|it| it.start <= t && t < it.end())
        .map(|it| (it.id, it.start))
}

/// The source's In to Out, as long as FilmCraft's source edits take it.
fn source_length(s: &Session) -> Option<Tick> {
    let item = s.state.source_item?;
    let pi = s.project.item(item)?;
    let (mark_in, mark_out) = match &pi.kind {
        ItemKind::Media(m) => (m.mark_in, m.mark_out),
        ItemKind::Sequence(q) => (q.mark_in, q.mark_out),
        ItemKind::Subclip { range, .. } => return Some(range.duration),
        _ => (None, None),
    };
    let frame = pi.frame_rate().frame_duration();
    let start = mark_in.unwrap_or(Tick::ZERO);
    let end = mark_out.map_or(pi.duration(), |o| o + frame);
    Some((end - start).max(frame))
}

/// The timeline's In and Out set to `marks`, as a step of the edit around it.
fn set_marks(s: &mut Session, marks: (Option<Tick>, Option<Tick>)) -> Result<(), EngineError> {
    s.edit_sequence("Marks", |q, _, _| {
        (q.mark_in, q.mark_out) = marks;
        Ok(())
    })
}

fn replace(s: &mut Session) -> Result<Value, EngineError> {
    let (clip, _) = clip_at_playhead(s).ok_or_else(|| refuse("no clip under the playhead on the destination track"))?;
    s.execute("clip.replaceFromSourceMatchFrame", json!({ "clips": [clip.0] }))
}

fn append_at_end(s: &mut Session) -> Result<Value, EngineError> {
    s.edit_group("Append at End of Timeline", |s| {
        let seq = s.active_sequence().ok_or(EngineError::NoSequence)?;
        let marks = (seq.mark_in, seq.mark_out);
        let end = seq.all_tracks().flat_map(|t| &t.items).map(|it| it.end()).max().unwrap_or(Tick::ZERO);
        // a source edit goes to the timeline's In when there is one
        set_marks(s, (None, None))?;
        s.set_playhead(end);
        let placed = s.execute("source.overwrite", json!({}))?;
        set_marks(s, marks)?;
        Ok(placed)
    })
}

/// The end of the last clip on any track.
fn timeline_end(s: &Session) -> Tick {
    s.active_sequence()
        .map(|q| q.all_tracks().flat_map(|t| &t.items).map(|it| it.end()).max().unwrap_or(Tick::ZERO))
        .unwrap_or(Tick::ZERO)
}

fn fit_to_fill(s: &mut Session) -> Result<Value, EngineError> {
    let length = source_length(s).ok_or_else(|| refuse("no source clip"))?;
    let sid = s.state.active_sequence.ok_or(EngineError::NoSequence)?;
    let seq = s.active_sequence().ok_or(EngineError::NoSequence)?;
    let (Some(from), Some(out)) = (seq.mark_in, seq.mark_out) else {
        return Err(refuse("Fit to Fill needs an In and an Out on the timeline"));
    };
    // the Out frame is in the range, as FilmCraft's lift takes it
    let to = out + seq.settings.frame_rate.frame_duration();
    if to <= from {
        return Err(refuse("the timeline's In is after its Out"));
    }
    let speed = length.seconds() / (to - from).seconds() * 100.0;
    s.edit_group("Fit to Fill", |s| {
        let patched = s.targeting();
        let video = patched.video_dest.ok_or_else(|| refuse("no video destination track"))?;
        // the destinations' In to Out cleared, and only theirs
        let mut only = patched.clone();
        only.targeted = patched.video_dest.into_iter().chain(patched.audio_dest).collect();
        s.state.targeting.insert(sid, only);
        let lifted = s.execute("sequence.lift", json!({}));
        s.state.targeting.insert(sid, patched);
        lifted?;
        // placed after everything, where its new length meets nothing, then retimed and moved in
        let marks = (Some(from), Some(out));
        let end = timeline_end(s);
        set_marks(s, (None, None))?;
        s.set_playhead(end);
        let placed = s.execute("source.overwrite", json!({}))?;
        let clips: Vec<u64> = placed["clips"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
        let seq = s.active_sequence().ok_or(EngineError::NoSequence)?;
        let picture = clips
            .iter()
            .copied()
            .find(|c| seq.find_item(ClipId(*c)).is_some_and(|(t, _)| t == video))
            .ok_or_else(|| refuse("the source has no picture"))?;
        s.execute("clip.speedDuration", json!({ "clips": clips, "speed": speed }))?;
        s.execute(
            "timeline.move",
            json!({
                "moves": [{ "clip": picture, "track": video.0, "time": from.0 }],
                "insert": false,
                "linked": true,
            }),
        )?;
        set_marks(s, marks)?;
        s.set_playhead(to);
        Ok(json!({ "clips": clips, "speed": speed }))
    })
}

fn ripple_overwrite(s: &mut Session) -> Result<Value, EngineError> {
    let (clip, start) =
        clip_at_playhead(s).ok_or_else(|| refuse("no clip under the playhead on the destination track"))?;
    s.edit_group("Ripple Overwrite", |s| {
        let seq = s.active_sequence().ok_or(EngineError::NoSequence)?;
        let marks = (seq.mark_in, seq.mark_out);
        s.state.selection = vec![clip];
        s.execute("edit.rippleDelete", json!({}))?;
        set_marks(s, (None, None))?;
        s.set_playhead(start);
        let placed = s.execute("source.insert", json!({}))?;
        set_marks(s, marks)?;
        Ok(placed)
    })
}

/// The first of `tracks` after `dest` (from the first, without one) with nothing from `from` to
/// `to`.
fn free_above(tracks: &[Track], dest: TrackId, from: Tick, to: Tick) -> Option<TrackId> {
    let after = tracks.iter().position(|t| t.id == dest).map_or(0, |i| i + 1);
    tracks[after..].iter().find(|t| t.items.iter().all(|it| it.end() <= from || it.start >= to)).map(|t| t.id)
}

fn place_on_top(s: &mut Session) -> Result<Value, EngineError> {
    let length = source_length(s).ok_or_else(|| refuse("no source clip"))?;
    let sid = s.state.active_sequence.ok_or(EngineError::NoSequence)?;
    s.edit_group("Place on Top", |s| {
        let seq = s.active_sequence().ok_or(EngineError::NoSequence)?;
        let at = seq.mark_in.unwrap_or(s.playhead());
        let to = at + length;
        let patched = s.targeting();
        let mut video = patched.video_dest.and_then(|d| free_above(&seq.video_tracks, d, at, to));
        let mut audio = patched.audio_dest.and_then(|d| free_above(&seq.audio_tracks, d, at, to));
        let (more_video, more_audio) =
            (patched.video_dest.is_some() && video.is_none(), patched.audio_dest.is_some() && audio.is_none());
        if more_video || more_audio {
            s.execute("sequence.addTracks", json!({ "video": u32::from(more_video), "audio": u32::from(more_audio) }))?;
            let seq = s.active_sequence().ok_or(EngineError::NoSequence)?;
            if more_video {
                video = seq.video_tracks.last().map(|t| t.id);
            }
            if more_audio {
                audio = seq.audio_tracks.last().map(|t| t.id);
            }
        }
        // the source edit goes to those tracks; the destinations are put back afterwards
        let mut on_top = patched.clone();
        (on_top.video_dest, on_top.audio_dest) = (video, audio);
        s.state.targeting.insert(sid, on_top);
        let placed = s.execute("source.overwrite", json!({}));
        s.state.targeting.insert(sid, patched);
        placed
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
