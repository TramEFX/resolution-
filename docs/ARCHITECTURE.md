# Architecture

## Layers

```
┌───────────────────────────────────────────────────────────────┐
│ Front ends: Resolve-style UI (pages)   MCP server   CLI         │  ← Resolution
├───────────────────────────────────────────────────────────────┤
│ Resolution-owned domain: keymap, colour node graph, page        │  ← Resolution
│ layouts, Fairlight buses, AI features (transcripts, search…)   │
├───────────────────────────────────────────────────────────────┤
│ crates/engine-api — Engine trait + command vocabulary           │  ← the boundary
├───────────────────────────────────────────────────────────────┤
│ crates/filmcraft-adapter — the only crate that imports filmcraft│  ← swap point
├───────────────────────────────────────────────────────────────┤
│ FilmCraft L0–L4: codecs, containers, time, project, edit,       │  ← upstream, pinned
│ render, gpu, export, interchange, engine (Session, commands)    │
└───────────────────────────────────────────────────────────────┘
```

## Invariants (enforced in review and, where possible, in tests)

1. **Only `crates/filmcraft-adapter` depends on `filmcraft-*` crates.** No FilmCraft type crosses
   the `Engine` trait. (To enforce: a CI check over `cargo metadata`.)
2. **Never depend on FilmCraft's `ui-egui`, `apps/*` or `automation`.** They are what we replace.
   FilmCraft keeps its playback (frame pool, play-ahead mixer, audio clock) in `ui-egui`, so
   Resolution owns a port of it, `crates/playback`, behind the `Playback` trait (LOG D-013).
3. **Every action is a command.** The UI, keymap, MCP server and AI features all call
   `Engine::execute` with a vocabulary id. No front end mutates project state directly.
   One undo history for people and agents.
4. **Command ids are stable API.** Vocabulary ids (`crates/engine-api/src/commands.rs`) are never
   renamed or reused. FilmCraft ids are an implementation detail of the adapter.
5. **Resolve-specific state lives in Resolution's namespace**, not in FilmCraft's project types
   (colour node graphs, page layouts, Fairlight routing): a sidecar or extension map, so engine
   syncs never need schema patches.
6. **Raw engine commands (`raw.filmcraft/<id>`) are an escape hatch** for agents and debugging.
   Nothing that must survive an engine swap may depend on them; promote anything we rely on
   into the vocabulary.
7. **Look like Resolve in workflow, not in pixels.** Page structure, edit types, terminology and
   keys are functional; icons, colours, styling and artwork are our own. No Blackmagic assets or
   screenshots in the repo.

## The adapter's command map

`crates/filmcraft-adapter/src/map.rs` maps every vocabulary id to one of:

| Target | Meaning |
|---|---|
| `Engine(id)` | Same behaviour in FilmCraft |
| `Approx(id, note)` | Close FilmCraft command; the note records how it differs from Resolve |
| `Adapter` | Implemented in the adapter: undo/redo, inspecting, commands on the project selection, the Color page's grades |
| `Ui` | Owned by the Resolution front end: pages, zoom, edit modes, playback (FilmCraft keeps these in its egui crate) |
| `Missing(note)` | No FilmCraft equivalent yet: Resolution work or an upstream PR |

Its tests are the upstream-drift gate: every vocabulary id must be mapped, and every FilmCraft id
we map to must still exist at the pinned revision.

## Colour grades (`crates/grade`, LOG D-028)

A clip's grade is Resolution's own type: serial corrector nodes, each with the Primaries
palette's controls and the Custom Curves, in Resolve's units. The math is ours and written down in
`crates/grade`: a node is a tone function per channel and then a saturation step. The adapter
(`color.rs`) keeps each grade as JSON in the timeline's metadata (`resolution.grade.<clip id>`)
and compiles it onto the clip as Lumetri Color effects at the front of its effect stack: the tone
functions as R, G and B curves, the saturation step as Saturation, Vibrance and a Hue vs Hue
curve. So every renderer (playback, frame renders, Deliver) shows the grade, and each edit is one
FilmCraft undo step holding both the metadata and the effects. A node's windows become effect masks on
its Lumetri effects, in the clip's pixels (LOG D-031). Lumetri runs these on the CPU; GPU grading is a
later step, and with it real key masks for qualifiers (D-032).

## The editor (`apps/resolution-app`)

An egui / eframe (wgpu) app (LOG D-012), and a client of `Engine` like the MCP server:

- **Engine thread.** The engine runs on its own thread (`worker.rs`). The UI sends commands and
  reads the replies each repaint, so a slow command never freezes the window, and an engine
  panic fails one command, not the app. After each batch the engine sends a snapshot: the
  timeline and project to draw, and a `Playback` the viewer renders and plays from.
- **Viewers and playback** (`crates/playback`, LOG D-013; `monitor.rs`, D-021). Two viewers, as in
  Resolve: the source viewer plays the clip open in it (the engine's `source_playback`, frames
  from FilmCraft's `render_item` and sound from the media), the timeline viewer the timeline
  (`playback`). Each has a `Monitor`: its frame server, frame on screen and transport. They share
  the audio device, and only one plays at a time. Frames render on worker threads into a cache,
  ahead of the playhead while playing; a viewer shows the exact frame or holds the newest one it
  has. Sound is mixed ahead on its own thread, and the audio device is the clock. Stopping sends
  `playhead.set` (or `source.seek`) with the frame on screen, so the engine's playhead is where
  you stopped. Playback keys, marks and frame steps act on the active viewer.
- **Keys are data.** A key press becomes a keymap chord (`keys.rs`), the keymap gives a vocabulary
  id, and the id runs. Front-end ids (`Target::Ui`: pages, panels, playback, zoom, edit modes) are
  handled in the app; everything else goes to the engine. The menus and toolbar buttons run the
  same ids, and their tooltips show the keymap's keys.
- **Fixed page layouts** (LOG D-016). The Edit page has Resolve's regions: a browser on the left
  (Media Pool, Effects Library, Index or Sound Library, one at a time), the viewer, the Inspector or
  Metadata on the right, the timeline below with the Mixer beside it. The interface toolbar shows
  and hides them with `workspace.*` ids (Resolve's Workspace > Show Panel in Workspace); panels
  never dock elsewhere. Effects, Sound Library, Metadata and Mixer are empty slots so far.
- **Editing with the mouse** (LOG D-020, `timeline_ui.rs`). The tool and where a drag starts pick
  the edit, as in Resolve: Selection moves, resizes and rolls; Trim ripples, rolls, slips and
  slides; Blade cuts the clicked clip. The drag previews where the edit lands, snapping if it is
  on, and becomes one vocabulary command when released (`timeline.moveClips`, `trimEdge`,
  `rollEdit`, `slipClip`, `slideClip`, `bladeClip`), so each drag is one undo step and agents
  can make the same edits.
- **One reader of engine JSON.** `model.rs` alone knows the shape of `timeline.inspect` and
  `project.inspect` (FilmCraft's today). A typed, engine-neutral model in `engine-api` should
  replace it.
- **Looks are Resolution's own** (LOG D-018). `theme.rs` has two layers: a `Skin` (the chrome:
  sizes, corners, gaps, icon weight, toolbar labels) and a `Theme` (colours only). One skin exists,
  Black Box, with five themes (Workspace > Theme, saved between runs). Every theme's greys are
  colourless, and a test checks that and the text contrast. Type is Geist and Geist Mono (OFL,
  bundled). Icons (`icons.rs`) are our own 16×16 line drawings in SVG path syntax, stroked in the
  theme's colours; `chrome.rs` has the shared buttons and panel strips. Resolve's layout and keys
  only, never its artwork.
- **Agents in the open editor** (LOG D-026, `crates/agent-link`). The editor listens on 127.0.0.1
  and writes the port and a random token to a file in the user's application-data folder;
  `resolution mcp --editor` connects and serves MCP over a `RemoteEngine`, which forwards each
  `Engine` call to the editor's engine thread. Agents' edits land in the same history and snapshots
  as the user's, so they show on screen as they happen.
- **Agent drafts** (LOG D-027, `worker.rs`). An agent's first edit is made on a copy of the open
  timeline, "<name> (agent draft)", and its later edits go there too. The worker keeps the draft's
  record of steps (made, undone, redone, by whom), which the banner lists. Accept and Reject are
  the user's alone: they are worker requests, not vocabulary commands.
- **The Color page** (LOG D-029, `color_page.rs`). It grades the clip under the playhead through
  `color.*` commands: the Node Editor, the clip strip (thumbnails rendered on a background thread
  from the playback snapshot), the Primaries wheels and the Custom Curves. A drag sends one
  `color.adjust` per frame under one merge key, so it is one undo step; while it lasts the page
  shows the values it sends rather than the engine's trailing answers. Its Scopes panel (LOG
  D-030) draws the scope of the frame the timeline viewer shows, on a background thread, through
  the engine's `Scopes` service: the engine draws the trace, the UI the graticule.
- **Tests drive the real UI.** `egui_kittest` presses keys and clicks clips headlessly.
  `resolution-app --screenshot out.png` saves the window once a frame is on screen; `--play <s>`
  plays and prints what was shown and heard.

## Playback (`crates/playback`)

Engine-neutral: it plays any `engine_api::Playback`, the open timeline frozen at one revision,
which renders frames and mixes sound on any thread (the FilmCraft adapter shares the session's
project snapshot). Ported from FilmCraft (LOG D-013; NOTICE):

| Module | Does | From FilmCraft |
|---|---|---|
| `frames` | Frame workers, priority queue, byte-budgeted cache; plays ahead and skips evenly when frames are slower than real time | `ui-egui/src/frames.rs` |
| `ahead` | Sound mixed 0.2 s ahead into a lock-free ring; underruns stay in sync | `ui-egui/src/play_ahead.rs` |
| `output` | The default audio device (cpal); its played samples are the clock | `apps/filmcraft/src/audio.rs` |
| `transport` | Preroll, play/stop/shuttle, audio or wall clock, device failure hand-over | `ui-egui/src/lib.rs` |

The FilmCraft adapter renders each frame worker's frames in a rayon pool of that worker's own:
apart from the sound's mixing, which shares rayon's global pool (LOG B-014), and apart from each
other, because FilmCraft's decoder holds its lock across rayon work (LOG B-015).

`examples/play_bench.rs` plays a clip in real time without a window and reports frames shown and
sound underruns; `--wav` saves the sound the device received. LOG D-013 has the numbers.

## MCP server (`crates/mcp`)

`resolution mcp [--project <path> | --demo]` serves MCP over stdio; `.mcp.json` wires it into
Claude Code for this repo. It is engine-agnostic (it only sees the `Engine` trait).

Ten fixed tools, not one per command: an agent's starting context stays small and the surface
works with every MCP client (no dynamic tool lists needed). Resolve's pages organise discovery
instead of tool loading.

| Tool | What |
|---|---|
| `list_pages` | Resolve's pages and how many commands each offers. Start here. |
| `search_commands` | By text, page, or Resolve shortcut (`Mod+B`, `F9`). Results carry page, risk, params, Resolve keys, and how this engine differs from Resolve. `include_raw` reaches the engine's own commands. |
| `run` | One command. Returns result, undo label, and `timeline_changes` (a diff of the timeline, matched by clip/track id). `dry_run` measures then reverts. |
| `run_batch` | Steps checked up front (preflight); `atomic` (default) undoes completed steps on failure; `dry_run` reverts everything after measuring the combined change. Undo and redo cannot be steps (LOG D-005). |
| `inspect` | `timeline`, `project`, or `session` (edits made through this connection, checkpoints). |
| `render_frame` | Program viewer as PNG at a time, without moving the playhead. |
| `undo` / `redo` | Shared history with the user. |
| `checkpoint` / `restore_checkpoint` | Name a point; undo this connection's edits back to it. |

Resources: `resolution://timeline`, `resolution://commands`.

Safety rules:

- **Risk tiers** on every command (`read`, `edit`, `destructive`). Destructive commands (open or
  save project files, delete files, exports) need `acknowledge: true`, cannot be dry-run, and may
  only be the last step of an atomic batch.
- **Strict arguments:** unknown fields are rejected, so a typo never silently does the default.
- **Unknown commands** get suggestions; vocabulary commands the engine can't do headless say so.
- Engine panics become an error for that call; the session survives.

Still to build (from the research's lessons): provenance **receipts** per operation (agents'
drafts, D-027, list edits by undo label only); per-command **JSON Schemas** (today `params` is a
description string, as upstream FilmCraft has it, issue #347); an HTTP transport on loopback with
a per-launch token; scope readouts so agents can grade by numbers.
