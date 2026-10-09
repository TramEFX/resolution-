//! Resolution's command vocabulary: what a Resolve user (or an agent) can do, named the way
//! Resolve names it. Ids are stable API: the keymap, the UI and MCP clients depend on them, so
//! an id is never renamed or reused, only added or deprecated.

use serde::Serialize;

/// Resolve's pages. Commands are grouped by the page that owns them; that grouping is also how
/// the MCP server loads tools on demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Page {
    /// Works everywhere (transport, undo, page switching).
    Global,
    Media,
    Cut,
    Edit,
    Fusion,
    Color,
    Fairlight,
    Deliver,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct CommandSpec {
    pub id: &'static str,
    /// Resolve's menu / keyboard-customization name for the action.
    pub label: &'static str,
    pub page: Page,
    pub params: &'static str,
}

const fn c(id: &'static str, label: &'static str, page: Page, params: &'static str) -> CommandSpec {
    CommandSpec {
        id,
        label,
        page,
        params,
    }
}

use Page::*;

pub const COMMANDS: &[CommandSpec] = &[
    // pages
    c("page.media", "Media Page", Global, ""),
    c("page.cut", "Cut Page", Global, ""),
    c("page.edit", "Edit Page", Global, ""),
    c("page.fusion", "Fusion Page", Global, ""),
    c("page.color", "Color Page", Global, ""),
    c("page.fairlight", "Fairlight Page", Global, ""),
    c("page.deliver", "Deliver Page", Global, ""),
    // project
    c(
        "project.open",
        "Open Project",
        Global,
        r#"{"path": string}"#,
    ),
    c("project.save", "Save Project", Global, ""),
    c(
        "project.saveAs",
        "Save Project As",
        Global,
        r#"{"path": string}"#,
    ),
    c(
        "project.inspect",
        "Inspect Project",
        Global,
        "query; returns the project as JSON",
    ),
    c(
        "media.import",
        "Import Media",
        Media,
        r#"{"paths": [string]}"#,
    ),
    // history
    c("edit.undo", "Undo", Global, ""),
    c("edit.redo", "Redo", Global, ""),
    // clipboard
    c("edit.cut", "Cut", Edit, ""),
    c("edit.copy", "Copy", Edit, ""),
    c("edit.paste", "Paste", Edit, ""),
    c("edit.pasteInsert", "Paste Insert", Edit, ""),
    c("edit.pasteAttributes", "Paste Attributes", Edit, ""),
    c("edit.selectAll", "Select All", Edit, ""),
    c("edit.deselectAll", "Deselect All", Edit, ""),
    // transport
    c("playback.playStop", "Play / Stop", Global, ""),
    c("playback.reverse", "Play Reverse (J)", Global, ""),
    c("playback.stop", "Stop (K)", Global, ""),
    c("playback.forward", "Play Forward (L)", Global, ""),
    c("playback.loop", "Loop", Global, ""),
    c("playback.inToOut", "Play In to Out", Global, ""),
    c("playhead.previousFrame", "Previous Frame", Global, ""),
    c("playhead.nextFrame", "Next Frame", Global, ""),
    c("playhead.previousEdit", "Previous Edit", Global, ""),
    c("playhead.nextEdit", "Next Edit", Global, ""),
    c("playhead.timelineStart", "Timeline Start", Global, ""),
    c("playhead.timelineEnd", "Timeline End", Global, ""),
    c(
        "playhead.set",
        "Go to Timecode",
        Global,
        r#"{"frame": int}"#,
    ),
    // marking
    c("mark.in", "Mark In", Edit, ""),
    c("mark.out", "Mark Out", Edit, ""),
    c("mark.clip", "Mark Clip", Edit, ""),
    c("mark.clearIn", "Clear In", Edit, ""),
    c("mark.clearOut", "Clear Out", Edit, ""),
    c("mark.clearInOut", "Clear In and Out", Edit, ""),
    c(
        "marker.add",
        "Add Marker",
        Edit,
        r#"{"name"?: string, "note"?: string}"#,
    ),
    c("marker.next", "Next Marker", Edit, ""),
    c("marker.previous", "Previous Marker", Edit, ""),
    // edit modes (timeline tools)
    c("mode.selection", "Selection Mode", Edit, ""),
    c("mode.trim", "Trim Edit Mode", Edit, ""),
    c("mode.blade", "Blade Edit Mode", Edit, ""),
    c("mode.dynamicTrim", "Dynamic Trim Mode", Edit, ""),
    // source → timeline edits
    c("edit.insert", "Insert Clip", Edit, ""),
    c("edit.overwrite", "Overwrite Clip", Edit, ""),
    c("edit.replace", "Replace Clip", Edit, ""),
    c("edit.placeOnTop", "Place on Top", Edit, ""),
    c("edit.rippleOverwrite", "Ripple Overwrite", Edit, ""),
    c("edit.fitToFill", "Fit to Fill", Edit, ""),
    c("edit.appendAtEnd", "Append at End of Timeline", Edit, ""),
    // timeline edits
    c("timeline.split", "Split Clip (at playhead)", Edit, ""),
    c(
        "timeline.delete",
        "Delete Selected (leave gap)",
        Edit,
        r#"{"clips"?: [id]}"#,
    ),
    c(
        "timeline.rippleDelete",
        "Ripple Delete",
        Edit,
        r#"{"clips"?: [id]}"#,
    ),
    c(
        "timeline.rippleStartToPlayhead",
        "Ripple Start to Playhead",
        Edit,
        "",
    ),
    c(
        "timeline.rippleEndToPlayhead",
        "Ripple End to Playhead",
        Edit,
        "",
    ),
    c(
        "timeline.toggleClipEnable",
        "Enable / Disable Clip",
        Edit,
        "",
    ),
    c("timeline.matchFrame", "Match Frame", Edit, ""),
    c("timeline.addTransition", "Add Transition", Edit, ""),
    c(
        "timeline.addVideoTransition",
        "Add Video Only Transition",
        Edit,
        "",
    ),
    c(
        "timeline.addAudioTransition",
        "Add Audio Only Transition",
        Edit,
        "",
    ),
    c("timeline.snapping", "Snapping", Edit, ""),
    c("timeline.linkedSelection", "Linked Selection", Edit, ""),
    c("timeline.closeGap", "Close Gap", Edit, ""),
    // view
    c("view.zoomIn", "Zoom In", Edit, ""),
    c("view.zoomOut", "Zoom Out", Edit, ""),
    c("view.zoomToFit", "Zoom to Fit", Edit, ""),
];

pub fn find(id: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = COMMANDS.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len(), "duplicate command id");
    }
}
