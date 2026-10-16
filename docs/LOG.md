# Decision & Issue Log

**Purpose.** A chronological path history: every design decision and every bug, with enough
context to reconstruct *why* things are the way they are. [ARCHITECTURE.md](ARCHITECTURE.md),
[UPSTREAM.md](UPSTREAM.md) and the other docs say what is true now; this file says how we got
there, including the wrong turns.

**Why both.** The other docs are current truth, and reading them should never require archaeology.
But current truth alone loses the reasoning, and reasoning is what stops a settled decision being
silently re-litigated, or a fixed bug being reintroduced. When an entry changes another doc, that
doc is updated *and* the entry says which section moved.

## Conventions

- **`D-nnn`** is a design decision. **`B-nnn`** is a bug or an incorrect assumption.
- IDs are permanent and never reused. Reference them from code comments and commit messages
  (`LOG B-001`).
- Record entries **when they come up**, not in a cleanup pass later.
- Entries are append-only. A reversed decision is not edited: a new entry supersedes it, and the
  old one's status says so.
- **`Caught by`** is always recorded (a test, review, running it). Over time it shows which defects
  tests catch and which only surface through reading or use.

## Index

| ID | Date | Area | Title | Status |
|---|---|---|---|---|
| D-001 | 2026-10-09 | engine | FilmCraft as the engine, behind a Resolution-owned `Engine` trait | active |
| D-002 | 2026-10-09 | upstream | Engine from a private mirror, pinned to `sync/fc-*` branches | active |
| D-003 | 2026-10-09 | mcp | Ten fixed MCP tools; Resolve's pages organise discovery | active |
| D-004 | 2026-10-09 | mcp | Risk tiers, strict arguments, acknowledgement for destructive commands | active |
| B-001 | 2026-10-09 | mcp | `edit.undo` run as a command was counted as a new agent edit | fixed |
| B-002 | 2026-10-09 | mcp | Redo claimed steps the agent never made | fixed |
| D-005 | 2026-10-09 | mcp | Undo and redo cannot be batch steps | active |
| D-006 | 2026-10-09 | testing | MCP tested end to end over stdio, in both protocol modes | active |
| B-003 | 2026-10-09 | ci | Mirror CI saved no cache on a failed run, and never caches the engine crates | mitigated |
| D-007 | 2026-10-09 | ci | Mirror CI builds with cargo's stock release settings | active |
| B-004 | 2026-10-09 | ci | Mirror CI's ffmpeg (Ubuntu 6.1.1) cannot read APV, failing upstream's APV oracle tests | fixed by D-008 |
| D-008 | 2026-10-09 | ci | Mirror CI uses a pinned static ffmpeg and requires the oracle tests | active |
| B-005 | 2026-10-10 | ci | Upstream's GPU parity tests fail on a GPU-less runner's software OpenGL | fixed |
| D-009 | 2026-10-10 | ci | Mirror CI runs every test binary before the gate | folded into D-010 |
| B-006 | 2026-10-10 | ci | Upstream's Japanese-UI test needs a system CJK font the runner lacks | fixed upstream (#1259), taken in D-015; the mirror's font install can go |
| B-007 | 2026-10-10 | ci | Upstream's mixer speed test needs 4× realtime; the GitHub runner manages 2.1× | worked around by D-010; cause corrected; fixed upstream after the pin |
| D-010 | 2026-10-10 | ci | Mirror CI runs upstream's gate phase by phase, leaving out one speed test | active |
| D-011 | 2026-10-12 | upstream | Sync to FilmCraft v0.6.0, a release tag, not main | active |
| B-008 | 2026-10-12 | tooling | upstream-status.sh printed "(none)" after listing command-id changes | fixed |
| B-009 | 2026-10-12 | tooling | sync-mirror.sh --apply did nothing after a sync | fixed |
| B-010 | 2026-10-12 | engine | MP4 with a leading empty edit plays one frame early | open, reported upstream |
| B-011 | 2026-10-12 | engine | Frames timestamped just after the timeline grid show one frame late | fixed upstream (#1328), taken in D-015; checked on the Android clip |
| D-012 | 2026-10-12 | ui | The editor is an egui app, a client of the command API like MCP | active |
| D-013 | 2026-10-12 | playback | Playback is ported from FilmCraft into an engine-neutral crate, fed by a `Playback` snapshot | active |
| D-014 | 2026-10-12 | playback | The editor decodes video with the operating system's hardware decoders | active |
| B-012 | 2026-10-12 | playback | Frames slower than real time were never shown: each was cancelled as the next fell due | fixed |
| B-013 | 2026-10-12 | engine | Hardware HEVC decoding gives up when the in-band PPS has one trailing zero byte fewer | fixed upstream (#1492), taken in D-015; checked on the 4K60 file |
| B-014 | 2026-10-12 | playback | Sound dropped out while frames rendered: the mixer queued behind them in rayon's global pool | fixed; its shared pool caused B-015, reworked there |
| B-015 | 2026-10-12 | playback | Two frame workers deadlocked on sideways phone video: their renders shared one rayon pool | fixed |
| B-016 | 2026-10-12 | tooling | play_bench did not count a frame held until the end of the run | fixed |
| B-017 | 2026-10-12 | playback | Footage decoded slower than real time can freeze instead of playing at a few frames a second | not reproduced since D-015's sync; cause not found |
| B-018 | 2026-10-13 | tooling | upstream-status.sh reported two live command ids as removed: rustfmt had split their `cmd!(` calls over lines | fixed |
| D-015 | 2026-10-13 | upstream | Sync to FilmCraft main (`cee1feb`), not the v0.7.2 tag, to take the fixes for our reports | active |
| B-019 | 2026-10-13 | tooling | Deleting merged PR #5's branch closed the PR stacked on it (#6) instead of retargeting it | worked around |
| B-020 | 2026-10-13 | ci | Upstream main fails three UI and desktop-app tests at the D-015 sync point (crates outside our graph) | open, upstream |
| D-016 | 2026-10-13 | ui | Take the Edit page to a usable bar, then agents and Color, not every page in turn | active |
| D-017 | 2026-10-13 | ui | What Resolve does comes from its Reference Manual, then from the local install | active |
| D-018 | 2026-10-13 | ui | The editor's look is "Black Box", with a few colour themes now and skins later | active |
| B-021 | 2026-10-13 | docs | HANDOFF counted 71 vocabulary commands and 46 headless; there were 75 and 50 | fixed |
| D-019 | 2026-10-13 | engine | The adapter answers `timeline.inspect`, adding the editor state FilmCraft keeps outside the sequence | active |
| D-020 | 2026-10-13 | ui | Mouse editing follows Resolve's tools; each drag is one command and one undo step | active |
| B-022 | 2026-10-13 | ui | Slipping a clip by dragging moved its media the wrong way | fixed |
| B-023 | 2026-10-13 | engine | FilmCraft refuses a ripple that Resolve makes by cutting sync-locked clips across the edit | open; worked around with sync lock switches |
| D-021 | 2026-10-13 | ui | A source viewer beside the timeline viewer: a second `Playback`, and keys act on the active viewer | active |
| D-022 | 2026-10-13 | ui | Waveforms come from `Playback::peaks`, read on a background thread | active |
| D-023 | 2026-10-13 | ui | Timeline navigation follows Resolve's wheel and scroll-bar controls; the view pages to the playhead | active |
| D-024 | 2026-10-13 | ui | New Timeline (Ctrl+N) starts empty in the open timeline's format; the source viewer shows the media's own timecode | active |
| D-025 | 2026-10-13 | ui | A minimal Deliver page: presets, a render queue the app keeps, renders as engine export jobs | active |
| D-026 | 2026-10-13 | mcp | Agents reach the open editor through a local, token-checked link; `resolution mcp --editor` serves MCP over it | active |
| B-024 | 2026-10-13 | docs | HANDOFF counted 70 tests in the workspace and 42 in the editor; there were 107 and 43 | fixed |
| B-025 | 2026-10-13 | ui | The editor sent an agent's draft before the timeline it described | fixed |
| B-026 | 2026-10-13 | ui | The draft's list of agent edits kept edits that had been undone | fixed |
| B-027 | 2026-10-13 | engine | `timeline.duplicate` put the copy at the top of the project; Resolve puts it in the original's bin | fixed |
| B-028 | 2026-10-13 | ui | An agent's edit was lost, though reported made, when its draft failed after the copy | fixed |
| D-027 | 2026-10-13 | ui | An agent's edits go to a draft of the timeline, which only the user accepts or rejects | active |
| D-028 | 2026-10-14 | color | Grades are Resolution's own node model, compiled to FilmCraft Lumetri effects on each clip | active (the compile: for FilmCraft and renders without a GPU since D-044) |
| B-029 | 2026-10-14 | color | `color.inspect` showed 0.30000001192092896 for a control set to 0.3 | fixed |
| D-029 | 2026-10-14 | ui | The Color page: viewer, Node Editor, clip strip, Primaries wheels and Custom Curves in Resolve's places | active |
| B-030 | 2026-10-14 | ui | The Color page's clip strip drew its tiles one text line tall: no pictures, no names | fixed |
| B-031 | 2026-10-14 | ui | An agent's draft had no Accept or Reject on the Color page | fixed |
| B-032 | 2026-10-14 | build | `target/` grew to 28 GB and filled the disk: Cargo keeps every engine pin's build | fixed |
| D-030 | 2026-10-14 | color | Video scopes: drawn by the engine from the frame the viewer shows; read as numbers by agents | active |
| B-033 | 2026-10-14 | ui | The Color page's palettes shrank a few points every frame, down to their minimum | fixed |
| D-031 | 2026-10-14 | color | Windows: Linear and Circle windows on a node, compiled to FilmCraft effect masks | active |
| D-032 | 2026-10-14 | color | Qualifiers: baked into a 3D LUT for now; real key masks in our own GPU renderer are the goal | done (built: D-033; real keys: D-036, D-044) |
| D-033 | 2026-10-14 | color | The qualifier as built: Resolve's HSL ranges, a qualified node baked into a 33-point project LUT | active (the LUT: the compile's only since D-044) |
| D-034 | 2026-10-14 | tools | An action journal: every command, who sent it, how long it took and how it ended, for finding failures and freezes | built in part: D-054 |
| D-035 | 2026-10-14 | color | Our own GPU renderer: the viewer's frames, with Resolution's grade math run exactly on the GPU | active (G1 to G3 built: D-036 to D-042) |
| B-034 | 2026-10-14 | color | The GPU renderer blended 8-bit sRGB pixels before decoding them: enlarged pictures went dark between pixels | fixed |
| B-035 | 2026-10-14 | color | The GPU renderer drew 8-bit camera video about one code value too red and too blue | fixed |
| D-036 | 2026-10-14 | color | G1 built: the viewers' frames on Resolution's GPU renderer, grades by their own math | active |
| D-037 | 2026-10-14 | color | G2, step 1: every frame FilmCraft plans as layers is drawn on the GPU; graded clips known by their pictures | active (Lumetri kept on clips with other effects: replaced by D-044) |
| D-038 | 2026-10-14 | color | G2, step 2: blend modes, Push and HDR media on the GPU, ported from FilmCraft's compositor | active |
| D-039 | 2026-10-14 | color | G2, step 3: the viewers show the renderer's frames on the GPU, without a copy back | off for now (B-036) |
| D-040 | 2026-10-14 | color | G2, step 4: a shrunk picture averages all its pixels; chroma read as sited left | active |
| D-041 | 2026-10-14 | color | The GPU port of FilmCraft's standard effects waits for an Effects panel | active |
| D-042 | 2026-10-14 | color | G3: Deliver renders through the GPU renderer, so a file matches the viewer | active |
| D-043 | 2026-10-14 | color | The hue curves: Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat | active |
| D-044 | 2026-10-14 | color | Grades render by their own math on every path: graded clips' pictures graded at their source | active (Deliver's: D-045) |
| B-036 | 2026-10-14 | color | The editor stopped when its window was resized while frames rendered (since D-039) | fixed |
| D-045 | 2026-10-14 | color | Deliver renders the graded copy, so its own CPU frames show grades by their own math | active |
| D-046 | 2026-10-14 | color | Matte finesse for the qualifier: Denoise, Clean Black, Clean White, Blur Radius; the GPU's grade stage | active |
| B-037 | 2026-10-14 | color | Since D-044 the colour tests of the Lumetri compile drew their frames on the GPU | fixed |
| D-047 | 2026-10-14 | color | Sat vs Lum, with a node carrying it baked into a table in the compile | active |
| B-038 | 2026-10-14 | color | A Deliver render could stop at 0 %: export frames waited for renderers their own threads held | fixed here; upstream #1531, #1956 (fix in PR #861) |
| D-048 | 2026-10-14 | color | The Highlight view: a front-end viewer option, drawn by the GPU's grade stage | active |
| D-049 | 2026-10-14 | tools | `--window-at`: the editor's self-tests open on a screen of their own | active |
| D-050 | 2026-10-14 | tools | `--highlight` self-test flag; the Highlight view, matte finesse and Sat vs Lum seen by eye on the Sony clip | active |
| B-039 | 2026-10-14 | ui | The keymap had Add Video Only and Add Audio Only Transition on each other's keys | fixed |
| D-051 | 2026-10-14 | ui | The keymap checked against the 21.1 Reference Manual: 69 of 78 bindings confirmed | active |
| D-052 | 2026-10-14 | color | The Gradient window: the picture divided by a line, the side above it keyed | active |
| B-040 | 2026-10-14 | color | Soft windows in the Lumetri compile stray from the grade's math at their soft edge | known (compile only) |
| D-053 | 2026-10-14 | color | The Polygon window: corners in the window's own space, edited on screen | active (point format provisional, for Truman's review) |
| D-054 | 2026-10-14 | tools | The action journal's core: the engine thread notes every command; `--journal` writes it to a file | active (panel and agents' access: for Truman) |
| D-055 | 2026-10-14 | color | Primaries: Shadows, Highlights and Midtone Detail | active |
| D-056 | 2026-10-14 | color | Deliver's speed checked after B-038's fix: unchanged | active |
| D-057 | 2026-10-14 | ui | Window handles on screen: size, rotation and softness | active |
| B-041 | 2026-10-14 | ui | A quick drag on the window control missed small handles and polygon corners | fixed |
| D-058 | 2026-10-14 | color | More matte finesse: Black Clip, White Clip and Morph | active |
| D-059 | 2026-10-14 | engine | Replace, Place on Top, Ripple Overwrite and Append at End, composed from FilmCraft's commands | active |
| D-060 | 2026-10-14 | engine | Fit to Fill: the source retimed to fill the timeline's In to Out | active |
| D-061 | 2026-10-15 | color | The Curve window: a closed Catmull-Rom curve through its points | active (format with D-053's, provisional) |
| D-062 | 2026-10-15 | color | The Highlight view for clips graded at their source, and frames FilmCraft draws itself | active |
| B-042 | 2026-10-15 | ui | The timeline did not show transitions | fixed |
| D-063 | 2026-10-15 | color | Inside and Outside Softness for Polygon and Curve windows | active (field names provisional) |
| D-064 | 2026-10-15 | edit | Dynamic Trim mode: what it takes, and two ways to build it | proposed, for Truman |
| B-043 | 2026-10-15 | engine | Ctrl+T added no transition from the editor | fixed |
| D-065 | 2026-10-15 | color | A Linear window's Soft 1 to 4: each side's own softness | active (side order and field name provisional) |
| B-044 | 2026-10-15 | tests | The one-minute edit's UI test timed out in a full run | fixed |
| D-066 | 2026-10-15 | color | Hue curves: image sampling from the viewer and the six-vector patches | active (widths provisional) |
| D-067 | 2026-10-15 | color | The Primaries palette's Bars mode | active |
| D-068 | 2026-10-15 | engine | Vocabulary additions proposed as one batch: edit points, Resolve's unbound keys, the Inspector | proposed, for Truman |
| B-046 | 2026-10-15 | engine | A reopened project's windowed grades looked foreign, and copies split from them lost their grade | fixed |
| B-047 | 2026-10-15 | tests | The Primaries drag test checked the timeline before its answer came | fixed |
| B-045 | 2026-10-15 | ui | Menu buttons drew a box where their "▾" should be | fixed |
| B-048 | 2026-10-15 | ui | "Outside Soft" ran into its value in the Window palette | fixed |
| D-069 | 2026-10-15 | tools | `--palette` and `--bars` self-test flags; D-063, D-065, D-066 and D-067 seen by eye | active |
| B-049 | 2026-10-15 | engine | Agents could not learn most of a node's controls: `color.adjust`'s description stopped at D-028 | fixed |
| B-050 | 2026-10-15 | engine | A clip of another size than its timeline showed cropped at its own size, not fitted as in Resolve | fixed |
| D-070 | 2026-10-15 | color | Highlight Difference: how far a node moves each pixel, drawn; its command waits on Truman | active (gain provisional; command proposed) |
| D-071 | 2026-10-15 | color | The Color page's Mini-Timeline | active |
| D-072 | 2026-10-15 | color | The qualifier's HSL and Luma modes | active |
| B-051 | 2026-10-15 | ui | Matte Finesse's numbers ran under the scopes; the qualifier's labels ran into their values | fixed |
| B-052 | 2026-10-15 | ui | Each window added grew the palettes and shrank the Color page's viewer | fixed |
| B-053 | 2026-10-15 | ui | Self-test runs saved their layout into the user's editor | fixed |
| B-054 | 2026-10-15 | ui | Nodes past the fourth ran off the Node Editor's edge | fixed |
| D-073 | 2026-10-15 | ui | The timeline's video clips show the picture at their head | active (Resolve's default view unchecked) |
| D-074 | 2026-10-15 | ui | Keys for the node resets and Save Project As | active |
| D-075 | 2026-10-15 | tests | A short grade rehearsed through the Color page's keys and mouse | active |
| B-055 | 2026-10-15 | ui | Alt+S right after a change left the old node current | fixed |
| B-056 | 2026-10-15 | ui | A window added right after a change left the old window chosen | fixed |
| D-076 | 2026-10-15 | tests | The Edit page's tools rehearsed in a row | active |
| B-057 | 2026-10-15 | ui | B-056's fix let go of the new window too early, one run in seven | fixed |
| D-077 | 2026-10-15 | engine | Thumbnails show each clip on its own | active |
| B-058 | 2026-10-15 | ui | On a laptop's screen the Color page's viewer was squeezed and the scope buttons ran over the Curves palette | fixed (the 1000 × 640 minimum still cramped) |
| B-059 | 2026-10-15 | ui | Narrow number fields: names ran into values, and the YRGB numbers into each other | fixed |
| B-060 | 2026-10-15 | ui | On a laptop's screen the Deliver page's Add to Render Queue was out of sight | fixed |
| B-061 | 2026-10-15 | ui | In a small window the two viewers' transport rows ran into each other | fixed |
| B-062 | 2026-10-15 | ui | A self-test with `--theme` saved its theme into the user's editor | fixed |
| B-063 | 2026-10-15 | ui | On the Color page, Delete, Backspace and Ctrl+B still cut the timeline | fixed |

D-001 to D-004 were recorded when this log was started, from the docs as of `3d38ba2`; they
summarise decisions made before it existed and point to where the full reasoning lives.

---

### D-001 — FilmCraft as the engine, behind a Resolution-owned `Engine` trait
**Area:** engine · **Status:** active · **Doc:** [research report](research/FilmCraft%20engine%20alternatives.md), ARCHITECTURE "Invariants" 1–2

**Decision:** build on FilmCraft's L0–L4 crates, consumed as a pinned dependency, never merged
into. Only `crates/filmcraft-adapter` may import `filmcraft-*`; everything above speaks
Resolution's own vocabulary through `Engine`.

**Why:** no other open engine is both permissively licensed and deep: pure-Rust codecs, ~549
typed commands, EDL/FCP7/FCPXML/OTIO/AAF/OMF interchange. Concat is the closest technical peer
but AGPL-3.0. **Cost:** FilmCraft is Premiere-shaped (no colour node graph), its playback lives in
its UI crate, and it churns fast; the trait keeps each of those an adapter problem.

### D-002 — Engine from a private mirror, pinned to `sync/fc-*` branches
**Area:** upstream · **Status:** active · **Doc:** [UPSTREAM.md](UPSTREAM.md)

**Decision:** `TramEFX/resolution-filmcraft` holds an untouched `upstream` branch, a `resolution`
patch stack, and one `sync/fc-YYYYMMDD` branch per sync point; `Cargo.toml` pins a sync point's
commit.

**Why:** upstream lands ~100 commits a day with no server-side CI and no stability promise, so
improvements are taken on purpose at tested points. Branches rather than tags because agent
sessions can push branches but not tags. The untouched copy is `upstream`, not `main`, because
upstream's workflows run on pushes to `main` and would spend the private repo's Actions minutes.
Windows and macOS mirror CI are manual runs for the same reason (2x and 10x minutes).

### D-003 — Ten fixed MCP tools; Resolve's pages organise discovery
**Area:** mcp · **Status:** active · **Doc:** ARCHITECTURE "MCP server"

**Decision:** a fixed tool set (`list_pages`, `search_commands`, `run`, `run_batch`, `inspect`,
`render_frame`, `undo`, `redo`, `checkpoint`, `restore_checkpoint`), not one tool per command.

**Why:** an agent's starting context stays small, and the surface works with every MCP client
because no dynamic tool lists are needed. Agents find commands by Resolve page, text, or Resolve
shortcut instead.

### D-004 — Risk tiers, strict arguments, acknowledgement for destructive commands
**Area:** mcp · **Status:** active · **Doc:** ARCHITECTURE "MCP server" (safety rules)

**Decision:** every command carries a risk (`read`, `edit`, `destructive`). Destructive commands
need `acknowledge: true`, cannot be dry-run, and may only end an atomic batch. Unknown argument
fields are rejected.

**Why:** an agent shares one undo history with the user, so everything it does must either be
undoable or explicitly agreed to; and a typo must never silently fall back to a default.

### B-001 — `edit.undo` run as a command was counted as a new agent edit
**Area:** mcp · **Status:** fixed · **Caught by:** review (reading the code); confirmed by tests written before the fix, which failed · **Doc:** ARCHITECTURE "MCP server"

**Symptom:** `edit.undo` is in the vocabulary, and `search_commands` returns it for `Mod+Z`, so an
agent can call it through `run` instead of the `undo` tool. A dry run of it undid *two* edits and
still reported `reverted: true`. In the test, the demo timeline had cuts at 2 s and 4 s. After the
"dry run", clip 30 was back to 119 frames, uncut, where it had been 47. A real undo was recorded in
`agent_edits` as a new edit, so a later `restore_checkpoint` would undo too many steps. In a batch,
an atomic rollback would undo one step further instead of redoing.

**Root cause:** the adapter returned the *undone* step's label as `Outcome.undo_label`, which the
trait documents as the undo step the command *created*. The session trusts that field: it adds it
to the agent's edits, and reverts a dry run with one more undo.

**Fix:** the adapter returns `undo_label: None` for undo and redo, and puts the label in `result`.
The session runs `edit.undo` and `edit.redo` itself, through the same bookkeeping as the undo and
redo tools. A dry run steps on the engine alone, measures, then steps back the other way. Both
commands take no parameters (strict arguments, D-004), and batches refuse them (D-005).

**Tests:** `undo_and_redo_commands_add_no_undo_step` (adapter);
`dry_running_undo_steps_back_once_and_returns` and `undo_as_a_command_is_not_counted_as_an_edit`
(mcp). Also checked over MCP stdio against the built binary, with a script that is not in the repo.

**Worth noting:** no existing test caught it. Each one undid through the `undo` tool, never through
`run`: two paths into the same history, with different bookkeeping.

### B-002 — Redo claimed steps the agent never made
**Area:** mcp · **Status:** fixed (it was latent: no front end shares the history yet) · **Caught by:** review while fixing B-001; confirmed by a test that failed before the fix

**Symptom:** suppose a step was already in the shared history when the agent connected (the
user's edit). If the agent undid it and then redid it, `agent_edits` listed it (`["Add Edit"]`), so
`restore_checkpoint` would undo the user's edit.

**Root cause:** `redo` added every redone label to the agent's edits without knowing whether it
had been one.

**Fix:** each undone step now records whether it was the agent's, and redo gives back only those.
Test: `redo_does_not_claim_edits_the_agent_did_not_make`.

**Still open:** steps are matched by label (`"Add Edit"`), and labels are not unique. Once the UI
shares the history, an agent's edit and a user's edit with the same label can be confused. That
belongs to the draft-timeline design (HANDOFF next step 4), not to this fix.

### D-005 — Undo and redo cannot be batch steps
**Area:** mcp · **Status:** active · **Doc:** ARCHITECTURE "MCP server" (`run_batch` row); the `run_batch` tool description

**Decision:** `run_batch` refuses `edit.undo` and `edit.redo` steps; agents use the `undo` and
`redo` tools instead.

**Why:** a batch promises it can roll back. After an undo, any later edit discards the redo
history, so a batch like `[undo, split]` cannot be reversed: undoing the split and then redoing
brings back the split, not the step the batch undid. Allowing only the orders that can be rolled
back would add rules for a rare case. Revisit if agents turn out to need it.

### D-006 — MCP tested end to end over stdio, in both protocol modes
**Area:** testing · **Status:** active · **Doc:** HANDOFF "Where things stand" (MCP row)

**Decision:** `apps/resolution-cli/tests/mcp_stdio.rs` starts the built `resolution mcp --demo`
binary and speaks JSON-RPC to it over stdio, as an MCP client does. It replaces the hand-run
stdio check from the hand-off and the throwaway script used for B-001. It needs no new
dependencies, and a 60 s per-response timeout makes a hung server fail the test instead of hanging it.

**Why:** the `crates/mcp` tests call `AgentSession` directly, so the wire layer had no test:
the handshake, argument checking (D-004: an unknown field such as `dryrun` must be an error), image
content from `render_frame`, and the protocol versions. Checked: against the pre-B-001 code, the
undo test fails at its dry-run assertion.

**Two protocol modes, both tested:** in rmcp 3.5, `2026-07-28` is a protocol version *without* an
`initialize` handshake. Every request carries `io.modelcontextprotocol/protocolVersion` and
`clientCapabilities` in `_meta`, and a request without them is rejected. A client that does
handshake is capped at `2025-11-25` (`ProtocolVersion::LATEST_WITH_INITIALIZE`), even if it asks
for `2026-07-28`. The `cache_hints` in `crates/mcp/src/server.rs` therefore only appear in the
handshake-free mode (`ttlMs` 600000 for resource lists, 0 for the live timeline, `cacheScope`
private). Only that mode exercises them, which is why it gets its own test.

### B-003 — Mirror CI saved no cache on a failed run, and never caches the engine crates
**Area:** ci · **Status:** mitigated · **Caught by:** review: reading `mirror-ci.yml` and rust-cache's `action.yml` while the first gate run was still going

**Symptom:** the first gate run on `sync/fc-20261009` (run 37984421765, started by hand) was still
in `cargo xtask ci` after 2h40m of its 180-minute limit. If it timed out, it would save nothing,
so a rerun would start just as cold and very likely time out again.

**Root cause:** two facts about `Swatinem/rust-cache@v2`, read from its source:
- It saves only when the job succeeds (`post-if: success() || env.CACHE_ON_FAILURE == 'true'`),
  and the workflow did not set `cache-on-failure`.
- It never caches workspace crates (`cache-all-crates` defaults to false), and FilmCraft's 47
  packages *are* the workspace. So even a green run only speeds up third-party dependencies.

**Fix (mirror `f96ed71`):** `cache-on-failure: true`, and a 165-minute limit on the gate step inside
the job's 180, so a slow gate fails as a step with time left for the save. The second fact is
inherent to rust-cache and stays: every gate run rebuilds FilmCraft itself. D-007 makes that
rebuild cheaper.

**Verified (run 38000818366):** the gate failed (B-004), and the cache step still ran
`... Saving cache ...`. The cancelled first run's log shows that step as `skipped`.

### D-007 — Mirror CI builds with cargo's stock release settings
**Area:** ci · **Status:** active · **Doc:** UPSTREAM.md "Mirror branches"; mirror commit `f96ed71`

**Decision:** `mirror-ci` sets `CARGO_PROFILE_RELEASE_LTO=false` and
`CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16` for the gate job. Upstream's `Cargo.toml` is untouched, so
this is mirror plumbing, not an engine patch.

**Why:** `cargo xtask ci` runs fmt, `clippy --release`, `test --release`, the layer and asset
checks, and a wasm32 build: three release builds of ~322k lines. Upstream's release profile (thin
LTO, one codegen unit) is the slowest to compile, and LTO re-optimises every test binary at link
time. Checked on a probe crate: with these variables, `cargo test --release` and
`cargo clippy --release` compile with `-C codegen-units=16` instead of
`-C codegen-units=1 -C lto=thin`, and `opt-level=3` is unchanged.

**Cost:** CI no longer builds exactly as upstream's release does, so a defect that only appears
under LTO or single-unit codegen would pass the gate. That is rare, and it would be a compiler or
undefined-behaviour problem rather than an engine-logic one. Resolution's own release profile
still uses thin LTO and one codegen unit.

**Measured**, from the two runs' logs (both Linux, ubuntu-24.04, cold cache):

| Phase | Upstream profile (run 37984421765) | These settings (run 38000818366) |
|---|---|---|
| `clippy --release` | 2m25s | 3m08s |
| `test --release` compile | not finished after 2h36m (cancelled) | 25m35s |
| Whole `cargo xtask ci` step | > 2h39m | 29m24s (stopped at B-004) |

In the first run, no new crate started compiling for 136 minutes: a few large crates sat in
single-unit release codegen. The layer, asset and wasm phases have not run under either setting yet.

**Correction (2026-10-10):** the Cost paragraph says these settings change "how well code is optimised,
not what correct code does". That misses one case: speed tests, which measure exactly that. Checked
for the only speed test that fails in CI (B-007): no measurable difference, 4.4× realtime under
both settings. So that failure comes from the runner, not from this decision. (Its actual
cause is in B-007's correction: contention inside the suite.)

### B-004 — Mirror CI's ffmpeg (Ubuntu 6.1.1) cannot read APV, failing upstream's APV oracle tests
**Area:** ci · **Status:** fixed by D-008 · **Caught by:** running it (mirror run 38000818366)

**Symptom:** the gate failed in `cargo test --workspace --release` at `filmcraft-apv`'s
`tests/oracle.rs`. All 3 tests failed with ffmpeg's `Unknown input format: 'apv'`. Cargo stops at
the first failing test binary, so the rest of the workspace's tests, and the layer, asset and
wasm checks, did not run. **The pin is not yet confirmed by CI.**

**Root cause:** an environment problem, not an engine bug and not D-007. The workflow installs
ffmpeg from Ubuntu 24.04's apt (`7:6.1.1-3ubuntu5`), which has no APV support. FilmCraft's oracle
tests compare its decoders with the system ffmpeg, and upstream (developing on macOS with a
current Homebrew ffmpeg) documents no minimum version. Its test kit skips an oracle test only when
ffmpeg is *missing*. When ffmpeg is present but too old, the test fails.

**Fixed** by D-008 (mirror `3ea75ec`). In run 38005601927, all three APV oracle binaries passed
(`decoder_oracle`, `encoder_oracle`, `oracle`) with ffmpeg 9.0.2, and 51 test binaries passed
before the next failure (B-005).

**Fix options considered:**
- Install a current static ffmpeg in CI and point `FILMCRAFT_FFMPEG` / `FILMCRAFT_FFPROBE` at it.
  This means downloading a third-party binary into CI; pin it by version and checksum.
- Also set `FILMCRAFT_REQUIRE_ORACLES=1`, which upstream's docs/testing.md recommends for CI. It
  turns a skipped oracle test into a failure, so CI cannot pass by quietly skipping.
- Upstream: have the APV oracle test skip, or fail clearly, when ffmpeg lacks the `apv` format.

**Reported upstream** 2026-10-12: storytold/filmcraft#1249, with FFmpeg's changelog for the
8.0 minimum.

### D-008 — Mirror CI uses a pinned static ffmpeg and requires the oracle tests
**Area:** ci · **Status:** active · **Doc:** mirror commit `3ea75ec` (`mirror-ci.yml`, step "ffmpeg for the oracle tests")

**Decision:** the Linux gate no longer installs ffmpeg from apt. It downloads BtbN/FFmpeg-Builds'
static `ffmpeg-n9.0.2-17-g2a571b6068-linux64-gpl-9.0.tar.xz` from release
`autobuild-2026-09-30-13-08`, checks it against SHA-256 `68ee6468…d66269` (the value BtbN
publishes in that release's `checksums.sha256`), confirms it lists the `apv` format, and points
`FILMCRAFT_FFMPEG`, `FILMCRAFT_FFPROBE` and `PATH` at it. It also sets
`FILMCRAFT_REQUIRE_ORACLES=1`, so a missing oracle fails the gate instead of skipping.

**Why this source:** BtbN is the most widely used static FFmpeg build on GitHub. It deletes daily
builds after about a month but keeps each month-end build (the oldest kept was November 2024), so
a month-end URL stays valid. 9.0 is the newest stable FFmpeg branch. The checksum means the bytes
cannot change under the pin. Checked before use: the tarball matched the published SHA-256. In WSL
Ubuntu, it reported ffmpeg/ffprobe `n9.0.2-17-g2a571b6068-20260930`, the `apv` demuxer and
decoder, and all 7 pixel formats the APV oracle test uses. The step's script ran cleanly under
`bash -e` against the same file.

**Cost and trust:** CI now runs a third-party binary. The pin and checksum fix *which* binary,
but trusting its contents still means trusting BtbN's build pipeline. The oracle tests only read
ffmpeg's output, and the runner holds no secrets beyond the job token. Windows and macOS runs get
no pinned ffmpeg yet, so their oracle tests still skip when ffmpeg is missing.

**Alternative to revisit:** `ubuntu-latest` moves to Ubuntu 26.04 from 2026-10-19. If 26.04's apt
ffmpeg can read APV (not checked), apt would remove the third-party download.

### B-005 — Upstream's GPU parity tests fail on a GPU-less runner's software OpenGL
**Area:** ci · **Status:** fixed · **Caught by:** running it (mirror run 38005601927)

**Symptom:** `filmcraft-golden`'s 4 GPU-vs-CPU parity tests (`gpu_matches_cpu_on_golden_scenes`,
`gpu_blend_modes_match_cpu_render`, `gpu_transition_of_blended_clips_matches_cpu_render`,
`gpu_effect_chains_match_cpu_render`) panicked in wgpu 30.0.1 with `Validation Error … In
Device::create_render_pipeline, label = 'fx-source' … Format Rgba32Float is not renderable`. Its 29
CPU golden tests passed. As before, cargo stopped at this binary, so the rest of the workspace and
the layer, asset and wasm checks still have not run in CI.

**Root cause, partly inferred:** the runner has no GPU. The tests skip only when wgpu finds *no*
adapter; here it found one that cannot render to `Rgba32Float`. The test asks for
`wgpu::Instance::default()` with default adapter options. In wgpu 30 that does not read
`WGPU_BACKEND` (checked in the wgpu source), so CI cannot force a backend. The only graphics
driver the apt step pulls in is Mesa's EGL/OpenGL (`libegl-mesa0`), so the adapter was very likely
software OpenGL (llvmpipe). The log does not name it. Same shape as B-004: the tests handle a
missing tool, but not a present one that lacks a capability. Upstream develops on Macs with real
GPUs, so it never sees this.

**Fixed** (mirror `1e65df6`, run 38016792934): the apt step installs `libvulkan1 mesa-vulkan-drivers
vulkan-tools`, and a step checks `vulkaninfo --summary` for a CPU-type device. The runner reported
`llvmpipe (LLVM 20.1.2, 256 bits)`, `PHYSICAL_DEVICE_TYPE_CPU`, and all 4 GPU parity tests passed.
Why wgpu picked Vulkan over the OpenGL adapter, from wgpu 30.0.1's source: with default options
(`PowerPreference::None`) it does not sort adapters, and it adds backends in the order Vulkan,
Metal, DX12, GLES.

**Fix options considered:**
- Install Mesa's software Vulkan driver (lavapipe, Ubuntu's `mesa-vulkan-drivers`). Vulkan
  requires `R32G32B32A32_SFLOAT` to be renderable. Unverified: whether wgpu then picks Vulkan over
  the OpenGL adapter, and whether software rendering stays within the parity tolerances.
- Upstream: have `device()` also skip when the adapter cannot render `Rgba32Float`.
- Run the workspace tests with `--no-fail-fast` before the gate, so one run lists every failure
  instead of one per 25-minute run.

**Reported upstream** 2026-10-12: storytold/filmcraft#1250. It suggests `device()` also check
`Rgba32Float` render support.

### D-009 — Mirror CI runs every test binary before the gate
**Area:** ci · **Status:** folded into D-010 (the separate pass is gone; the gate's own `test` phase runs `--no-fail-fast`) · **Doc:** mirror commit `1e65df6` (step "cargo test, every failure")

**Decision:** before `cargo xtask ci`, the workflow runs `cargo test --workspace --release
--no-fail-fast` with `continue-on-error`. The gate after it still decides the result. The job's
limit rose from 180 to 240 minutes (60 for this pass, 165 for the gate).

**Why:** the gate's `cargo test` stops at the first failing test binary, so each ~25-minute run
showed one failure (B-004, then B-005). The first run with this pass listed the whole state at once:
219 binaries, 2,293 passed, 2 failed, 40 ignored, 0 skipped. **Cost:** the gate reuses this pass's
builds (same profile and flags), so it adds only the second test execution, about 4 minutes.

### B-006 — Upstream's Japanese-UI test needs a system CJK font the runner lacks
**Area:** ci · **Status:** fixed upstream (#1259), taken in D-015 · **Caught by:** running it (mirror run 38016792934, the D-009 pass)

**Symptom:** `filmcraft-ui-egui`'s `i18n::tests::language_commands_switch_and_persist_without_a_document`
failed with `no Japanese font is installed on this system (for example Noto Sans CJK JP); the
interface stays in English`.

**Root cause:** FilmCraft bundles no Japanese UI font unless built with `CRAFT_FONTS_DIR`. Otherwise
it scans the system font folders for a face covering its Japanese sample text (`Noto Sans CJK JP`,
`Noto Sans JP`, …). The runner has none, and the test unwraps the result instead of skipping.

**Fix:** install Ubuntu's `fonts-noto-cjk` in the apt step (mirror `e199881`).

**Reported upstream** 2026-10-12: storytold/filmcraft#1251. The test is unchanged on main, and
its neighbour `japanese_needs_an_installed_font` already handles the no-font case.

### B-007 — Upstream's mixer speed test needs 4× realtime; the GitHub runner manages 2.1×
**Area:** ci · **Status:** open · **Caught by:** running it (mirror run 38016792934, both test passes)

**Symptom:** `filmcraft-render`'s `mixer_tests::perf_24_tracks_3_effects_realtime_factor` reported
`10 s of 48 kHz stereo in 4.804 s of CPU on one core = 2.1x realtime` and failed `realtime factor
2.08`. The test requires more than 4.0× in release builds (1.0× in debug). On Unix it measures the
thread's own CPU time, so other work on the runner is not the cause.

**Root cause:** single-core speed. D-007 was ruled out by measurement. On this Windows desktop (16
threads), the test was built from the pinned source both ways and run three times each, alternating:
- upstream's profile (thin LTO, 1 codegen unit): 2.273 / 2.253 / 2.272 s, 4.4×;
- the CI profile (no LTO, 16 codegen units): 2.250 / 2.288 / 2.247 s, 4.4–4.5×.

The runner took 4.804 s for the same work, about 2.1× slower per core. Even this desktop clears
the threshold by only ~10%. Caveat: on Windows the test times wall-clock instead of thread CPU. It
ran single-threaded on an idle machine, and both variants were measured the same way.

**Worked around** by D-010: CI leaves this one test out of the gate and runs it alone,
report-only, so its realtime factor is still logged at every sync. The real fix is upstream: a
threshold that does not assume a fast developer machine.

**Correction (2026-10-10, run 38019701912):** the root cause above is wrong. Run alone on the same
runner type, the test measured **6.6× realtime (1.523 s)**, faster than this desktop. The 2.1×
figures (4.804 s and 4.825 s, both passes of run 38016792934) were measured *inside the full
suite*, beside ~150 other `filmcraft-render` tests running in parallel. So the failure is most
likely contention, even though the test measures its own thread's CPU time: threads sharing a
core still slow each other through hyperthreading and shared caches. Not proven: the logs do not
name the CPU, so the two runs may have had different hardware. One job measuring the test both
in the suite and alone would settle it. The "Fix" line stands, but the upstream issue is a
speed test that is sensitive to parallel load, not one tuned for fast machines. The mirror's
workflow comments were corrected in `023f52f`.

**Fixed upstream after the pin** (checked 2026-10-12): storytold/filmcraft `dcabbda` (2026-10-11,
"isolate throughput gate") moved the test to `crates/render/tests/mixer_perf.rs`, its own
executable, "so parallel correctness tests do not contend for CPU frequency, caches, or memory
bandwidth during its timed workload", with the threshold unchanged. That is the same diagnosis
as the correction above, reached independently. **At the next sync:** the test's path becomes
`perf_24_tracks_3_effects_realtime_factor` in the `mixer_perf` binary, so our `--skip
mixer_tests::…` no longer matches it and the report-only step runs zero tests. Remove both, and
let the gate run it.

**Second correction (2026-10-12, run 38216382326):** run *alone* in the report-only step, the test
measured **2.6× (3.916 s)**, against 6.6× alone in run 38019701912. The evidence so far:
- run 38016792934, pin, inside the suite: 2.1× (twice);
- run 38019701912, pin, alone: 6.6×;
- run 38216382326, v0.6.0, alone: 2.6×.

The swing between runs is larger than the gap between in-suite and alone, so the first
correction's "most likely contention" is not established either. Runner hardware, noisy
neighbours on the host, or a change in v0.6.0's mixer could each explain it. The logs do not
name the CPU, so this cannot be told apart yet. **Revised next-sync plan:** upstream's
isolation alone may not make the test pass on GitHub's runners. So keep it out of the gate,
and point the skip and the report step at its new location (`--test mixer_perf`) instead of
removing them. Logging the runner's CPU model in each run would let the numbers be compared.

**CPU logging added** (mirror `57b937a`, 2026-10-12): a "Runner hardware" step prints lscpu's
model, CPU and core counts, L3 cache and hypervisor. It also writes "Runner CPU: <model>, <n>
CPUs" to the run summary. Tested in WSL. The first CI run with it will be the next sync, or a
manual run.

### D-010 — Mirror CI runs upstream's gate phase by phase, leaving out one speed test
**Area:** ci · **Status:** active · **Doc:** UPSTREAM.md "Mirror branches"; mirror commit `e199881`

**Decision:** CI no longer runs `cargo xtask ci` as one step. It runs that command's phases as
separate steps, with upstream's exact arguments: `fmt --check`; `clippy --workspace --all-targets
--release -- -D warnings`; `test --workspace --release`; `xtask layers`; `xtask assets`;
`xtask wasm`. There are two differences. `test` adds `--no-fail-fast` (D-009) and
`--skip mixer_tests::perf_24_tracks_3_effects_realtime_factor` (B-007). That test then runs alone,
`continue-on-error`, so its number is logged without deciding the result. Every phase runs even if
an earlier one failed, and each has its own time limit inside the job's 240 minutes.

**Guard against drift:** a step before the phases compares `fn ci` in `xtask/src/main.rs` with a copy
kept in the workflow. If upstream changes it, CI fails with a diff, saying the phases must be
updated. Checked: it passes on the pinned source, and flags a simulated edit with a readable diff.

**Why** (chosen over patching the test's threshold, or leaving the gate red): the test's 4× bar
assumes a fast developer machine; the runner is ~2.1× slower per core (B-007). Splitting the gate
keeps FilmCraft's code untouched, so no engine patch has to be carried and rebased at every sync.
It also shows which phase failed. **Cost:** CI no longer literally runs `cargo xtask ci`. The
drift guard makes any divergence loud rather than silent, but the phases must be kept in step by
hand when it fires. Before any switch to upstream's own gate, check that `--skip` is still needed.
The Why above repeats B-007's original cause; see B-007's correction.
Checked 2026-10-12: upstream main's `fn ci` still matches the copy, so the next sync's drift
guard should pass. ~~B-007 is fixed upstream; its skip and report step come out at the next
sync.~~ Superseded by B-007's second correction: at the next sync, move the skip and report
step to the test's new location rather than removing them.

**Result: green.** Run 38019701912 on `resolution` @ `e199881` passed every phase in 23m20s:
fmt 3s; clippy 41s; test 17m35s (219 binaries, 2,294 passed, 0 failed, 40 ignored, 1 left out,
0 skipped); mixer speed alone 6.6×; `layers: ok`; `assets: ok (69 assets attributed)`;
`wasm: ok (42 crates)`. Between the pin `2ba0629` and `e199881` the only changed file is
`.github/workflows/mirror-ci.yml` (GitHub compare), so **this run confirms the pinned engine
code.**

### D-011 — Sync to FilmCraft v0.6.0, a release tag, not main
**Area:** upstream · **Status:** active · **Doc:** UPSTREAM.md "Current pin"

**Decision:** the sync point is `sync/fc-20261012` = `9acf816` = storytold/filmcraft@`2ff3f5e`
(v0.6.0, published 2026-10-11) plus mirror plumbing. That is 537 commits past the previous base
`7b6c134`. `main` was 378 commits further on (`78fe010`).

**Why:** UPSTREAM.md prefers release tags, and FilmCraft tags almost daily (v0.4.0 10-08, v0.5.0
10-10, v0.6.0 10-11), so a tag costs little freshness. **Cost:** v0.6.0 predates `dcabbda`, the
speed-test isolation, so B-007's skip stays for one more sync.

**What the sync found:**
- No command ids were removed. Five were added: `effects.resetParam`, `sequence.removeTransition`,
  `source.close`, `source.closeAll`, `timeline.selectGap`. None fills a `Missing` Resolve edit.
- The adapter needed no change. All 30 Resolution tests pass against it (1m47s build), and fmt
  and clippy are clean.
- One new FilmCraft crate is in our graph: `filmcraft-tts`, for narration. The dev-profile
  overrides match v0.6.0's; upstream's new ones (`dng`, `flac`, `vp8`) arrived after the tag and
  are for crates we do not use.
- The rebase met modify/delete conflicts on 4 upstream workflows we remove. The script kept them
  removed, as designed. No new upstream workflow came in.
- Pushing the sync branch started mirror-ci by itself (run 38216382326, event `push`). That is the
  first proof that `6015b2f`'s push-ordering fix works; earlier runs were all started by hand.

**Mirror CI: green** (run 38216382326 on the sync point itself, 45m48s). 245 test binaries, 2,808
passed, 0 failed, 41 ignored, 0 skipped. The speed test was still left out. `layers` and
`assets` are ok, and `wasm: ok (44 crates)`. Twice the previous run's time: compiling took
23m25s against 11m21s. The dependency cache only partly matched, because `Cargo.lock` changed,
and the workspace grew by 26 test binaries and 514 tests. Expect roughly 45 Actions minutes
for a sync that moves many crates.

### B-008 — upstream-status.sh printed "(none)" after listing command-id changes
**Area:** tooling · **Status:** fixed · **Caught by:** running it (the v0.6.0 sync)

**Symptom:** the command-id section listed 5 added ids, then printed `(none)`.

**Root cause:** under `set -o pipefail`, `diff` exits 1 when there *are* differences, so the
pipeline's `|| echo "(none)"` fired as well. The ROADMAP section had the same shape: piping into
`head -n 60` would end the script with SIGPIPE on a long diff.

**Fix:** capture the output, then print it or `(none)`. The ROADMAP excerpt goes through a
here-string, so no pipe can fail. Tested both ways: against v0.6.0 it lists only the 5 ids, and
against the pinned base it prints `(none)` for both sections. Both runs exit 0.

### B-009 — sync-mirror.sh --apply did nothing after a sync
**Area:** tooling · **Status:** fixed · **Caught by:** review, then confirmed by running it

**Symptom:** UPSTREAM.md's order is: sync, wait for mirror-ci, then run with `--apply`. That last
step printed `mirror is already at v0.6.0 (2ff3f5e2); nothing to sync`, exited 0, and left the
pin unchanged. A documented step failed silently.

**Root cause:** the "nothing to sync" check exits before the apply block, and after a sync the
mirror's `upstream` branch is always already at the ref. A second defect was in the apply block:
its comment edit replaced only the branch name, so `Cargo.toml` would have kept the old
`storytold/filmcraft@…` commit.

**Fix:** the apply logic is one function. When the mirror is already synced, `--apply` pins the
newest `sync/*` branch that points at `resolution`, and errors if there is none. The comment is
rewritten whole. Tested: `sync-mirror.sh v0.6.0 --apply` pinned `sync/fc-20261012` = `9acf816`,
wrote `storytold/filmcraft@2ff3f5e` in the comment, updated `Cargo.lock`, and ran the tests (30
passed).

### B-010 — MP4 with a leading empty edit plays one frame early
**Area:** engine (FilmCraft) · **Status:** open · **Caught by:** running real footage, then confirmed with a synthetic clip · **Doc:** [research/real-footage-2026-10.md](research/real-footage-2026-10.md)

**Symptom:** a real file whose video starts 53 ms into the presentation showed a steady −2.0 / −2.1
frame offset against ffmpeg. A synthetic reproduction confirmed it: H.264 `testsrc` with a
3-decimal timer, 24 fps, `-bf 2`, written with `-output_ts_offset 0.0533`. Its edit list is
`[53 ms empty (media_time -1), 10 000 ms from media_time 1024]` (timescale 12 288, so 2 frames).
At 2.1042 s FilmCraft renders the frame whose timer reads **2.000** (frame 48). The file's
presentation timeline puts frame 49 there (presented from 2.0946 s); counting from the clip's first
frame, it is frame 50. The same clip without the empty edit is exact (timer 2.083, frame 50), as is
a variable-frame-rate clip.

**Root cause:** not investigated. FilmCraft's `crates/codecs/src/mp4.rs` handles empty edits (a
test covers one followed by a media_time 0 edit) and B-frame skip edits. This case combines the two.

**Impact:** 2 of 127 local files start late (53 ms, 67 ms). If audio follows its own timestamps,
picture and sound would be one or two frames apart. Audio was not measured.

**Reported upstream** 2026-10-12: storytold/filmcraft#1276. It includes the one-line ffmpeg
reproduction, and cross-references the closed #714 and #715.

### B-011 — Frames timestamped just after the timeline grid show one frame late
**Area:** engine (FilmCraft) · **Status:** fixed upstream (#1328), taken in D-015 · **Caught by:** running real footage (an Android 4K clip), then confirmed with a synthetic clip · **Doc:** [research/real-footage-2026-10.md](research/real-footage-2026-10.md)

**Symptom:** the clip was Android 15, 3840×2160 H.264 High, 30 fps, with no B-frames, no edit list
and starting at 0. Its first frame lasts 34.14 ms, so later frames sit up to ~0.7 ms after the
1/30 s grid. At 8 of 10 test times FilmCraft rendered the previous source frame, and the margin was
decisive: the previous frame matched at 39.8 dB against 23.6 dB for the frame on screen. The 4K
decode itself is correct: the full-resolution render matches that previous frame at 45.8 dB.

**Root cause (measured, not read from FilmCraft's code):** the shown frame is the last source frame
at or before the *start of the timeline frame* containing T. That rule predicts all 10
measurements; "the frame on screen at T" predicts 2. So a source frame that arrives even a
fraction of a millisecond after the grid line misses its own timeline frame.

**Confirmed in isolation:** a 30 fps `testsrc` clip (3-decimal timer, `-bf 0`, no edit list) with
every frame after the first moved 1 ms late. FilmCraft labels it 30.0 fps. At (k + 0.5)/30 for
k = 1, 75, 150 it shows timers **0.000, 2.466, 4.966**; the same clip on the grid shows 0.033, 2.500,
5.000. So the late clip runs one frame behind for its whole length, and frame 0 shows twice. When a
real clip's drift crosses the grid (−1.6 ms by frame 900 here), a source frame should also be
skipped.

**Impact:** phone footage commonly has timestamp jitter like this; both Android clips tested show
the offset. It means a stutter and a one-frame offset in the timeline, and possibly a ~33 ms
offset between picture and sound (audio not measured). Related upstream: the closed #73, the same
"floors the time against timestamps" class, fixed for WebM. Tested at v0.6.0; upstream `main` not
tested.

**Reported upstream** 2026-10-12: storytold/filmcraft#1282. It includes the one-line ffmpeg
reproduction and the timer table, cross-references #73, and suggests picking the nearest source
frame.

### D-012 — The editor is an egui app, a client of the command API like MCP
**Area:** ui · **Status:** active · **Doc:** ARCHITECTURE "The editor"; ROADMAP M2

**Decision:** `apps/resolution-app` uses egui / eframe 0.36 with the wgpu renderer, and rfd for file
dialogs. These are the same versions FilmCraft's own UI uses. The engine runs on a worker thread;
every key, menu item and click becomes a vocabulary id run through it.

**Why egui** (chosen by Truman, 2026-10-12, over Slint, Iced and a web UI under Tauri): one language;
frames go straight to the GPU; wgpu leads to a GPU viewer and colour page. FilmCraft's 42.8k-line egui
UI is a working reference on the same engine. Its timeline panel is ~3k lines, and its playback core
(frame pool, read-ahead, control: ~1.8k lines) is what M2 ports, which its licence allows (invariant
2 forbids depending on `ui-egui`, not learning from it). **Cost:** egui looks like a tool by default,
so a Resolve-calibre finish is our own styling work; accessibility is weaker than native toolkits.

**What came with it:**
- **Vocabulary** grew by four ids the UI needs, each mapped and covered by the drift test:
  `timeline.select`, `source.open`, `timeline.createFromClips` (Resolve's "Create New Timeline Using
  Selected Clips"), `timeline.open`.
- **One reader of engine JSON** (`model.rs`), since the inspect output is FilmCraft's shape; a typed
  model in `engine-api` is the follow-up.
- **Always dark, with our own palette.**
- **Resolve on this machine** (21.1.1) is a reference for workflow, names and keys only (invariant 7).

**First slice, verified:**
- Window screenshots via `--screenshot` with the demo project and an Android 4K clip.
- 10 app tests, 3 of them UI tests: Ctrl+B splits and Ctrl+Z undoes; Right steps one frame; Shift+2
  and Shift+4 switch pages; clicking a clip selects it in the engine.
- Two defects the screenshots caught were fixed: missing glyphs (shown as boxes), and ruler labels
  stepping in seconds (00:00:01:23 at 23.976).
- Not yet: playback with audio, a source viewer next to the record viewer, trimming by drag.

### D-013 — Playback is ported from FilmCraft into an engine-neutral crate, fed by a `Playback` snapshot
**Area:** playback · **Status:** active · **Doc:** ARCHITECTURE invariant 2 and "The editor"; ROADMAP M2

**Decision:** real-time playback is Resolution's own code in `crates/playback`, ported from
FilmCraft (MIT): the frame server (`ui-egui/src/frames.rs`), sound mixed ahead
(`ui-egui/src/play_ahead.rs`), the clock (`ui-egui/src/lib.rs`) and audio output
(`apps/filmcraft/src/audio.rs`, moved to cpal 0.18). It depends only on `engine-api`. The engine
side is a new trait, `engine_api::Playback`: the open timeline frozen at one revision, which renders
frames and mixes sound on any thread. `Engine::playback()` hands one out after every batch of
commands. The FilmCraft adapter implements it over the session's shared project snapshot, as
FilmCraft's own UI does. NOTICE carries FilmCraft's MIT notice for the ported code.

**Why:** invariant 2 forbids depending on `ui-egui` and `apps/*`, where FilmCraft keeps playback.
The other option, a UI-free `filmcraft-playback` crate upstream, would wait on upstream's
schedule. The engine thread stays single-threaded and owns edits; frames need parallel decoding
ahead of the playhead, so the trait gives out a read-only snapshot instead.

**How it plays:** the audio device is the clock (the picture follows the sound). The clock waits for the
first 6 frames, or 0.5 s. Sound is mixed 0.2 s ahead on its own thread, and edits made while
playing are heard. J and L double the speed up to 8x; sound plays at 0.5x to 4x forward and is
silent otherwise. Marks (I, O) made while playing keep playing; other commands stop playback
first. The viewer shows the exact frame, or holds the newest one it has.

**Where it differs from FilmCraft, each measured** (`crates/playback/examples/play_bench.rs`, this
machine: AMD Ryzen 7 5700X3D, 8 cores / 16 threads; Radeon RX 9070 XT):
- **Render cost counts the whole job.** FilmCraft leaves decoding out. Here that made the demo's
  composited section 4.5 fps against 9.7, because generated media counts as decoding.
- **Queued frames nearer than the plan's lead are dropped** (B-012).
- **2 frame workers, not 2 to 6.** FilmCraft's renderer already uses every core for one frame; 1 to 3
  workers played every clip measured as well as 6.
- **Frames render in their own rayon pool** (B-014).
- **Not ported:** GPU frame plans, render previews, thumbnails, profiling, audio device settings.

**Verified:**
- 69 workspace tests pass, among them FilmCraft's six play-ahead tests (ported), six frame-server
  tests on a fake timeline, the clock's tests, the adapter rendering pixel-identical frames on other
  threads and mixing at 48 and 44.1 kHz, and three UI tests: Space plays and stops where it stopped;
  I marks while playing without stopping; J, K and L.
- `resolution-app --play 6` on this PC: the Android 4K clip played 29.2 fps (timeline 30.01) with
  sound on the default device (an M-Audio M-Track) driving the clock, 0 underruns.
- **Not verified by ear:** no person had listened to the sound when this was written.

| Benchmark (6 to 10 s, simulated 60 Hz display, simulated 48 kHz device) | Frames shown | Sound |
|---|---|---|
| Android 4K30 H.264, hardware decoding, 1920 px | 30.0 of 30 fps | 0 underruns |
| 1080p60 H.264, hardware decoding, 1920 px | 58 fps (display-limited) | 0 underruns |
| FilmCraft demo, single layer, 1440 px | 22 of 24 fps | 0 underruns |
| FilmCraft demo, composited section, 1920 px | 8 to 9.5 fps (CPU-bound: about 100 ms a frame) | 0 underruns |
| Android 4K30, software decoding, 1440 px | 7.6 to 9 fps (in order the decoder does 20) | 0 underruns |
| 4K60 HEVC, software decoding (B-013), 1920 px | 1 fps (in order the decoder does 17) | 0 underruns |

**Known limits:** footage that cannot be decoded in real time plays at a few frames a second; no
scheduling fixes that. It needs hardware decoding (D-014; B-013 blocks some HEVC) or proxies.
Effects-heavy timelines are limited by CPU compositing until frames composite on the GPU, as
FilmCraft's GPU frame plans do.

### D-014 — The editor decodes video with the operating system's hardware decoders
**Area:** playback · **Status:** active · **Doc:** README; HANDOFF §2

**Decision:** `resolution-app` calls `filmcraft_platform::register()` at startup, as FilmCraft's own
app does. That puts Media Foundation (Windows), VideoToolbox (macOS) or VA-API/NVDEC (Linux) in front
of FilmCraft's software decoders. A decoder that declines a stream falls back to software.
`--software-decode` turns it off. The MCP server and the tests keep software decoding, so frames
rendered for agents and goldens do not depend on the GPU.

**Why:** measured on the Android 4K30 clip at a 1440 px viewer: software decoding showed 5.2 fps,
with up to 1.8 s on one frame; Media Foundation showed 30.0 fps, the exact frame on 100% of
refreshes. Software 4K H.264 decodes at about 20 fps in order, under real time even before rendering.

**Cost:** `filmcraft-platform` holds FilmCraft's only `unsafe` code (OS FFI, behind their ADR 0001).
On Windows it also registers an NVENC export encoder and starts a probe for one (absent here: no
NVIDIA driver). Hardware and software decoders may differ in the last bit of a pixel; editing never
depends on that.

### B-012 — Frames slower than real time were never shown: each was cancelled as the next fell due
**Area:** playback · **Status:** fixed · **Caught by:** running it (`resolution-app --demo --play 4`
showed 3.2 fps), then `play_bench` · **Doc:** `crates/playback/src/frames.rs`

**Symptom:** the demo's composited section played at 3.2 fps, holding one frame for up to 5 s, with
58 of 136 frame jobs cancelled. One frame alone takes about 100 ms there, so about 10 fps was
possible. Software-decoded 4K showed 2.6 fps where the decoder manages 20 in order.

**Cause:** the frame server renders the nearest frame first. Frames queued earlier rose in priority as
the playhead approached them, so the workers always started the frame about to fall due. When a
frame takes longer than one frame period, it was late before it finished, and it was cancelled
the moment the playhead passed it. Almost nothing completed. My first port had left out
FilmCraft's `playback_plan`, which starts frames far enough ahead and skips evenly. Porting it
was not enough: frames *already queued* closer than the plan's lead still won.

**Fix:** port `playback_plan`, and drop queued frames nearer than the lead; frames already
rendering finish unless the playhead has passed them. The demo's section went from 3.2 to 9.7 fps
(1 worker), with the longest hold 0.4 s. Software 4K went to 8 to 9 fps.

### B-013 — Hardware HEVC decoding gives up when the in-band PPS has one trailing zero byte fewer
**Area:** engine (FilmCraft) · **Status:** fixed upstream (#1492), taken in D-015 · **Caught by:**
`play_bench` on a local 4K60 HEVC file, with the engine's log printed

**Symptom:** a 3840×2160 60 fps HEVC Main file played at 1 fps although Media Foundation opened on the
Radeon (HEVC Video Extension 2.5.33). The engine's log said: `Media Foundation HEVC failed (decode
error: in-band parameter sets differ from the sample entry); continuing with the software decoder`.

**Cause (read from FilmCraft's code, confirmed on the bytes):** `NalStreamInfo::parameters_changed`
(`codecs/src/hw.rs:176`) compares each in-band VPS, SPS and PPS byte for byte with the `hvcC`
copies. In this file the VPS and SPS match exactly. The PPS in `hvcC` is `4401e0f7c04c9000`; the
one in the first sample is `4401e0f7c04c90`. The only difference is one zero byte after the PPS's
RBSP trailing bits: byte-stream padding the muxer kept inside the stored NAL unit, which carries no
syntax. So the same PPS reads as a change, and
`HybridDecoder` (`platform/src/hybrid.rs:188`) moves the whole stream to software. The file was
muxed by ffmpeg (`Lavf60.16.100`), which many capture tools use.

**Fix (upstream):** compare parameter sets with trailing zero bytes removed (or compare their
RBSPs). Not patched here (rule: no edits to FilmCraft in place).

**Reported upstream** 2026-10-12: storytold/filmcraft#1483, with the bytes above; the file itself
is not shared. The same code is on upstream `main` (71438c6).

### B-014 — Sound dropped out while frames rendered: the mixer queued behind them in rayon's global pool
**Area:** playback · **Status:** fixed; its shared pool caused B-015, reworked there · **Caught by:**
`play_bench`'s underrun counter · **Doc:** `crates/filmcraft-adapter/src/playback.rs` (`frame_pool`)

**Symptom:** on the demo's composited section, with 2 frame workers the simulated device hit 7 to 62
underruns in 6 s, up to 0.57 s of silence; with 3 workers or more it was worse.

**Cause:** FilmCraft's compositor (`render/src/blend.rs`, `effects.rs` and others) and its audio mixer
(`render/src/mixer.rs:874`) both run parallel loops on rayon's global pool. Frame jobs filled the
pool, and the sound's mixing tasks waited in line behind them. Raising the audio thread's OS
priority would not have helped.

**Fix:** the adapter renders playback frames inside a rayon pool of their own (one thread fewer than
the machine has). Same section: 0 underruns, at least 135 ms of sound still buffered, and the
picture unchanged.

### B-015 — Two frame workers deadlocked on sideways phone video: their renders shared one rayon pool
**Area:** playback · **Status:** fixed · **Caught by:** running real footage (a Samsung portrait 4K
clip) through `play_bench`; the process sat at 0 % CPU and never exited · **Doc:**
`crates/filmcraft-adapter/src/playback.rs` (`in_frame_pool`)

**Symptom:** a Samsung 4K clip stored sideways with a −90° rotation tag stopped producing frames
after 35 to 75 of them (3.7 fps over 20 s), then hung at exit, with hardware or software decoding.
One frame worker played it, and so did both workers on the unrotated Android and Sony clips.

**Cause (confirmed from the hung process's thread stacks):** FilmCraft's decoder turns each picture
upright as it stores it (`VideoFrame::rotated`, a rayon `par_chunks_mut`, `frame/src/lib.rs:668`)
while holding that decoder's lock (`codecs/src/gop.rs:561`, `store` → `finish`). B-014's fix ran
both workers' renders as jobs in one shared rayon pool. In the stacks, pool thread `frame-render-1`
held the lock for frame A and waited for the rotation's pieces. `frame-render-14`, waiting inside
one of those pieces, had picked up worker B's whole render (rayon lets a waiting thread run other
jobs), and that render waited for the same lock. Frame A's rotation could not finish until
`frame-render-14` returned. FilmCraft guards against a nested request on the thread that holds the
lock (`DecodingGuard`, `gop.rs:544`), not on another one. FilmCraft's app renders on plain threads,
which never pick up other jobs, so it is not exposed.

The stacks were dumped with a Python script over Windows' `dbghelp.dll` (no debugger was
installed). By mistake, the dumped run used the pre-fix adapter: a source file restored after an
A/B build kept its older timestamp, so cargo did not rebuild it. Its pool threads carry the old
names, so it is the original bug's deadlock that the stacks show.

**Fix:** each rendering thread (each frame worker) gets a rayon pool of its own, each with half the
cores less one, so the two workers together use what the shared pool did. A thread waiting on a
frame's work can then only pick up work of the same frame. Sound still mixes outside them, so
B-014 holds. Test `playback_renders_a_rotated_clip_from_two_threads` uses a 37 KB rotated clip made
with ffmpeg (`crates/filmcraft-adapter/testdata/`). Before the fix it deadlocked on its first run
(60 s timeout); after it, it passed 8 of 8 runs (0.3 s each). The Samsung clip with hardware decoding,
from 4 start points: 24.6 to 26.1 fps, no stall, clean exit.

Same runs before and after, 2 rounds each (`play_bench`, simulated 60 Hz display and 48 kHz device):

| Case | Before | After | Sound |
|---|---|---|---|
| Demo, composited section, 1920 px | 7.8, 8.6 fps | 9.8, 10.0 fps | 0 underruns both |
| Demo, simple section, 1440 px | 21.1, 21.2 fps | 21.8, 22.8 fps | 0 underruns both |
| Android 4K30, software decoding | 7.5, 9.6 fps | 9.1, 12.3 fps | 0 underruns both |
| Android 4K30, hardware decoding | 30.0, 30.0 fps | 30.0, 30.0 fps | 0 underruns both |
| Sony 4K 23.976, software decoding | 23.6, 23.9 fps | 23.2, 23.9 fps | 0 underruns both |

The composited section's longest hold grew, from 0.47 to 0.73 s before to 1.0 to 1.1 s after. Not
investigated.

**Lesson:** a lock held across a rayon call is safe only while no job that takes the same lock can
run in the same pool.

### B-016 — play_bench did not count a frame held until the end of the run
**Area:** tooling · **Status:** fixed · **Caught by:** B-015's numbers not adding up: 75 frames shown
in 20 s, yet a "longest hold" of 68 ms · **Doc:** `crates/playback/examples/play_bench.rs`

**Cause:** the longest hold was measured only when a new frame replaced the one on screen. A picture
frozen until the end of the run never counted, so B-015's deadlock read as smooth playback with
too few frames.

**Fix:** the hold still open at the end counts too. With it, B-017 shows as an 11 s hold.

### B-017 — Footage decoded slower than real time can freeze instead of playing at a few frames a second
**Area:** playback · **Status:** not reproduced since D-015 · **Caught by:** `play_bench` on the Samsung portrait 4K clip
with software decoding

**Symptom:** with 2 workers, 1.6 fps, and one frame held for 11.3 s of a 12 s run: the picture stops
after the preroll. 61 frame jobs finished, 42 of them cancelled, at about 380 ms each. With 1 worker,
the same (1.5 fps, 11.4 s hold). Alone and in order on one thread, the decoder does 64 ms a frame at
the same size (15.6 fps). The sound is unaffected (0 underruns). The Android clip of the same size,
not rotated, plays at 9 to 12 fps in software.

**Cause:** not found yet. Each job costs 3 to 6 times an in-order decode, and most finish after the
playhead has passed their frame and are cancelled. Not yet measured: whether the requests send the
decoder back to a keyframe, and whether the plan's lead is too short.

**Matters for:** footage the GPU decoders decline (B-013's HEVC) or `--software-decode`. With
hardware decoding this clip plays at 24.6 to 26.1 fps. D-013's "Known limits" says such footage plays
at a few frames a second; this clip freezes instead.

### B-018 — upstream-status.sh reported two live command ids as removed
**Area:** tooling · **Status:** fixed · **Caught by:** checking the report's claim in upstream's
`commands.rs` before acting on it · **Doc:** `scripts/upstream-status.sh`

**Symptom:** against v0.7.2 and against `main`, the report listed `markers.markIn` and
`markers.markOut` as removed. Resolution's I and O keys map to them (`map.rs:63-64`).

**Cause:** both still exist. Upstream PR #1043 gave them `frame`, `seconds` and `timecode`
parameters, and rustfmt then put `cmd!(` and the id on separate lines. The script read one line at
a time. It had also missed `effects.moveKeyframes`, added in v0.7.2 in the same layout. The adapter's
drift test (`every_mapped_id_exists_upstream`) asks the engine itself, so it would not have been
fooled; only the report was.

**Fix:** the script joins the file into one line before matching. Against v0.7.2: nothing removed,
`effects.moveKeyframes` added. Against `main` (`cee1feb`): also `edit.rippleDeleteHead`.

### D-015 — Sync to FilmCraft main (`cee1feb`), not the v0.7.2 tag, to take the fixes for our reports
**Area:** upstream · **Status:** active · **Doc:** UPSTREAM.md "Current pin"

**Decision (Truman's, 2026-10-13):** the sync point is `sync/fc-20261013` = `29d01cf` =
storytold/filmcraft@`cee1feb` (main, 2026-10-13 07:00 UTC) plus mirror plumbing. That is 568
commits past v0.6.0 and 186 past the v0.7.2 tag (2026-10-12). This is an exception to D-011's
preference for release tags.

**Why:** upstream fixed four of our six reports on 2026-10-13, all after v0.7.2: B-013 (#1483, by
#1492), B-011 (#1282, by #1328), B-004's APV oracle (#1249, by #1257) and B-006's font (#1251, by
#1259). B-013 decides whether the HEVC footage Truman is getting decodes on the GPU. Mirror CI runs
upstream's own test gate on the sync point either way, which is most of what a tag would add.
**Cost:** the base is not a release. If mirror CI fails, the next tag is the fallback.

**Also in the range:** upstream's fix for the deadlock behind B-015 (#846, in v0.7.2). The decoding
it does under a cache's lock now runs on a rayon pool of its own. Resolution's one pool per frame
worker stays: B-014 needs it.

**What the sync found:**
- No command id we map was removed; the drift test passes. Added upstream: `effects.moveKeyframes`
  and `edit.rippleDeleteHead`. Neither fills a `Missing` Resolve edit (`rippleDeleteHead` overlaps
  `timeline.rippleStartToPlayhead`). `markers.markIn`/`markOut` gained `frame`, `seconds` and
  `timecode` parameters (#1043); our calls still work.
- The adapter needed no change. All 70 Resolution tests pass (3.6 min build); fmt and clippy are
  clean.
- Five FilmCraft crates are new in our graph: `avi`, `dng`, `flac`, `mpa`, `vp8`. Upstream builds
  `dng`, `flac` and `vp8` at `opt-level = 3` in dev; `Cargo.toml` now does too, so the overrides
  match upstream's again.
- The rebase kept upstream's workflows removed (they were reworked upstream, #1051). Only
  `mirror-ci.yml` is in the mirror.

**On Truman's footage, this PC, before → after** (`play_bench`, hardware decoding unless noted):

| Check | Old pin (v0.6.0) | New pin |
|---|---|---|
| 4K60 HEVC muxed by ffmpeg (B-013's file) | 1 fps, software fallback | **49.9 fps**, decoded on the GPU (`HEVCVideoExtension`) |
| Samsung portrait 4K | 24.6 to 26.1 fps | **30.0 fps**, the exact frame on every refresh |
| Samsung portrait 4K, software decoding | froze (B-017) | 10.1 fps, longest hold 0.64 s |
| Demo, composited section | 9.8, 10.0 fps | 13.5 fps |
| Sony α7R II 4K 23.976, Android 4K30 | 24.0 and 30.0 fps, exact | the same |
| Sound against ffmpeg (Sony, Samsung, Android) | sample-exact | sample-exact |
| Android clip, frame shown at 10 times (B-011) | the frame on screen at 2 of 10 | **10 of 10** |
| Samsung clip, the same 10 times | | the frame on screen at each timeline frame's start, 10 of 10 |

The Samsung clip's frames sit a steady 12.4 ms after the 1/30 s grid (its first frame lasts 45.8 ms).
At 4 of the 10 times, a source frame starts inside the timeline frame. The engine shows the
source frame on screen at the timeline frame's start, which is the usual rule, not B-011.

**Mirror CI was red, and the pin was taken anyway (Truman's call, 2026-10-13).** Run 38413841087
on the sync point failed three tests, all in FilmCraft's UI crate and desktop app, which Resolution
never depends on (B-020). Every crate in our graph passed. UPSTREAM.md's procedure moves the pin
only on a green run, so this is a recorded exception, not a change to the procedure. Waiting for
upstream to fix its own UI tests would have held back the HEVC and phone-footage fixes above.

### B-019 — Deleting merged PR #5's branch closed the PR stacked on it (#6) instead of retargeting it
**Area:** tooling · **Status:** worked around · **Caught by:** running it: `gh pr view 6` showed it
closed, with a `base_ref_deleted` event

**Symptom:** `gh pr merge 5 --merge --delete-branch` merged #5 and deleted `ui-walking-skeleton`.
GitHub then closed #6, whose base it was, instead of moving #6 to `main`. #6's description had said
GitHub would retarget it; that was stated without checking.

**Workaround:** pushed the branch back at the same commit, reopened #6, changed its base to `main`,
then deleted the branch again.

**Next time:** retarget a stacked PR to `main` (`gh pr edit N --base main`) before deleting the
branch under it.

### B-020 — Upstream main fails three UI and desktop-app tests at the D-015 sync point
**Area:** ci · **Status:** open, upstream · **Caught by:** mirror CI run 38413841087 on
`sync/fc-20261013` · **Doc:** D-015

**Symptom:** 3,473 tests passed, 3 failed and 44 were ignored, in 304 test binaries. fmt, clippy,
`layers`, `assets` and `wasm` passed. The failures:
- `filmcraft-ui-egui --test graphics_notice_ui`, `the_notice_follows_the_interface_language`: the
  test expects the old Spanish wording ("renderizado por software"). Upstream's Spanish review
  (#1233) changed the catalog to "procesamiento por software". Open upstream PR #1664 fixes the test.
- `filmcraft --bin filmcraft`, `dev_icon::tests::desktop_file_matches_the_packaged_fields`: the
  packaged Linux desktop file gained a `MimeType=` line that the generated one lacks. Upstream PRs
  #1640 and #1671 call it a pre-existing failure on main.
- `filmcraft-ui-egui --test timeline_marquee_ui`, `alt_box_selects_only_video_and_delete_keeps_linked_audio`
  ("release preserves the individual selection"): no upstream mention found; not investigated.

**Impact on Resolution:** none found. `filmcraft-ui-egui` and the `filmcraft` app are not in our
graph (invariant: we never depend on them), and every crate we use passed. These are main-branch
breakages, the risk D-015 accepted by not syncing to a tag.

### D-016 — Take the Edit page to a usable bar, then agents and Color, not every page in turn
**Area:** ui · **Status:** active · **Doc:** ROADMAP "Order of work", M2–M5

**Decision (agreed with Truman, 2026-10-13):** the UI work goes in this order:
1. shared look and layout, starting with mockups of two or three visual directions that Truman
   chooses between;
2. the Edit page up to a bar: Truman cuts a one-minute piece from the Sony and Samsung clips with
   Resolve habits;
3. a minimal Deliver page, so the edit ends in a file;
4. agents editing the open timeline, as drafts the user accepts;
5. a first slice of the Color page.

Cut and Media pages follow. **Pages lay out like Resolve's**: each has a fixed layout whose panels
are shown or hidden from its toolbar. Panels do not dock freely, as they do in Premiere and in
FilmCraft's UI.

**Why:** Resolve's Edit page is too large to finish before anything else, so "finished" is replaced
by a bar that can be tested. The engine already has the edits (ripple, roll, slip, slide) and the
export commands (`file.exportMedia`, EDL, FCPXML, OTIO, AAF), so steps 2 and 3 are mostly UI work.
Agents come next because they are half of the product's identity, and they change how the UI
receives edits it did not make. Color comes before Cut and Media because it is the riskiest work,
where FilmCraft is thinnest (no node graph, CPU colour). Cut and Media mostly re-arrange panels
built in steps 1–3.

**Why fixed layouts:** they match Resolve, and they are simpler to build and test. An agent can also
name what is on screen ("the Edit page with the Inspector open"). **Cost:** someone who wants a
custom arrangement cannot have one. Resolve's own answer is its per-page panel toggles, plus dual
screen.

### D-017 — What Resolve does comes from its Reference Manual, then from the local install
**Area:** ui · **Status:** active · **Doc:** ROADMAP "Order of work"; `scripts/resolve-manual.py`

**Decision (Truman's, 2026-10-13):** the spec for Resolve's behaviour, keys and features is the
DaVinci Resolve Reference Manual for the installed version. Anything the manual does not cover is
found by trying it in the local install. The manual ships with Resolve:
`C:\Program Files\Blackmagic Design\DaVinci Resolve\Documents\DaVinci Resolve.pdf`, the
**21.1 Reference Manual**, 4,351 pages, matching the installed 21.1.1. `scripts/resolve-manual.py`
writes its text and bookmarks to `target/resolve-manual/` for searching.

**What the manual has, and lacks:**
- There is no single shortcut table. Shortcuts appear after menu commands ("Edit > Undo
  (Command-Z)"), in per-chapter "Keyboard Shortcuts in This Chapter" lists (Edit part, pp. 864, 889,
  942, 967), and in trimming (p. 1069) and multicam (p. 1043).
- It writes keys the macOS way: Command is Ctrl and Option is Alt on Windows (p. 68).
- Parts by page: Interface 12, Edit 791, Color 3032, Fairlight 3745, Deliver 4088, and Menu
  Descriptions 4336.

**Not available from the install:** the default keymap is built into the program. The user
preferences hold only an empty preset list (`%APPDATA%\Blackmagic Design\DaVinci Resolve\Preferences\keyboard.preset.xml`).
Keyboard Customization can export a preset, but only from the GUI. The free version also blocks
external scripting, which an earlier attempt on this machine ran into.

**Rules:** the manual is Blackmagic's. Read it, cite page numbers, and never copy its text,
screenshots or illustrations into the repo (invariant 7). The extracted text stays in `target/`,
which git ignores. The extractor uses pypdf (BSD), not PyMuPDF (AGPL).

### D-018 — The editor's look is "Black Box", with a few colour themes now and skins later
**Area:** ui · **Status:** active · **Doc:** ROADMAP "Order of work" step 1

**Decision (Truman's, 2026-10-13):** of the three step-1 directions, Truman chose **Black Box**: a
near-black ground, panels divided by hairlines rather than raised fills, icon-only toolbars, Geist
and Geist Mono, and thin-stroke icons. It ships with **a few colour themes**. **Skins** are pinned
for later. The other two directions (Graphite, Soft Light) are candidate skins.

**Terms, as the code will use them:**
- A **colour theme** changes colours only: accent, ground levels, clip and waveform colours.
- A **skin** changes the chrome: fonts and sizes, corner radius, gaps, icon stroke weight, toolbar
  labels on or off, and panel separation (hairline or raised).

The two are separate layers from the start (a `Skin` and a `Theme` in `theme.rs`). One skin,
Black Box, exists now, so adding skins later means adding data, not refactoring.

**Constraint on every theme:** grounds and panel greys stay colourless (R = G = B). A tinted UI
around the viewer biases colour judgement, which matters most on the Color page. Themes vary the
accent and how deep the ground is, never its hue.

**Mockups:** a private Artifact canvas, "Resolution Edit page directions" (2026-10-13).

**Built (same day):** `theme.rs` (the Black Box skin; themes Midnight, Ember, Mint, Rose and
Charcoal, chosen in Workspace > Theme or with `--theme`, saved between runs through eframe's
persistence), `icons.rs` (33 icons), `chrome.rs` (shared widgets), Geist and Geist Mono bundled
under the OFL. Tests check that every theme's greys are colourless and its text meets 4.5:1
contrast, and that every icon parses and stays in its box. **What came with it:** eight
vocabulary ids, `workspace.*` (Resolve's Workspace > Show Panel in Workspace, and Reset UI
Layout), so the panel buttons are commands like everything else (ARCHITECTURE invariant 3).
They are front-end ids (`Target::Ui`) and do not stop playback. The edit-mode buttons set the
mode; trimming and blade clicks are step 2. Checked by eye in screenshots of the Sony and
Samsung clips in Midnight, Ember and Charcoal.

### B-021 — HANDOFF counted 71 vocabulary commands and 46 headless; there were 75 and 50
**Area:** docs · **Status:** fixed · **Caught by:** reading: counting the vocabulary while adding
D-018's `workspace.*` ids

**Symptom:** HANDOFF's "Engine boundary" row said 71 commands, 46 of them headless. Before D-018,
`COMMANDS` held 75 and `map.rs` sent 50 of them to the engine or the adapter: ids added since
had not been counted. (This entry's first draft repeated the 46 without checking it.)

**Fix:** counted from the source (ids in `COMMANDS`, targets in `map.rs`). After D-018 there are
83 commands: 50 run headless (47 engine, 1 approximate, 2 in the adapter), 27 are front-end and
6 are missing. HANDOFF says so.

### D-019 — The adapter answers `timeline.inspect`, adding the editor state FilmCraft keeps outside the sequence
**Area:** engine · **Status:** active · **Doc:** ARCHITECTURE "The editor"; `crates/filmcraft-adapter/src/lib.rs`

**Decision:** `timeline.inspect` maps to the adapter (`Target::Adapter`) rather than straight to
FilmCraft's `sequence.inspect`. The adapter runs `sequence.inspect` and adds three things the
track headers and the timeline toolbar need. FilmCraft keeps them in its session state, not in
the sequence:
- `targeting`: the targeted tracks (Resolve's auto track selectors), and the video and audio
  destinations for source edits (destination controls);
- `snapping`;
- `linkedSelection`.

Two vocabulary ids came with it, both passed straight to FilmCraft: `track.set` (lock, enable,
mute, solo; `timeline.setTrack`) and `track.target` (auto track selector and destination;
`timeline.setTargeting`).

**Why:** the UI must show these switches' state. The alternatives were worse. The UI could
remember what it last sent, but that is wrong after undo, project open, or an agent's edit. Each
switch could get a query of its own, but that means more round trips per snapshot. MCP clients
read the same JSON, so agents see the same state.

**Caught by / checked:** an adapter test round-trips lock, mute, targeting and destination
through `timeline.inspect`; UI tests click "Lock V1", "Auto Select V1", "Show V1" and the
Snapping button.

### D-020 — Mouse editing follows Resolve's tools; each drag is one command and one undo step
**Area:** ui · **Status:** active · **Doc:** ARCHITECTURE "The editor"; `apps/resolution-app/src/timeline_ui.rs`

**Decision:** the timeline tool and where a drag starts choose the edit, as the Reference Manual
describes (pp. 1061-1066):
- **Selection:** drag a clip's body to move it, up or down onto another track of its kind. Drag
  an edge with nothing against it to resize it; drag the cut between two clips to roll it.
- **Trim:** drag an edge to ripple it, and within 3 px of a cut to roll it. Drag a clip's upper
  half to slip it and its lower half to slide it.
- **Blade:** click a clip to cut that clip, and that clip only.

While dragging, the timeline draws where the clip or cut will land and the change in timecode.
On release, one vocabulary command goes to the engine. The six new ids pass straight to
FilmCraft: `timeline.moveClips`, `trimEdge`, `rollEdit`, `slipClip`, `slideClip` and `bladeClip`
(FilmCraft's `timeline.move`, `trim`, `roll`, `slip`, `slide`, `razor`). With snapping on, moving
edges snap within 8 px to the playhead, marks, markers and other clips' edges.

**Why one command per drag:** an edit sent on every mouse move would put dozens of steps in the
undo history, and would re-render the viewer and remix the sound on every pixel. The engine
clamps each edit (media limits, neighbouring clips, linked partners), so the preview can be
optimistic. **Cost:** the preview does not show the engine's clamping. A drag past the media's
end lands where the engine stops it, not where the outline was.

**Checked:** an adapter test runs each command on the demo project and checks the result.
UI tests drag with a simulated mouse: resize and move in Selection, slip and ripple in Trim, and
a Blade click.

### B-022 — Slipping a clip by dragging moved its media the wrong way
**Area:** ui · **Status:** fixed · **Caught by:** a UI test (the slip drag changed nothing at the
start of the clip's media)

**Symptom:** a slip dragged to the left asked the engine for a negative delta. That moves the
clip's media earlier, but the media should follow the pointer. The manual's overlay "moves left
and right as you drag" (p. 1063), and FilmCraft's own UI negates the drag the same way
(`ui-egui/src/panels/timeline.rs`).

**Fix:** a slip sends minus the drag's frames: dragging right shows earlier frames.

### B-023 — FilmCraft refuses a ripple that Resolve makes by cutting sync-locked clips across the edit
**Area:** engine · **Status:** open; worked around with sync lock switches · **Caught by:** the
adapter test, rippling in the demo project

**Symptom:** rippling the demo's first cut fails with "this edit would break sync: A2 is
sync-locked and has a clip in the way". The music on A2 spans every cut. Resolve keeps sync
instead: "clips on other tracks may be cut to allow you to perform insert trims" (pp.
1074-1076), so the music is cut at the edit and trimmed.

**Workaround:** each track header now has Resolve's Sync Lock switch (`track.set` `syncLock`).
Turning it off on the music track lets the ripple through without moving the music. A UI test
checks exactly that.

**Not done:** Resolve's own behaviour. One way, not tried: before rippling, the adapter cuts
clips on other sync-locked tracks at the edit (`timeline.razor`). Whether FilmCraft's ripple then
trims them the way Resolve does is unverified. It may need an upstream change.

### D-021 — A source viewer beside the timeline viewer: a second `Playback`, and keys act on the active viewer
**Area:** ui · **Status:** active · **Doc:** ARCHITECTURE "The editor"; `monitor.rs`; ROADMAP step 2

**Decision:** the Edit page shows two viewers side by side, as Resolve does: source on the left,
timeline on the right.
- **Engine:** `Engine` gains `source_playback()`, a second `Playback` for the clip open in the
  source viewer.
  - On FilmCraft, `SourceSnapshot` renders frames with `render_item`, as FilmCraft's Source
    monitor does: media as SDR Rec. 709. It reads sound straight from the media at any sample
    rate.
  - Frame 0 is the first frame of the clip's range.
- **App:** each viewer is a `Monitor` (`monitor.rs`), holding what the app used to hold for its
  one viewer: frame server, frame on screen, transport, pending seeks and meter. The two share
  the audio device (`Sound`), and starting one stops the other.
- **Active viewer:** playback keys (Space, J/K/L), frame steps, Home/End, I/O and markers act on
  the active viewer. Clicking a viewer makes it active, and so does Q (`view.toggleViewer`,
  manual p. 795). Opening a clip from the Media Pool makes the source viewer active.
- **Marks:** source marks are the vocabulary's own mark commands with `"target": "source"`, which
  FilmCraft routes to the clip.
- **New vocabulary:**
  - `source.inspect`: the adapter adds the clip's name and exact frame rate, since FilmCraft
    gives a float.
  - `source.seek`: FilmCraft's `source.setPlayhead`.
  - `view.toggleViewer`.

F9 and F10 already used FilmCraft's three-point source edits (`source.insert`/`overwrite`).
With a source viewer they now take the clip's In and Out from it.

**Why a second `Playback` rather than a source mode of one:** the two viewers hold different
frames at once, and Resolve plays either. One snapshot per viewer keeps the frame cache, the
transport and the playhead of each separate. Mixing them would make "which frame is this?"
depend on a mode. **Cost:** four frame workers instead of two, with only one viewer's playing
at a time. The other's cache stays warm.

**Checked:**
- An adapter test opens a demo clip, renders and mixes it, and moves its playhead and In.
- UI tests step the source with the arrow keys and mark In and Out, then insert with F9 and
  switch viewers with Q.
- Another UI test plays the source while the timeline stays still.
- A screenshot with `--source`, a new start-up option that opens a clip by name.

**Not yet:**
- The source viewer counts timecode from 0, not from the clip's own start timecode
  (02:01:12:00 for the Sony clip).
- The other F-key edits (Replace, Place on Top, Ripple Overwrite, Fit to Fill, Append) are
  still `Missing`.

### D-022 — Waveforms come from `Playback::peaks`, read on a background thread
**Area:** ui · **Status:** active · **Doc:** `crates/engine-api/src/playback.rs`; `apps/resolution-app/src/waveforms.rs`

**Decision:** `Playback` gains `peaks(item)`. It returns the lowest and highest sample of every
256 samples at 48 kHz of a media item's first audio stream. On FilmCraft it is a port of its
UI's `source_peaks`: the whole stream is decoded four seconds at a time.

The editor asks for the peaks of every audio clip's media after each snapshot. One background
thread reads them and keeps them in a cache. The timeline draws whatever has arrived: one column
per pixel, mirrored under the clip's name. The scale is decibels from -48 dB, and quiet
recordings are raised by up to 12 dB, as FilmCraft's timeline does. Audio tracks grew from 40 to
48 px to give the waveform room.

**Why on `Playback` and off the UI thread:** decoding a clip's sound takes a while for long
clips, too long for the engine thread, which would hold up edits. The snapshot already holds the
project and the media pool, and it is thread-safe, so any thread can read peaks from it.

**Checked:** an adapter test reads the demo music's peaks (about 60 s, loudest within 0..1). A
UI test waits for every demo audio clip's waveform. A unit test checks the dB scale. A screenshot
of the Samsung clip was checked by eye: quiet start, louder later.

**Not yet:** waveforms in the source viewer; per-channel display; the clip's gain and volume
keyframes do not scale the drawing (FilmCraft's does).

### D-023 — Timeline navigation follows Resolve's wheel and scroll-bar controls; the view pages to the playhead
**Area:** ui · **Status:** active · **Doc:** `apps/resolution-app/src/timeline_ui.rs` (module comment)

**Decision:** the timeline keeps a `Nav` (zoom, first second in view, vertical scroll), held by
the app between frames. Moving around follows the manual (p. 853):

| Input | What it does |
|---|---|
| Wheel | Scrolls the tracks up and down |
| Alt + wheel | Zooms about the pointer |
| Ctrl + wheel | Moves through time |
| Shift + wheel, sideways trackpad swipe | Scrolls sideways |
| Middle-drag | Pans |
| Scroll bars (bottom, right) | Drag or click to move; the bottom one marks the playhead |

The zoom commands zoom about the playhead, and Zoom to Fit shows the whole timeline. Zooming out
past the whole timeline snaps back to fit. When the playhead moves out of view (playing,
stepping, a seek from a viewer), the view pages so it sits near the left. A view the user
scrolled away from stays put until the playhead next moves.

**Why:** before this, a zoomed-in timeline always centred on the playhead, so there was no way to
look at another part without moving the playhead. Tracks past the panel's height could not be
reached at all.

**Detail worth knowing:** egui reads Ctrl with the wheel as a zoom gesture (`zoom_delta`) and
gives no scroll delta. The timeline turns that back into a distance through time. Edits and
scrubbing take only the primary button, so a middle-drag pans and does not seek.

**Checked:** UI tests roll the wheel (the demo's six tracks scroll; Alt zooms) and zoom in, then
press End: the view follows the playhead, and Zoom to Fit restores the whole timeline. A
screenshot shows the scroll bars and A3 reachable by scrolling.

### D-024 — New Timeline (Ctrl+N) starts empty in the open timeline's format; the source viewer shows the media's own timecode
**Area:** ui · **Status:** active · **Doc:** `apps/resolution-app/src/app.rs` (`timeline.new`)

**Decision:**
- **New Timeline.** File > New Timeline, Ctrl+N (manual p. 839), adds `timeline.new`, which is
  FilmCraft's `file.newSequence`.
  - The new timeline is named "Timeline n" and starts empty.
  - It copies the open timeline's size and frame rate. With none open, it takes FilmCraft's
    default, 1920 × 1080 at 23.976.
  - It does not use FilmCraft's `fromItem`, which would also place the clip: that is the Media
    Pool's "New timeline from clip".
- **Source timecode.** The source viewer counts in the media's own timecode. The adapter adds
  `startTimecodeFrames` to `source.inspect`, from FilmCraft's `MediaInfo.start_timecode`.

**Why:** the one-minute-edit bar (D-016) needs a way to start an empty timeline and fill it from
the source viewer, which is Resolve's usual source-to-timeline workflow. Resolve's own dialog asks
for a name and format. Until a dialog exists, the open timeline is the format most likely wanted.
Camera timecode (the Sony clip's 02:01:12:00) is what an editor logs and reads, so the source
viewer shows it.

**Cost:** no format choice yet; Resolve's New Timeline dialog is not built.

**Checked:** a UI test presses Ctrl+N: one more timeline, open, empty, the same format. A model
test reads the start timecode. A screenshot of the Sony clip in the source viewer reads
02:01:12:00.

### D-025 — A minimal Deliver page: presets, a render queue the app keeps, renders as engine export jobs
**Area:** ui · **Status:** active · **Doc:** ROADMAP step 3; `apps/resolution-app/src/deliver.rs`

**Decision:** the Deliver page follows Resolve's layout.
- **Left:** Render Settings.
  - Six presets, each mapped to one of FilmCraft's: Match Source (H.264), H.264 1080p,
    H.264 2160p 4K, YouTube 1080p, Vertical 1080 × 1920, ProRes 422 HQ.
  - A file name, defaulting to the timeline's name, and a location, defaulting to the user's
    Videos folder.
  - Entire Timeline or In/Out Range, then Add to Render Queue.
- **Middle:** the timeline viewer.
- **Right:** the Render Queue, with a progress bar and time left for each job, and Render All.
- **Below:** a shorter timeline, for In and Out.

Three vocabulary ids pass straight to FilmCraft:
- `deliver.render` is `file.exportMedia`. It is destructive, because it writes a file, so agents
  must acknowledge it.
- `deliver.jobs` is `jobs.list`, a read.
- `deliver.cancel` is `jobs.cancel`.

The queue lives in the app. FilmCraft has a queue type but no commands for it. Render All starts
one job at a time and polls `deliver.jobs` every 0.4 s while one runs. A name already on disk or
already queued gets a number, so nothing is overwritten.

**Why:** an edit must end in a file before the bar test means anything. FilmCraft already
renders well (H.264, ProRes, DNxHR and more, with background jobs and progress), so this page is
UI over it.

**Checked:**
- A UI test marks two seconds of the demo, switches to Deliver with Shift+8, chooses In/Out,
  adds, renders and finds the file.
- The same two seconds rendered over MCP (`deliver.render` with `wait`) and checked with
  ffprobe: H.264 1920×1080 at 23.976, exactly 48 frames, AAC 48 kHz stereo, 2.005 s.
- A screenshot of the page.

**Not yet:** custom settings (codec, size, bitrate), audio-only renders, and keeping the queue
in the project. Resolve also has per-job thumbnails and a render-time readout.

### D-026 — Agents reach the open editor through a local, token-checked link; `resolution mcp --editor` serves MCP over it
**Area:** mcp · **Status:** active · **Doc:** ARCHITECTURE "The editor"; HANDOFF §3; `crates/agent-link`

**Decision:** the first half of step 4 (D-016). An agent edits the timeline that is open on
screen, through the editor's own engine:

- **Editor side.** While it runs, the editor listens on 127.0.0.1 at a port the system picks. It
  writes `{port, token, pid}` to `agent-link.json` in the user's application-data folder
  (`%LOCALAPPDATA%\Resolution` on Windows).
  - The token is 128 bits from the system's random hash keys. A call without it is refused.
  - Closing the editor removes the file. A force-quit leaves it behind, and connecting then fails
    with "not answering"; the next editor overwrites it.
- **Agent side.** `resolution mcp --editor` reads the file, connects, and serves the same MCP
  tools as before over a `RemoteEngine`. That type implements `Engine` by forwarding each call,
  one JSON line each way. The MCP crate is unchanged.
- **Engine thread.** The editor runs agents' calls on its engine thread, between the user's own
  commands.
  - Edits land in one history and one set of snapshots, so they appear on screen as the agent
    makes them.
  - The status bar shows "Agent: <undo label>". The user's Ctrl+Z undoes an agent's edit like
    their own.
  - Engine notices from agents' calls go both to the agent and to the status bar.

**Why a bridge, not an MCP server inside the app:** MCP clients start stdio servers themselves
and cannot attach to a running GUI. An HTTP MCP endpoint in the app would need a fixed port in
`.mcp.json`. With the bridge, `.mcp.json` stays a command (`resolution-editor` is registered next
to the demo server), and the engine stays behind the same trait.

**Security, flagged:** an open port that runs editing commands. It is bound to 127.0.0.1 only and
needs a token that only the user's own account can read from disk; a web page cannot read it.
Destructive commands still need the agent's `acknowledge`, as before (D-004). Other local
processes running as the same user can read the file, as they can read the user's projects.

**Not yet (step 4's second half):** draft timelines. Today an agent edits the live timeline
directly. The safety net is undo, and checkpoints over MCP.

**Checked:**
- Link tests: an agent edits the demo through the link (split, undo, render, errors keep their
  kind); a wrong token is refused; with no editor running, connecting says so.
- A UI test: an agent's split shows in the open editor and its status bar, and Ctrl+Z undoes it.
  Closing the editor removes the link file.
- By hand: the editor opened with the Sony clip, and `resolution mcp --editor` driven by an MCP
  client split it at 10 s (two cuts, video and audio) and rendered a frame. A capture of the
  window showed the cut.

### B-024 — HANDOFF counted 70 tests in the workspace and 42 in the editor; there were 107 and 43
**Area:** docs · **Status:** fixed · **Caught by:** reading: counting the tests while updating
HANDOFF for D-027

**Symptom:** HANDOFF's MCP row said "70 tests across the workspace" and its UI row "42 tests". On
`main` after D-026 there were 107 (`#[test]` attributes; `cargo test` agrees) and 43 in
`apps/resolution-app`. The workspace total was set at the D-015 sync and never raised since. D-026
added an editor test without raising the UI row. This is B-021's mistake again, for test counts.

**Fix:** counted from the source and from `cargo test`'s per-crate results. After D-027 there are
113 tests, 47 of them in the editor. HANDOFF says so.

### B-025 — The editor sent an agent's draft before the timeline it described
**Area:** ui · **Status:** fixed · **Caught by:** test (`rejecting_a_draft_leaves_the_original_as_it_was`)

**Symptom:** the test waited until the draft listed two agent splits, then counted V1's clips. It
found one cut, not two. The engine had made both: the agent's own `timeline.inspect` of the draft
showed both cuts.

**Wrong turn:** I first suspected the engine and replayed the same steps over MCP (copy, open,
split at 2 s and 4 s). They cut correctly, and the original stayed untouched.

**Cause:** after each batch the engine thread sent `Reply::Draft` and then the snapshot. The UI
could read the draft's new edit count a frame before the timeline that held them.

**Fix:** the snapshot goes first, so whoever sees the edits listed sees them on screen too. The
test now also waits for the clip count it asserts.

### B-026 — The draft's list of agent edits kept edits that had been undone
**Area:** ui · **Status:** fixed · **Caught by:** reading the MCP session's dry run while writing D-027

**Symptom (in D-027's first version, never merged):** the draft only appended. An undo of an
agent's edit left it listed. The undo could be the user's Ctrl+Z, the agent's `undo` tool, or an
MCP dry run or rolled-back batch, which are a run followed by an undo. A dry run as the agent's
first edit left a draft "with 1 edit" that held none.

**Fix:** the draft follows the undo history, as the MCP session does for its own steps. The worker
records every step made, undone or redone, by the user or an agent; they all pass through the
engine thread. The banner lists the agent's steps still in the draft. A UI test covers the
agent's undo, the user's Ctrl+Z and redo.

**Wrong turn, same review:** I also expected MCP's report of what an agent's first draft edit
changed to list every clip. The report compares the original (before) with the copy (after), and
matches tracks and clips by id. I wrote a fix that reported "switched timeline" instead. A test
showed that the copy keeps its tracks' and clips' ids: the plain report was right, and listed the
edit. The fix was dropped. A test (`a_copy_of_the_timeline_keeps_its_ids`) now guards the property
drafts rely on.

### B-027 — `timeline.duplicate` put the copy at the top of the project; Resolve puts it in the original's bin
**Area:** engine · **Status:** fixed · **Caught by:** running it: the live draft check's project tree

**Symptom:** the agent's draft did not appear next to the original under Sequences. FilmCraft's
`edit.duplicate` adds the copy at the top of the project. The Resolve manual ("Duplicate
Timeline", Creating and Working with Timelines) says Resolve creates it "in the same bin".

**Fix:** the adapter moves the copy into the original's bin (`project.moveToBin`). It folds the move
into the duplicate's undo step the way FilmCraft folds its own (`clip_ops`): the first step's
"before" project stands for both. One undo removes the copy. The adapter test checks the bin,
undo and redo.

**Still differs:** FilmCraft names the copy "<name> Copy" where Resolve appends "copy", and Resolve
also opens the copy in a new timeline tab.

### B-028 — An agent's edit was lost, though reported made, when its draft failed after the copy
**Area:** ui · **Status:** fixed · **Caught by:** reading the diff before committing D-027; a test
confirmed it

**Symptom (in D-027's first version, never merged):** to make a draft, the worker undoes the
agent's first edit, copies the timeline, and makes the edit again on the copy. If a step after the
copy failed (renaming or opening it), the worker fell back to `redo` to put the edit back on the
original. But the copy was itself an undo step, which empties the redo history. So the redo did
nothing: the edit was gone while the agent was told it had been made. With the old code, a test
engine that cannot rename ends with 6 clips on V1 instead of 7.

**Fix:** when the redo brings nothing back, the edit runs again on the open timeline (still the
original: every failing step comes before the copy is opened). Test:
`an_edit_whose_draft_cannot_be_made_stays_on_the_timeline`. In the adapter, a failed move of a
copy into its bin (B-027) now keeps the copy instead of failing the duplicate.

### D-027 — An agent's edits go to a draft of the timeline, which only the user accepts or rejects
**Area:** ui · **Status:** active · **Doc:** ARCHITECTURE "The editor"; HANDOFF §3; `apps/resolution-app/src/worker.rs`

**Decision:** the second half of step 4 (D-016, after D-026). In the open editor, an agent never
edits the user's timeline itself:

- **The draft.** An agent's first command that makes an undo step:
  - runs on the open timeline, and is undone there;
  - is made again on a copy, "<name> (agent draft)". The copy goes in the original's bin and opens
    at the same playhead.

  The agent gets a notice saying so. Its later edits go to the same draft. If the user has opened
  another timeline in the meantime, the draft is opened again first. Commands that change nothing
  (looking, moving the playhead) make no draft.
- **The banner** above the timeline reads "Agent draft of “<name>”: N edits, latest: <label>".
  Hovering lists the edits (a first receipt). Its buttons are **Accept Draft** and **Reject Draft**.
  - **Accept:** the draft takes the original's place. The original is deleted and the draft is
    renamed to its name.
  - **Reject:** the original opens again and the draft is deleted.
  - Both are ordinary undoable edits.
- **Only the user decides.** Accept and Reject are not vocabulary commands, so no agent can accept
  its own work.
- **Still one history.** The list of edits follows the undo history: an undone edit drops off and a
  redo brings it back (B-026).
- **New vocabulary**, 101 commands now:
  - `timeline.duplicate`: FilmCraft's `edit.duplicate` on the project selection, with the copy in
    the original's bin (B-027);
  - `media.rename`: `clip.rename`;
  - `media.delete`: `project.delete` on the selection.

**Why a copy, not edits in place with a backup:** the agent's work stays apart from the user's.
The user can go on working on the original while the agent works, and nothing that plays the
original sees the agent's edits until they are accepted. The copy keeps its tracks' and clips' ids,
so MCP's reports of what changed stay right across the switch (tested; B-026).

**Why run, undo, copy, run again:** only running a command shows whether it edits. `playhead.set`
is an "edit" by risk tier but makes no undo step. The cost is one extra run per draft.

**Known limits:**
- A dry run or a rolled-back batch as the agent's first edit leaves an empty draft open. The banner
  says "no edits", and Reject removes it.
- One draft at a time, held in the editor's memory. After a restart the draft is an ordinary
  timeline named "(agent draft)", with no banner.
- Accepting deletes the original. **Unverified:** what FilmCraft does when another timeline nests
  the deleted one.
- An undo past the draft's start works like any undo: first the copy loses its name, then it is
  gone, and the banner with it.
- Edits the user makes in the draft are accepted with it, but are not listed as the agent's.
- Receipts are undo labels ("Add Edit"). Fuller receipts (what, where, why) are still to build.

**Checked:**
- UI tests (`apps/resolution-app`):
  - an agent's split goes to a draft, and after Accept one timeline of the original's name holds
    the split;
  - rejecting leaves the original as it was and deletes the draft;
  - the draft lists only the agent's edits still in it (the agent's undo, Ctrl+Z, redo).
- Adapter test: duplicate (in the original's bin, one undo step), rename, delete. MCP test: a copy
  of the timeline keeps its ids. Worker test: an edit whose draft cannot be made stays on the open
  timeline (B-028).
- By hand, 2026-10-13: the editor on the demo, driven by `resolution mcp --editor`.
  - A dry-run split was reverted and left an empty draft open (the banner said "no edits").
  - Two splits went to that draft: V1 went from 6 to 8 clips, and the banner said "2 edits".
  - The `undo` tool took one back (7 clips).
  - The draft sat in the Sequences bin beside the original.
  - A capture of the window showed the banner and the cuts. Accept and Reject were not clicked by
    hand; the UI tests click them.

### D-028 — Grades are Resolution's own node model, compiled to FilmCraft Lumetri effects on each clip
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "Colour grades"; `crates/grade`; `crates/filmcraft-adapter/src/color.rs`

**Decision:** the first half of step 5 (D-016), the engine side of the Color page. Truman chose
this route on 2026-10-14 from three, after the measurements below.

- **Our own model.** `crates/grade` (no FilmCraft types) holds a clip's grade: serial corrector
  nodes. Each node has the Primaries palette's controls (Lift, Gamma, Gain and Offset wheels as
  YRGB numbers, Contrast, Pivot, Saturation, Hue, Color Boost, Temp, Tint) and the Custom Curves
  (Y, R, G, B). Numbers are in Resolve's units where the manual gives them (Gain 1, Offset 25,
  Pivot 0.435, Saturation 50 neutral).
- **Our own math.** Resolve does not publish its formulas. Each control does what the manual says
  it does, by formulas written down in `crates/grade`; they are ours, not Resolve's. Every control
  except Saturation, Hue and Color Boost acts on each channel alone, so a node is a tone function
  per channel followed by a saturation step.
- **Kept in the project.** The grade is JSON in the timeline's metadata map, under
  `resolution.grade.<clip id>`: invariant 5's "extension map", no FilmCraft schema change.
- **Compiled to Lumetri.** Each node becomes at most two Lumetri Color effects at the front of the
  clip's effect stack:
  - its tone functions as Lumetri's R, G and B curves, every other section off. The adapter
    samples each tone function adaptively, adding points until the joined curve is within a
    quarter of an 8-bit code value, at most 129 points.
  - its saturation step as Lumetri's Saturation and Vibrance, both of which keep luma, with Hue as
    a constant Hue vs Hue curve.

  A neutral or disabled node compiles to nothing. Playback, frame renders, Deliver and agents see
  the grade with no other change.
- **One undo step per edit.** The adapter writes the metadata and the effects in one
  `Session::edit`; a drag passes a `merge` key and uses `Session::edit_merged`, so a whole drag
  is one step.
- **Copies keep their grade.** FilmCraft copies effects with a clip (a split, a paste) but not our
  metadata. A clip with no stored grade whose Lumetri effects are exactly what another of the
  timeline's grades compiles to takes that grade. Lumetri effects that match no grade are reported
  as `foreign`, and the next grade edit replaces them.
- **Vocabulary:** `color.inspect`, `color.adjust` (any controls of one node, partial wheels
  allowed, unknown fields refused), `color.addSerialNode`, `color.addNodeBefore`,
  `color.appendNode`, `color.deleteNode`, `color.toggleNodes`, `color.toggleAllNodes`,
  `color.resetNode`, `color.resetGradesKeepNodes`, `color.resetAll`. `color.previousNode` and
  `color.nextNode` are front end. Nodes are numbered from 1 as in the Node Editor; `clip`
  defaults to the clip under the playhead on the highest video track. 114 commands now.

**Why, with numbers.** Nothing in our stack grades on the GPU today. FilmCraft's Lumetri runs its
curves, wheels, HSL keys and LUTs on the CPU, and our playback renders on the CPU. A FilmCraft render
hook or GPU effect would be an upstream feature; the mirror's patch stack may hold only fixes on
their way upstream (UPSTREAM.md). Measured on the Sony 4K clip, per frame on top of the ungraded
render (render_frame over MCP, 2026-10-14):

| Grade | 960 px | 1920 px | 3840 px |
|---|---|---|---|
| one node, curves | +8 ms | +58 ms | +236 ms |
| one node, curves and HSL key | +21 ms | +105 ms | +432 ms |
| three nodes | +41 ms | +199 ms | +759 ms |

The choices put to Truman:
- **Compile to Lumetri** (chosen). Fast to build and right everywhere at once. Grading on a still
  and scrubbing are fine; real-time playback of a graded 4K timeline drops frames past one or two
  nodes until a GPU step.
- **GPU first.** Weeks of work before anything visible, depending on upstream accepting it.
- **Bake grades to 3D LUTs in the project.** Exact for any colour math, including parallel and
  layer nodes, but about 1 MB of project file per graded node at 33³, with LUT interpolation error
  and harder HDR.

**Limits:**
- Serial nodes only. Parallel and layer mixers need a 3D LUT or several Lumetri passes per branch;
  qualifiers can use Lumetri's HSL Secondary (whose correction is temperature, tint, saturation and
  hue only); windows can use Lumetri's effect masks. None of these is built yet.
- The master ring moves Y only. In Resolve it moves Y, R, G and B together; here R, G and B show
  the balance alone.
- Hue turns HSL hue, as Lumetri does, not Resolve's rotation, which keeps luma.
- SDR only. Lumetri holds the signal to 0…1 after each effect, so super-whites do not pass between
  nodes as they do in Resolve's 32-bit float pipeline. HDR timelines are untested.
- Every undo step holds a copy of the project, so each graded clip's JSON and Lumetri effects
  are copied with every step. **Not measured** on a large project.
- Speed as above: GPU grading is the next step for playback (ROADMAP M4).

**Checked:**
- `crates/grade` tests (17): each control against the manual's description (Lift moves black and
  keeps white; Gain scales from black; Gamma keeps black and white; Contrast keeps the pivot and
  cannot clip; Saturation keeps luma, 0 is grey; Color Boost favours dull colours; Hue 100 turns red
  to cyan), curves, patching, JSON round trips, node add, delete and reset.
- Adapter tests (8):
  - FilmCraft's own curve code (`curve_lut`) reproduces our tone functions within half a code value;
  - a full-size render of the demo frame matches our math within one code value for a grade of
    Lift, Gamma, Gain, Contrast and Saturation together;
  - gain 0 renders black and saturation 0 renders grey;
  - an edit is one undo step, and a merged drag is one step;
  - nodes are added, toggled and reset by number;
  - both halves of a split clip keep the grade;
  - mistakes are refused.
- Wrong turn: the render test first failed by 10 code values at 96 px. A full-size render matched
  every control within 0.8 code values. Small renders grade at an in-between size and then scale
  down, and averaging after a curve is not the curve of the average. The test renders at full size.
- Not yet checked by hand: there is no Color page to look at. It is the next PR.

### B-029 — `color.inspect` showed 0.30000001192092896 for a control set to 0.3
**Area:** color · **Status:** fixed · **Caught by:** test (`a_split_copy_keeps_the_grade`)

**Symptom (in D-028 before merging):** controls are `f32`. `serde_json::to_value` widens an `f32`
to `f64` digit for digit, so `color.inspect` reported Gamma 0.3 as 0.30000001192092896. Agents
would have read, and repeated, numbers nobody typed.

**Fix:** `color.inspect` turns each node into JSON through text (`to_string`, which writes an
`f32` at its own shortest precision, then `from_str`), so 0.3 reads 0.3. Stored grades were
already written as text.

### D-029 — The Color page: viewer, Node Editor, clip strip, Primaries wheels and Custom Curves in Resolve's places
**Area:** ui · **Status:** active · **Doc:** ARCHITECTURE "The editor"; `apps/resolution-app/src/color_page.rs`

**Decision:** the second half of step 5's first slice (D-016), over D-028's grades. The page
grades the clip under the playhead (the highest video track that shows one). Its layout follows
Resolve's default Color page:

- **Viewer** in the middle: the timeline viewer, with its scrub bar and transport.
- **Node Editor** on the right: the clip's serial nodes from input to output. Each node is
  numbered as in Resolve, shows its label (or "Corrector", or "Off" when disabled) and a dot when
  it changes the picture. Click a node to make it current. Right-click it for Add Serial Node, Add
  Before Current, Enable/Disable, Reset Node Grade and Delete Node; the first, second and last
  are also buttons.
- **Clip strip** under them: every video clip in timeline order, with its number, track, a
  picture from its middle and its name. The current clip is outlined; graded clips have an accent
  number. Click a clip to move the playhead to it. Pictures are rendered small on a background
  thread (`Playback::render`), and the current clip's again when its grade changes.
- **Primaries palette** (Wheels mode), bottom left:
  - Temp, Tint, Contrast and Pivot above; Color Boost, Saturation and Hue below. Drag sideways
    to change one; double-click to reset it.
  - The Lift, Gamma, Gain and Offset wheels. Drag a wheel to move the balance (relative, at half
    speed, like a trackball); Shift jumps to the pointer; double-click resets. A balance move keeps
    luma. Under each wheel is its master ring (drag sideways, double-click resets) and its Y, R, G
    and B numbers.
  - A reset button for the palette's controls. Changed values show in the accent colour.
- **Curves palette** (Custom), bottom centre: Y, R, G and B buttons choose the curve. Click to add
  a point, drag a point to move it, drag elsewhere to make a point there and move it, right-click a
  point to remove it. The other curves show faintly.
- **Scopes** keeps its place at the bottom right but is not built yet.
- **Keys** (Reference Manual, "Node Editing Basics"; Option is Alt): Alt+S Add Serial Node,
  Shift+S Add Before Current, Alt+K Append Node, Ctrl+D Enable/Disable Selected Nodes, Alt+D
  Enable/Disable All Nodes, Alt+Shift+; and Alt+Shift+' Previous and Next Node (looping round).
  They act on the Color page only.
- **An agent's draft** shows its banner across the top of the Color page too (B-031).

**How a drag works:** each frame of a drag sends a `color.adjust` under one merge key, so the
viewer shows every step as the engine renders it and the whole drag is one undo step (D-028).
While a drag lasts, the controls show the values being sent, not the engine's answers, which
trail them; when it ends the page asks for the grade again.

**Not built:** the Gallery, LUT browser, Mini-Timeline and Lightbox; Bars and Log modes of the
Primaries; Shadows, Highlights, Midtone Detail and Lum Mix; the other palettes (HDR, RGB Mixer,
Qualifier, Window, Tracker and the rest); scopes; parallel and layer nodes; dragging nodes.
Thumbnails of clips other than the current one are not redrawn when an agent or an undo changes
their grade. Grading plays back on the CPU (D-028's numbers).

**Checked:**
- UI tests (7), driving the page with the pointer and keys:
  - a drag on the Gain master grades the clip, and Ctrl+Z takes the whole drag back;
  - a drag on the Lift wheel moves the balance, keeps luma and leaves the master;
  - Alt+S adds a node and selects it; Ctrl+D turns it off; Previous Node loops round; clicking a
    node selects it;
  - the node keys do nothing on the Edit page;
  - a click on the curve adds a point, dragging it bends the curve, and a drag elsewhere makes a
    new point;
  - clicking a clip in the strip grades that clip, and its tile has room for its picture;
  - an agent's grade arrives as a draft on the Color page, and Reject Draft there restores the
    original.
- Wrong turn: the curve-click test first failed. The harness moves its clock a quarter second a
  step, and the test helper held the button for eight steps; egui takes a press longer than
  0.8 s for the start of a drag. The helper now presses and releases in one step. A real
  improvement came out of it: a drag that starts away from every point now makes a point there.
- By hand, 2026-10-14: the editor on the demo, graded by an agent over `mcp --editor` (two nodes;
  the wheels, Contrast, Pivot and Saturation on the first, a Y curve and Temp on the second).
  Window captures showed the draft banner, the two nodes, the controls with their changed values,
  and the clip strip with pictures. They also showed B-030 and B-031, both fixed and captured
  again. The mouse interactions were not tried by hand: they would have moved the pointer on
  Truman's desktop. The UI tests drive them.
- Not tried on the Sony and Samsung clips: their unpacked folder was gone from Downloads on
  2026-10-14 (the zip is still there).

### B-030 — The Color page's clip strip drew its tiles one text line tall: no pictures, no names
**Area:** ui · **Status:** fixed · **Caught by:** running it (a window capture of the page)

**Symptom:** each tile showed its number and track and nothing under them. The UI test clicked a
tile through its accessibility node, which has no height to check, so it passed.

**Cause:** the tiles took `ui.available_height()` inside a horizontal scroll area, whose row is
one line of text tall.

**Fix:** the strip measures its height before the scroll area. The test now checks that a tile is
over 100 points tall.

### B-031 — An agent's draft had no Accept or Reject on the Color page
**Area:** ui · **Status:** fixed · **Caught by:** running it (a window capture of the page)

**Symptom:** an agent's grade went to a draft (D-027), but the draft's banner lived in the Edit
page's timeline panel, which the Color page does not show. The user would have had to leave the
page to accept or reject the grade.

**Fix:** the banner is its own function, drawn above the Edit page's timeline and across the top
of the Color page. A UI test grades as an agent on the Color page and rejects the draft there.

### B-032 — `target/` grew to 28 GB and filled the disk: Cargo keeps every engine pin's build
**Area:** build · **Status:** fixed · **Caught by:** the linker failing (LNK1180, "insufficient disk
space"; LNK1318 on PDB writes) while building D-028

**Symptom:** C: had 0 bytes free and `target/` held 28 GB. HANDOFF still gave the 3.0 GB measured
on 2026-10-09.

**Wrong turn:** I first deleted `target/debug/incremental` (6.5 GB) and built with
`CARGO_INCREMENTAL=0`. That got the build going but treated the smaller part. After Truman freed
space, a `cargo clean` and a build of everything (tests, clippy, both binaries) measured the real
size: 8.3 GB, of which incremental is 1.0 GB and debug symbols (PDBs) 2.4 GB.

**Cause:** about 20 GB was leftovers. Cargo keeps every artifact it has built and never removes the
ones a newer build replaces. Most likely the bulk is old engine pins: each sync builds all ~45
FilmCraft crates again under new hashes, and this machine has built three (its git checkouts are
9acf816, 2ba0629 and 29d01cf). The 20 GB was not broken down before the clean, so that split is
not measured.

**Fix:** `cargo clean` after each engine sync, now step 3 of UPSTREAM.md's sync procedure;
HANDOFF gives the measured size and time. Incremental builds stay on. Debug symbols stay full:
the stack dumps of B-015 needed them, and 2.4 GB is affordable.

### D-030 — Video scopes: drawn by the engine from the frame the viewer shows; read as numbers by agents
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "The editor"; `crates/engine-api/src/scopes.rs`; `crates/filmcraft-adapter/src/scopes.rs`

**Decision:** the Color page's Scopes panel and an agent's view of the same thing.

- **A `Scopes` service on the engine boundary**, like `Playback`: `Engine::scopes()` hands out a
  thread-safe drawer, and `draw(frame, kind)` returns the scope as a premultiplied RGBA picture.
  The adapter implements it with FilmCraft's scope maths (`filmcraft-scopes`, now a direct
  dependency at the same pin). That crate samples the frame (at most 480 × 270, nearest sample),
  counts it into grids, and paints them with a phosphor-like intensity curve. The histogram is
  painted in the adapter, levels across as Resolve shows it: FilmCraft's own UI draws it sideways.
- **What it draws:** Waveform (R, G and B overlaid and tinted: white where they agree), Parade (R,
  G and B side by side), Vectorscope (Rec. 709 Cb across and Cr up, coloured by hue), Histogram
  (R, G and B levels, black on the left), as in the manual's "Explanation of Each Video Scope".
- **The panel** (bottom right of the Color page) shows one scope at a time, chosen in its title
  strip. The graticule is the UI's:
  - levels in 10-bit code values, 0 to 1023, as Resolve labels them;
  - the vectorscope's crosshair, the 75 % colour-bar targets (R, Mg, B, Cy, G, Yl) and the
    skin-tone line at 123°.
- **From the viewer's own frame.** The timeline viewer keeps the picture it shows. A background
  thread draws the chosen scope of it whenever it changes: while scrubbing, while playing, and at
  every step of a grade drag. Only the newest picture is drawn; older requests are dropped. So the
  scope is the scope of what is on screen, at the viewer's resolution.
- **For agents: `scopes.read`** (vocabulary, Color page, a read): FilmCraft's command of the same
  name. It returns the frame at the playhead as numbers: R, G, B and Y minimum, maximum and mean,
  levels by column, histogram bins, the vectorscope's mean and densest spots, hue and saturation.
  115 commands now.

**Why the engine draws and the UI only lays out:** the maths is FilmCraft's and tested there, and
an engine swap brings its own. The UI stays free of image analysis.

**Not built:** CIE chromaticity; the YRGB and Y'CbCr parade and waveform modes; colorize and
extents options; 2, 4 or 9 scopes at once; zooming a scope; HDR (nits) scales.

**Checked:**
- Adapter tests (6): a grey frame draws one white waveform line at its level, 256 rows; the parade
  puts each channel's pane at its level; grey sits at the vectorscope's centre and red up to the
  left; the histogram puts each channel in its level's column; an empty frame draws nothing;
  `scopes.read` on the demo returns numbers and changes nothing.
- A UI test: the Color page draws the waveform of the viewer's picture, switches to Vectorscope,
  Histogram and Parade by name, and draws again after a grade drag changes the picture.
- By hand, 2026-10-14, on the Sony clip: the waveform matched the picture (the blue light tube
  near 1023 on the left, red highlights on the right, the rest low). After an agent set Gain to
  0.6, the waveform topped out near 614, and `scopes.read` gave every channel's maximum as 60 %.
  The capture also showed the panel's title running into its buttons in a narrow panel; the title
  now gives way.

### B-033 — The Color page's palettes shrank a few points every frame, down to their minimum
**Area:** ui · **Status:** fixed · **Caught by:** test
(`a_custom_curve_takes_a_point_where_clicked_and_bends_where_dragged`, after D-030)

**Symptom:** the curve test began failing every time. A drag landed somewhere other than where
the test aimed. Printing the plot's rectangle frame by frame showed it 4 points smaller each frame.

**Cause:** the palettes sit in a resizable bottom panel. egui sizes such a panel to what its
contents allocate, and the palettes allocated a few points less than the panel's height. So the
panel shrank every frame until it reached its minimum size. This was so since D-029. Before D-030
the panel had always reached its minimum by the time the test measured it; the scopes changed the
timing.

**Fix:** the panel's contents allocate its whole rectangle. The curve test now checks that the
plot keeps its place for ten frames. The Edit and Deliver pages' panels were checked the same way
and hold still.

### D-031 — Windows: Linear and Circle windows on a node, compiled to FilmCraft effect masks
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "Colour grades"; `crates/grade` (`Window`); `crates/filmcraft-adapter/src/color.rs`; `apps/resolution-app/src/color_page.rs`

**Decision:** a node's correction can be limited to windows, as in Resolve's Window palette.

- **The model** (`crates/grade`): a node holds a list of windows. Each has a shape (Linear: a
  rectangle; Circle: an ellipse), on/off, invert, centre, size, rotation (degrees clockwise),
  softness and opacity.
  - Positions are fractions of the clip's picture.
  - Sizes are fractions of the picture's width on both axes, so equal numbers draw a circle or a
    square on any picture.
  - Enabled windows add up. With none enabled, the node applies everywhere.
  - Grades stored before windows existed load with none (the field defaults, and a node without
    windows is stored without it). Resetting a node clears its windows.
- **The engine side:** each enabled window becomes a FilmCraft effect mask on all of its node's
  Lumetri effects, in the clip's pixels (`Project::source_size`; the timeline's size when the
  media has none).
  - A Circle window is an ellipse path, a Linear one a rectangle, turned about the centre.
  - Softness is the mask's feather; opacity and invert carry over; masks combine by adding.
  - `color.inspect` now also gives the clip picture's size.
- **The page:** the centre palettes are chosen in their title strip (Curves, Window), as Resolve's
  palette bar chooses them. The Window palette:
  - Add Linear and Add Circle;
  - a row per window: choose it, turn it on or off, invert it, delete it;
  - the chosen window's Pan, Tilt, Size, Aspect, Rotate, Softness and Opacity. Drag a number
    sideways to change it; double-click it to reset it.
- **On-screen control:** while the Window palette is open, the viewer shows the node's window
  outlines, the chosen one in the accent colour. Drag inside one to move it (and choose it).
  Each drag is one undo step, as elsewhere on the page.

**Limits:**
- No Polygon, Curve or Gradient windows; no handles to resize or turn a window on screen; no
  tracking (FilmCraft's masks have a tracker, not used yet).
- The outline assumes the clip fills the frame as placed by default (fitted and centred). A clip
  moved or scaled in the Inspector draws its outline in the wrong place; the mask itself is right.
- A node of two Lumetri effects (tone, then saturation) masks each. In the soft edge the result is
  close to, not exactly, the node's result blended once.
- Pan, Tilt, Size and Aspect are percentages of the picture, not Resolve's own scales.

**Checked:**
- Grade tests: grades stored without windows load; windows are set whole and held to their
  ranges; an unknown shape is refused; reset clears them.
- Adapter tests:
  - A full-size render of the demo with Gain 0 inside a centred circle: black inside, untouched
    outside, and round, not squashed to the frame's height. Inverted, the reverse.
  - A Linear window turned a quarter clockwise puts its corners where the arithmetic says.
- A UI test: Window palette, Add Circle, drag the on-screen control (the circle moves), drag
  Softness, Invert, Delete, each through the engine.
- By hand, 2026-10-14, on the Sony clip: an agent set Gamma 0.45 and Gain 1.3 inside a soft
  circle. The capture showed a soft bright disc where the circle was, and the rest unchanged.
- Wrong turn: my expected corner for the turned rectangle was wrong (348 instead of 156). The
  code was right; the test's arithmetic was fixed.

### D-032 — Qualifiers: baked into a 3D LUT for now; real key masks in our own GPU renderer are the goal
**Area:** color · **Status:** active (to build) · **Doc:** ROADMAP M4

**Decision (Truman, 2026-10-14):** in Resolve a qualifier (an HSL key) limits a node's whole
correction: Lift, Gamma, Gain and curves apply inside the key. FilmCraft cannot do that directly:

- its masks are shapes only;
- Lumetri's own key (HSL Secondary) allows only Temperature, Tint, Saturation and Hue inside it.

The options put to Truman:

1. **Bake a qualified node into a 3D LUT stored in the project.** A key depends only on a pixel's
   colour, so the node's correction blended by its key is a colour function, and a LUT holds it.
   FilmCraft applies LUTs with tetrahedral interpolation. Every control works inside the key.
   The costs: about 1 MB of project file per qualified node at 33 points, and no key clean-up
   (denoise, blur), since a LUT sees one pixel at a time.
2. **Lumetri's HSL Secondary:** small files and key clean-up, but no Lift, Gamma, Gain or curves
   inside the key.
3. **Real key masks in our own GPU renderer:** wait for the GPU grading step, where the key can
   mask the whole node as in Resolve.

Truman: **3 is the goal; build 1 now.** The grade stays Resolution's own model (D-028), and the
LUT is only how today's compile carries a qualified node. When the GPU renderer arrives,
qualifiers move to real key masks and the LUTs go away, with no grade lost.

### D-033 — The qualifier as built: Resolve's HSL ranges, a qualified node baked into a 33-point project LUT
**Area:** color · **Status:** active · **Doc:** `crates/grade` (`Qualifier`); `crates/filmcraft-adapter/src/color.rs`; `apps/resolution-app/src/color_page.rs`

**Decision:** D-032 built.

- **The model** (`crates/grade`): a node may hold a qualifier. Its parts:
  - On, Invert, and three ranges, each with its own switch, numbered 0 to 100 as in Resolve:
    - Hue: Center, Width, Soft. Hue goes once round the wheel from red, so red is both ends.
    - Saturation: Low, High, Low Soft, High Soft (HSL saturation).
    - Luminance: Low, High, Low Soft, High Soft (Rec. 709 luma).
  - Each range is full inside and falls off smoothly over its soft width outside.
  - The key is taken on the node's input. The node's correction is blended by it, so every
    control (wheels, curves, saturation) applies inside the key.
  - `Qualifier::picked` centres the ranges on one colour, as Resolve's picker does.
  - Grades stored before qualifiers load with none.
- **The engine side:** a qualified node with a correction compiles to one Lumetri effect, Basic
  Correction on and nothing else. Its input LUT is the node itself, key and all, baked into a
  33-point `.cube` (`Node::table`). The LUTs:
  - go in the project's LUT library, named `resolution-<hash>`;
  - are hashed on what the table depends on, so moving a window or renaming the node keeps the
    same table;
  - are dropped once no clip of any timeline uses them, at the next grade edit.

  Windows still mask the effect. An undo puts the qualifier and its LUT back together.
- **The page:** a Qualifier tab beside Curves and Window.
  - With no qualifier: Add Qualifier. With one: On, Invert, Remove Qualifier, then the three
    ranges as columns. Each column has its switch; drag a number sideways, double-click to reset.
  - While the palette is open, a click on the viewer picks the colour under the pointer as the
    key.

**Measured** (full-size demo frame, Gamma and Saturation keyed on its most orange colour, against
the node's own math): median error 0, 99 % of pixels within 2.3 code values, 99.9 % within 5.1,
worst 6.5. The error sits in the key's soft edge, between the LUT's points. A test guards the
figures. On the Sony clip an agent's whole edit, bake included, took 36 ms.

**Limits:**
- No matte finesse (denoise, blur, clean black and white): a LUT sees one pixel at a time
  (D-032).
- No Highlight view of the key.
- The picker reads the picture as the viewer shows it, after the grade, not the node's input.
- No RGB or Luma-only qualifier modes.
- About 1 MB of project per qualified node (D-032).

**Checked:**
- Grade tests: the key's ranges, its soft hue edge and red at both ends of the wheel; a qualified
  node changes only what its key takes; the table matches the node at its points; old grades load.
- Adapter tests:
  - A full-size demo render with Gain 0 keyed on the frame's most orange colour: that pixel goes
    black and the most blue pixel is unchanged. One LUT is in the project; turning the qualifier
    off drops it and grades everything; undo brings both back.
  - A window move keeps the LUT.
  - The accuracy figures above.
- A UI test: Qualifier palette, pick from the viewer, widen the hue by dragging, Invert, Remove,
  each through the engine.
- By hand, 2026-10-14, on the Sony clip: an agent keyed the magenta light (hue 92) with
  Saturation 0. The pink on the arm and hair went grey; the blue light and background stayed blue.

### D-034 — An action journal: every command, who sent it, how long it took and how it ended, for finding failures and freezes
**Area:** tools · **Status:** planned · **Doc:** ROADMAP "Tools"

**Idea (Truman, 2026-10-14):** a debugger for the program that shows a log of the actions taken
and how each resolved, to find out why something fails or freezes.

**Why it fits:** every action is already one vocabulary command on one engine thread (invariant
3): the user's keys, buttons and menus, and agents over the link. One place can record them all:

- the command, its parameters, and who sent it (the user or an agent);
- when it started and how long it ran;
- its result or error, and its undo step.

So the journal would show directly:

- a freeze, as a command that started and never ended (and what else was waiting);
- a slow action, by its time;
- a silent failure, with its error;
- what an agent did, step by step.

**Build it** when testing and debugging features begins in earnest. Not built yet; to be designed
then. Likely shape: the worker records each request and reply into a bounded in-memory journal; a
panel shows it live; it can be saved to a file for a bug report; agents can read it over MCP.

### D-035 — Our own GPU renderer: the viewer's frames, with Resolution's grade math run exactly on the GPU
**Area:** color · **Status:** active (to build) · **Doc:** ROADMAP M4

**Decision (Truman, 2026-10-14):** grades will render in Resolution's own GPU renderer, not through
FilmCraft's effects. The choices put to him:

1. **Our own GPU renderer** (chosen). Port FilmCraft's GPU compositor into Resolution (MIT or
   Apache-2.0, as with playback in D-013) and add our own grade stage. Our node math then runs
   exactly on the GPU:
   - real key masks for qualifiers (D-032's goal);
   - windows, and later parallel and layer nodes;
   - float precision between nodes.
2. **FilmCraft's GPU compositor, plus an upstream contribution** making Lumetri's curves, LUTs and
   masks run on the GPU. Less code for us, but it depends on upstream, and qualifiers would stay
   baked LUTs for good.
3. **Groundwork only** (our playback on a GPU compositor), deciding the grade stage later.

**Why:** Resolve's colour engine is Resolve's identity, and FilmCraft's Premiere-style effect stack
cannot hold it. Effects run in series, each clamped to 0…1, and masks are shapes only. D-028's
Lumetri compile is the bridge until this lands. Grades stay Resolution's own model, so nothing
stored changes.

**Facts found:**
- FilmCraft already plans frames as plain data (`plan_frame`: layers of decoded frames, placement,
  opacity, blend, GPU-able effects) and runs them on a wgpu compositor (`filmcraft-gpu`). It has
  GPU 3D-LUT and mask code, and a GPU export renderer.
- Its wgpu is version 30, the same as our egui's, so frames can stay on the GPU to the screen.
- Lumetri on the GPU covers Basic Correction only. Curves, LUTs and masks send a layer back to the
  CPU, and those are what our grades compile to.

**The program:**
- **G1:** a Resolution GPU renderer for the viewers' frames, with the grade stage as WGSL:
  - the tone functions as 1D tables;
  - Saturation, Color Boost and Hue;
  - the qualifier's key and the windows' coverage, computed exactly per pixel.

  Frames it does not cover yet (transitions, other blend modes, adjustment layers, HDR) fall
  back to FilmCraft's CPU renderer. Tested against `crates/grade`'s reference math.
- **G2:** measure graded playback on the Sony clip; widen the renderer's coverage (blend modes,
  standard effects); show frames on screen without the trip back to memory.
- **G3:** Deliver through the same renderer, feeding FilmCraft's encoders, so viewer and file are
  identical.
- **Later:** parallel and layer nodes, matte finesse, no clamping between nodes.

Until G3, Deliver keeps D-028's compile, which follows the same math within a code value (6.5 at
a qualifier's soft edge, D-033).

### B-034 — The GPU renderer blended 8-bit sRGB pixels before decoding them: enlarged pictures went dark between pixels
**Area:** color · **Status:** fixed · **Caught by:** test (`playback_renders_the_same_frames_on_other_threads`,
while building D-036)

**Symptom:** with playback on the GPU renderer, the threaded playback test's frame at one second
differed from the engine's CPU frame: 99 % of values within 7 code values, the worst 18. Its other
frames matched exactly.

**Cause:** at the test's 96-pixel size the decoder hands over a 90 × 51 picture, which is enlarged
a little to fill the frame. To keep a grade's input codes exact, decoding of 8-bit sRGB had moved
from the texture unit into the shader (D-036, "Facts found"). The shader then blended the four
neighbouring pixels' sRGB codes and decoded the blend. The texture unit and FilmCraft's CPU decode
each pixel first and blend in linear light. Blending codes darkens the pixels between contrasting
ones. Blending straight colour, not premultiplied, also lets a clear pixel's unseen colour bleed in.

**Fix:** the shader decodes and premultiplies each of the four pixels, then blends them. The frame
now matches within 1 code value for 99 % of values (the worst 4). A renderer test
(`an_enlarged_picture_blends_its_pixels_in_linear_light`) draws black beside white, and opaque red
beside a clear green pixel, twice as wide. It fails with the old shader.

### B-035 — The GPU renderer drew 8-bit camera video about one code value too red and too blue
**Area:** color · **Status:** fixed · **Caught by:** running it (D-036's hand check: a Sony clip
frame in the editor, drawn by each renderer and compared pixel by pixel)

**Symptom:** on the Sony clip the GPU's frame was redder and bluer than the CPU's by about one code
value on average (R +1.1, G −0.2, B +1.3), at every size, graded or not. The tests had not seen
it. The demo's pictures are RGB, and the renderer's own Y'CbCr test checked only white, black and
pure red, which clip and hide a small offset.

**Cause:** the video-range constants (Y' from 16 in 219 steps, chroma about 128 in 224 steps) are
scaled to the picture's bit depth by 2^bits / 256. FilmCraft's compositor keeps two numbers: the
scale the texture holds codes at (255 for 8-bit, since the texture returns code / 255) and the
number of code levels (256). The port used the first for both. So 8-bit video had its chroma centre
at 127.5, not 128: a half-code push towards red and blue. 10-bit video was right.

**Fix:** the levels come from the picture's bit depth.
- A renderer test (`ycbcr_mid_tones_decode_as_the_formula_says`) sweeps mid-tone Y'CbCr at 8 and 10
  bits, video and full range, against the conversion's formula. Now: worst 0.6 code values, mean
  lean under 0.03. With the old code: worst 2.5, mean lean 1.05.
- An adapter test draws the decoded test clip (`testdata/rotated.mp4`) both ways. Its flat areas
  agree within 2 code values and no channel leans. It fails with the old code.
- On the Sony clip the lean is now under 0.1 code values.

### D-036 — G1 built: the viewers' frames on Resolution's GPU renderer, grades by their own math
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "The GPU renderer"; `crates/renderer`;
`crates/filmcraft-adapter/src/gpu.rs`

**Decision:** D-035's G1, built.

- **`crates/renderer`** (new; wgpu 30, the version egui uses) draws a frame from layers.
  - A layer is one of:
    - 8-bit RGBA (sRGB, straight alpha);
    - float RGBA (linear, premultiplied);
    - planar Y'CbCr at 8 to 16 bits, any chroma subsampling, with an optional alpha plane.

    With it go its colour coding (matrix; video or full range; sRGB/Rec. 709 or linear transfer),
    its placement (a 2D affine), its opacity and its grade.
  - Shrinking averages up to 4 × 4 samples over each output pixel's footprint, as FilmCraft's GPU
    compositor does. Its shader code was ported (MIT or Apache-2.0, as in D-013).
  - Layers add up over black in linear light, premultiplied, at half-float precision. The result
    is encoded to 8-bit sRGB and read back.
  - The grade stage (WGSL) runs `crates/grade`'s math on each pixel, node by node:
    - the tone functions, as 4096-point tables;
    - then Saturation, Color Boost and Hue;
    - then the qualifier's key and the windows' coverage (up to 8 windows a node), which blend the
      node's result into its input.

    As in `crates/grade`, a grade works on sRGB codes: the shader unpremultiplies and encodes,
    grades, then decodes and premultiplies.
  - There is one renderer for the program, made when a frame first wants it. Without a GPU there is
    none, and every frame is drawn on the CPU.
- **In the adapter** (`gpu.rs`): each playback snapshot keeps a copy of its timeline without the
  grades' Lumetri effects, and each graded clip's grade. FilmCraft plans a frame from that copy
  (`plan_frame`, which also decodes its layers). If the plan is a plain stack of layers, the GPU
  renderer draws it, with each graded clip's grade. Otherwise FilmCraft's CPU renderer draws it from
  the real project, compiled grades and all. These frames go to the CPU:
  - a transition at that time, an adjustment layer, captions, an HDR timeline;
  - a nested timeline, or any clip that is not plain media;
  - a layer with effects FilmCraft runs itself, a blend mode, or a transfer other than sRGB, Rec. 709
    or linear;
  - a graded clip FilmCraft had to pre-render (other effects on it);
  - a clip with a Lumetri of its own, not a Resolution grade: its Lumetri runs on the CPU.

  FilmCraft's plans do not say which clip a layer is. Its planner adds one layer per enabled media
  clip on an enabled track, bottom first. With the cases above excluded, the layers are the visible
  clips in that order. Each pairing is checked against the clip's picture shape, and any doubt falls
  back to the CPU.
- **Viewers only.** Frame renders for agents (`render_frame`), the scopes and Deliver still use the
  CPU and the Lumetri compile (until G3).
- **Switches and counts:**
  - `resolution-app --cpu-render` draws every frame on the CPU, to compare the two or to get round a
    GPU that draws wrong.
  - The adapter counts the frames drawn each way (`frame_counts()`), and `--play` prints them.
  - `play_bench` can play a saved project and takes `--cpu-render`.

**Measured** on this PC (AMD Radeon RX 9070 XT through Vulkan; hardware decoding):
- Against `crates/grade`'s math: the grade within 1.0 code value; the qualifier's key within 0.6.
- Against FilmCraft's CPU frames, on the demo at a 960-pixel viewer:
  - ungraded: identical;
  - graded: mean 0.16, 99 % within 1 code value;
  - qualified: mean 0.51, 99 % within 8. Most of that is the CPU's baked LUT, 6.5 code values off at
    the key's soft edge (D-033). Also, at a viewer's size the CPU grades the picture before
    shrinking it and the GPU after, which differs on a steep key edge.
- The Sony clip (4K, 8-bit 4:2:0) in the editor, GPU against CPU: mean 0.49 ungraded and 0.40
  graded, 99 % within 1 code value, and no channel leaning (after B-035). What differs is on hard
  edges. The CPU takes each pixel's nearest chroma sample and the GPU blends them, and the two shrink
  differently at uneven scales.
- Speed: the graded Sony clip at a 1440-pixel viewer, measured with `play_bench`.

  | | GPU | CPU |
  |---|---|---|
  | One frame after another on one thread | 8.4 ms a frame | 124 ms a frame |
  | Played in real time (several frame workers) | 24.0 fps; exact frame on 100 % of refreshes; 7.5 ms a frame, 3.9 ms of it getting the decoded picture | 14.0 fps; 148 ms a frame |
  | Played at double speed | 47.9 fps; exact on 99.8 % | 11.0 fps |
- In the editor (`--play 8`, with the default window's viewer about 744 pixels wide), both ways
  play at 23.9 fps, since the CPU keeps up at that size. All 207 frames were drawn on the GPU.
  Playback started in 44 ms on the GPU and 189 ms on the CPU.

**Facts found:**
- Decoding 8-bit sRGB in the texture unit (`Rgba8UnormSrgb`) put a grade's result up to 9 code
  values off the key's math on this GPU, after the round trip to codes that the grade needs.
  Loading the codes as they are (`Rgba8Unorm`) and decoding in the shader brought it to 0.6. That
  change then caused B-034.
- egui and the renderer share one wgpu, so G2 can hand frames to the screen without the copy back to
  memory.

**Limits (G2's list):**
- Frames are read back to memory and uploaded again for display.
- There is one renderer, behind a lock, so frame workers take turns on the GPU. It did not hold back
  24 or 48 fps here; heavier loads are not measured.
- Transitions, blend modes, adjustment layers and clips with other effects are CPU frames, and
  graded clips in them use the Lumetri compile.
- Chroma is blended as if each chroma sample sat at the centre of its pixels. Camera video usually
  sites it at the left. Not yet measured against a reference.
- Shrinking by more than 4× under-samples (4 × 4 samples a pixel). FilmCraft's decoders often hand
  over a reduced picture, which hides it.

**Checked:**
- Renderer tests (10, run on the GPU here):
  - RGBA comes back as it went in;
  - Y'CbCr in video and full range, at 8 and 10 bits, against the formula;
  - the grade and the qualifier against `crates/grade`;
  - a window stays round on a 2:1 picture;
  - layers with opacity;
  - a shrunk checkerboard averages to grey;
  - an enlarged picture blends in linear light.
- Adapter tests (5):
  - GPU against CPU frames, ungraded, graded and qualified;
  - a foreign Lumetri falls back to the CPU;
  - camera video (B-035).

  The threaded playback test now allows for the two renderers' rounding (99 % within 3 code values).
- By hand, 2026-10-14: the figures above. The graded Sony clip looks the same in the editor on both
  renderers by eye.

### D-037 — G2, step 1: every frame FilmCraft plans as layers is drawn on the GPU; graded clips known by their pictures
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "The GPU renderer";
`crates/filmcraft-adapter/src/gpu.rs`

**Decision:** G1 (D-036) drew a frame on the GPU only when it was a plain stack of clips, because
FilmCraft's frame plans do not say which clip a layer comes from. G1 paired layers with clips by
order. Transitions, titles, captions and clips FilmCraft renders itself all add layers, so any of
them sent the whole frame to the CPU. Now:

- **A graded clip's layer is known by its picture.** In the timeline copy the GPU path plans from,
  each graded clip we draw points at a stand-in media item, a copy of its media item under a new
  id. The source provider handed to FilmCraft's planner opens the stand-in's real media and notes
  which clip each decoded picture is for. A layer drawing that picture is that clip's, wherever
  the planner put it. Checks:
  - every picture decoded for a graded clip must be drawn as decoded, without effects or colour
    management run on it first; otherwise its grade cannot be put back, and the frame goes to the
    CPU;
  - two graded clips showing the very same picture send the frame to the CPU.
- **Every other layer is drawn as FilmCraft planned it:** the solid of a dip, the two clips of a
  dissolve with their opacities, captions, titles, clips FilmCraft rendered on the CPU, and stills.
- **Standard effects FilmCraft would run on its GPU** run on the CPU for that layer alone
  (FilmCraft's own `effect_image`), and the layer is drawn on the GPU. Their GPU port is a later
  step of G2.
- **A graded clip with other standard effects or opacity masks** keeps its Lumetri effects in the
  copy. Its grade comes first in its effect stack, so FilmCraft renders that clip on the CPU,
  graded, and the GPU draws it like any pre-rendered layer.
- **Still on the CPU, the whole frame:** a Push transition (`FramePlan::Composite`), blend modes
  other than Normal, and colour management on FilmCraft's GPU path (HDR or wide-gamut media on a
  Rec. 709 timeline). FilmCraft also draws an adjustment layer, other transitions and HDR timelines
  on the CPU itself.
- **Draft playback** (asked for when playback falls behind) no longer shrinks decoded pictures on
  the GPU path, since the pictures are how graded clips are known. The CPU path keeps it.

**Considered:** copying the planner's order of layers into the adapter (fragile against each engine
sync), or asking FilmCraft upstream to name each layer's clip (a patch to carry, and a PR to post).
The stand-ins need neither.

**Measured** (this PC, the demo timeline with five of its clips graded, at a 1440-pixel viewer):

| | GPU path | CPU only |
|---|---|---|
| Frames drawn on the GPU / CPU (26 s) | 617 / 13 (the Push) | 0 / 598 |
| A frame's job, on average | 42 ms (27 of it decoding) | 85 ms |
| Exact frame on screen | 97.1 % | 92.0 % |

The demo's media decode in software; most of the rest is the drop shadow and Desert Dunes'
Lumetri, run on the CPU for their layers. In the editor (default window) both play the demo at
24.0 fps; 615 of 639 frames were the GPU's, the rest the Push.

Against FilmCraft's CPU frames at a 960-pixel viewer, 99 % of values within 1 code value in every
case tested: a dip from black (mean 0.22), a dissolve between two graded clips (0.22 early, 0.24
late), a dip to black from a graded clip into Desert Dunes' own Lumetri (0.48), a graded clip
under a clip FilmCraft renders on the CPU (0.27), a graded clip with a drop shadow (0.02), and the
same media on another track with its own grade (0.18).

**Checked:**
- Adapter tests: the comparisons above; a Push still falls back; draft playback keeps graded
  frames on the GPU; a clip with a Lumetri of its own is drawn as FilmCraft draws it. Mutation
  check: taking over graded clips that have other effects fails a test (and at run time the frame
  falls back, since that clip's picture never reaches a layer).
- By hand, 2026-10-14: the graded demo in the editor, as measured above; a still mid-dissolve on
  both renderers: mean 0.17, worst 2 code values, no channel leaning.

### D-038 — G2, step 2: blend modes, Push and HDR media on the GPU, ported from FilmCraft's compositor
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "The GPU renderer"; `crates/renderer`
(`render.wgsl`, `push.wgsl`); `crates/filmcraft-adapter/src/gpu.rs`

**Decision:** the renderer takes over the rest of what FilmCraft's GPU compositor draws, with its
shader code ported (MIT or Apache-2.0, as in D-013 and D-036):

- **The 27 blend modes**, in FilmCraft's order (our own `Blend`; the adapter maps FilmCraft's by
  index, and a test checks every name matches).
  - Normal and Dissolve use fixed-function "over"; Dissolve keeps a pixel by FilmCraft's 64-bit
    pixel hash, reproduced on the GPU.
  - The other 25 modes copy the canvas under the layer's quad into a backdrop texture first. The
    shader composites with the W3C formulas on sRGB-encoded straight colour, as FilmCraft's CPU
    does.
  - A graded layer blends after its grade.
- **The Push transition.** A frame becomes a list of steps: runs of layers, and Push steps whose
  two sides are drawn apart into their own textures. The sides are pushed together as FilmCraft's
  shader does (eased travel, the CPU camera's card geometry, its shutter's motion blur) and laid
  over the frame so far.
- **Colour management of HDR and wide-gamut Y'CbCr** on a Rec. 709 timeline, stage by stage as
  FilmCraft's `InputTransform` runs it: the curve, HLG's OOTF, the gamut matrix, BT.2390 tone
  mapping, and desaturation into the working gamut. Also PQ and HLG transfers without it.
- **A layer with standard effects and colour management** has both run on the CPU for that layer
  (FilmCraft's `effect_image_with`), as before for effects alone.

Still drawn whole on the CPU: what FilmCraft itself draws there (adjustment layers, transitions
other than dissolves, dips and Push, HDR timelines), log media (its colour management has no GPU
form), and a graded clip that has colour management to run before its grade.

**Measured** (this PC):
- Against FilmCraft's CPU executor (`execute_cpu`) on the same plan, drawn one to one:
  - each of the 27 blend modes: 99 % of values within 1 code value, mean 0.11 to 0.17. Hard Mix,
    Darker Color and Lighter Color pick one of two colours per pixel, and a few pixels on the
    threshold land the other way after half-float rounding (worst 66 to 88), as FilmCraft's own
    tests note for Hard Mix;
  - HDR: HLG, PQ, PQ mastered at 4000 cd/m², and BT.2020 SDR, each within 1 code value.
- The demo's Push between two graded clips, at three moments: mean 0.12 to 0.17, 99 % within 1
  code value. In the editor a still mid-Push: mean 0.11, worst 2.
- The graded demo (D-037's project) at a 1440-pixel viewer: all 641 frames on the GPU (13 were on
  the CPU before); the exact frame on 100 % of refreshes (97.1 %); 38.7 ms a frame (42.4); the
  longest a frame stayed on screen, 55 ms (796 ms, at the Push). In the editor: 639 of 639 frames
  on the GPU, 24.0 fps.

**Checked:**
- Renderer tests: the existing ten, on the reworked renderer (frames as steps; one pass per layer).
- Adapter tests: the blend modes and HDR media against FilmCraft's CPU, as above; the Push between
  graded clips against the CPU frame. 187 tests in the workspace.
- By hand, 2026-10-14: the graded demo in the editor, as measured above; the mid-Push still looks
  right (both sides, with motion blur).

### D-039 — G2, step 3: the viewers show the renderer's frames on the GPU, without a copy back
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "The GPU renderer";
`crates/engine-api` (`Frame::gpu`, `RenderHints::gpu`); `crates/renderer` (`Picture`);
`apps/resolution-app/src/monitor.rs`

**Decision:** the editor puts the GPU renderer on egui's own wgpu device at start-up. Playback's
frames are then left on the GPU and drawn by egui from the renderer's texture, instead of being
read back to memory and uploaded again.

- **The engine API:**
  - `Frame` may carry `gpu`, a frame on the GPU, with `rgba` empty;
  - `Frame::pixels()` gives the pixels either way, reading a GPU frame back once when asked;
  - a front end asks for GPU frames with `RenderHints::gpu`.

  The texture crosses the API as an opaque `GpuPicture` (with wgpu, a `TextureView`), so
  `engine-api` takes no GPU library. Only the playback frame server asks, and only for viewers
  that show GPU frames. Frame renders for agents and thumbnails still come as pixels.
- **The renderer:**
  - `draw_steps` submits the frame and returns a `Picture` without waiting;
  - the output is `Rgba8Unorm`, sRGB-encoded by the resolve shader (as FilmCraft's compositor
    does), since egui's renderer takes native textures in that form;
  - `pub use wgpu`, so callers name the same device type.
- **The adapter:** `use_gpu_device(device, queue)` makes the program's renderer on that device;
  it works only before the renderer is first wanted.
- **The editor:** a viewer registers the frame's texture with egui's wgpu renderer and swaps it
  for each new frame. The scopes and the qualifier's picker read the frame back
  (`Frame::pixels`), on the scopes' thread or at the click. The frame cache counts a GPU frame by
  its size. Without a wgpu device (the UI tests) or with `--cpu-render`, frames come as pixels as
  before.

**Measured** (the graded Sony clip, `--play 8`, the editor's default viewer, two runs each):

| | Read back (step 2) | Left on the GPU |
|---|---|---|
| A frame's job | 7.9–8.0 ms (5.0–5.1 of it decoding) | 6.8–6.9 ms (4.8–4.9 decoding) |
| On screen | 23.9 fps | 23.9 fps |

About 1.1 ms a frame, and the upload on the UI thread is gone. The `--play` report now gives
the average frame job and whether the last frame was shown straight from the GPU. The CPU path in
the same run: 49.5 ms a frame.

**Checked:**
- Renderer test: a frame left on the GPU reads back exactly as a rendered one, and only once.
- 188 tests in the workspace.
- By hand, 2026-10-14:
  - the editor's report says the frames were shown straight from the GPU;
  - a screenshot of the graded Sony frame shown this way against the CPU renderer's: mean 0.40,
    99 % within 1 code value, no channel leaning (the colours are not encoded twice);
  - the Color page's waveform, drawn from a frame read back, matches what it showed before.

### D-040 — G2, step 4: a shrunk picture averages all its pixels; chroma read as sited left
**Area:** color · **Status:** active · **Doc:** `crates/renderer` (`MAX_TAPS`, `Siting`)

**Decision:** the two picture-quality limits D-036 listed.

- **Shrinking.** The shader averages samples over each frame pixel's footprint in the source,
  capped at 4 × 4 as in FilmCraft's compositor. Shrunk more than 4 times, the samples skip source
  pixels.
  - Measured with a test: white stripes 2 pixels wide every 8, shrunk 16 times, came out **black**
    at some phases (every sample landed between stripes). They should be a quarter grey at every
    phase.
  - Now: at least one sample per source pixel across the footprint, up to 32 a side. The work
    stays about one sample per source pixel whatever the size.
  - Cost, graded Sony 4K one frame at a time: 6.5 ms a frame at a 720-pixel viewer (6.3 before),
    6.2 at 360 (5.7). The test now passes at every phase.
- **Chroma siting.** The shader read subsampled chroma as if each sample sat halfway between its
  luma columns (centred: MPEG-1, JPEG). H.264, HEVC and ProRes put it on the left column of each
  pair by default (vertically halfway), and the Sony clip's file says so (`chroma_location=left`,
  ffprobe). FilmCraft's frames do not carry the chroma location, so the adapter reads all Y'CbCr
  as sited left (`Coding::siting`).

  The evidence:
  - **Definition.** A test makes 4:2:0 chroma from samples taken at the left column of each pair,
    across a sharp red | blue edge. Read as sited left, the sampled columns come back within 1
    code value; read as centred, 52 off.
  - **The Sony frame's own edges.** Luma averaged to chroma resolution on the centred grid, then
    edge strength cross-correlated with the chroma planes: the chroma edges sit 0.40 chroma pixels
    left of the centred grid. Sited left predicts 0.25, centred 0.
  - **Not decisive, recorded:** zscale (zimg, through ffmpeg 9) with `chromalin=left` agreed better
    with our centred reading than with our left one at chroma edges (mean 5.7 against 6.1 code
    values), as if its whole chroma grid sat a quarter chroma pixel from the definition above.
    Unexplained. (A first try set `chromal`, the output's location, by mistake.) FilmCraft's CPU
    takes the nearest chroma sample, which is arithmetically nearer a centred blend: on the Sony
    frame at full size, GPU against CPU is now mean 0.83 (0.78 read centred), 99 % within 5.
    Neither is the truth for this question.
- **Limit:** phone video stored sideways is turned upright by FilmCraft's decoder, plane by plane,
  which turns its siting by a quarter turn too. Read as sited left, its chroma is a quarter chroma
  pixel off. FilmCraft's frames would need to carry the siting (and the turn) to fix it.

**Checked:** renderer tests (13): the stripes at every phase, the siting test. 190 tests in the
workspace.

### D-041 — The GPU port of FilmCraft's standard effects waits for an Effects panel
**Area:** color · **Status:** active · **Doc:** ROADMAP M4

**Decision (Truman, 2026-10-14):** G2 is closed without porting FilmCraft's standard effects
(Brightness & Contrast, Tint, Levels and the rest of its GPU list) to our renderer.

**Why:** Resolution has no Effects panel yet, so these effects reach a timeline only through an
imported project or an agent's raw command. They already render correctly: FilmCraft runs them on
the CPU for that one layer, and the rest of the frame stays on the GPU (D-037). The port is FilmCraft's
effect shaders and compute stage (about 1,300 lines) plus our own types for each effect's settings.
It speeds up something that is rare today.

The choices put to Truman: defer and start G3 (chosen); port now; something else first. The port
is listed under the Effects panel's work in the ROADMAP.

### D-042 — G3: Deliver renders through the GPU renderer, so a file matches the viewer
**Area:** color · **Status:** active · **Doc:** ARCHITECTURE "The GPU renderer";
`crates/filmcraft-adapter/src/gpu.rs` (`ExportFrames`); `crates/renderer` (`render_linear`)

**Decision:** FilmCraft's export takes a registered frame renderer (`register_frame_renderer`) for
the frames it would otherwise render on the CPU, when the export asks for GPU rendering. It builds
up to four, one per export worker, and falls back to its CPU renderer frame by frame when one
declines. Resolution registers its own:

- **`ExportFrames`.** On its first frame it makes the graded copy of the export's project (as a
  playback snapshot does). Every frame is then planned, tagged and drawn exactly as the viewers'
  frames are (D-037 to D-040). The frame comes back as the renderer's canvas: linear light,
  premultiplied, not laid over black (`Renderer::render_linear`, read back from half floats), which
  is what FilmCraft's CPU renderer hands an export. Alpha survives for formats that keep it.
- **`deliver.render` asks for it.** The adapter adds `gpuRendering: auto` to FilmCraft's export
  parameters unless the caller gives one, or `--cpu-render` turned the GPU renderer off. FilmCraft's
  own default is off. The registration happens when the engine starts.
- Frames FilmCraft draws on the CPU itself (adjustment layers, other transitions, HDR timelines)
  stay on the CPU, as in the viewers.

So a file now renders with the same renderer as the viewer: graded clips by the grade's own math,
qualifiers as real keys. The Lumetri compile (D-028) still sits in the project, for FilmCraft's own
renders and for frames the GPU declines.

**Measured** (this PC):
- Deliver's frames against FilmCraft's CPU renderer, the graded demo at half size, as the export
  takes them (a dissolve, a clip FilmCraft renders on the CPU, a Push, a clip with its own
  Lumetri): mean 0 to 0.11, 99 % within 1 code value, alpha within 0.0005.
- Ten seconds (241 frames) of the graded Sony 4K clip, from the CLI (software decoding):

  | Preset | GPU renderer | CPU | |
  |---|---|---|---|
  | H.264 1080p | 13.4 s (17.9 fps) | 39.3 s (6.1 fps) | 2.9× |
  | H.264 2160p 4K | 38.4 s (6.3 fps) | 81.3 s (3.0 fps) | 2.1× |

  The two files against each other (ffmpeg's PSNR): 47.8 dB at 1080p and 47.2 at 4K on average,
  46.2 at worst, about a code value. The rest of the time is decoding and FilmCraft's H.264 encoder,
  both on the CPU here.
- In the editor (an agent's `deliver.render` over the agent link, the renderer on egui's device,
  hardware decoding): five seconds of the graded Sony clip to H.264 1080p at 22.3 fps.

**Checked:**
- Adapter tests: Deliver's frames against the CPU, as above; `deliver.render` of one second of the
  graded demo writes the file, with every frame counted as the GPU renderer's
  (`gpu_render_stats`).
- Renderer test: the linear canvas keeps alpha, and encoded it gives the 8-bit frame.
- 193 tests in the workspace.
- By hand, 2026-10-14: the renders and measurements above.

### D-043 — The hue curves: Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat
**Area:** color · **Status:** active · **Doc:** `crates/grade` (`HslCurve`, `Node::color`);
`crates/renderer`; `crates/filmcraft-adapter/src/color.rs`; `apps/resolution-app/src/color_page.rs`

**Decision:** the Curves palette gets Resolve's curve modes beyond Custom. They come from a menu in
the palette's title strip, as Resolve's Curves palette chooses them.

- **The model** (`crates/grade`): each node holds five hue curves, `HslCurve`.
  - Points run 0 to 1 both ways. An output of ½, the middle line, changes nothing.
  - A curve over hue runs round the colour wheel (its last point joins its first). Lum vs Sat
    and Sat vs Sat hold their end points' values beyond them.
  - Grades stored before them load with none, and are written back byte for byte, so their
    compiled Lumetri effects do not change.
- **The math** (`Node::color`): after the saturation step, the hue curves and Hue's rotation run
  together in HSL. Each curve is read at the pixel's hue, saturation or lightness as it was before
  any of them:
  - hue turns by the rotation plus Hue vs Hue's distance from the middle line;
  - saturation is multiplied by twice each of Hue vs Sat, Lum vs Sat and Sat vs Sat;
  - lightness moves by half of Hue vs Lum's distance from the middle line.

  These are the operations FilmCraft's Lumetri HSL curves perform, so a grade compiles to them
  exactly. Without hue curves a node is what it was before them.
- **The GPU renderer** samples each curve into a 4096-point table, five more per node, wrapping
  round the wheel for the hue curves.
- **The compile:** the curves go into the node's saturation Lumetri effect (`hue_vs_hue` with
  Hue's rotation added, `hue_vs_sat`, `hue_vs_luma`, `luma_vs_sat`, `sat_vs_sat`), sampled at 96
  points.
- **The page:** a hue curve's plot runs the width of the palette, with what it is over along its
  foot: the colour wheel from red, black to white, or grey to full colour. Click the line to add a
  point, drag one to move it (a drag elsewhere makes one and moves it), right-click one to remove
  it. Each drag is one undo step.

**Not here:** Resolve's Sat vs Lum, because Lumetri, which frames off the GPU render with, has no
such curve. There is also no picker from the viewer yet (in Resolve, a click on the picture adds
points around its hue).

**Facts found:** Lumetri joins every HSL curve as if it ran round the wheel, Lum vs Sat and Sat vs
Sat too. Sampled at the middles of even steps, a held curve's two ends met in a join inside
Lumetri's table. Colours that more Saturation pushes to full saturation read Sat vs Sat there, and
the CPU's render strayed from the math by 9 code values for 99 % of pixels (Saturation 60 with Hue
vs Sat and Sat vs Sat). Bisected with renders of each curve alone, in pairs and in threes. The fix
samples a held curve from just inside 0 to just inside 1, since Lumetri's table only runs from
0.0003 to 0.9997.

**Measured:**
- The GPU against the math: within 1 code value, all five curves and Hue's rotation at once
  (renderer test).
- The CPU's render through Lumetri against the math, the demo at full size, all five with
  Saturation and Hue: 99 % within 0.73, worst 0.85 code values.
- In the editor, the Sony clip with Hue vs Sat taking the colour out of blues: GPU against CPU mean
  0.38, 99 % within 2, worst 28. The worst pixels sit on the curve's steep fall, which the two sample
  differently and grade before or after shrinking, as at a qualifier's soft edge (D-033).

**Checked:**
- Grade tests (6 new): the curves' wrap and ends; Hue vs Sat taking colour from one hue only; Hue
  vs Hue and Hue vs Lum on one hue; Lum vs Sat and Sat vs Sat; the rotation and Hue vs Hue adding
  up, and nodes without curves unchanged; tidying, and grades stored before them.
- Renderer test against the math; mutation check: Hue vs Lum at half strength in the shader fails
  it (19.6 code values).
- Adapter test: the CPU's render against the math.
- UI test: Hue vs Sat chosen from the menu, a drag at red, reds less saturated through the engine.
- By hand, 2026-10-14: the Sony clip, as measured above. The blue light and background went grey;
  the magenta and pink highlights kept their colour.

### D-044 — Grades render by their own math on every path: graded clips' pictures graded at their source
**Area:** color · **Status:** active (Deliver's own CPU frames: next) · **Doc:** `crates/filmcraft-adapter/src/gpu.rs`

**Decision (Truman, 2026-10-14):** before building Color features that Lumetri cannot carry (key
blur and denoise, Midtone Detail, parallel and layer nodes), Resolution grades each clip's
pictures itself on every path. FilmCraft's renderers then receive pictures already graded, and
the Lumetri compile (D-028) stays only so a project still opens sensibly in FilmCraft. The choices
put to him:

1. grade at the source everywhere (chosen);
2. keep the Lumetri bridge, the new features missing where FilmCraft draws a frame itself;
3. build only features Lumetri can carry, and decide later.

**How:**
- **Stand-ins.** Every graded clip in the working copy now points at a stand-in media item, at
  ids from 2^60 up, a range no project reaches.
  - A clip with nothing else on it is graded in the shader as before.
  - A clip with other effects or opacity masks gets a **`GradedSource`**: the real media's
    pictures, graded by the GPU renderer at the size they are decoded at (`render_linear`), and
    handed to FilmCraft as linear-light pictures. FilmCraft then runs the clip's other effects on
    the graded picture, the grade first in its stack as in Resolve. D-037's rule that such a clip
    keeps its Lumetri effects is gone.
- **Frames FilmCraft draws itself** (an adjustment layer, transitions other than dissolves, dips
  and Push, an HDR timeline, or a frame the GPU declines) come from the copy with every graded
  clip's pictures graded at their source (`Graded::render_cpu`). This covers the viewers' CPU
  frames.
- **Agents' snapshots** (`render_frame`) are the viewers' frames: the GPU path, then the copy with
  graded sources. Only without a GPU, or with `--cpu-render`, does FilmCraft draw from the real
  project with the compiled grades.
- **Not yet:** Deliver's frames that FilmCraft draws whole on the CPU. Its export skips the
  registered frame renderer for those, so they still take the compile. Next: Deliver exports the
  copy, with the graded sources registered in the media pool (`MediaPool::insert`), so no FilmCraft
  change is needed.

**Measured:**
- A Wipe between two clips keyed hard on a band of warm hues (a sharp key edge), half size,
  against the math: 99.9 % within 1.00 code value. Through the Lumetri compile, as before this
  change: 99.9 % within 17.6, where the 33-point table cannot follow the key's edge.
- An agent's frame against the viewer's, a qualifier grade: identical.
- The graded demo at a 1440-pixel viewer: every frame on the GPU, the exact frame on every refresh,
  38.8 ms a frame (38.7 before). V2's drop-shadow clip is now graded at its source.

**Checked:**
- Adapter tests: the Wipe against the math (it fails through the compile); the agent's frame is
  the viewer's; a graded clip with other effects stands in and is graded at its source; a foreign
  Lumetri still gets no stand-in. 204 tests in the workspace.
- By hand, 2026-10-14: the graded demo in the editor and in `play_bench`, as measured; while doing
  so, B-036.

### B-036 — The editor stopped when its window was resized while frames rendered (since D-039)
**Area:** color · **Status:** fixed (D-039's direct path off) · **Caught by:** running it (the
graded demo's `--play` check for D-044 panicked once, then a stress test reproduced it)

**Symptom:** the editor panicked in eframe's wgpu painter: "Validation Error: In
Surface::configure: Failed to wait for GPU to come idle before reconfiguring the Surface".

**Cause:** since D-039 the GPU renderer shared egui's wgpu device, so its frame workers (and an
export's threads) submitted work to the same queue as the window. When eframe reconfigures the
window's surface (on a resize, and at start-up), wgpu waits for the queue to be idle and refuses if
another thread submitted meanwhile. Its own source says so ("It can only be non-empty if another
thread is submitting at the same time"). eframe leaves wgpu's default error handler, which panics.

**Reproduced:** a temporary self-test resized the window every four frames while the graded demo
played. Two runs in three stopped this way.

**Wrong turn:** an error handler that logged surface errors, with egui told to reconfigure a
surface left unconfigured. Runs then went past the refused reconfigure, but the next frame drew
at the new window size into the old-size surface, and wgpu stopped the editor on that instead
(a scissor rectangle larger than the render target). Two runs in six.

**Fix:** the renderer keeps a device of its own again. Frames reach the viewers as pixels, read
back as before D-039, which costs about 1.1 ms a frame (D-039's measurement). Under the same stress
test, six runs in six played to the end. D-039's machinery stays, unused (`Monitor::draw_on_gpu`).
Sharing the device again needs the GPU work done on the UI thread, as FilmCraft's own viewer does
it, and exports and graded sources kept on a device of their own.

**Seen by Truman:** the window resizing very fast during the stress test, its left edge still and
its right edge moving about an inch. That was the test itself, run only with a flag. It is
removed.

### D-045 — Deliver renders the graded copy, so its own CPU frames show grades by their own math
**Area:** color · **Status:** active · **Doc:** `crates/filmcraft-adapter/src/deliver.rs`;
`crates/filmcraft-adapter/src/gpu.rs` (`register_sources`, `exporting`)

**Decision:** the rest of D-044. FilmCraft's export draws some frames whole on the CPU without asking
the registered frame renderer (an adjustment layer, a transition other than dissolves, dips and
Push). So `deliver.render` no longer exports the project; it exports its graded copy:

- **The graded sources go in the pool.** Every stand-in is put into the session's media pool as a
  graded source of its real media (`MediaPool::insert`), so FilmCraft's own CPU frames of the copy
  get graded pictures.
- **FilmCraft runs the export.** It is FilmCraft's job, with its settings, range and output
  (`settings_from_params`, `range_param`, `spawn_export`). The output path follows FilmCraft's rules,
  repeated in `deliver.rs` because FilmCraft's helper for them is private.
- **GPU frames grade in the shader.** The export's GPU frame renderer finds the copy in a registry
  (`exporting`), by its project's address, and draws as the viewers do, rather than grading at the
  source.
- **When FilmCraft's own export runs instead:** nothing graded, no GPU, `--cpu-render`, or a graded
  subclip. The pool resolves a subclip to its media item before its own id, so a subclip's stand-in
  would never be asked for.
- `deliver.render`'s answer no longer lists `offlineMedia` (FilmCraft's helper for it is private).

**Measured:**
- One frame inside a Wipe between two clips keyed hard on warm hues, to PNG, against the grade's
  math applied to the same frame delivered ungraded: 99 % within 0.00, 99.9 % within 1.00 code
  values. Through FilmCraft's export of the project, the compile's baked key, as before: 99.9 % within
  17.6.
- Ten seconds of the graded Sony clip to H.264 1080p: 13.6 s (13.4 s before), so the GPU frames
  still grade in the shader.

**Checked:** adapter test of the Wipe to PNG (it fails through FilmCraft's export of the project);
the earlier Deliver tests. 205 tests in the workspace.

### D-046 — Matte finesse for the qualifier: Denoise, Clean Black, Clean White, Blur Radius; the GPU's grade stage
**Area:** color · **Status:** active · **Doc:** `crates/grade` (`Qualifier::matte`, `Grade::apply_image`);
`crates/renderer` (`Renderer::stage`, `render.wgsl`'s grade stage); `apps/resolution-app/src/color_page.rs`

**Decision:** the first Color feature that needs a pixel's neighbours, built on D-044's grading at
the source. On real footage a raw HSL key is speckled and hard-edged; Resolve cleans it in the
Qualifier palette's Matte Finesse. Ours, in Resolve's 0 to 100, cleans the key in this order:

1. **Denoise.** Passes of a three-pixel median across and then down. Up to three passes at 1920
   pixels wide, in proportion at other widths, at least one when set.
2. **Clean Black and Clean White.** The key's black point raised by Clean Black / 200 and its white
   point lowered by Clean White / 200, the key stretched between them. These work pixel by pixel,
   so they also go into the per-pixel grade and the Lumetri compile's baked table.
3. **Blur Radius.** A Gaussian to three standard deviations. Its standard deviation is up to 10
   pixels at 1920 pixels wide, in proportion at other widths, so a key looks the same at any
   size.

Pixels beyond the picture's edge are taken as the edge's. Qualifiers stored before this load with
all four at 0 and are written back byte for byte.

**The math** is in `crates/grade` as a reference over whole pictures (`Grade::apply_image`;
`Grade::apply` stays per pixel, without Denoise and Blur).

**The GPU's grade stage** (`Renderer::stage`) runs for a layer whose grade has a node whose
qualifier denoises or blurs. The layer is drawn at its working size (as big as it shows, at most
its own size) into a texture of straight, display-encoded colour, and graded there node by node.
A node with such a qualifier first makes its matte in a mask: the key, the medians, Clean, then the
Gaussian across and down. Every node is then applied through its matte and windows. The result is
composited like an 8-bit picture. Other layers are graded in the compositing shader as before.

**Not in the compile:** Denoise and Blur Radius cannot go in a baked table, so a render FilmCraft
makes itself (no GPU, `--cpu-render`) keys without them. The table's id leaves them out, so
dragging them makes no new table.

**The page:** a fourth column, Matte Finesse, in the Qualifier palette, beside Hue, Saturation and
Luminance: drag a number sideways, double-click it to reset.

**Measured:**
- The GPU's stage against the math: a 768 × 64 picture with a red block and red specks in grey
  noise, keyed on red with Denoise 40, Clean Black and White 20 and Blur Radius 60, then a second
  node: within 0.93 code values everywhere.
- Mutation checks: a Gaussian of the wrong width fails the test (19.2 off); a median that returns
  its middle value fails it (12.3 off).
- On the Sony clip at a 1440-pixel viewer, keyed on the magenta light and desaturated:
  - playback at 24.0 fps, every refresh the exact frame;
  - 8.5 ms a frame with matte finesse against 8.1 ms without.

**Wrong turns:**
- The first stage test used a 96-pixel picture, where Blur Radius 60 is a third of a pixel. A
  Gaussian of the wrong width passed it. It is now 768 wide.
- The grade tests' first qualifier keyed greys too (grey's hue reads as red). Their qualifier now
  has a saturation floor.

**Checked:**
- Grade tests: Clean levels; Denoise taking out a lone keyed pixel and keeping a block; Blur Radius
  in proportion to the picture; a picture without spatial mattes graded pixel by pixel; old
  qualifiers loading and values held to 0 to 100.
- The renderer test above.
- UI test: Matte Finesse's Blur Radius and Clean Black dragged, through the engine. 212 tests in
  the workspace.
- By hand, 2026-10-14: the Sony clip in the editor, plain key against matte finesse. The plain key
  left hard, broken edges and pink specks on the face and hand; with matte finesse the edges are
  soft and the specks are gone.

### B-037 — Since D-044 the colour tests of the Lumetri compile drew their frames on the GPU
**Area:** color · **Status:** fixed · **Caught by:** review, confirmed by a mutation check (while
adding Sat vs Lum, D-047)

**Symptom:** with Hue vs Sat taken out of the compile, the test that the compiled hue curves follow
the grade's math still passed (0.60 code values off).

**Cause:** `crates/filmcraft-adapter/src/color/tests.rs` takes its frames from
`Engine::render_frame`. Before D-036 that was FilmCraft's CPU render of the project, the compile.
Since D-044 it is the viewer's frame, the grade's own math on the GPU renderer. So the six tests
that render (the grade, the hue curves, Gain 0 and Saturation 0, a window, a qualifier, a qualified
node's table) checked the GPU path, already covered by the renderer's tests, and nothing checked
the compile end to end.

**Fix:** the tests' `frame` renders the project through FilmCraft itself
(`Session::try_render_program_at`), as before D-036. All pass, with the figures measured before
(the hue curves within 0.73 code values). With Hue vs Sat taken out of the compile, the test now
fails, 54.6 code values off.

**Lesson:** when a decision moves what a function returns (here D-044, for `render_frame`), check
the tests that call it for what they meant to test.

### D-047 — Sat vs Lum, with a node carrying it baked into a table in the compile
**Area:** color · **Status:** active · **Doc:** `crates/grade` (`Curves::sat_lum`, `Node::color`);
`crates/filmcraft-adapter/src/color.rs` (`baked`); `apps/resolution-app/src/color_page.rs`

**Decision:** the sixth hue curve. D-043 left it out because Lumetri has no such curve, and frames
off the GPU were then rendered by Lumetri. Since D-044 they are not, so it is built:

- **The math.** By saturation, the lightness: as Hue vs Lum, lightness moves by half the curve's
  distance from the middle line, read at the pixel's saturation before any hue curve. The two add
  up. In Resolve's manual it is the inverse of Lum vs Sat: lighter or darker by saturation.
- **The GPU.** A ninth table per node (`TABLES`), on its own bit of the node's curve flags.
- **The compile.** A node with Sat vs Lum is baked into a 33-point table, as a qualified node is
  (D-032): the whole node in one Lumetri effect's input LUT, named "Resolution node" in the
  project's LUT library. It is coarse: 99 % of pixels within 5.4 code values, against 2.3 for the
  qualified node of D-032's measurement. The pixels more than 3 off are darker than the frame
  (mean lightness 0.17 against 0.30), where saturation moves fast between table points. The
  alternative, leaving the curve out of the compile, would be tens of code values off. The compile
  is only what FilmCraft and renders without a GPU show.
- **The page:** Sat vs Lum at the foot of the Curves palette's menu, over grey to full colour,
  as in Resolve's list.

**Measured:**
- The renderer against the math, all six hue curves at once with Hue's rotation on noise: within
  1 code value. With the shader reading Sat vs Lum at the lightness instead, 35.8 off.
- The compile on the demo frame (Saturation 70 and a Sat vs Lum curve): 99 % within 5.4, worst
  7.0 code values. Without the table (the node compiled to Lumetri's sections), the test fails:
  no table is made.

**Checked:** grade test (a full red lightened by a quarter at the curve's top, dull colours and
grey as they were); the renderer and adapter tests above; UI test: Sat vs Lum chosen from the
Curves menu and dragged, through the engine. Grades stored before it load and are written back
as they were (the existing test). 215 tests in the workspace.

### B-038 — A Deliver render could stop at 0 %: export frames waited for renderers their own threads held
**Area:** color · **Status:** fixed in Resolution; likely upstream too (not checked) · **Caught
by:** running the tests (the workspace's test run hung for 25 minutes; the Deliver page's UI test
then failed about one run in five)

**Symptom:** `the_deliver_page_renders_the_timeline_to_a_file` sometimes never finished:
`Rendering { progress: 0.0 }` after 120 s. Alone it passed. With three other UI tests beside it,
it failed on runs 1, 5 and 8 of a loop. The same happened on main (built from main and run, it
hung at once), so this was not D-047's doing. It can have happened since D-042, when
`ExportFrames` became the export's frame renderer.

**Found with:** temporary probes in `gpu.rs` keeping each thread's last step in memory, dumped
when the export had made no progress for 30 s (printing every step changed the timing so much
that 40 runs passed). At the stop, four export threads were inside FilmCraft's decode of the same
demo clip, and the other twelve had finished a frame and gone no further.

**Cause:** FilmCraft's export draws 16 frames at a time on rayon's global pool. Each frame takes
one of four frame renderers (ours, `ExportFrames`) and waits on a condition variable when all are
taken. A frame holding a renderer decodes its pictures, and the demo's generated clips render with
`par_chunks_mut`. A rayon worker waiting for its own parallel work runs other queued work
meanwhile, and that can be another of the export's frames. That frame then waits for a renderer,
one of them held further up the same thread, so the outer frame can never finish. Once all four
holders are caught this way, no renderer comes back. Other work on the global pool (the other
tests' renders here, the viewers' in the editor) makes it likelier, since it changes which work is
queued where.

**Fix:** `ExportFrames` runs each frame's work on a thread pool of its own (`apart` in `gpu.rs`).
The export's worker waits for it on a plain thread join, which takes no other work. A panic is
passed back, so FilmCraft's export still retires its renderers when one panics. With the fix, the
same four tests passed 60 runs in 60.

**Upstream:** FilmCraft's own GPU frame renderer takes the same pool, and probably decodes inside
it too, so its exports could stop the same way. Not checked yet. A report would need Truman's
approval first.

**Not done:** a test that brings this about every time. The Deliver UI test catches it about one
run in five, beside other tests.

### D-048 — The Highlight view: a front-end viewer option, drawn by the GPU's grade stage
**Area:** color · **Status:** active · **Doc:** `crates/engine-api` (`Highlight`,
`RenderHints::highlight`); `crates/renderer` (`Layer::highlight`, `fs_highlight`);
`crates/grade` (`Grade::highlight_image`); `apps/resolution-app` (`color.highlight`, `color.highlightBW`)

**Decision (Truman, 2026-10-14, from two options put to him):** the Highlight view is a viewer
option the front end owns, as Resolve's viewer menu has it, passed down to the renderer with each
frame. Not an engine command: which node is current already lives in the front end
(`color.nextNode` is a front-end command), and a vocabulary id is stable API for good. Agents can
be given it later as an option on rendering a frame.

**What it shows**, as in Resolve's manual ("Using Highlight to See What You're Isolating"):
- **Highlight** (Shift+H): the picture where the current node's matte keys it, flat grey (0.5,
  display-encoded, `HIGHLIGHT_GRAY`) elsewhere, in proportion to the matte. The keyed part shows
  the whole grade, so a correction can be judged while keying.
- **Highlight B/W** (Alt+Shift+H): the matte itself, white where keyed.
- The matte is the node's qualifier with its matte finesse, times its windows. A node without a
  qualifier keys everything. Each key turns its own kind on, or off again; View > Highlight and
  Highlight B/W do the same, on the Color page.

**How:**
- `RenderHints::highlight` names the clip and node. The frame cache's key carries it
  (`FrameKey::highlight`), so turning it on or off draws anew and never shows the other kind's
  frame.
- The adapter matches the clip to its layer (`Graded::render`). A grade with a key or a window now
  gets a stand-in even before its nodes correct anything, so a key can be seen as it is set up.
- The renderer draws a highlighted layer through the grade stage (D-046). The highlighted node is
  drawn even if it corrects nothing; its matte is made in a mask and kept, and a last pass
  (`fs_highlight`) shows it.
- The scopes and the qualifier's picker read the frame without the highlight
  (`Monitor::graded_picture`), which the viewer also asks for while paused.

**Not covered yet:** a clip with other effects (graded at its source, D-044) and frames FilmCraft
draws itself show as they are, without the highlight. So does a clip whose grade has neither a
correction, a key nor a window (there is no stand-in to draw). Agents cannot ask for it yet.

**Measured, the renderer against `Grade::highlight_image`** (768 × 64, a red block in grey noise;
four nodes: a key with matte finesse, a key read pixel by pixel, a key on a node that corrects
nothing, no key):
- in flat grey, within 0.81 to 1.20 code values;
- in black and white, within 0.93 where the matte is fully on or off. On its soft edge the matte
  shows what its key read magnified: 0.64 for the first node, 1.92 and 3.26 for the nodes keying
  the results before them, which the stage holds at half-float precision.

**Wrong turns:**
- The first lightness test of a key on greys keyed by hue as well, and was 57.5 code values off.
  The grey pixels nearest the blurred red key carry the faintest tint, and that tint decides the
  hue, so the CPU and GPU keyed them differently. Keyed by lightness alone, it is within 0.94.
- The first renderer tolerance was one figure for the whole matte, before measuring where the
  error lies. It is now one for the matte's on and off parts and one for its soft edge.

**Checked:** grade test (keyed as graded, the rest grey; the matte in black and white; no key
keys everything; a disabled or missing node shows the grade); the renderer tests above, and a
window's circle in black and white (with the windows left out of the last pass, it fails); UI
test: a key with no correction, Alt+Shift+H shows only greys, white and black, the graded picture
beside it for the scopes, Shift+H flat grey, Shift+H again off. 219 tests in the workspace.

**Not checked by eye** in the editor yet.

### D-049 — `--window-at`: the editor's self-tests open on a screen of their own
**Area:** tools · **Status:** active · **Doc:** `apps/resolution-app/src/main.rs`; memory note on
test windows

**Decision (Truman, 2026-10-14):** visual test runs (`--play`, `--screenshot`) go on his second
screen, not the one he works on. `resolution-app --window-at X,Y` puts the window's top left
corner at X,Y in the screens' own pixels, as Windows lays them out. His second screen is at
-1920,165 (1920 × 1080, 100 %), left of the main one (3440 × 1440 at 150 %). The window is not kept
there for the next ordinary start (`persist_window` off).

**Facts found:**
- eframe reads the window's last position back even with `persist_window` off, and that wins over
  `ViewportBuilder::with_position`. So the editor moves its window on the first frame instead
  (`ViewportCommand::OuterPosition`), before it is shown.
- egui's positions are points, scaled by the screen the window is on. The flag's pixels are
  divided by the window's scale, so the same numbers work whatever the scaling.
- A DPI-unaware process (PowerShell by default) sees the main screen as 2293 × 960; per-monitor
  aware, it is 3440 × 1440. Screen positions for the flag come from the latter.

**Checked:** run with `--window-at -1920,165 --screenshot`, the window's first visible position
was on the second screen at -1920,165 (read with `GetWindowRect` and `Screen.FromHandle`), and the
screenshot is right. A test of the flag's parsing.

### D-050 — `--highlight` self-test flag; the Highlight view, matte finesse and Sat vs Lum seen by eye
**Area:** tools · **Status:** active · **Doc:** `apps/resolution-app/src/main.rs`

**Decision:** `resolution-app --highlight grey|bw` starts with the Color page's Highlight view on
(D-048), so a screenshot self-test can show it. It is a front-end flag for tests, like `--page`,
not a command.

**Seen by eye, 2026-10-14 (overnight), on the second screen (D-049):** the Sony clip at 5 s,
graded through the CLI (`color.adjust` with a qualifier picked on the magenta light, 170, 31, 108,
as `Qualifier::picked` makes it), each screenshot at the Color page's viewer size:
- **Highlight B/W** of a key with matte finesse and no correction yet: a white matte on the hand,
  the forearm, the face's edge and the guitar's pegs, soft-edged, black elsewhere; a few small
  specks remain in the shadows.
- **Highlight (grey)** of the same: those parts in their own pink on flat grey. With Saturation 10
  on the node, the keyed parts show desaturated on grey.
- **Matte finesse against a plain key,** both desaturating the magenta: the forearm loses its pink
  more evenly with matte finesse.
- **Sat vs Lum** lowered towards full saturation: the saturated blue wall goes much darker, and the
  light bar, near white and so less saturated, stays bright. The pink on the arm darkens a little.
- Nothing looked wrong. Not compared with Resolve itself.

### B-039 — The keymap had Add Video Only and Add Audio Only Transition on each other's keys
**Area:** ui · **Status:** fixed · **Caught by:** review, checking the keymap against the
Reference Manual (D-051)

**Symptom:** Shift+T added a video-only transition and Alt+T an audio-only one.

**Cause:** the seed keymap (D-017) had them the wrong way round. The manual's Edit page chapter on
transitions gives Option-T (Alt+T here) for video only and Shift-T for audio only.

**Fix:** swapped in `keymaps/resolve.toml`, both marked verified.

### D-051 — The keymap checked against the 21.1 Reference Manual: 69 of 78 bindings confirmed
**Area:** ui · **Status:** active · **Doc:** `keymaps/resolve.toml` (its header); HANDOFF §4

**Decision:** `verified = true` now marks a key the DaVinci Resolve 21.1 Reference Manual gives
for that action. The header said a real install; HANDOFF's step was the manual first, then the
install for what the manual leaves out (D-017). `resolve_version` is now 21.1.

**How:** a subagent read the manual's text (`target/resolve-manual/manual.txt`) for each of the
78 bindings and reported each one with line numbers. Nothing was copied into the repo. I checked
its two discrepancies and a sample of its confirmations against the text myself (transitions,
the Color page's node keys, Delete and Forward Delete, Split).

**Result:**
- 67 confirmed as they were, and the two transition keys once swapped (B-039): 69 verified.
- Not settled by the manual, so not verified:
  - the page keys Shift+2 to Shift+8: the manual says Shift and the page's number in page order,
    but its list did not survive extraction, and 21.1 adds a Photo page whose key it doesn't give;
  - Mod+B: the manual gives Command-B as Razor (in the Fairlight chapter), which cuts at the
    playhead as Split Clip (`Mod+\`, verified) does;
  - Shift+Backspace for ripple delete: not in the manual.

**Found on the way:**
- **The keymap is not page-scoped,** but Resolve's is. Shift+S is Add Before Current on the
  Color page and Audio Scrubbing on the Edit page; Alt+K, Mod+D and Mod+T differ the same way.
  Today each key has one binding and the Color page's commands act only on that page, so nothing
  clashes yet. The Edit page's meanings of these keys need page-scoped bindings first. That is a
  change to the keymap's format, left for Truman to decide.
- **Keys the manual gives that have no command yet:**
  - Shift+I and Shift+O, go to In and Out;
  - Shift+Left and Shift+Right, a second at a time;
  - V and Shift+V, select the nearest edit or the clip at the playhead;
  - comma and period, nudge;
  - Shift+[ and Shift+], trim to the playhead;
  - E, extend the edit;
  - Mod+Shift+X, ripple cut;
  - Mod+M, Shift+M and Alt+M, markers;
  - Shift+J, Shift+K and Shift+L, play speeds;
  - Alt+1 to 8, destination tracks.

  Each needs a new vocabulary id first.

### D-052 — The Gradient window: the picture divided by a line, the side above it keyed
**Area:** color · **Status:** active · **Doc:** `crates/grade` (`Shape::Gradient`);
`crates/renderer/src/render.wgsl` (`coverage`); `crates/filmcraft-adapter/src/color.rs` (`mask`);
`apps/resolution-app/src/color_page.rs`

**Decision:** Resolve's fifth window, the Gradient. The Reference Manual describes it as a
two-handled control dividing the screen in two, with centre, angle and feathering, good for
skies. Ours:
- **The shape:** `Shape::Gradient`, an added value. Grades stored before it load and save
  unchanged.
- **What it keys:** the side above a line through the window's centre at Rotate 0. Rotate turns
  it clockwise, so at 90 the right side is keyed.
- **Softness** is the width of a ramp across the line, as a fraction of the picture's width,
  centred on the line as the other windows' edges are. New gradients start at 0.2, since a
  gradient is usually soft. Its size is not used.
- **On the GPU:** the window's distance from its edge is the turned y.
- **In the compile:** a rectangle on the keyed side reaching four picture-sizes past the line.
- **On the page:** an Add Gradient button in the Window palette, which hides Size and Aspect for
  a gradient. The viewer shows the gradient's line across the picture, a ring at its centre and a
  tick towards the side it keys. Drag on the keyed side to move it.

**Not yet:** dragging the line's handles to turn it or change its softness on screen. Rotate and
Softness are numbers in the palette.

**Checked:**
- Renderer test: above the line black and below it untouched; a quarter turn makes it the right
  side; soft, a rising ramp half way at the line. With the keyed side flipped in the shader, it
  fails.
- Compile test through FilmCraft's render: the same halves. Flipped in the compile, it fails.
- The GPU against the compile: hard, mean 0.14 code values, 99 % within 1.0; soft, see B-040.
- Grade test: written as `"gradient"`. UI test: Add Gradient, without Size and Aspect.

### B-040 — Soft windows in the Lumetri compile stray from the grade's math at their soft edge
**Area:** color · **Status:** known; the compile only (D-044) · **Caught by:** test (the Gradient
window's GPU-against-compile test, D-052)

**Symptom:** a soft Gradient (softness 0.2, Gain 0.4) on the demo frame: the GPU against
FilmCraft's render of the compile, mean 2.44 code values, 99 % within 22.0. Hard, they agree
(0.14 and 1.0).

**Cause:**
- **The blend:** FilmCraft applies an effect through a mask by blending the original and the
  effected picture in linear light. The grade blends its display-encoded signal (`Node::apply`).
- **The ramp:** FilmCraft's feather falls off along a smoothstep, centred on the edge as ours is,
  where the grade's ramp is straight.

Switching the shader's ramp to a smoothstep only took the 99 % figure from 22 to 18, so most of
it is the blend, which the compile cannot change. The experiment was not kept.

This applies to the soft edge of every window shape since D-031, measured here on a gradient. It
matters only where the compile is what's shown: FilmCraft, and renders without a GPU.

**Not fixed:** the GPU path is the grade's own math, the reference. The test keeps both figures
(hard agreeing, soft within its measured bound), so a change in either shows.

### D-053 — The Polygon window: corners in the window's own space, edited on screen
**Area:** color · **Status:** active; the point format is provisional, for Truman's review ·
**Doc:** `crates/grade` (`Window::points`, `MAX_POLYGON_POINTS`); `crates/renderer`
(`polygon_distance`); `crates/filmcraft-adapter/src/color.rs` (`mask`);
`apps/resolution-app/src/color_page.rs` (`window_overlay`)

**Decision:** Resolve's Polygon window. The Reference Manual describes it as a four-point shape
that grows corners, up to 128, which are dragged on screen. Clicking its surface adds a corner;
middle-clicking one removes it.

**The format (provisional):** `points` on a window, written only for polygons, so older grades
load and save unchanged. Each corner is in the window's own space: -1 to 1 across its size,
before Rotate and placing. A Linear window's corners are ±1 in the same space. So Pan, Tilt,
Size, Aspect and Rotate move a polygon as they move the other shapes. This is a new field in a
saved grade, so it is marked for review. The other way to store corners is as fractions of the
picture, placed directly; then the palette's transforms would not apply to a polygon.

Corners are held to ±4, at most 128. Fewer than three give the rectangle back, and other shapes
keep none.

**How:**
- **GPU:** each polygon's corners, turned and placed in the picture's pixels, follow the node
  tables in the same storage buffer. The window's slot in the node table holds where. The shader
  takes the signed distance to the nearest side, inside by even-odd, with the softness centred on
  the edge as for the other shapes.
- **Compile:** `MaskPath::polygon` of the corners, placed and turned as the other shapes are.
- **On screen:** the chosen polygon shows its corners as handles. Dragging a corner moves it, and
  dragging elsewhere inside moves the window. A click on the surface adds a corner on the nearest
  side, where the click falls across it. A middle click removes a corner while more than three
  remain.

**Simplified from Resolve:** one softness, centred on the edge. Resolve has an inside and an
outside softness for polygons.

**Checked:**
- Grade test: a new polygon has four corners; the JSON name; a circle writes no points; two
  corners give the rectangle back; 200 are cut to 128; a circle keeps none.
- Renderer test: a triangle, black inside and untouched outside, and turned half way. With the
  inside test inverted, it fails.
- Compile test through FilmCraft's render: the same triangle. Compiled as its bounding rectangle,
  it fails.
- The GPU against the compile, a turned five-cornered polygon: mean 0.06 code values, 99 % within
  1.0.
- UI test: Add Polygon, a corner dragged out, a click on the surface adds a fifth on the bottom
  side, and a middle click removes it.

### D-054 — The action journal's core: the engine thread notes every command; `--journal` writes it to a file
**Area:** tools · **Status:** active; its panel and agents' access wait for Truman · **Doc:**
`apps/resolution-app/src/journal.rs`; `worker.rs`; HANDOFF §2

**Decision:** D-034's journal, the part that needs no new command id:
- **What it notes:** the engine thread notes every command it runs, with who sent it, when it
  started, how long it ran, and how it ended (its undo step, or its error).
  - From the user: every vocabulary command.
  - From agents: their commands, undo, redo and frame renders, but not their polling.
  - The user's accepting or rejecting an agent's draft, described in brackets.
- **Where:** the newest 2000 entries are kept in memory. With `--journal <file>`, each start and
  each end is also written as a line of JSON and flushed at once. A freeze leaves a start without
  an end, and the file stays current if the editor stops. Parameters are written in full to the
  file and cut to 400 characters in memory.

**Why only the core:** a panel showing it needs a front-end id to open it, and agents reading it
over MCP need a query id or a tool parameter. Both are stable API (invariant 4), so they are
options for Truman (the overnight run's rule):
1. `workspace.journal` (a floating panel, live, with Save) and `journal.read` (a query: the last N
   entries, or those since a number);
2. the same panel without an id, as a debug tool, and agents given the file's path instead;
3. no panel: the file only.

The first is the recommendation: it keeps every action a command, and agents debugging the editor
see what the user sees.

**Checked:**
- Unit tests: a start and an end noted as they happen, in memory and as lines of the file, an
  unfinished command left open, an error, the newest 2000 kept, long parameters cut.
- UI tests: the demo's opening and two commands noted as the user's, one ending in an error; an
  agent's `color.adjust` noted as the agent's, with its undo step. With the user's commands noted
  as the agent's, the test fails.
- A two-second `--play` of the demo on the second screen with `--journal`: the file has the
  demo's opening (3 ms) and the stop's `playhead.set`, each with its start and end.

### D-055 — Primaries: Shadows, Highlights and Midtone Detail
**Area:** color · **Status:** active · **Doc:** `crates/grade` (`shadows_highlights`,
`Node::detailed`, `Node::is_spatial`); `crates/renderer` (the grade stage, `fs_luma`, `fs_node`);
`apps/resolution-app/src/color_page.rs`

**Decision:** three of the Primaries palette's adjustment-bar controls Resolve has and we did not,
each -100 to 100 with 0 neutral, as the Reference Manual gives them. Each is a new node field,
written only when set, so older grades load and save unchanged.

- **Shadows and Highlights** move the darkest and the lightest tones while black, white and mid
  grey stay.
  - Below mid grey, a tone moves by Shadows / 100 × 0.5·t·(1 − t)², with t its distance from
    black over the distance to mid grey. That is most a third of the way up: 0.074 at full
    Shadows. Above mid grey, the same mirrored from white, by Highlights.
  - Every value in range keeps the tone function rising.
  - They are part of each channel's tone function, after Contrast and before the curves. So the
    GPU's tables, the Lumetri compile and the math all have them with nothing more.
  - Our formulas follow the manual's description; Resolve's are unknown.
- **Midtone Detail** is local contrast.
  - The node's input luma minus that luma blurred (a Gaussian, 10 pixels at 1920 wide, in
    proportion) is added to each channel by Midtone Detail / 100, weighted towards the midtones by
    4·l·(1 − l).
  - Lowered, it pulls towards the blur, less where there is more detail and not at all past 0.08,
    so low-detail regions soften and edges stay, as the manual says.
  - It needs a pixel's neighbours: it runs in the GPU's grade stage (D-046), as a luma pass and
    two blur passes before the node's pass. It is left out of `Node::apply` (pixel by pixel), of
    the Lumetri compile and of a qualified node's table, as Denoise is.
- **On the page:** Mid/Detail (Resolve's label) at the end of the top bar; Shadows and Highlights
  between Color Boost and Saturation on the bottom one; Reset Primaries resets them too.

**Not built:** Lum Mix. It sets how Y-only and YRGB adjustments mix, and our wheels' master and
channel adjustments are not kept apart that way.

**Checked:**
- Grade tests: black, white and mid grey kept and the tone rising at every extreme; the peak
  value; lowered Highlights darken near white. Midtone Detail sharpens an edge and leaves flat
  regions; per pixel it does nothing; lowered, it softens a faint ripple and keeps the strong edge.
- Renderer tests: Shadows and Highlights on noise within 1 code value of the math. Midtone Detail
  at ±80 against `Grade::apply_image` within 0.94 and 1.01, and clearly different from the grade
  without it. With its sign flipped in the shader, the test fails.
- Compile test: Shadows and Highlights through FilmCraft's render, within 1 code value.
- UI test: the three dragged through the engine.
- By eye on the second screen: the Sony clip with Shadows 60, Highlights -40 and Midtone Detail
  80: shadow detail lifted and edges crisper, subtle as they should be; the bars' labels fit.

### D-056 — Deliver's speed checked after B-038's fix: unchanged
**Area:** color · **Status:** active · **Doc:** B-038

**Decision:** keep `apart` (B-038). It starts a thread for each export frame and runs the
frame's work on a pool of its own, so its cost needed measuring.

**Measured, 2026-10-14 (overnight), this PC:** 10 s (241 frames) of the Sony clip graded
(Saturation 70, Gain 1.1, Lift 0.02) to H.264 1080p through the CLI, three runs each:

| Build | Export |
|---|---|
| main, with `apart` | 17.1 to 17.3 s |
| main, `apart` bypassed | 17.1 to 17.4 s |
| D-045's commit, before B-038 | 17.6 to 17.9 s |

`apart` costs nothing measurable. All three are slower than D-045's 13.6 s for "the graded Sony
clip". That figure's exact grade is not recorded, and other programs (OBS, Reaper) were running
tonight. D-045's own build was as slow, so the gap is not a regression.

### D-057 — Window handles on screen: size, rotation and softness
**Area:** ui · **Status:** active · **Doc:** `apps/resolution-app/src/color_page.rs` (`handles`,
`dragged`, `window_overlay`)

**Decision:** the chosen window's on-screen handles, in the spirit of Resolve's (the manual's
"Onscreen controls for window transforms"), each changing the numbers the Window palette shows:
- **Width, Height, Scale** (squares, for Circle, Linear and Polygon windows): the right side's
  middle sets the width, the top's the height, and the bottom right corner scales both, keeping
  the aspect. Each sizes the window symmetrically about its centre.
- **Rotate** (a dot, 28 pixels above the window; for a Gradient, at the tip of its tick): its
  angle from the centre is the rotation, 0 straight up and clockwise.
- **Softness** (a ring below the window, 14 pixels plus half the softness below its edge;
  for a Gradient, below its line): dragged down, softer.

Every handle follows the window's rotation. A polygon's corners come first, then the handles,
then dragging inside a window to move it.

**Not as Resolve:** one softness handle rather than Resolve's several magenta ones per side, and
no corner-pinning of a Linear window, whose model is a turned rectangle.

**Checked:**
- Unit test: each size and softness handle, dragged out by a third, gives the window that puts
  it under the pointer (or softer); the rotate handle to the right of the centre is a quarter
  turn.
- UI test: a circle widened, softened and turned through its handles, through the engine.

### B-041 — A quick drag on the window control missed small handles and polygon corners
**Area:** ui · **Status:** fixed · **Caught by:** test (D-057's UI test)

**Symptom:** a drag starting on the softness handle did nothing. The test's drag moves 5 pixels a
step.

**Cause:** the on-screen control looked for what a drag took hold of at the pointer's position
when egui started the drag. egui starts a drag only once the pointer has moved a few pixels, so
on a quick drag that position was about 10 pixels from where the button went down, past the
handles' 8-pixel reach. D-053's polygon corners had the same flaw.

**Fix:** what a drag takes hold of is found at the press's origin (`press_origin`). The UI test
passes, and failed before the fix.

### D-058 — More matte finesse: Black Clip, White Clip and Morph
**Area:** color · **Status:** active · **Doc:** `crates/grade` (`Qualifier::clip`, `Morph`,
`Qualifier::matte`); `crates/renderer` (`fs_morph`, `fs_clip`); `color_page.rs` (Matte Finesse)

**Decision:** three more of the Qualifier palette's Matte Finesse controls, as the Reference Manual
describes them. Each is a new qualifier field, written only when set, so older grades load and
save unchanged.

- **Morph Operation and Morph Radius.**
  - Shrink takes each pixel's least neighbour, and Grow its most, over a square of the radius.
  - Opening is shrink then grow: specks smaller than the radius go and the rest stays. Closing is
    grow then shrink: holes smaller than it fill.
  - The radius is up to 10 pixels at 1920 wide, in proportion at other widths, at least one when
    set.
  - It is spatial: a min or max pass across and one down in the GPU's grade stage, per step.
- **Black Clip and White Clip,** 0 to 100 (White Clip 100 when unset): levels on the finished
  matte, its black point raised and its white point lowered.
  - They work pixel by pixel, so `Node::apply` and a qualified node's baked table have them.
  - In the shader they put the layer through the grade stage, since the node table has no room
    left for them.
- **The order:** the key, Denoise, Clean Black and White, Morph, Blur Radius, then Clip.
- **On the page:** Morph Radius, Black Clip and White Clip join the Matte Finesse column, in that
  order, with a Morph Operation button below that steps through None, Shrink, Grow, Opening and
  Closing.

**Not built:** In/Out Ratio. It moves where a blurred key's edge falls (or, without blur, fills
holes or removes specks), and no simple, faithful form of it is clear from the manual. Morph's
Opening and Closing cover its second use.

**Checked:**
- Grade test: Shrink takes off a speck and an edge's pixel; Grow widens the edge and fills a
  2-pixel hole; Opening takes off the speck and keeps the block; Closing fills the hole and keeps
  the edge; Clip's levels; a qualifier without these fields writes none.
- Renderer test: Opening and Closing with both clips, against `Grade::apply_image`, within 0.24
  and 0.52 code values. With min and max swapped in the shader, 148 off.
- UI test: Morph Operation stepped to Shrink; Morph Radius and Black Clip dragged, through the
  engine.

### D-059 — Replace, Place on Top, Ripple Overwrite and Append at End, composed from FilmCraft's commands
**Area:** engine · **Status:** active · **Doc:** `crates/filmcraft-adapter/src/edits.rs`;
`map.rs`

**Decision:** four of Resolve's source edits that the map had as missing, built in the adapter.
Each is composed from FilmCraft's commands and run as one undo step (`Session::edit_group`, which
also rolls the whole edit back if a part fails). Their vocabulary ids and keys were already there
(F11, F12, Shift+F10 and Shift+F12, confirmed against the manual in D-051).

- **Replace (F11):** the clip under the playhead on the video destination track takes the source
  clip, keeping its place and length. The source playhead's frame is lined up with the timeline
  playhead's. That is FilmCraft's `clip.replaceFromSourceMatchFrame`, given that clip and its
  linked clips.
- **Place on Top (F12):** the source's In to Out goes at the timeline's In, or the playhead.
  - The video goes on the first video track above the video destination that is free over it,
    and the sound on the first audio track after the audio destination that is free.
  - A track is added where none is.
  - The destinations are moved for the overwrite and put back.
- **Ripple Overwrite (Shift+F10):** the clip under the playhead, with its linked sound, is
  ripple-deleted, and the source is inserted where it began. What follows moves up or back by the
  difference.
- **Append at End (Shift+F12):** the source goes after the last clip on any track. The timeline's
  In and Out are set aside for the edit and kept.

**Not built:** Fit to Fill (Shift+F11) needs the clip's speed set so its In to Out fills the
timeline's; it stays in the map as missing. The vocabulary now has 83 commands running headless,
32 front-end, 2 missing.

**Limits:**
- Ripple Overwrite meets B-023: FilmCraft refuses a ripple across a sync-locked track that has a
  clip in the way, such as the demo's music on A2, so it fails there as Ripple Delete does.
- Replace takes the clip under the playhead, not a timeline In to Out.

**Checked:**
- Adapter tests on the demo, with two seconds of City_Night_Drive marked in the source:
  - Append at End: after the last clip, the rest of V1 as it was, the timeline's In kept, one
    undo step.
  - Replace: the clip takes the source, in its place, at its length.
  - Ripple Overwrite (A2's sync lock off): Misty Forest gives way to the source, the next clip
    moves back by the difference and stays whole, and on A1 the source's sound replaces the old
    clip's; one undo step. With an overwrite in place of the insert, the test fails: the next
    clip loses two seconds.
  - Place on Top: V2 at 1.5 s, V3 the second time, a new track the third; the destinations
    restored.
- UI test: Shift+F12 appends on V1 and F12 places the whole clip on the first free track above.

**Wrong turn:** the UI test first expected F12's clip on V2. The whole source, 14 s, meets V2's
clip, so it rightly went to V3. The test now looks above V1.

### D-060 — Fit to Fill: the source retimed to fill the timeline's In to Out
**Area:** engine · **Status:** active · **Doc:** `crates/filmcraft-adapter/src/edits.rs`

**Decision:** the last of D-059's edits, Resolve's Fit to Fill (Shift+F11). The source's In to Out
takes the timeline's In to Out on the destination tracks, its speed changed to fit: slower when
the timeline's range is longer, faster when shorter. It is one undo step (`edit_group`), in this
order:

1. The timeline's In to Out is lifted, on the destination tracks only. FilmCraft's lift takes
   every targeted track, which would cut other tracks too, such as the demo's music. The Out
   frame is in the range, as FilmCraft takes it.
2. The source is placed after the last clip, where its changed length meets nothing.
3. It and its linked sound get the speed (`clip.speedDuration`).
4. It is moved into the gap, its sound with it (`timeline.move`, linked).

The timeline's In and Out are kept, and the playhead ends after the edit. Fit to Fill needs a
timeline In and Out; without both it says so. The vocabulary now has 84 commands running
headless, 32 front-end, 1 missing (dynamic trim).

**Checked:** adapter test on the demo, 48 source frames into 84 timeline frames (57 %) and into
24 (200 %):
- the clip in place at the timeline's In, its length the range's within a frame, its speed;
- the clip it was cut into kept before it, and everything after as it was;
- the music on A2 untouched, nothing left past the end, one undo step.

With the speed inverted, it fails.

### D-061 — The Curve window: a closed Catmull-Rom curve through its points
**Area:** color · **Status:** active; its format is D-053's, provisional · **Doc:** `crates/grade`
(`curve_spans`, `curve_corners`); `crates/renderer` (`grade_data`); `color.rs` (`mask`);
`color_page.rs`

**Decision:** Resolve's fifth window, as a closed smooth curve through its points. It shares the
Polygon's `points` (D-053), and so the same provisional format, and the same editing on screen:
drag a point, click the surface to add one on the nearest side, middle-click to remove one.

- **The curve:** each span between two points is a cubic Bézier, from one point to the next. At
  each point its tangent lies along the line from the point before to the point after, a sixth of
  that line's length on either side. That is a closed Catmull-Rom spline.
- **On the GPU:** the spans flattened into at most 256 corners (16 a span at most), drawn as the
  polygon of D-053.
- **In the compile:** FilmCraft's mask takes the same Bézier spans as smooth vertices, so both
  draw the same curve.
- **A new curve** starts on the Polygon's four corners, a rounded shape.

**Not as Resolve:** Resolve draws a curve from nothing, a click at a time, with Bézier handles to
drag. Ours are always the spline's; they could be stored on the points later.

**Checked:**
- Grade test: the curve goes through its points and closes; the tangent; through a square's
  corners it bulges a quarter past each side; 100 points share 200 corners; the JSON name; a
  curve keeps its points.
- Renderer test: black inside the curve, including in the bulge outside the square, and untouched
  beyond it. Drawn as a plain polygon, the bulge test fails.
- The GPU against the compile, a turned five-point curve: mean 0.07 code values, 99 % within 1.0.
- UI test: Add Curve, a point dragged out.

### D-062 — The Highlight view for clips graded at their source, and frames FilmCraft draws itself
**Area:** color · **Status:** active · **Doc:** `crates/filmcraft-adapter/src/gpu.rs` (`Grading`,
`GradedSource`, `graded_picture`, `render_cpu`)

**Decision:** two of D-048's limits gone. A clip whose pictures are graded at their source shows
the Highlight view too: one with other effects (D-044), and every graded clip in a frame
FilmCraft draws itself.

**How:** the stand-ins' source provider (`Grading`) now carries the highlight with each frame,
on the GPU path and on FilmCraft's own one (`render_cpu`, which playback's CPU fallback now asks
with the frame's hint). The highlighted clip's `GradedSource` draws its pictures through the
renderer's highlight stage (D-048), rather than the plain grade. FilmCraft then lays the clip's
other effects over the highlighted picture, as over the graded one: the drop shadow stays.

**Still not:** a clip with no stand-in (nothing on its grade), and agents asking for it (an MCP
parameter, for Truman).

**Checked:** adapter test, V2's Misty Forest (graded at its source, under its drop shadow) in
black and white with a node that keys everything:
- on the GPU path and in FilmCraft's own render, over 1000 pixels changed, at least nine in ten
  of them white;
- with the highlight not handed to the source, nothing changed and the test fails.

### B-042 — The timeline did not show transitions
**Area:** ui · **Status:** fixed · **Caught by:** rehearsing the one-minute edit (below), then
by eye on the second screen

**Symptom:** a dissolve added with Add Video Only Transition was there in the engine's timeline,
but the Edit page drew nothing. The demo's four transitions on V1 were never shown either.

**Cause:** the model read each track's clips but not its transitions, and the timeline drew none.

**Fix:** each track keeps its transitions (start, end, the effect's name: "Cross Dissolve" for
`cross_dissolve`). The timeline draws them over their cuts: a box with a diagonal, named when
there is room. The box sits in the lower part of the lane so the clips' names stay readable.
Seeing them is all for now: selecting a transition to change or delete it is not built.

**The rehearsal:** the one-minute edit (ROADMAP step 2's bar) run through the engine with the
Sony and Samsung clips (`scripts/rehearse-edit.py`):
- a new timeline at 23.976;
- ranges marked in the source viewer, then Insert, Overwrite, Append at End, Place on Top (a
  cutaway on V2, its sound on A2) and an Insert mid-timeline;
- two splits and a ripple delete of the second between;
- a 12-frame ripple trim and a 6-frame roll, both by the frame;
- a dissolve, a marker, undo and redo;
- Deliver to H.264 1080p.

All 19 steps worked. A1 followed V1 exactly. The file is 799 frames, 33.3 s of 1920 × 1080 H.264
with AAC sound, as long as the timeline. Only the missing transitions turned up.

**Checked:** UI test: the demo's four V1 transitions in the model, the dissolve at 4.46 to 5.46 s.
By eye: the demo's transitions on V1 and its audio crossfade on A1, the clips' names readable.

### D-063 — Inside and Outside Softness for Polygon and Curve windows
**Area:** color · **Status:** active; the two field names are provisional, for Truman's review ·
**Doc:** `crates/grade` (`Window::edge_softness`); `crates/renderer` (`grade_data`, `coverage`);
`color.rs` (`mask`); `color_page.rs` (`window_fields`, the soft handle)

**Decision:** a Polygon's or a Curve's soft edge has two widths, as in Resolve: how far it reaches
into the shape (Inside Soft) and how far out of it (Outside Soft). Before, it had one softness,
centred on the edge.

- **In the grade:** a window gains `inside_softness` and `outside_softness`, fractions of the
  picture's width like `softness`. Each is optional and written only when set. Without one, it is
  half the softness, so every window stored before draws as it did.
- **Other shapes:** a Linear, Circle or Gradient window keeps its one softness. Its two fields are
  dropped when it is loaded.
- **On the GPU:** the polygon path puts the two widths in the window slot's size floats (o+3 and
  o+4), which its corners make unused. The ramp runs from full at the inside width to none at the
  outside width; with both zero, it is hard.
- **In the compile:** FilmCraft's feather is centred on the edge, so it takes the sum of the two.
  Its expansion moves the centre out by half the outside width less half the inside width.
- **In the palette:** a Polygon or Curve shows Inside Soft and Outside Soft in place of Softness
  (two rows of four numbers). The soft handle below the window shows the Outside Soft and sets it.

**Provisional:** the field names are an addition to the grade's JSON. Optional fields are cheap
to rename now (a serde alias keeps old files loading) and harder once projects carry them.

**Not as Resolve:** the numbers are in percent of the picture's width, not Resolve's scale. There
is no handle for the Inside Soft; it is set by its number.

**Checked:**
- Grade test: half the softness each way by default; not written until set; one set leaves the
  other at half; kept in range on a node; dropped from a circle.
- Renderer test: a square 20 pixels soft all outside is black to its edge, half at 10 pixels out
  and untouched from 20; all inside, the mirror of that.
- The compile: the feather and expansion of both widths, either way round.
- The GPU against the compile, a five-point polygon soft all outside and all inside: mean 0.78
  and 0.65 code values, 99 % within 14 and 17. They differ along the ramp as soft Gradients do
  (B-040). With the compile's ramp left centred on the edge, it is 5.0 and 74.
- UI test: the palette's fields for each shape; the two fields set the window's own; the soft
  handle sits at the Outside Soft and dragged lower reaches further, the rest kept.
- Mutation checks: each test fails with its part broken (defaults, clamping, the shader's ramp,
  the widths swapped, the expansion reversed, the handle, the fields).

### D-064 — Dynamic Trim mode: what it takes, and two ways to build it
**Area:** edit · **Status:** proposed, for Truman; not built, since it needs new vocabulary ids ·
**Doc:** `mode.dynamicTrim` (the vocabulary's last Missing command, `map.rs`); FilmCraft's
`crates/engine/src/trim.rs`

**What Resolve does** (21.1 Reference Manual, "Dynamic Trimming Using JKL Controls"):
1. Select one or more edit points or clips. With none selected, the edit nearest the smart
   indicator is used.
2. W turns the mode on. The playhead turns yellow while it is on, and J, K and L trim instead of
   playing.
3. The mode trims edit points, or it slips or slides clips (chosen from the tool's menu).
4. U steps through the edit point's types.
5. J and L trim while the program plays, with sound from every track, faster when pressed again.
   K with J or L trims slowly, and K while tapping J or L trims a frame at a time.
6. Space plays around the edit. Up and Down move to another edit, and W turns the mode off.

**What Resolution lacks:**
- **Edit-point selection.** Today a trim is a mouse drag sent as one `timeline.trimEdge` or
  `timeline.rollEdit`. Nothing selects an edit and keeps it selected. The same gap limits Add
  Transition, which `map.rs` marks Approx for this reason, and keyboard trimming with nudges.
- **Ids for selecting edits.** The vocabulary has none for selecting the nearest edit (V), for
  stepping its type (U) or for shuttling it.

**What FilmCraft has:**
- Selected edit points: `trim.selectEditPoint`, `trim.selectNearest` and `trim.toggleType`.
- Dynamic trimming: `trim.shuttle` (J, L), `trim.shuttleStop` (K, which commits the whole trim as
  one undo step), `trim.cancelDynamic` and `trim.playAround`.
- `trim.tick {clock}`, which advances a dynamic trim and is called once a frame.

The timeline changes live while it runs.

**Option A, the engine's (recommended):**
- New ids map onto FilmCraft's:
  - `timeline.selectEditPoint {clip, edge, kind, add?}` (a click on an edge);
  - `timeline.selectNearestEdit` (V);
  - `timeline.cycleEditPointType` (U);
  - `trim.shuttle {direction, slow?}`, `trim.stop` and `trim.cancel`.
- `mode.dynamicTrim` is the front end's toggle, and routes J, K and L to the trim ids while it is
  on.
- The adapter ticks the trim from its own clock, as it runs playback, so the journal and agents
  see one command a key press, not one a frame.
- It is what Resolve does: the cut plays as it moves, with sound.
- The cost is the largest: selection drawn on the timeline, the mode, the routing, and the clock
  tied to playback.

**Option B, the front end's:**
- J and L move the selected edge as a keyboard drag, previewed as a mouse trim is.
- K sends one `timeline.trimEdge`, `rollEdit`, `slipClip` or `slideClip` with the total, so only
  the edit-point ids are new.
- It is smaller, but the cut does not play while it moves. That loses the point of the feature,
  trimming while watching the program play.

**Either way, first:** edit-point selection with its ids (select, nearest, type), drawn green on
the timeline as Resolve draws it. Add Transition, nudge trims (`,` and `.`) and Extend Edit
(E) need it as well. Dynamic trim builds on that afterwards.

**Waiting on Truman:**
- The ids above: their names, their parameters, and whether edit points are engine state, as
  FilmCraft keeps them, or the front end's.
- A or B.

### B-043 — Ctrl+T added no transition from the editor
**Area:** engine · **Status:** fixed · **Caught by:** the one-minute edit rehearsed through the
editor's keys and mouse (a new UI test, below)

**Symptom:** Ctrl+T (Add Transition) at a cut did nothing in the editor. Alt+T and Shift+T (video
only, audio only) worked.

**Cause:** `timeline.addTransition` was mapped (Approx) to FilmCraft's
`trim.applyDefaultTransition`, which acts only on selected edit points. The editor cannot select
edit points (D-064), so the command always failed. The map's note recorded the difference, but
not that it left the key with nothing to act on.

**Fix:** Add Transition is now one of the adapter's composed edits (D-059's `edits.rs`). It runs
FilmCraft's video and audio transition commands together, as one undo step named Add Transition.
Each puts its transition where Alt+T and Shift+T put theirs:
- at the selected clips' edges;
- else at the selected edit points;
- else at the edit within three frames of the playhead on the targeted tracks.

It fails only when neither can be placed, and then nothing changes. Resolve's manual has Ctrl+T
add a video and audio transition at the selected edit points or clips, with V to select the
nearest edit. Here the playhead stands in for V until edit points can be selected.

**Seen on the way:** FilmCraft leaves a clip selected after inserting it. Alt+T or Ctrl+T straight
after an insert so puts transitions at both of the new clip's edges. Whether Resolve selects an
inserted clip is not checked.

**The rehearsal:** a UI test cuts the one-minute edit (ROADMAP step 2's bar) on the demo through
keys and the mouse only, as `scripts/rehearse-edit.py` does through the engine (B-042):
1. Ctrl+N for an empty timeline.
2. Source ranges marked with the arrow keys, I and O. F9 inserts the first. The playhead moves to
   the end of the edit, as in Resolve, so F10's overwrite follows on. Shift+F12 appends a third.
3. Q to the timeline, then Home, and Down to the first cut. Two splits with Ctrl+B half a second
   apart; the piece between is clicked and ripple-deleted with Shift+Backspace, leaving no gap.
4. The last clip's end dragged earlier with the selection tool.
5. Ctrl+T at the first cut, M for a marker, Ctrl+Z and Ctrl+Shift+Z.
6. A1 cut as V1 is.

Only Ctrl+T failed.

**Checked:**
- Adapter test: on a timeline of two clips with nothing selected, Add Transition a second from the
  cut fails and changes nothing. Two frames from it, it adds one video and one audio transition
  as one undo step named Add Transition, which one undo removes.
- The UI test, as above.
- Mutation checks:
  - without the audio transition, without the undo group, and with the old mapping, the tests
    fail;
  - a plain Backspace (delete, leaving a gap) fails the rehearsal's no-gap check.

### D-065 — A Linear window's Soft 1 to 4: each side's own softness
**Area:** color · **Status:** active; which number is which side, and the field's name, are
provisional, for Truman's review · **Doc:** `crates/grade` (`Window::side_softnesses`);
`crates/renderer` (`grade_data`, `coverage`); `color.rs` (`mask`); `color_page.rs`
(`field_cells`, `Handle::Side`)

**Decision:** a Linear window's soft edge can differ on each side, as in Resolve. The 21.1
Reference Manual ("Window Softness") gives a Linear window four parameters, Soft 1 to 4, one for
each side, with a control point on the top, bottom, left and right.
- **In the grade:** `side_softness`, four fractions of the picture's width for the window's top,
  bottom, left and right. They are its own sides, turned with it. The field is optional and
  written only when set; without it, every side has the softness, so stored windows draw as they
  did. Other shapes drop it when loaded.
- **On the GPU:** the four widths sit in the tables, where a Polygon keeps its corners, and the
  window slot points to them. Each side's soft edge is centred on it. Where two meet, the lesser
  coverage wins, which is what one softness did, so four equal sides draw exactly as before.
- **In the compile:** FilmCraft's mask has one feather, so it takes the mean of the four. Like a
  soft Gradient's (B-040), FilmCraft's own render differs at the edges; Resolution's renders
  draw each side.
- **In the palette:** Soft 1 to 4 replace Softness for a Linear window, on a row of their own. Its
  numbers take three rows; the other shapes keep two, since their softness fits on the second.
  Opacity now comes before the softness on the second row.
- **On screen:** a ring beside each side, as far out as that side's soft edge reaches. Dragged,
  each sets its own side. The top one sits halfway towards the right corner, clear of the rotate
  handle.

**Provisional:** the manual does not say which number is which side. Soft 1 to 4 are the top,
bottom, left and right here, the order the manual names the control points in. Resolve's palette
will show the real order: drag one side's control point and watch which number changes.

**Found in the manual on the way** (not changed):
- Resolve's Polygon has no on-screen softness control. D-063 gave ours a handle for its Outside
  Soft.
- Resolve's Curve has inside and outside softness control points at each point, "creating any
  softness shape you need". Ours has one inside and one outside softness for the whole curve.

**Checked:**
- Grade test: the softness on every side by default; not written until set; kept in range; dropped
  from a circle, a polygon and a gradient.
- Renderer test: a rectangle soft only on its left is half way at that edge, black 20 pixels in
  and untouched 20 out, and hard on the other three sides. Soft only on its top, likewise. Turned
  half round, its left side's softness is on the picture's right.
- Compile test: the feather is the softness, or the four sides' mean.
- UI test:
  - Soft 1 to 4 on a row of their own and no Softness for a Linear window; two rows for the
    other shapes;
  - a handle beside each side and none below; each, dragged 20 pixels further out on a turned
    window, softens its own side by 4 % of the width, keeps the other three, and is then where it
    was dragged to.
- The palette's third row measured in the UI harness at 1600 × 920: Soft 1 to 4 at y 679 to 701,
  under the palette's header at 515. Not looked at by eye.
- Mutation checks: each test fails with its part broken. The parts: the clamping, dropping the
  sides from other shapes, the default, the renderer's widths, the shader's left/right and
  top/bottom, the sides turning with the window, the compile's mean, the row, Softness hidden, a
  handle's side, and a handle's place.

### B-044 — The one-minute edit's UI test timed out in a full run
**Area:** tests · **Status:** fixed · **Caught by:** the full test run for D-065, twice in a row;
the test had passed on its own and in the run for its PR (#58)

**Symptom:** `the_one_minute_edit_through_the_keys_and_mouse` waited a minute for "the playhead
there" and failed, with the other app tests running beside it.

**Cause:** the test, not the editor. To reach a frame it pressed Right, stepped the UI once, and
read the playhead, again until the playhead got there. Under load the playhead lags a step behind
the presses, so the test pressed too often and overshot. It then waited for a frame the playhead
had passed.

**Fix:** it presses Right as many times as there are frames to go, then waits for the playhead.
The wait checks that each press moved it exactly one frame, none lost or doubled. Three full runs
of the app's tests passed.

### D-066 — Hue curves: image sampling from the viewer and the six-vector patches
**Area:** color · **Status:** active; the widths around a sample or a vector are provisional ·
**Doc:** `color_page.rs` (`curve_sampler`, `sampled`, `patched`, `samples`)

**Decision:** two of Resolve's ways to put points on a hue curve, from the 21.1 Reference
Manual ("Image Sampling for Hue and Sat Curves" and "Additional Controls in the Hue and Sat
Curves"):
- **Image sampling:** with any hue curve open, a click in the viewer, or a drag over a region,
  puts three points on that curve. They span what was sampled: its hues, saturations or
  lightnesses, whichever the curve is over.
  - The outer two sit just beyond the range: a 24th of the wheel over hue, 0.05 otherwise.
  - The middle one sits in its centre.
  - Over hue the range is the shortest arc holding every sample, so reds either side of 0 give
    a range through red.
  - A drag reads every pixel under it, or an even spread of 4096. A white outline shows the
    region while dragging.
- **The six-vector patches:** over hue, six colour buttons in the title strip (Red, Yellow,
  Green, Cyan, Blue, Magenta) each put three points round their vector. The middle point sits on
  the vector and the outer two half way to its neighbours.

Every new point sits on the curve as it is, so nothing changes until one is dragged; the curve's
own points stay. Both go through `color.adjust` as any curve edit does: no new vocabulary.

**Provisional:** the manual says three points but not how far apart. Ours are a guess, to be
compared with Resolve's.

**Not as Resolve:**
- The sampler reads the picture as the viewer shows it, after the grade, as the qualifier's
  picker does (D-032). Resolve samples relative to the node's input. On a node whose own hue
  curves already move a colour, the sample is of the moved colour.
- The manual's list of curves that sample leaves out Sat vs Lum; ours samples on all six.

**Checked:**
- Unit tests:
  - hues either side of red span the arc through red;
  - on a bent curve, its points kept and the new ones on it;
  - lightness and saturation ranges held inside 0 to 1, and nothing sampled adds nothing;
  - the Yellow and Red patches' points;
  - each curve reads hue, saturation or lightness;
  - a region reads only the pixels under it, a click one pixel.
- UI test: with Hue vs Sat open, a click in the viewer puts three points on it, the middle at the
  hue of the pixel under the pointer, and changes nothing. Then the Yellow button adds its three.
- Mutation checks, each caught:
  - the hue range the long way round;
  - the points off the curve;
  - an outer point on the wrong side;
  - a narrower patch;
  - Lum vs Sat reading hue;
  - the region ignored;
  - the sampler missing from the viewer;
  - the buttons a vector off.
- Not looked at by eye. At the UI tests' 1600-pixel window, the six buttons fit between the
  palettes' names and the curve menu, but only just.

### D-067 — The Primaries palette's Bars mode
**Area:** color · **Status:** active · **Doc:** `color_page.rs` (`primaries`, `bars`)

**Decision:** Resolve's Primaries palette has three modes, Wheels, Bars and Log (21.1 Reference
Manual, "Primaries Palette"). This adds Bars, chosen from a mode menu in the palette's title strip
("Wheels ▾" or "Bars ▾"); the title follows it ("Primaries - Color Bars").
- **The bars:** Lift, Gamma, Gain and Offset each get four vertical sliders, Y, R, G and B. They
  are the same numbers the wheels set: each wheel's master and its three channels. Neutral is at
  the middle, the change is filled from there, and the value is under the bar.
- **Using them:** drag a bar up to raise it; the bar's whole height spans twice the wheel's reach
  at its edge (Lift and Gamma ±0.5, Gain 0 to 2, Offset 0 to 50). Shift drags at a tenth of
  that. A double-click resets it.
- **Unchanged:** the bars above and below (Temp to Midtone Detail, Color Boost to Hue) are as in
  Wheels mode. Each change is a `color.adjust` of one channel, so no new vocabulary.

**Not as Resolve:** Resolve's Bars mode gives Offset "a slider and button-driven interface" for
printer points. Here Offset has the same four bars as the others. Log mode and Lum Mix are not
built.

**Learned on the way:** egui_kittest steps the UI a quarter second at a time, one queued event a
step. A double-click's two clicks then arrive half a second apart, past egui's 0.3 s, so no UI
test could double-click until now. `demo_stepping(dt)` builds the harness with a finer step, and
this test uses 0.05 s.

**Checked:**
- UI test, choosing Bars from the menu:
  - Gain's R bar dragged a quarter up raises red gain alone, by no more than half Gain's reach.
    egui drops the first points of a drag, so it is a little short.
  - Lift's B bar dragged down lowers blue lift.
  - A double-click puts red gain back to 1.
  - Back to the wheels.
- Mutation checks, each caught: a drag down raising, every bar setting Y, no reset, a menu that
  does nothing, the bars never drawn.
- Not looked at by eye. At 1600 × 920 a bar is 183 points tall.

### D-068 — Vocabulary additions proposed as one batch: edit points, Resolve's unbound keys, the Inspector
**Area:** engine · **Status:** proposed, for Truman; nothing built · **Doc:**
`crates/engine-api/src/commands.rs` (where they would go); D-051 (the keys), D-064 (dynamic trim)

**Why a batch:** the work left on the Edit page now needs new vocabulary ids, which are stable API
and Truman's to choose. D-051 listed Resolve keys with no command, and D-064 found that dynamic
trim and Add Transition wait on edit-point selection. The Inspector needs ids to set a clip's
properties. Approved together, a later run can build them all.

Each id below names the key it takes (21.1 Reference Manual, as D-051 checked it), and the
FilmCraft command it would map to (`map.rs`) or "adapter" where the adapter would compose it.
Names follow the vocabulary's own: `playhead.previousFrame`, `timeline.rippleDelete`.

**1. Edit points** (D-064's prerequisite):

| Id | Key | Maps to |
|---|---|---|
| `timeline.selectEditPoint {clip, edge, kind?, add?}` | a click on an edge | `trim.selectEditPoint` |
| `timeline.selectNearestEdit` | V | `trim.selectNearest` |
| `timeline.cycleEditPointType` | U | `trim.toggleType` |
| `timeline.nudgeBack`, `timeline.nudgeForward` | Comma, Period | `trim.backward`, `trim.forward` |
| `timeline.nudgeBackMany`, `timeline.nudgeForwardMany` | Shift+Comma, Shift+Period (5 frames) | `trim.backwardMany`, `trim.forwardMany` |
| `timeline.extendEdit` | E | `trim.extendToPlayhead` |
| `timeline.trimStartToPlayhead`, `timeline.trimEndToPlayhead` | Shift+[, Shift+] | adapter, from `timeline.trimEdge` |

**2. Dynamic trim** (D-064 option A): `trim.shuttle {direction, slow?}`, `trim.stop` and
`trim.cancel`, mapped to `trim.shuttle`, `trim.shuttleStop` and `trim.cancelDynamic`. W
(`mode.dynamicTrim`) stays the front end's toggle and routes J, K and L to them while it is on.

**3. The rest of D-051's keys:**

| Id | Key | Maps to |
|---|---|---|
| `playhead.goToIn`, `playhead.goToOut` | Shift+I, Shift+O | `markers.goToIn`, `markers.goToOut` |
| `playhead.previousSecond`, `playhead.nextSecond` | Shift+Left, Shift+Right | adapter (`playhead.set`) |
| `timeline.selectClipAtPlayhead` | Shift+V | adapter (`timeline.select`) |
| `timeline.rippleCut` | Mod+Shift+X | adapter (`edit.cut`, then `edit.rippleDelete`) |
| `marker.modify`, `marker.delete` | Shift+M, Alt+M | `markers.edit`, `markers.clearCurrent` |
| `marker.addAndModify` | Mod+M | front end (`marker.add`, then its dialog) |
| `playback.fastReverse`, `playback.fastForward`, `playback.slow` | Shift+J, Shift+L, Shift+K | front end |

Alt+1 to 8 (video destination to that track) needs either eight ids or a key binding that
carries parameters (`params = { track = "V1" }`). Parameters are the keymap-format change already
waiting on Truman (D-051's page-scoped bindings), and scale better; recommended.

**4. The Inspector** (its Edit page panel is empty today). FilmCraft sets a clip's transform,
crop and opacity with `effects.setParam` on its `motion` and opacity effects. Three ways to name
that:
- **(a) Grouped ids (recommended):**
  - `clip.setTransform {clip, zoom?, position?, rotation?, anchor?, flip?}`;
  - `clip.setCrop {clip, left?, right?, top?, bottom?, softness?}`;
  - `clip.setComposite {clip, mode?, opacity?}`.

  These are Resolve's Inspector sections. Each is easy for an agent to find and gets its own risk
  tier, and the parameter names stay ours if the engine's change.
- **(b) One generic id:** `clip.setProperty {clip, property, value}`, with Inspector labels as
  property names. It is smaller, but its parameters' meaning moves out of the vocabulary.
- **(c) `raw.filmcraft/effects.setParam`:** no new ids, but FilmCraft's parameter names cross into
  agents' calls and the front end. That breaks the rule that no FilmCraft type crosses the
  `Engine` trait.

Reading the values back would come from `timeline.inspect`, extended with each clip's transform,
crop and composite.

**Waiting on Truman:** yes or no to each group, names changed where he likes. With 1 to 3
approved, the keymap's bindings are added as each id is built. The Alt+1 to 8 choice rides on the
keymap-format decision.

### B-046 — A reopened project's windowed grades looked foreign, and copies split from them lost their grade
**Area:** engine · **Status:** fixed · **Caught by:** looking at screenshots of D-063's polygon on
the second screen. The Node Editor of a project saved by the CLI and reopened said "This clip has
Lumetri effects made outside the Color page"

**Symptom:** after a project was saved and reopened, a clip graded with a soft Linear window, or a
Polygon with corners off the grid, showed the foreign-effects warning. A copy split off it after
reopening took no grade. It showed an empty node tree and drew ungraded on Resolution's renderer,
while FilmCraft's own render kept the Lumetri effects. A square polygon, a hard Linear window and a
Circle at softness 0.02 were fine. The grade itself was kept in the project's metadata, and the
next grade edit wrote matching effects again.

**Cause:** `grade_in` takes a clip's Lumetri effects as its grade's own only when they equal what
the grade compiles to, exactly (D-031's copy rule). FilmCraft writes the masks' numbers exactly,
but reads them with serde_json's default float parsing, which can land a unit in the last place
off. A feather of 115.19999742507935 came back as 115.19999742507936, and a corner's y
1848.0000114440918 as 1848.000011444092. Whether a project hit it depended on its numbers.

**Fix:** serde_json's `float_roundtrip` feature, on in the workspace's dependency. There is one
serde_json in the build, and Cargo unites its features, so FilmCraft's project loading reads floats
exactly too. Its code is not touched. Projects saved before the fix were written exactly, so they
read back right now.

**Checked:**
- Adapter test: a grade with eight Linear and Polygon windows at odd softnesses, turns and corners
  is saved and reopened. It is still the clip's own, not foreign, with the same nodes. A copy split
  off after reopening takes the same grade.
- The test fails without the feature: that is the bug, and it serves as the mutation check.
- Through the CLI, the five projects that showed it (a triangle polygon, a Linear window at
  softness 0.03, and three controls) read back with nothing foreign.

### B-047 — The Primaries drag test checked the timeline before its answer came
**Area:** tests · **Status:** fixed · **Caught by:** the full test run for B-046, once; three runs
after it passed

**Symptom:** `shadows_highlights_and_midtone_detail_are_dragged_in_the_primaries` (D-055) failed
its last line, that the dragged clip shows as graded.

**Cause:** the test waited for the grade's numbers from `color.inspect`, then at once read the
timeline's `graded` mark. That arrives in `timeline.inspect`'s own answer, which can come a step
later.

**Fix:** it waits for the mark as it waits for the numbers.

### B-045 — Menu buttons drew a box where their "▾" should be
**Area:** ui · **Status:** fixed · **Caught by:** screenshots on the second screen (D-069)

**Symptom:** the Curves palette's mode menu read "Custom □" or "Hue vs Sat □" since D-043; the
Primaries' new mode menu (D-067) read "Wheels □".

**Cause:** "▾" (U+25BE) is in neither Geist nor egui's own fonts behind it, and egui draws a box
for a missing glyph.

**Fix:** `theme::MENU_MARK`, "⏷" (U+23F7), which egui's own symbol font has; both menus use it.

**Checked:**
- Theme test: our fonts have the mark, and every other non-ASCII character the UI's strings use
  ("…×·–é", found by a sweep of the source). The test can see a missing glyph: it also asserts
  "▾" is missing.
- By eye: "Wheels ⏷" and "Hue vs Sat ⏷".

### B-048 — "Outside Soft" ran into its value in the Window palette
**Area:** ui · **Status:** fixed · **Caught by:** screenshots on the second screen (D-069)

**Symptom:** a Polygon's Outside Soft read "Outside Sof8.00" at the 1600-pixel window: the name
and the number overlapped in their cell.

**Fix:** the two fields are named In Soft and Out Soft. Resolve's own labels for them are not
checked.

### D-069 — `--palette` and `--bars` self-test flags; D-063, D-065, D-066 and D-067 seen by eye
**Area:** tools · **Status:** active · **Doc:** `apps/resolution-app/src/main.rs`

**Decision:** two front-end flags for screenshot self-tests, like `--highlight` (D-050):
- `--palette curves|qualifier|window`, or `curves:` and a hue curve's name (`curves:hue_sat`),
  starts the Color page on that palette;
- `--bars` starts the Primaries in Bars mode.

**Seen by eye, 2026-10-15, on the second screen:** the Sony clip graded through the CLI, a picture
grey and brighter inside each window, screenshots at the Color page:
- **A Linear window, Soft 3 (left) at 15:** grey inside, its left edge fading over a wide band,
  the other three hard. A ring beside each side, the left one further out, the top one aside of
  the rotate handle. Soft 1 to 4 on their own row.
- **A Polygon soft all outside:** the grey reaching past the triangle's edges. The palette shows
  In Soft and Out Soft.
- **Hue vs Sat dipped at magenta:** the six patches fit in the title strip, and the curve wraps
  round past red. In the viewer, the pink light's saturation fell from 0.81 to 0.24 on average
  (pixel statistics against the ungraded picture).
- **Bars mode:** four groups of four bars, Gain R's small fill at 1.15, values under the bars.
- **Found:** B-045 (the menu mark), B-046 (the foreign-effects warning on reopened projects, fixed
  in #63) and B-048 (the label).
- Not compared with Resolve itself.

**Checked:** unit test of `--palette`'s names, curves included, and unknown names refused.

### B-049 — Agents could not learn most of a node's controls: `color.adjust`'s description stopped at D-028
**Area:** engine · **Status:** fixed · **Caught by:** review, checking that agents can reach this
run's new window fields (the AI-native invariant)

**Symptom:** the vocabulary's description of `color.adjust`, which is all an agent reads about it,
listed the Primaries, the four Custom Curves and the label. It left out everything added since:
- Shadows, Highlights and Midtone Detail (D-055);
- the hue curves (D-043, D-047);
- the qualifier and its matte finesse (D-032, D-046, D-058);
- the windows and every field of theirs (D-031, D-052, D-053, D-061, D-063, D-065).

The engine took them all. An agent only found them in `color.inspect`'s answer, and only those
already set: the optional fields are not written until they are.

**Fix:** the description names every control, with its range or neutral where that is not plain:
- the hue curves' middle line;
- the qualifier's fields and Morph's choices;
- each window field and which shapes have it.

It also says how a call combines with the node: objects merge, lists such as `windows` replace.
Nothing new is accepted; the parameters are as they were.

**Checked:** adapter test: a node with every optional part set is serialised, and each of its keys,
each window shape and each Morph choice must be in the description. It also checks those names are
the grade's own. Against the old description the test fails, first on `hue_hue`.

### B-050 — A clip of another size than its timeline showed cropped at its own size, not fitted as in Resolve
**Area:** engine · **Status:** fixed · **Caught by:** a screenshot pass of the Edit page on the
second screen, with the rehearsal's Sony and Samsung clips (B-042) in a 1920 × 1080 timeline

**Symptom:** the Samsung portrait clip (3840 × 2160 stored, turned −90°) filled the viewer's width
and showed only a torso; its top and bottom were cut away. A 4K clip in a 1080 timeline likewise
showed only its middle quarter. The one-minute edit's picture, with Truman's own clips, would have
been cropped throughout.

**Cause:** FilmCraft places a clip by its "Default Media Scaling" preference, which defaults as
Premiere's does to `none`: the clip at its own size, the rest cropped. Resolve's default (21.1
Reference Manual, "Mismatched resolution files") is "Scale entire image to fit": the longest side
fitted, the rest letterboxed or pillarboxed. Resolution never set the preference. The Color page's
window overlay already assumed fitted clips (`clip_on_screen`), so on such clips it was misplaced
too.

**Fix:** the adapter's session sets the preference to `scaleToFrameSize` when it starts. FilmCraft
then marks each clip of another size Scale to Frame Size as it is edited in, which fits it.
FilmCraft's own render, the GPU renderer and Deliver all draw from that mark. Clips already in a
project keep how they were placed. Resolve's other choices (crop with no resizing, scale full frame
with crop, stretch) are not offered.

**Checked:**
- Adapter test: a clip edited into a timeline of its own size is left as it is; into one half that
  size it is fitted. Without the preference, the test fails.
- By eye on the second screen: the rehearsal run again (19 of 19 steps). The Samsung clip is
  pillarboxed, the whole person in frame, and the Sony 4K clip is whole in the 1080 timeline.
  Deliver's file is 1920 × 1080.

### D-070 — Highlight Difference: how far a node moves each pixel, drawn; its command waits on Truman
**Area:** color · **Status:** active; the brightness gain is provisional, and the command to turn
it on is proposed, not built · **Doc:** `crates/grade` (`Grade::difference_image`,
`DIFFERENCE_GAIN`); `crates/renderer` (`stage`, `fs_highlight`); engine-api `Highlight`

**Decision:** Resolve's third Highlight view, after Highlight and Highlight B/W (D-048). The 21.1
Reference Manual describes its matte as "a difference matte generated by the node's input compared
against its output" (Split Screen > Highlight Modes).
- **What it shows:** for the current node, how far it moves each pixel from its input, as grey: the
  most any channel moves, times four (`DIFFERENCE_GAIN`), so a quarter of the range or more is
  white. It is black where the node changes nothing; its key and windows are in what it changes.
  The nodes after it are not drawn.
- **On the GPU:** the grade stage keeps the node's input texture, stops after the node, and the
  Highlight pass draws the difference from the two.
- **On the CPU:** `Grade::difference_image`, for FilmCraft's own frames of clips graded at their
  source (D-062).
- **Turning it on:** engine-api's `Highlight` gains `difference`, beside `black_white`, and
  `--highlight diff` starts the app with it for self-tests. A key or menu item needs a vocabulary
  id, which is Truman's to approve. Proposed: `color.highlightDifference` (in Resolve, the
  Color viewer's option menu, Highlight > Difference; no key found in the manual), a Ui command beside `color.highlight` and
  `color.highlightBW`.

**Provisional:** the gain of four is a guess; the manual does not say how bright Resolve draws the
difference.

**Checked:**
- Grade test, of the middle of three nodes, keyed on reds, making them bluer: each pixel is four
  times the most any channel moves between that node's input and output. Greys are black. A
  neutral node is black everywhere.
- Renderer test: the GPU against the CPU for each of three nodes, within 0.96, 0.00 and 1.21 code
  values; greys only. The keyed node is light on the reds and black elsewhere.
- Adapter test: a clip graded at its source shows a grey matte on the GPU and in FilmCraft's own
  frames.
- Mutation checks, each caught:
  - the nodes after it drawn, on the CPU and on the GPU;
  - the red channel alone;
  - no input kept on the GPU;
  - no gain in the shader;
  - the flag dropped in the adapter.
- By eye on the second screen: the Sony clip with Hue vs Sat dipped at magenta and red gain 1.15.
  The skin, the pink lights and the light bar's edge show light, and the deep blue background
  dark.

### D-071 — The Color page's Mini-Timeline
**Area:** color · **Status:** active · **Doc:** `color_page.rs` (`mini_timeline`, `strip_buttons`);
`app.rs` (`color_page`)

**Decision:** Resolve's Color page has a Mini-Timeline, "a small representation of the Timeline
in the Edit page wherein each clip is as long as its actual duration" (21.1 Reference Manual). It
is hidden by default and opened with a button. Ours is the same:
- **Opening it:** a Timeline button beside the clips opens the strip above them and closes it again.
  It starts closed. `--mini-timeline` opens it for self-tests.
- **What it draws:** each video track as a row, V1 at the bottom and named at the left. Each clip
  is as long as it is, in the Edit page's video colour, with a point's gap at each cut. The clip
  being graded is bright, and the playhead is a line.
- **Using it:** a click or a drag moves the playhead there, and the clip graded follows it, as
  everywhere on the page.

It is the front end's own view, drawn from the timeline the editor already holds: no new
commands.

**Not as Resolve:** Resolve's opens from the toolbar at the top of the Color page, which ours does
not have yet. No audio tracks, clip names or thumbnails in it.

**Checked:**
- UI test: closed at first; opened; a click three quarters along moves the playhead there, and
  another clip is graded; closed again.
- Mutation checks, each caught: the click mirrored, the button doing nothing, the strip never
  drawn.
- By eye on the second screen, with the rehearsal's timeline:
  - the first version drew the clips not being graded in a colour too close to their row's, so
    only the current clip showed;
  - now in the video colour: V1's six clips with their cuts, V2's clip at 5 s, the graded clip
    bright, and the playhead in it.

### D-072 — The qualifier's HSL and Luma modes
**Area:** color · **Status:** active · **Doc:** `color_page.rs` (`qualifier`)

**Decision:** Resolve's qualifier has modes. The 21.1 Reference Manual says Luma "is identical to
the HSL qualifier with H and S turned off", and that the mode "automatically disables the Hue and
Saturation controls". Our qualifier already has Hue, Sat and Lum switches (D-032). So two buttons
in the qualifier palette's top row choose the mode:
- **Luma** turns Hue and Sat off and Lum on;
- **HSL** turns all three on.

The mode shown is what the switches say: Luma while Hue and Sat are off and Lum is on. Nothing
new is stored. RGB and 3D modes are not built; RGB would need ranges of its own in the grade.

**Checked:** the qualifier's UI test: Luma, then HSL, each through the engine. With the buttons'
switches swapped, it fails.

### B-051 — Matte Finesse's numbers ran under the scopes; the qualifier's labels ran into their values
**Area:** ui · **Status:** fixed · **Caught by:** a screenshot of D-072's buttons on the second
screen

**Symptom:** the qualifier palette's fourth column, Matte Finesse, ran past the palette and under
the scopes panel, so its numbers could not be seen. With the columns fitted, the names "Sat Low
Soft" and "Lum Low Soft" ran into their values.

**Cause:** the palette's top row (On, Invert, the new mode buttons, Remove Qualifier and the hint
"Click the viewer to pick a colour.") ran over the palette's width. egui widened the area to hold
it, and the columns took their width from that area. The hint alone may already have pushed it
over before D-072's buttons; that was not checked on the earlier code.

**Fix:**
- The columns take the palette's own width, and the hint is cut short with "…" when it does not
  fit.
- Inside a column, a name drops what its title already says ("Center", "Low Soft"); its
  accessible name keeps it all, so tests and agents are unaffected.
- Matte Finesse, whose names are longest, gets a column a third wider than the others.

**Checked:**
- The qualifier's UI test: White Clip, Matte Finesse's last number, ends before the scopes panel
  begins. On the old layout it fails, running 22 points under.
- By eye: all four columns readable inside the palette.

### B-052 — Each window added grew the palettes and shrank the Color page's viewer
**Area:** ui · **Status:** fixed · **Caught by:** screenshots of the Window palette with four and
six windows on the second screen

**Symptom:** the Window palette lists every window above the chosen one's numbers. With six
windows the palettes' panel grew to hold them, and the viewer above went from about 270 points
tall to 190. Each window more shrank the picture further.

**Cause:** the palettes' panel is resizable, and egui grows such a panel to what its contents take.
The list had no limit.

**Fix:** the list scrolls past four windows (`ScrollArea`), so the palette keeps its height.
Resolve's palettes keep theirs too.

**Checked:**
- UI test: nine windows added through the palette, and the viewer's picture is the same size as
  before. Without the limit it fails: 229 points tall became 166.
- By eye: six windows, four rows shown and the rest by scrolling, the numbers below.

### B-053 — Self-test runs saved their layout into the user's editor
**Area:** ui · **Status:** fixed · **Caught by:** B-052. A later screenshot run opened with the
palettes still grown by the run before, so eframe had stored that size.

**Symptom:** eframe saves egui's memory (panel sizes, scroll positions) at exit, and self-test runs
saved theirs too. A self-test that grew a panel, like B-052's six windows, left it grown in the
user's next session. D-049 already kept self-tests from saving the window's position.

**Fix:** a run with `--window-at`, `--screenshot` or `--play` does not keep egui's memory
(`persist_egui_memory`). The colour theme is still kept, as `--theme` means it to be.

**Left as it is:** Truman's stored state (`%APPDATA%\Resolution\data\app.ron`, written 16:20 on
2026-10-15 by this run's screenshots) may hold the grown Color palette height. It was not edited,
since it may also hold panel sizes he chose. Dragging the palette's top edge back puts it right.

**Checked:** by code; there is no harness for eframe's storage. B-052's fix also stops the growth
at its source.

### B-054 — Nodes past the fourth ran off the Node Editor's edge
**Area:** ui · **Status:** fixed · **Caught by:** a screenshot of a six-node grade on the second
screen

**Symptom:** node 05 was cut by the panel's edge and node 06 not drawn at all, nor the line to the
output. Nothing scrolls or zooms in the Node Editor, so those nodes could not be reached.

**Cause:** nodes were a fixed 96 × 60 points with at least 12 between them. At the panel's default
width, about four fit.

**Fix:** the chain is scaled down to fit the panel, as Resolve's Node Editor zooms out. Gaps shrink
with the nodes. A node too narrow for its label shows only its number; the label is measured, not
guessed.

**Checked:**
- UI test: eight nodes, each inside the panel. Unscaled, node 5 runs past the edge.
- By eye: six nodes, each with its number, its label (Desat on 03) and its graded mark.

**Also:** a doc comment D-071 had put on `MiniClip` belonged to `strip`; it is back above it.

**Also:** B-052's UI test added its nine windows a click at a time. In one full run in four, one
click was lost and the test timed out. Not traced: perhaps the click arrives in a frame where the
palette is not drawn while the grade is fetched again. The test now sets the nine windows through
the engine in one `color.adjust`, since its point is the viewer's size. The circle test still
clicks Add Circle.

### D-073 — The timeline's video clips show the picture at their head
**Area:** ui · **Status:** active; which view Resolve starts in is from memory, unchecked ·
**Doc:** `timeline_ui.rs` (`draw_thumbnail`, `View::thumbnails`); `app.rs` (`edit_thumbs`)

**Decision:** Resolve's timeline can show clips as Filmstrip, Thumbnail or Minimized (21.1
Reference Manual, Timeline View Options). The manual does not say which is the default; from
memory it is Thumbnail, the picture at each clip's head. Resolution's clips showed only their names.
- **What shows:** each video clip shows the picture at its head under its name, as tall as the room
  there. It is left out where the clip is too short for it, and dimmed on a clip that is off or a
  hidden track's.
- **How:** a second thumbnail renderer, as the Color page's clip strip has (D-029), draws each
  clip's head frame at 160 pixels. A clip's picture is asked for again when its start or its
  source In moves, so a moved, trimmed or slipped clip shows its new head.

**Not as Resolve:**
- The picture is the timeline's frame at the clip's head, so a clip with another over it shows
  both. Resolve shows the clip on its own.
- There is no Filmstrip or Minimized view, and no menu to choose, until Timeline View Options is
  built.

**Cost:** playback stays real time: 287 frames in 12 s, 23.9 fps, as before. The first play
straight after opening a project waited 359 ms instead of 44 ms to start, while the heads were
still rendering.

**Checked:**
- UI test: on the demo, a picture for every video clip; a clip moved is asked for again. Mutation
  checks: not asked again on a move, and never asked for, each caught.
- By eye on the second screen, the rehearsal's timeline: each clip's head under its name, the
  Samsung clips pillarboxed, the V2 cutaway's head over V1's picture.

### D-074 — Keys for the node resets and Save Project As
**Area:** ui · **Status:** active · **Doc:** `keymaps/resolve.toml`

**Decision:** four of the vocabulary's commands had no key, though the 21.1 Reference Manual gives
them one ("Resetting Nodes"; "Methods of saving projects"):
- Shift+Home resets the current node's grade (`color.resetNode`);
- Mod+Home resets all grades and nodes (`color.resetAll`);
- Mod+Shift+Home resets the grades and keeps the nodes (`color.resetGradesKeepNodes`);
- Mod+Shift+S is Save Project As (`project.saveAs`).

All four are marked verified: 73 of 82 bindings now come from the manual.

The Home keys reset only on the Color page, as the node keys do (D-051). On the Edit page Home
still goes to the timeline's start; the keymap is not page-scoped yet.

**Checked:**
- Key test: each chord reaches its command.
- UI test: Shift+Home resets a graded node through the engine.
- With Shift+Home bound elsewhere, both fail.

### D-075 — A short grade rehearsed through the Color page's keys and mouse
**Area:** tests · **Status:** active · **Doc:** `app.rs` (`a_short_grade_through_the_keys_and_mouse`)

**Decision:** the Color page's side of B-043's one-minute edit test. Each step had a test of its
own; together they had none. A UI test grades two clips as a colourist would, through keys and the
mouse only:
1. Gain's ring dragged down on node 1, then Alt+S for node 2.
2. A circle window on node 2, and its key picked in the viewer.
3. Highlight on and off with Shift+H.
4. The next clip from the strip, its own one-node grade, then back to the first with its two nodes
   as they were.
5. Ctrl+Z takes back the key and keeps the window.

It found B-055 at its second step.

### B-055 — Alt+S right after a change left the old node current
**Area:** ui · **Status:** fixed · **Caught by:** D-075's rehearsal

**Symptom:** Alt+S straight after an adjustment added node 2, but node 1 stayed current. The next
adjustments went to node 1, the wrong node, with nothing to show it but the Node Editor's outline.
Alt+S on an untouched clip, as the node-key test does it, was fine.

**Cause:** Alt+S makes the new node current at once, before the engine has it. The adjustment's own
answer, still on its way, brought the one-node grade, and taking it clamped the current node back
into range: node 1. When the two-node grade came, nothing moved it back.

**Fix:** a node asked for (Add Serial Node, Append Node) is remembered. Grades that arrive without
it leave the current node alone; every colour command answers with its grade, in order, so those
are from before. The add's own answer settles it. If the node still is not there, the add failed,
and the current node is put back in range. A node chosen by hand meanwhile stays chosen.

**Checked:**
- D-075's UI test.
- Unit test (`ColorPage::take` and `added`):
  - a grade from before keeps the new node current;
  - the add's answer clears it;
  - a failed add puts the current node back in range;
  - a node chosen meanwhile stays chosen;
  - without an add, a shorter grade still clamps.
- Mutation check: clamping on grades from before fails both tests.

### B-056 — A window added right after a change left the old window chosen
**Area:** ui · **Status:** fixed · **Caught by:** looking for B-055's kind elsewhere: a UI test
written for it, which failed three runs in three

**Symptom:** dragging a window's Size and at once clicking Add Linear added the second window, but
the first stayed chosen. Its numbers and handles were the ones then changed.

**Cause:** as B-055. Add chose the new window at once; the Size drag's answer, a grade with one
window, arrived after, and the palette clamped the chosen window into that grade's range.

**Fix:** as B-055. The window added, and on which node, is remembered while grades from before it
arrive. A grade that has it, another node, or a failed adjustment lets the palette clamp again.

**Checked:** the UI test (a Size drag, then Add Linear at once: window 2 is chosen) passes three in
three; before, it failed three in three. Curve points need nothing: a point index is checked
against the curve before use.

### D-076 — The Edit page's tools rehearsed in a row
**Area:** tests · **Status:** active · **Doc:** `app.rs` (`the_edit_pages_tools_in_a_row`)

**Decision:** after D-075 found B-055, the Edit page's tools get the same treatment, chained in one
UI test:
- M at two places, among the demo's own markers;
- Shift+Up and Shift+Down between them;
- a clip clicked, then D to turn it off and on;
- B and a click to cut it;
- A for the selection tool;
- Ctrl+Z back through all of it.

It found no bug in the editor. One lesson for the tests: egui_kittest takes a key press at the next
step. A key pressed just before a command sent straight to the engine (`worker.run`) is seen
after it. So the first draft's marker landed where the second's playhead was.

**Checked:** with Shift+Up mapped to the next marker, the test fails.

### B-057 — B-056's fix let go of the new window too early, one run in seven
**Area:** ui · **Status:** fixed · **Caught by:** B-056's UI test, failing in about one full run of
the app's tests in seven. Debug output tagged by test thread found why.

**Symptom:** as B-056, now and then. Window 2 was added but window 1 ended chosen.

**Cause:** an adjustment patches the grade on show at once, before the engine answers
(`ColorPage::adjust`). So the frame after Add showed two windows, and B-056's fix took that as the
window having arrived and let go of it. The Size drag's answers, from before the add, then put
back a one-window grade, and the palette clamped the choice to window 1.

**Fix:** the window added is held until a grade from the engine has it (`ColorPage::take`). The
grade the palette shows is not asked. A switch to another node, or a failed adjustment, still lets
go of it. Nodes need no such care: adding one does not patch the grade on show.

**Checked:** eight full runs of the app's tests, every one passing.

### D-077 — Thumbnails show each clip on its own
**Area:** engine · **Status:** active · **Doc:** engine-api `RenderHints::solo`;
`crates/filmcraft-adapter/src/playback.rs` (`Snapshot::solo`); `color_page.rs` (`Thumbs`)

**Decision:** a clip's thumbnail is that clip alone, graded, as Resolve's thumbnails show it. That
applies to the Color page's clip strip (D-029) and the timeline's clip heads (D-073). Before, both
showed the timeline's whole picture at that frame, so a cutaway's thumbnail had the clip under it
showing through its pillarbox. The pillarbox comes from B-050's fitting.
- **The hint:** `RenderHints::solo`, a clip's id: render that clip of the timeline's video on its
  own.
- **In the adapter:** the frame is rendered from a copy of the project whose video tracks hold
  only that clip and no transitions. Its grade, and any graded stand-in (D-044), come with it.
- **Who asks:** the thumbnail renderer asks for every thumbnail so. Viewers and Deliver never do.

**Cost:** a copy of the project, and its grades worked out again, for each thumbnail. These are
made once per clip and change.

**Checked:**
- Adapter test: the demo's V1 clip under the V2 cutaway, rendered alone at a time with no
  transition, matches the timeline rendered with the cutaway deleted (under 1 code value apart),
  and differs from the two together. With the hint ignored it fails.
- By eye: the strip's V2 cutaway, and its head on the timeline, show the portrait clip alone
  between black bars.

### B-058 — On a laptop's screen the Color page's viewer was squeezed and the scope buttons ran over the Curves palette
**Area:** ui · **Status:** fixed at 1366 × 768; the 1000 × 640 minimum is still cramped ·
**Caught by:** screenshots at smaller window sizes on the second screen, with a new self-test flag,
`--window-size w,h`

**Symptom:**
- **At 1366 × 768:** the viewer's picture was 77 points tall. The palettes, a fixed 340 points, and
  the clip strip took the rest. The scopes' four buttons, right-aligned and wider than their panel,
  ran left over the Curves palette's menu.
- **At the minimum, 1000 × 640:** the viewer was all but gone. The Primaries' names ran into their
  values ("Contras1.000"), and the wheels' YRGB numbers ran together.

**Cause:** the palettes started at a fixed height whatever the window's, and the scopes' buttons
had no form for a narrow panel.

**Fix:**
- The palettes start at two fifths of the window's height, between 220 and 340 points; they are
  still resizable.
- When the four scope buttons do not fit, the scopes header is one menu named for the scope shown.
- `--window-size` makes the window that size on its first frame, as `--window-at` moves it
  (D-049): eframe would otherwise restore the size it last saved.

**Still not right:** at 1000 × 640 the Primaries' numbers and the page's height. The minimum
window could be raised, or the Primaries made compact; for later.

**Checked:**
- UI test at 1366 × 768: the viewer's picture is 162 points tall (77 with the fixed 340), and the
  scopes' menu is clear of the Curves menu. Each part, undone, fails it.
- Screenshots of a stored session open with the user's own palette height, which a default does
  not change.

### B-059 — Narrow number fields: names ran into values, and the YRGB numbers into each other
**Area:** ui · **Status:** fixed · **Caught by:** B-058's screenshots at 1000 × 640

**Symptom:** at narrow widths, a number field's name ran into its value ("Contras1.000",
"Saturati50.00"), and a wheel's four YRGB numbers ran into one another ("0.000.000.000.00").

**Fix:**
- A number field's name is cut short with "…" where it would reach its value. It is laid out to
  the room left once the value is drawn, so its accessible name is untouched.
- A wheel's or a bar group's four numbers take a smaller font, down to 7 points, where four of the
  widest ("25.00") would not fit side by side.

**Checked:**
- Unit test: a long name is cut to the room it has and a short one is whole.
- By eye at 1000 × 640: "Cont…", "Satu…" and "Shado…" clear of their values, and the YRGB numbers
  apart.
- The 1600 × 920 tests are unchanged: names and numbers fit there as before.

### B-060 — On a laptop's screen the Deliver page's Add to Render Queue was out of sight
**Area:** ui · **Status:** fixed · **Caught by:** a screenshot of the Deliver page at 1366 × 768
(B-058's pass)

**Symptom:** the Render Settings panel ended at its Render heading. The range (Entire Timeline,
In/Out Range) and Add to Render Queue were under the edge, reachable only by scrolling a panel that
showed no sign of scrolling.

**Fix:** the range and the Add button have a strip of their own at the panel's foot. Above it the
presets, the file name and the location scroll when the panel is short. Resolve keeps its Add to
Render Queue at the panel's foot too.

**Checked:**
- UI test at the smallest window, 1000 × 640: Add to Render Queue and Entire Timeline are above
  the timeline. On the old layout Add was at 529 points, under the timeline at 472. At 1366 × 768
  the test's fresh panels left room; a real session's stored panel sizes did not.
- The Deliver render test still passes.
- By eye: the foot strip in view at 768 points tall.

### B-061 — In a small window the two viewers' transport rows ran into each other
**Area:** ui · **Status:** fixed · **Caught by:** a screenshot of the Edit page at 1000 × 640
(B-058's pass)

**Symptom:** each viewer's row of seven buttons (Go to Start, Reverse, Stop, Play, Go to End, Mark
In, Mark Out) is about 240 points wide. At 1000 × 640 the viewers are about 200 wide, so the source
viewer's row ran into the timeline viewer's.

**Fix:** a row too wide for its viewer drops Go to Start and Go to End, then the mark buttons if it
must. Home, End, I and O still work. Play, Reverse and Stop always stay.

**Checked:**
- UI test at 1000 × 640: the source viewer's buttons end before the timeline viewer's begin. With
  nothing dropped they reach 511.5 points, past the timeline viewer's at 489.
- By eye: both rows within their viewers.

### B-062 — A self-test with `--theme` saved its theme into the user's editor
**Area:** ui · **Status:** fixed · **Caught by:** review, before a screenshot pass of the five
themes. B-053 kept self-tests from saving egui's memory, but `save` still wrote the theme.

**Fix:** a self-test run (`--window-at`, `--window-size`, `--screenshot` or `--play`) saves
nothing, its theme included. `--theme` outside a self-test is kept as before (D-018).

**Checked:**
- Five self-test runs, one in each theme, left Truman's saved theme as it was (midnight).
- By eye: the Edit page in all five themes, each accent carried through the toolbar, the active
  track, the playhead and the page bar, with nothing unreadable.

### B-063 — On the Color page, Delete, Backspace and Ctrl+B still cut the timeline
**Area:** ui · **Status:** fixed · **Caught by:** review of what the keymap not being page-scoped
(D-051) allows, then a UI test that failed: 14 clips became 16

**Symptom:** a clip selected on the Edit page stayed selected on the Color page. There, Delete,
Backspace, Shift+Backspace and Ctrl+B ripple-deleted, lifted or split it, though the page shows no
timeline to edit. Someone pressing Delete to remove a node would cut the timeline instead.

**Cause:** keys reach the same commands on every page. Only the colour keys were held to the Color
page (D-051).

**Fix:** on the Color page, keys bound to commands that cut the timeline do nothing and say so ("…
works on the Edit page"):
- the source edits and the clipboard;
- splitting, deleting and ripple-deleting;
- trimming to the playhead;
- clip enable;
- transitions;
- Close Gap.

Menu items still run them; a choice from a menu is deliberate. In Resolve those keys edit only on
the Edit and Cut pages. Giving Delete its Resolve meaning on the Color page, deleting the current
node, waits on the page-scoped keymap (D-051).

**Checked:** UI test: a clip selected on the Edit page, then Delete, Backspace, Shift+Backspace and
Ctrl+B on the Color page; the timeline keeps all its clips. It failed before the fix.
