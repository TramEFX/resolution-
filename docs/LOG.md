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
| B-006 | 2026-10-10 | ci | Upstream's Japanese-UI test needs a system CJK font the runner lacks | open |
| B-007 | 2026-10-10 | ci | Upstream's mixer speed test needs 4× realtime; the GitHub runner manages 2.1× | worked around by D-010; cause corrected; fixed upstream after the pin |
| D-010 | 2026-10-10 | ci | Mirror CI runs upstream's gate phase by phase, leaving out one speed test | active |

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
**Area:** ci · **Status:** open · **Caught by:** running it (mirror run 38016792934, the D-009 pass)

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
guard should pass. B-007 is fixed upstream; its skip and report step come out at the next sync.

**Result: green.** Run 38019701912 on `resolution` @ `e199881` passed every phase in 23m20s:
fmt 3s; clippy 41s; test 17m35s (219 binaries, 2,294 passed, 0 failed, 40 ignored, 1 left out,
0 skipped); mixer speed alone 6.6×; `layers: ok`; `assets: ok (69 assets attributed)`;
`wasm: ok (42 crates)`. Between the pin `2ba0629` and `e199881` the only changed file is
`.github/workflows/mirror-ci.yml` (GitHub compare), so **this run confirms the pinned engine
code.**
