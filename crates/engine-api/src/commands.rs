//! Resolution's command vocabulary: what a Resolve user (or an agent) can do, named the way
//! Resolve names it. Ids are stable API: the keymap, the UI and MCP clients depend on them, so
//! an id is never renamed or reused, only added or deprecated.

use serde::{Deserialize, Serialize};

/// Resolve's pages. Commands are grouped by the page that owns them; agents browse the vocabulary
/// one page at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

impl Page {
    pub const ALL: [Page; 8] =
        [Page::Global, Page::Media, Page::Cut, Page::Edit, Page::Fusion, Page::Color, Page::Fairlight, Page::Deliver];

    pub fn name(self) -> &'static str {
        match self {
            Page::Global => "global",
            Page::Media => "media",
            Page::Cut => "cut",
            Page::Edit => "edit",
            Page::Fusion => "fusion",
            Page::Color => "color",
            Page::Fairlight => "fairlight",
            Page::Deliver => "deliver",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Page::Global => "Works on every page: project, history, transport, playhead, page switching.",
            Page::Media => "Import and organise media in the Media Pool.",
            Page::Cut => "Fast assembly editing (Resolve's Cut page).",
            Page::Edit => "Timeline editing: marking, source-to-timeline edits, trimming, transitions, markers.",
            Page::Fusion => "Node-based compositing and effects.",
            Page::Color => "Grading: nodes, wheels, curves, qualifiers, scopes.",
            Page::Fairlight => "Audio: mixing, buses, effects.",
            Page::Deliver => "Rendering and export.",
        }
    }
}

/// How careful a caller must be. Agents must pass `acknowledge` for [`Risk::Destructive`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// Reads state; changes nothing.
    Read,
    /// Changes the project, undoably.
    Edit,
    /// Touches files or replaces the open project: not undoable.
    Destructive,
}

/// Vocabulary commands that only read.
const READ: &[&str] = &["project.inspect", "timeline.inspect"];
/// Vocabulary commands that write files or replace the open project.
const DESTRUCTIVE: &[&str] = &["project.open", "project.save", "project.saveAs"];

/// The risk of a vocabulary command (`None` for ids outside the vocabulary).
pub fn risk(id: &str) -> Option<Risk> {
    find(id)?;
    Some(if READ.contains(&id) {
        Risk::Read
    } else if DESTRUCTIVE.contains(&id) {
        Risk::Destructive
    } else {
        Risk::Edit
    })
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
    CommandSpec { id, label, page, params }
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
    // Workspace > Show Panel in Workspace: the Edit page's interface toolbar (LOG D-016)
    c("workspace.mediaPool", "Media Pool", Global, ""),
    c("workspace.effects", "Effects Library", Global, ""),
    c("workspace.index", "Index", Global, ""),
    c("workspace.soundLibrary", "Sound Library", Global, ""),
    c("workspace.mixer", "Mixer", Global, ""),
    c("workspace.metadata", "Metadata", Global, ""),
    c("workspace.inspector", "Inspector", Global, ""),
    c("workspace.resetLayout", "Reset UI Layout", Global, ""),
    // project
    c("project.open", "Open Project", Global, r#"{"path": string}"#),
    c("project.save", "Save Project", Global, ""),
    c("project.saveAs", "Save Project As", Global, r#"{"path": string}"#),
    c("project.inspect", "Inspect Project", Global, "query; returns the project as JSON"),
    c(
        "timeline.inspect",
        "Inspect Timeline",
        Edit,
        "query; returns the active timeline (tracks, clips, in/out, playhead) as JSON",
    ),
    c("media.import", "Import Media", Media, r#"{"paths": [string]}"#),
    c("timeline.createFromClips", "Create New Timeline Using Selected Clips", Media, r#"{"items": [id]}"#),
    c("source.open", "Open in Source Viewer", Edit, r#"{"item": id}"#),
    c("timeline.open", "Open Timeline", Media, r#"{"item": id}"#),
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
    c("playhead.set", "Go to Timecode", Global, r#"{"frame": int}"#),
    // marking
    c("mark.in", "Mark In", Edit, ""),
    c("mark.out", "Mark Out", Edit, ""),
    c("mark.clip", "Mark Clip", Edit, ""),
    c("mark.clearIn", "Clear In", Edit, ""),
    c("mark.clearOut", "Clear Out", Edit, ""),
    c("mark.clearInOut", "Clear In and Out", Edit, ""),
    c("marker.add", "Add Marker", Edit, r#"{"name"?: string, "note"?: string}"#),
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
    c("timeline.select", "Select Clips", Edit, r#"{"clips": [id], "add"?: bool, "toggle"?: bool}"#),
    c("timeline.split", "Split Clip (at playhead)", Edit, ""),
    c("timeline.delete", "Delete Selected (leave gap)", Edit, r#"{"clips"?: [id]}"#),
    c("timeline.rippleDelete", "Ripple Delete", Edit, r#"{"clips"?: [id]}"#),
    c("timeline.rippleStartToPlayhead", "Ripple Start to Playhead", Edit, ""),
    c("timeline.rippleEndToPlayhead", "Ripple End to Playhead", Edit, ""),
    c("timeline.toggleClipEnable", "Enable / Disable Clip", Edit, ""),
    c("timeline.matchFrame", "Match Frame", Edit, ""),
    c("timeline.addTransition", "Add Transition", Edit, ""),
    c("timeline.addVideoTransition", "Add Video Only Transition", Edit, ""),
    c("timeline.addAudioTransition", "Add Audio Only Transition", Edit, ""),
    c("timeline.snapping", "Snapping", Edit, ""),
    c("timeline.linkedSelection", "Linked Selection", Edit, ""),
    c("timeline.closeGap", "Close Gap", Edit, ""),
    // track headers
    c(
        "track.set",
        "Track Settings (Lock, Enable, Mute, Solo)",
        Edit,
        r#"{"track": "V1"|"A1"|id, "locked"?: bool, "enabled"?: bool, "muted"?: bool, "solo"?: bool}"#,
    ),
    c(
        "track.target",
        "Auto Track Selector and Destination Control",
        Edit,
        r#"{"track": "V1"|"A1"|id, "targeted"?: bool (auto track selector), "sourcePatch"?: bool (destination for source edits)}"#,
    ),
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

    #[test]
    fn risk_lists_name_real_commands() {
        for id in READ.iter().chain(DESTRUCTIVE) {
            assert!(find(id).is_some(), "{id} is not in the vocabulary");
        }
        assert_eq!(risk("timeline.split"), Some(Risk::Edit));
        assert_eq!(risk("project.save"), Some(Risk::Destructive));
        assert_eq!(risk("nope"), None);
    }
}
