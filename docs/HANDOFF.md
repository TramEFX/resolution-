# Picking up locally

Where things stand and how to continue on your own machine. Last updated 2026-10-15 (the first unattended run: windows, edits, matte finesse, the journal).

## 1. Set up (once)

You need **Rust 1.95+** (`rustup update stable`) and **git** with access to both private repos
(`TramEFX/resolution-` and `TramEFX/resolution-filmcraft`). The engine is pure Rust, so there are
no system libraries to install. On Windows, Rust's default MSVC toolchain also needs the Visual
Studio C++ Build Tools for its linker.

```sh
git clone https://github.com/TramEFX/resolution- resolution
cd resolution
cargo test              # first build compiles ~45 FilmCraft crates
cargo run -p resolution-cli -- run project.inspect
```

Measured on Windows 11 (Rust 1.99): on 2026-10-14, from a `cargo clean`, building everything took
about 6.5 minutes (tests 293 s, clippy 88 s, the two binaries 13 s; the tests then run in 11 s)
and left `target/` at 8.3 GB. Cargo keeps the builds of earlier engine pins beside the current
one, so `target/` grows with every engine sync: it reached 28 GB and filled the disk on
2026-10-14 (LOG B-032). Run `cargo clean` after each engine sync (UPSTREAM.md, step 3).

Cargo fetches the engine from the private mirror with your normal git credentials
(`.cargo/config.toml` sets `git-fetch-with-cli`). If it fails with an authentication error, make
sure `git clone https://github.com/TramEFX/resolution-filmcraft` works in the same terminal
(on GitHub: a credential helper, `gh auth login`, or SSH with a `url.insteadOf` rewrite).

The scripts in `scripts/` are bash: on Windows run them from Git Bash or WSL.

## 2. Run the editor

```sh
cargo run -p resolution-app                                   # empty project: import with Ctrl+I or drop files
cargo run -p resolution-app -- --import "test media/clip.mp4" # import, and make a timeline from it
cargo run -p resolution-app -- --demo                         # FilmCraft's demo project
```

Resolve keys work: Space / J / K / L to play (with sound; L and J again for 2x, 4x, 8x), Left / Right to
step a frame, Up / Down for edits, I / O for marks (also while playing), Ctrl+B to split, Delete to ripple
delete, Ctrl+Z / Ctrl+Shift+Z for undo and redo, and Shift+2…8 for pages. Double-click a clip in the
Media Pool to open it in the source viewer, mark In and Out there (I, O), then F9 / F10 to insert or overwrite
at the playhead (F11 replaces the clip under the playhead, F12 places on top, Shift+F10 ripple-overwrites,
Shift+F11 fits to fill the timeline's In to Out, Shift+F12 appends at the end; LOG D-059, D-060); the keys act on whichever viewer was clicked last, and Q switches. The buttons along
the top show and hide panels (as Workspace > Show Panel in Workspace); Workspace > Theme picks a colour
theme, or start with `--theme ember` (midnight, ember, mint, rose, charcoal). Click a clip to
select it; click or drag the ruler to move the playhead. While playing, the viewer's top right shows the
frames per second reaching the screen. Put local test clips in `test media/`, which git ignores.

Video is decoded by the GPU where the operating system can (LOG D-014); `--software-decode` turns that
off, `--mute` plays without sound, `--cpu-render` draws every frame on the CPU instead of Resolution's
GPU renderer (LOG D-036). Two self-tests: `--play 6` plays six seconds and prints what was shown
and heard; `--screenshot out.png` saves the window; `--window-at X,Y` opens it at that screen
position in pixels, for self-tests on a second screen (`-1920,165` here; LOG D-049); `--journal out.jsonl` writes the action
journal as it goes, a line of JSON as each command starts and ends, for finding failures and freezes
(LOG D-054). To measure playback without a window:
`cargo run -p resolution-playback --example play_bench -- <clip | project.fcproj> [--hardware] [--side 1920] [--cpu-render]`; add
`--wav heard.wav` to save the sound it played, for comparing with ffmpeg's decode.

## 3. Use the MCP server from Claude Code

Open Claude Code in the repo folder. `.mcp.json` registers a `resolution` server
(`cargo run -q -p resolution-cli -- mcp --demo`, FilmCraft's demo project); approve it when asked,
or check it with `/mcp`. The first start compiles the app, so give it a minute.

Then just ask, for example: *"Look at the timeline, split the timeline at 2 s and at 4 s as one
batch, dry-run it first, then render the frame at 2.5 s."*

To work on a real project instead of the demo, edit `.mcp.json` locally (`"mcp", "--project",
"C:/path/to/edit.fcproj"`) or run `cargo run -p resolution-cli -- mcp --project <path>` from any MCP
client. Destructive commands (save/open/delete) ask the agent to confirm with you first.

**Agents in the open editor** (LOG D-026): `.mcp.json` also registers `resolution-editor`
(`mcp --editor`). With `resolution-app` running, an agent on that server edits the timeline you are
looking at, in the same undo history: its edits appear on screen as it makes them, the status bar says
"Agent: …", and Ctrl+Z undoes them like your own. Start the editor first; with none open the server
says so and exits.

The agent's edits go to a **draft** (LOG D-027). Its first edit copies the open timeline to "<name>
(agent draft)" in the same bin, and its edits land there. A banner above the timeline lists them,
with **Accept Draft** (the draft replaces the original) and **Reject Draft** (the draft is deleted).
Only you can accept or reject a draft; agents cannot.

## 4. Where things stand

| Area | State |
|---|---|
| Engine | FilmCraft **main at `cee1feb`** (2026-10-13, after v0.7.2; not a release tag) via the private mirror, pinned at `sync/fc-20261013` (`29d01cf`), synced 2026-10-13 (LOG D-015) |
| Mirror CI | **Red outside our graph** for the pin: run [38413841087](https://github.com/TramEFX/resolution-filmcraft/actions/runs/38413841087) (2026-10-13, Linux, ~53 min) on the sync point: 3,473 tests passed, 3 failed, 44 ignored; fmt, clippy, layers, assets, wasm ok. The three failures are in FilmCraft's UI crate and desktop app, which Resolution doesn't use (LOG B-020); the pin was taken anyway (D-015). The mixer speed test stays outside the gate (LOG B-007) |
| Engine boundary | `engine-api` trait + vocabulary (117 Resolve-named commands; 84 run headless on FilmCraft today, 32 are front-end, 1 not yet supported), FilmCraft adapter, drift test |
| Keymap | Resolve defaults as data (`keymaps/resolve.toml`); 73 of 82 bindings confirmed against the 21.1 Reference Manual (LOG D-051, D-074), the other 9 still to check in a real install; bindings are not page-scoped yet |
| MCP | 10 tools: search by page/Resolve key, run with risk tiers and dry runs, atomic batches, checkpoints, render frame; agents read the video scopes as numbers (`scopes.read`). 289 tests across the workspace (passing on Windows, 2026-10-15), 5 of them end to end over MCP stdio against the built binary (`apps/resolution-cli/tests/mcp_stdio.rs`) |
| GPU parity, real hardware | Local, 2026-10-12, at the previous pin (`2ba0629`): FilmCraft's 33 golden tests pass, the 4 GPU-vs-CPU ones on an AMD Radeon RX 9070 XT through Vulkan (the adapter wgpu's default request picks, checked with a probe). CI covers the same tests on software Vulkan (LOG B-005) |
| UI | **Edit page in the Black Box look** (`apps/resolution-app`, LOG D-012, D-016, D-018): interface toolbar showing and hiding panels in Resolve's fixed places, Media Pool, Index, source and timeline viewers side by side (scrub bars, transport, Mark In/Out; keys act on the active one, Q switches), Inspector, timeline toolbar (modes, insert/overwrite, snapping, linked selection, marker, zoom), track headers (destination control, lock, sync lock, auto track selector, show, solo, mute), markers on the ruler, waveforms on audio clips and the picture at each video clip's head (D-073), scrolling and zooming as in Resolve (wheel, Alt or Ctrl with the wheel, scroll bars, middle-drag), editing with the mouse in Resolve's Selection, Trim and Blade modes (move, resize, roll, ripple, slip, slide, blade) with snapping, page bar; a minimal Deliver page (six presets, render queue, Render All; renders checked with ffprobe); five colour themes; Geist type and our own icons. Effects, Sound Library, Metadata and Mixer slots are empty. Agents edit the open timeline through the agent link, into a draft the user accepts or rejects (D-026, D-027). **Color page** (D-028, D-029): viewer, Node Editor (serial nodes), clip strip with thumbnails and the Mini-Timeline (D-071), Primaries wheels with master rings and adjustment bars, or Bars mode (D-067) (with Shadows, Highlights and Midtone Detail; D-055), Custom Curves, Resolve's node keys, the Scopes panel (Waveform, Parade, Vectorscope, Histogram of the viewer's picture), Linear, Circle, Polygon, Curve and Gradient windows with an on-screen control and handles for size, rotation and softness, a Polygon's or Curve's Inside and Outside Softness and a Linear window's Soft 1 to 4 (D-031, D-052, D-053, D-057, D-061, D-063, D-065), an HSL qualifier with a picker and matte finesse (D-046, D-058), the Highlight view (Shift+H, Alt+Shift+H; D-048, D-062), and the hue curves (Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat, Sat vs Lum; D-043, D-047), sampled from the viewer or set with the six-vector patches (D-066); the viewers, agents' frame renders and Deliver show grades by their own math, drawn on Resolution's GPU renderer (D-036, D-042, D-044, D-045); grades also compile to Lumetri, so a project opens sensibly in FilmCraft and renders without a GPU (or with `--cpu-render`) still show them, less what Lumetri cannot carry; the other palettes are not built. 93 tests, including UI tests that press keys, click buttons, drag clips, wheels and curves, roll the wheel, render a file and drive the editor as an agent |
| Playback | **Real time, with sound** (`crates/playback`, LOG D-013): frames rendered ahead, sound mixed ahead, the audio device as the clock, GPU video decoding (D-014). Measured on this PC: phone 4K30, Sony α7R II 4K 23.976 (100 Mbit/s) and 1080p60 H.264 play at full rate with no dropouts, the sound sample-exact against ffmpeg and checked by ear (2026-10-13); a sideways (portrait) phone 4K clip at full rate too since the 2026-10-13 engine sync; 4K60 HEVC at about 50 fps on the GPU (B-013, fixed upstream); footage decoded in software at a few to 10 fps (B-017 no longer freezes it). Frames are drawn on Resolution's GPU renderer (D-036 to D-038; 2026-10-14): the graded Sony 4K clip at a 1440-pixel viewer at 24.0 fps, the exact frame on every refresh (14 fps on the CPU); the graded demo, whose composited section managed about 13 fps on the CPU, with every frame on the GPU and the exact frame on every refresh. Standard effects still run on the CPU for their layer (D-041) |
| CI for this repo | None yet: needs a token that can read the private mirror |

Gates before pushing: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.
Bugs and design decisions go in [LOG.md](LOG.md) as they come up.

## 5. Next steps

**The UI comes first, in the order agreed on 2026-10-13** ([ROADMAP](ROADMAP.md) "Order of work", LOG
D-016). Shared look and layout, then the Edit page up to the one-minute-edit bar, then a minimal
Deliver page, then agents in the editor, then the Color page. Steps 1 to 4 are built (D-018 to
D-027); the bar itself, Truman cutting a one-minute piece from the Sony and Samsung clips, has not
been tried yet. Step 5's first slice of the Color page is built: grades and the `color.*` vocabulary
(D-028), the page (D-029), its scopes (D-030), windows (D-031) and the HSL qualifier (D-032, D-033).

Resolution's own GPU renderer (D-035) is built in its three stages:
- **G1:** the viewers' frames on the GPU, with grades drawn by their own math and real key masks
  (D-036).
- **G2:** everything FilmCraft plans as layers drawn there too: dissolves, dips, titles, captions,
  CPU-rendered clips (D-037), blend modes, Push and HDR media (D-038), with chroma siting and large
  shrinks right (D-040). Two parts wait: showing frames straight from the GPU (D-039) is off,
  since it crashed the editor on a resize (B-036; it needs the GPU work on the UI thread), and the
  GPU port of FilmCraft's standard effects waits for an Effects panel (D-041).
- **G3:** Deliver through the same renderer, so a file matches the viewer, two to three times
  faster on graded 4K (D-042).

Grades now render by their own math on every path, graded at their source where FilmCraft draws a
frame itself (D-044; Deliver's own frames, D-045). The Lumetri compile stays only so a project opens
in FilmCraft and renders without a GPU. That opened Color features Lumetri cannot carry: the
qualifier's matte finesse (Denoise, Clean Black and White, Blur Radius), drawn by the GPU's grade
stage (D-046), and Sat vs Lum, the last of Resolve's hue curves (D-047; the others are D-043).

The qualifier's matte can be seen in the viewer: the Highlight view, flat grey or black and
white (D-048).

Not started, for Truman to pick from:
parallel and layer nodes, the one-minute-edit bar (rehearsed through the engine with
`scripts/rehearse-edit.py`, 19 steps fine; B-042; and through the editor's keys and mouse in a UI
test, where only Ctrl+T failed, now fixed; B-043), the action journal (D-034).
For what Resolve does, read the installed 21.1 Reference Manual (`scripts/resolve-manual.py`
extracts it for searching; LOG D-017), then try it in the local install.

Alongside, pick any:

1. **Follow the upstream reports.** As of the 2026-10-13 sync (LOG D-015), upstream has fixed #1249 (APV
   oracle), #1251 (Japanese font), #1282 (B-011) and #1483 (B-013), all taken. The mirror's font install
   (B-006) can now go. Still open: #1276 (B-010; upstream PR #1289 in review) and #1250, where a
   maintainer says it is fixed since `4ccc1a84` and asks us to retry; our CI's software Vulkan (B-005)
   hides the original failure, so a retry needs a run without it.
   The fixes are small (skip or fail clearly when the tool lacks a capability), and they shrink
   what our CI has to work around. A failure in FilmCraft's own tests is upstream's bug, not ours.
2. **Real footage:** first pass done on 2026-10-12 ([report](research/real-footage-2026-10.md)). 33 profiles
   from 127 local files all import and render, including HEVC 4K60, 5-track audio, rotation and
   variable frame rate. A phone 4K clip decodes correctly. Two timing defects were found (LOG B-010,
   reported as storytold/filmcraft#1276; B-011, fixed upstream and taken). Still needed: **camera and phone
   originals** (iPhone HEVC/HDR, 10-bit, GoPro, mirrorless, ProRes, MTS). The local files are mostly
   downloads. (The Sony and Samsung clips are in `test media/sony-samsung-2026-10-12/` since 2026-10-14,
   unpacked from `H:\Archive\Downloads-remainder\robnadj_rob-mid-shot-mp4_2026-10-12_2301.zip`;
   `test media/` is not in git.) Third pass, 2026-10-12: a Sony α7R II clip and a Samsung portrait clip decode, play and
   sound correctly; the rotated clip exposed a playback deadlock (B-015, fixed) and a software-decoding
   freeze (B-017, gone since the 2026-10-13 sync). This decides how soon we need a fallback decoder (research risk #6).
3. **Finish verifying the keymap** (D-017, D-051): 73 of 82 bindings are confirmed against the
   manual. Check the other 9 (the page keys, Mod+B, Shift+Backspace) in the local install's
   Keyboard Customization.
4. **CI for this repo:** a GitHub Actions workflow with a fine-grained token (read access to
   `resolution-filmcraft`) stored as a secret.
5. **Faster playback where it is slow** (LOG D-013 "Known limits"): frames are composited on
   Resolution's GPU renderer since D-036 to D-038, but FilmCraft's standard effects still run on
   the CPU for their layer (D-041), and adjustment layers and transitions other than dissolves,
   dips and Push are FilmCraft's CPU frames; proxies for footage too slow to decode. The 4K60 HEVC
   file plays at about 50 of 60 fps since B-013's fix. Measure with `play_bench` before and after.

Weekly: `scripts/upstream-status.sh` to see what FilmCraft changed; `scripts/sync-mirror.sh` to
take it (docs/UPSTREAM.md).
