# The one-minute edit, in Resolution

ROADMAP step 2's bar: cut a one-minute piece from the Sony and Samsung clips, as in Resolve.
This page lists what works today and what doesn't. Each step has been run through the editor's
keys and mouse in a UI test (LOG B-043, D-075, D-076), or through the engine
(`scripts/rehearse-edit.py`, LOG B-042).

## Start

```sh
cargo run -p resolution-app --release -- --import "test media/sony-samsung-2026-10-12/ROB MID SHOT.MP4"
```

`--import` loads a clip and makes a timeline from it. To start from nothing, import media with
File > Import Media (Ctrl+I), or drop the files on the window. With no timeline open, double-click
a clip in the Media Pool to make one from it. Ctrl+N makes another, empty, in the open timeline's
format.

A clip of another size is fitted into the timeline, letterboxed or pillarboxed, as Resolve's
default does (LOG B-050). That covers 4K in a 1080 timeline and a phone's portrait clip.

## Cut

| Step | How |
|---|---|
| Open a clip in the source viewer | Double-click it in the Media Pool |
| Mark the range | Play (Space, J/K/L) or step (Left/Right) in the source viewer; I and O |
| Edit it in | F9 Insert, F10 Overwrite, F11 Replace, F12 Place on Top, Shift+F10 Ripple Overwrite, Shift+F11 Fit to Fill, Shift+F12 Append at End |
| Switch viewers | Q (keys act on the viewer clicked last) |
| Move about | Up/Down to the previous/next cut, Home/End, Shift+Up/Down between markers |
| Split | Ctrl+B (or Ctrl+\\) at the playhead, or B for the Blade and a click |
| Delete | Click a clip; Backspace leaves a gap, Shift+Backspace or Delete closes it |
| Trim | T for Trim mode: drag an edge to ripple, the upper half of a clip to slip, the lower half to slide; A for Selection mode, where an edge drag resizes |
| Transitions | Down to the cut, then Ctrl+T (video and audio), Alt+T (video only) or Shift+T (audio only) |
| Markers | M |
| Undo, redo | Ctrl+Z, Ctrl+Shift+Z |
| Clip off and on | D |

The playhead goes to the end of each edit, as in Resolve, so the next edit follows on.

## Colour (optional)

Shift+6 opens the Color page.
- **Nodes:** Alt+S adds a serial node.
- **Primaries:** the wheels, rings and bars (Bars mode from the palette's menu).
- **Palettes:** Curves, Qualifier (click the viewer to pick a colour) and Window.
- **Highlight:** Shift+H, or Alt+Shift+H for black and white.
- **Resets:** Shift+Home resets the current node; Mod+Home resets all grades and nodes.
- **Clips:** the strip under the viewer moves between clips, and its Timeline button opens the
  Mini-Timeline.

## Deliver

Shift+8 for the Deliver page. Choose a preset (H.264 1080p, YouTube 1080p, Vertical 1080 × 1920,
ProRes 422 HQ and others), a file name and a folder, then Entire Timeline or In/Out Range. Then
Add to Render Queue and Render All.

## Not there yet

- **Edit points can't be selected:** no V, U, Comma/Period nudges or E. Ctrl+T acts on the cut at
  the playhead, within three frames of it. Proposed: LOG D-064 and D-068.
- **No dynamic trim (W):** the options are in D-064.
- **The Inspector shows the timeline only:** a clip's zoom, position, crop and opacity can't be set
  yet (D-068).
- **Empty panels:** Effects, Sound Library, Metadata, the Mixer, the Media page, and the Cut, Fusion
  and Fairlight pages are placeholders.
- **Unchecked keys:** the page keys (Shift+2 to Shift+8), Mod+B and Shift+Backspace are Resolve's
  as far as known, but not checked against the 21.1 install (D-051).

## If something goes wrong

Start the editor with `--journal <file.jsonl>`. Each command is written to the file as it starts
and ends, with who sent it (LOG D-054), so the steps can be replayed or reported.
