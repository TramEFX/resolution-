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
   FilmCraft's playback (frame pool, play-ahead mixer, audio clock) currently lives in `ui-egui`;
   we either upstream a UI-free `filmcraft-playback` crate or own a port behind the trait.
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

## MCP design (next milestone)

Lessons from existing video MCP servers (details in the research report):

- **Page-scoped tool domains loaded on demand** (Media, Cut, Edit, Fusion, Color, Fairlight,
  Deliver, Project) behind a small always-loaded kernel: `list_domains`, `activate_domain`,
  `search_commands`, `describe_command`, `run_command`. Not one tool per command.
- **One result envelope:** operation id, diff summary, undo label, known limitations.
- **Safety:** dry-run on every mutating tool; risk tiers with explicit `acknowledge` for
  destructive operations; snapshot before mutation; source media is never modified; analysis
  goes to sidecars.
- **Agents work on a draft timeline** the user reviews and accepts.
- **See what it did:** `render_frame` (PNG) plus numeric scope readouts.
- **Transport:** stdio by default; any HTTP transport binds to loopback with a per-launch token.
