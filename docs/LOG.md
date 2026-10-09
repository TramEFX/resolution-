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

**Not yet verified:** that a failed run now saves the cache. That needs a run that fails.

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

**Not yet verified:** how much faster the gate runs. No run has used these settings yet; record the
first run's duration here when it lands.
