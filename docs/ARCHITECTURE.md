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
| `Adapter` | Implemented in the adapter (undo/redo) |
| `Ui` | Owned by the Resolution front end: pages, zoom, edit modes, playback (FilmCraft keeps these in its egui crate) |
| `Missing(note)` | No FilmCraft equivalent yet: Resolution work or an upstream PR |

Its tests are the upstream-drift gate: every vocabulary id must be mapped, and every FilmCraft id
we map to must still exist at the pinned revision.

## The editor (`apps/resolution-app`)

An egui / eframe (wgpu) app (LOG D-012), and a client of `Engine` like the MCP server:

- **Engine thread.** The engine runs on its own thread (`worker.rs`). The UI sends commands and
  reads the replies each repaint, so a slow command never freezes the window, and an engine
  panic fails one command, not the app. After each batch the engine sends a snapshot: the
  timeline and project to draw, and a `Playback` the viewer renders and plays from.
- **Viewer and playback** (`crates/playback`, LOG D-013). Frames render on two worker threads into
  a cache, ahead of the playhead while playing; the viewer shows the exact frame or holds the
  newest one it has. Sound is mixed ahead on its own thread, and the audio device is the clock.
  Stopping sends `playhead.set` with the frame on screen, so the engine's playhead is where you
  stopped.
- **Keys are data.** A key press becomes a keymap chord (`keys.rs`), the keymap gives a vocabulary
  id, and the id runs. Front-end ids (`Target::Ui`: pages, playback, zoom, edit modes) are handled
  in the app; everything else goes to the engine. The menus run the same ids.
- **One reader of engine JSON.** `model.rs` alone knows the shape of `timeline.inspect` and
  `project.inspect` (FilmCraft's today). A typed, engine-neutral model in `engine-api` should
  replace it.
- **Looks are Resolution's own.** Dark theme and its own palette; Resolve's layout and keys only.
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

`examples/play_bench.rs` plays a clip in real time without a window and reports frames shown and
sound underruns; LOG D-013 has the numbers.

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

Still to build (from the research's lessons): agents editing a **draft timeline** the user
accepts; provenance **receipts** per operation; per-command **JSON Schemas** (today `params` is a
description string, as upstream FilmCraft has it, issue #347); an HTTP transport on loopback with
a per-launch token; scope readouts so agents can grade by numbers.
