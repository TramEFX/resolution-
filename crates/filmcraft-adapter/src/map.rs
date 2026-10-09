//! Resolution command id → FilmCraft command id.
//!
//! This table is where upstream command churn lands. `every_mapped_id_exists_upstream` fails the
//! build when a FilmCraft sync renames or removes a command we rely on.

/// How a Resolution command is carried out on FilmCraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Run this FilmCraft command, passing the parameters through.
    Engine(&'static str),
    /// A close but not identical FilmCraft command; the note says how it differs from Resolve.
    Approx(&'static str, &'static str),
    /// Handled by the adapter itself (undo, redo).
    Adapter,
    /// Owned by the front end: page switching, timeline zoom, edit modes and playback. FilmCraft
    /// keeps these in its egui crate, which we replace; Resolution's UI implements them.
    Ui,
    /// No FilmCraft equivalent yet; the note says what is missing.
    Missing(&'static str),
}

use Target::*;

pub const MAP: &[(&str, Target)] = &[
    ("page.media", Ui),
    ("page.cut", Ui),
    ("page.edit", Ui),
    ("page.fusion", Ui),
    ("page.color", Ui),
    ("page.fairlight", Ui),
    ("page.deliver", Ui),
    ("project.open", Engine("file.open")),
    ("project.save", Engine("file.save")),
    ("project.saveAs", Engine("file.saveAs")),
    ("project.inspect", Engine("project.inspect")),
    ("media.import", Engine("file.import")),
    ("edit.undo", Adapter),
    ("edit.redo", Adapter),
    ("edit.cut", Engine("edit.cut")),
    ("edit.copy", Engine("edit.copy")),
    ("edit.paste", Engine("edit.paste")),
    ("edit.pasteInsert", Engine("edit.pasteInsert")),
    ("edit.pasteAttributes", Engine("edit.pasteAttributes")),
    ("edit.selectAll", Engine("edit.selectAll")),
    ("edit.deselectAll", Engine("edit.deselectAll")),
    ("playback.playStop", Ui),
    ("playback.reverse", Ui),
    ("playback.stop", Ui),
    ("playback.forward", Ui),
    ("playback.loop", Ui),
    ("playback.inToOut", Ui),
    ("playhead.previousFrame", Engine("playhead.stepBack")),
    ("playhead.nextFrame", Engine("playhead.stepForward")),
    ("playhead.previousEdit", Engine("playhead.prevEdit")),
    ("playhead.nextEdit", Engine("playhead.nextEdit")),
    ("playhead.timelineStart", Engine("playhead.start")),
    ("playhead.timelineEnd", Engine("playhead.end")),
    ("playhead.set", Engine("playhead.set")),
    ("mark.in", Engine("markers.markIn")),
    ("mark.out", Engine("markers.markOut")),
    ("mark.clip", Engine("markers.markClip")),
    ("mark.clearIn", Engine("markers.clearIn")),
    ("mark.clearOut", Engine("markers.clearOut")),
    ("mark.clearInOut", Engine("markers.clearInOut")),
    ("marker.add", Engine("markers.add")),
    ("marker.next", Engine("markers.goNext")),
    ("marker.previous", Engine("markers.goPrev")),
    ("mode.selection", Ui),
    ("mode.trim", Ui),
    ("mode.blade", Ui),
    (
        "mode.dynamicTrim",
        Missing("no dynamic trim mode (JKL-driven trimming)"),
    ),
    ("edit.insert", Engine("source.insert")),
    ("edit.overwrite", Engine("source.overwrite")),
    (
        "edit.replace",
        Missing("Resolve Replace aligns the source playhead to the timeline playhead"),
    ),
    ("edit.placeOnTop", Missing("no place-on-top edit")),
    ("edit.rippleOverwrite", Missing("no ripple overwrite edit")),
    ("edit.fitToFill", Missing("no fit-to-fill edit")),
    ("edit.appendAtEnd", Missing("no append-at-end edit")),
    ("timeline.split", Engine("sequence.addEdit")),
    ("timeline.delete", Engine("edit.clear")),
    ("timeline.rippleDelete", Engine("edit.rippleDelete")),
    (
        "timeline.rippleStartToPlayhead",
        Engine("trim.ripplePrevious"),
    ),
    ("timeline.rippleEndToPlayhead", Engine("trim.rippleNext")),
    ("timeline.toggleClipEnable", Engine("clip.enable")),
    ("timeline.matchFrame", Engine("sequence.matchFrame")),
    (
        "timeline.addTransition",
        Approx(
            "trim.applyDefaultTransition",
            "applies to the selected edit points rather than the edit nearest the playhead",
        ),
    ),
    (
        "timeline.addVideoTransition",
        Engine("sequence.applyVideoTransition"),
    ),
    (
        "timeline.addAudioTransition",
        Engine("sequence.applyAudioTransition"),
    ),
    ("timeline.snapping", Engine("sequence.snap")),
    (
        "timeline.linkedSelection",
        Engine("sequence.linkedSelection"),
    ),
    ("timeline.closeGap", Engine("sequence.closeGap")),
    ("view.zoomIn", Ui),
    ("view.zoomOut", Ui),
    ("view.zoomToFit", Ui),
];

pub fn target(id: &str) -> Option<Target> {
    MAP.iter().find(|(r, _)| *r == id).map(|(_, t)| *t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolution_engine_api::COMMANDS;

    #[test]
    fn every_vocabulary_command_is_mapped() {
        let missing: Vec<_> = COMMANDS
            .iter()
            .filter(|c| target(c.id).is_none())
            .map(|c| c.id)
            .collect();
        assert!(
            missing.is_empty(),
            "unmapped Resolution commands: {missing:?}"
        );
    }

    #[test]
    fn every_mapped_id_is_in_the_vocabulary() {
        let extra: Vec<_> = MAP
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| resolution_engine_api::commands::find(id).is_none())
            .collect();
        assert!(
            extra.is_empty(),
            "mapped ids not in the vocabulary: {extra:?}"
        );
    }

    /// The upstream-drift gate: run on every FilmCraft sync.
    #[test]
    fn every_mapped_id_exists_upstream() {
        let gone: Vec<_> = MAP
            .iter()
            .filter_map(|(_, t)| match t {
                Engine(fc) | Approx(fc, _) => Some(*fc),
                _ => None,
            })
            .filter(|fc| filmcraft_engine::find_command(fc).is_none())
            .collect();
        assert!(
            gone.is_empty(),
            "FilmCraft commands no longer exist upstream: {gone:?}"
        );
    }
}
