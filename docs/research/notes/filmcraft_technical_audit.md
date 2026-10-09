# FilmCraft technical audit: is it a sound fork base for "Resolution", and how should the fork work?

Method: I cloned github.com/storytold/filmcraft (blobless clone, plus a full bare clone to measure size). It was main at commit `7b6c134472287db4367fca80bcb498240ba36567` on 2026-10-09. I ran `cargo metadata --no-deps` (offline) and used grep, `wc` and `git log`. I did not build or run anything. I read issues through GitHub's web pages; the GitHub API and MCP were not authorised for this repo in this session. "Repo" links point to files on `main` as of that commit. Counts I computed myself are cited as "(local analysis of clone @7b6c134)" with the relevant repo path.

Key surprise for the report writer: **FilmCraft is a Premiere Pro clone, not a Resolve clone.** Its ids, menus, workspaces, Lumetri colour model and shortcut presets all follow Premiere. Its public git history is only **10 days long** (2026-09-30 to 2026-10-09). It now has **~322k lines of Rust in 47 packages**, more than the 30 or so crates in the brief: mxf, cfb, mpegts, mpeg2v, ac3, ogg, dnx, apv, av1, speech, scopes, platform and apps/filmcraft-web were missing from it.

---

## Q1. Workspace structure, and whether the engine/front-end boundary is real

### Takeaway
The layering is real and machine-enforced: `cargo xtask layers`, and nothing below L5 may use egui, winit, cpal and similar crates. Front ends mutate projects only through `Session::execute`. However, the reusable "engine" is a large monolithic `filmcraft-engine` crate (46k lines) that pulls in nearly every lower crate. Its command registry is a closed static list with no external registration API. Several things a new front end needs live in `ui-egui`, not in the engine: playback scheduling, the frame worker pool, the control-channel server and UI-only commands (workspaces, tools, playback). Resolution would have to reimplement them.

### Cited Findings
- **Workspace setup.** `members = ["crates/*", "apps/*", "xtask"]`, resolver 3, edition 2024, `rust-version = "1.95"`, version 0.4.0. Licence: `MIT OR Apache-2.0`. Workspace lint: `unsafe_code = "forbid"`. — [Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml)
- **Layer map** (from docs/architecture.md §1):
  - L6: apps/filmcraft, filmcraft-cli, filmcraft-web
  - L5: ui-egui, automation, platform
  - L4: engine
  - L3: render, gpu, export, golden (test-only), scopes
  - L2: edit, codecs, interchange, captions, speech
  - L1: frame, media, project, audio-dsp, text
  - L0 foundation: time, geom, color, bitstream, testkit (dev-only)
  - L0 codecs and containers: isobmff, matroska, mxf, cfb, mpegts, ogg, h264, h264enc, hevc, vp9, av1, mpeg2v, prores, dnx, apv, aac, ac3, opus

  — [docs/architecture.md](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md)
- **What `xtask layers` enforces** (it reads `cargo metadata`):
  - Every crate must have a layer.
  - Dependencies point downward only; a same-layer edge must be on an allowlist.
  - L0 codec crates may depend only on `filmcraft-bitstream`.
  - `egui`, `eframe`, `egui-wgpu`, `winit`, `rfd`, `cpal` and `muda` are allowed only in L5 and L6.
  - `cargo xtask wasm` checks that every L0–L4 crate builds for wasm32.

  — [docs/architecture.md](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md), [xtask/src/main.rs](https://github.com/storytold/filmcraft/blob/main/xtask/src/main.rs)
- **Crate table.** Internal normal dependencies come from `cargo metadata`, with the `filmcraft-` prefix dropped. LOC counts `.rs` files under the crate, excluding `tests/` dirs but including inline `#[cfg(test)]` modules. "Tests" counts `#[test]` attributes. Note that `rustfmt.toml` sets `max_width = 160`, so lines are long and LOC understates size. (Local analysis of clone @7b6c134; [rustfmt.toml](https://github.com/storytold/filmcraft/blob/main/rustfmt.toml).)

  **Codec and container crates.** Each depends on `bitstream` at most.

  | Crate (path) | Purpose | Internal deps | LOC | Tests |
  |---|---|---|---|---|
  | h264 | H.264 decoder | bitstream | 8,985 | 51 |
  | h264enc | H.264 encoder | bitstream | 7,515 | 23 |
  | hevc | HEVC Main/Main10 decoder | bitstream | 6,963 | 29 |
  | vp9 | VP9 decoder | bitstream | 7,474 | 32 |
  | av1 | AV1 decoder | bitstream | 19,548 | 15 |
  | prores | ProRes decoder and encoder | bitstream | 2,014 | 24 |
  | dnx | DNxHD/HR decoder and encoder | bitstream | 4,376 | 14 |
  | apv | APV decoder and encoder | bitstream | 2,147 | 6 |
  | mpeg2v | MPEG-1/2 video decoder | — | 3,962 | 23 |
  | aac | AAC-LC decoder and encoder | bitstream | 5,067 | 29 |
  | opus | Opus decoder | — | 6,309 | 34 |
  | ac3 | AC-3 decoder | — | 1,222 | 8 |
  | isobmff | MP4/MOV demux and mux | bitstream | 4,360 | 37 |
  | matroska | MKV/WebM demux | — | 3,982 | 36 |
  | mxf | MXF demux and mux | — | 4,220 | 28 |
  | mpegts | TS/PS demux | — | 2,103 | 7 |
  | ogg | Ogg demux | — | 667 | 5 |
  | cfb | Compound File Binary, for AAF | — | 995 | 8 |
  | bitstream | Bit reader/writer | — | 433 | 8 |

  **Foundation crates (L0–L1).**

  | Crate | Internal deps | LOC | Tests |
  |---|---|---|---|
  | time | — | 698 | 12 |
  | geom | — | 194 | 3 |
  | color | — | 2,414 | 26 |
  | frame | color, time | 1,206 | 12 |
  | media | color, frame, time | 2,093 | 21 |
  | project | color, geom, media, time | 10,396 | 56 |
  | audio-dsp | — | 12,277 | 122 |
  | text | color | 3,009 | 38 |

  **Editing and interchange (L2).**

  | Crate | Internal deps | LOC | Tests |
  |---|---|---|---|
  | edit | project, time | 3,435 | 52 |
  | codecs | the hub: all codec and container crates plus color, frame, media, time; uses symphonia (MPL-2.0) | 8,378 | 116 |
  | interchange | cfb, color, geom, media, project, time | 11,497 | 62 |
  | captions | color, project, text, time | 3,566 | 49 |
  | speech | project, time; candle optional | 2,146 | 16 |
  | format | media, project | 779 | 28 |

  **Rendering and output (L3).**

  | Crate | Internal deps | LOC | Tests |
  |---|---|---|---|
  | render | audio-dsp, captions, color, frame, geom, media, project, text, time | 20,504 | 159 |
  | gpu | color, frame, geom, project, render; wgpu 30.0.1 | 3,208 | 26 |
  | scopes | color | 1,321 | 15 |
  | export | aac, apv, audio-dsp, color, dnx, frame, geom, h264enc, isobmff, media, mxf, project, prores, render, text, time | 6,095 | 71 |

  **Engine (L4).** `engine` depends on audio-dsp, captions, codecs, color, edit, export, format, frame, geom, interchange, media, mxf, project, render, scopes, speech, text and time. It has 46,285 LOC and 431 tests.

  **Front ends (L5–L6).**

  | Crate (path) | Internal deps | Other notable deps | LOC | Tests |
  |---|---|---|---|---|
  | ui-egui | audio-dsp, color, edit, engine, frame, geom, gpu, media, project, render, scopes, text, time | — | 39,357 + 11,639 in tests/ | 276 |
  | automation | engine, time | rmcp 3.5, tokio | 1,416 | 9 |
  | platform | bitstream, codecs, color, export, frame, h264, hevc, isobmff, time | objc2-*, windows, libloading | 10,163 | 128 |
  | apps/filmcraft | engine, platform, ui-egui | eframe, cpal, rfd, muda, winit | 2,122 | 36 |
  | apps/filmcraft-cli | automation, codecs, engine, media, mpegts, mxf, ogg, platform, time | — | 817 | 16 |
  | apps/filmcraft-web | codecs, color, engine, format, frame, isobmff, media, time, ui-egui | — | 2,507 | 6 |

  **Test support.** `golden` has dev-dependencies only and 8 tests. `testkit` is 762 LOC.
- **Command system.**
  - `CommandSpec { id, label, menu, shortcut, params: &'static str, enabled: fn(&Session)->Result<(),String>, run: fn(&mut Session,&Value)->Result<Value>, journal }`.
  - It is built once into a `OnceLock<Vec<CommandSpec>>` by `commands::build()`, which `extend`s per-module `commands()` lists (captions, mixer, multicam, color, shortcuts, proxies, masks, scopes and others).
  - `Session::execute(id, params)` looks up the id, checks `enabled` and runs it.
  - The `params` schema is a free-form string such as `r#"{"time":ticks?}"#`, not a typed or JSON-Schema definition.

  — [crates/engine/src/commands.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/commands.rs), [crates/engine/src/lib.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/lib.rs)
- **Command count.** I statically found **549 distinct command ids** in crates/engine/src. The largest namespaces are graphics 60, clip 57, file 56, sequence 45, timeline 39, markers 31, trim 24, captions 22, edit 21, mixer 20, export 19 and multicam 17. Macros such as `camera_keys!` generate more. The README claims "more than 650 engine commands". (Local analysis of clone @7b6c134; [README](https://github.com/storytold/filmcraft/blob/main/README.md))
- **No public way to register outside commands.** I found no `register_command`-style API; the list is closed inside `filmcraft-engine`. However, `Session` fields (`project`, `history`, `state`) and `Session::edit` / `Session::edit_sequence` are `pub`. Undoable edits can therefore be written outside the crate. (Local analysis of clone @7b6c134; [crates/engine/src/lib.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/lib.rs))
- **How ui-egui uses the engine.**
  - 217 `.execute(` calls and 22 `invoke(` calls.
  - The only `Arc::make_mut(&mut app.session.project)` is inside a `#[test]` in `panels/effect_controls.rs`.
  - It also imports lower crates directly (render 60 refs, gpu, edit, scopes) for drawing, hit-testing and previews. Example: `filmcraft_edit::through::track_through_edits` in `panels/timeline.rs`.

  (Local analysis of clone @7b6c134, crates/ui-egui/src)
- **UI-only commands live in the front end.** Tools, playback, zoom, panels and workspaces are in `crates/ui-egui/src/menus.rs` (`UI_COMMANDS`). `menus::invoke` is the single entry point for menus, shortcuts and the control channel. — [docs/architecture.md §4](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md)
- **Playback-critical code is also in ui-egui:**
  - `frames.rs` (1,053 lines): the background frame worker pool, prefetch and byte-budgeted cache.
  - `play_ahead.rs`: the audio ring-buffer mixer.
  - `source_playback.rs`
  - `control.rs` (555 lines): the control-channel handler that MCP bridge mode talks to.
  - The cpal audio master clock is in `apps/filmcraft/src/audio.rs`.

  (Local analysis of clone @7b6c134)
- **The README's own claim:** "The interface is one crate (`ui-egui`) over the engine, so a different front end can replace it without touching editing logic." — [README](https://github.com/storytold/filmcraft/blob/main/README.md)

### Inferences
- **What Resolution should consume:** everything from L0 to L4. That is the codec and container crates, time, geom, color, frame, media, project, audio-dsp, text, edit, codecs, interchange, captions, speech, format, render, gpu, scopes, export, `engine`, and optionally `platform` (hardware decode/encode, L5 but UI-free).
- **What it replaces:** `ui-egui`, `apps/filmcraft`, `apps/filmcraft-web`, and probably `automation` (it would wrap or extend that rather than fork it). `filmcraft-cli` is a useful reference.
- **The boundary is real for mutations,** since every edit goes through `Session::execute`. It is **leaky for playback and preview.** A Resolve-style front end must port or re-derive about 1.5k lines of playback, frame-pool and audio-clock code from ui-egui, or depend on ui-egui for those modules. Depending on ui-egui would pull egui in.
- **Command extension.** Without a registry extension point, Resolution needs either its own dispatcher that falls back to `Session::execute`, or a small upstreamable patch adding `register_commands(Vec<CommandSpec>)`. The second option is the cleanest, minimal patch to propose upstream.
- **Engine granularity.** `filmcraft-engine` is coarse: 46k LOC with 18 internal dependencies, including speech and interchange. Resolution cannot easily pick parts of it; it is all-or-nothing at L4.

### Gaps
- I did not verify that the L0–L4 crates compile cleanly outside the workspace (no build allowed).
- I did not count `UI_COMMANDS` entries; the macro form defeated a simple grep.

---

## Q2. Code health signals

### Takeaway
The code is test-dense: 2,230 `#[test]`s, 27 golden PNGs and ffmpeg-oracle tests. It has a strict never-crash and no-unsafe policy, with `unsafe` confined to `platform` (216 lines matching `unsafe`). But the project is extremely young and extremely fast. All 841 commits fall within 10 days, at 36–134 commits per day. About 93% of non-merge commits carry AI co-author trailers. There is **no GitHub CI that runs tests on PRs or main**: the `cargo xtask ci` gate runs locally. The repo is small (24 MB full clone) with no large binaries.

### Cited Findings
- **Totals:**
  - About 322k lines of `.rs` in the repo.
  - 2,230 `#[test]`/`#[tokio::test]` attributes.
  - The ROADMAP header (2026-10-05) says "35 crates, 1605+ tests". The workspace now has 43 crates + 3 apps + xtask.

  (Local analysis of clone @7b6c134; [ROADMAP.md](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md))
- **Biggest crates by LOC:** engine 46.3k, ui-egui 39.4k (+11.6k in tests/), render 20.5k, av1 19.5k, audio-dsp 12.3k, interchange 11.5k, project 10.4k, platform 10.2k. Most tests: engine 431, ui-egui 276, render 159, platform 128, audio-dsp 122, codecs 116. (Local analysis of clone @7b6c134)
- **Golden tests:**
  - 27 committed PNGs in `crates/golden/goldens/`, each with `.attribution` sidecars.
  - `tests/golden.rs` compares CPU renders against the PNGs and GPU against CPU.
  - Transition goldens are in `src/transitions/goldens.txt`.

  — [docs/testing.md](https://github.com/storytold/filmcraft/blob/main/docs/testing.md)
- **Unsafe code:**
  - `unsafe_code = "forbid"` workspace-wide. `platform` alone uses `deny` plus `#[allow]` on its FFI modules, with `// SAFETY:` comments (ADR 0001).
  - Grep found 216 non-comment lines containing `unsafe` in crates/platform and 0 elsewhere.
  - Clean crates also `#![deny(clippy::unwrap_used, expect_used, panic, …)]`.

  — [AGENTS.md §0](https://github.com/storytold/filmcraft/blob/main/AGENTS.md); local grep
- **Rules (AGENTS.md / CLAUDE.md):**
  - "Never crash" outranks feature work.
  - No Adobe assets.
  - Clean-room codecs from specs only; no GPL code; ffmpeg only as an external test oracle.
  - Pure Rust.
  - Layering.
  - Exact `Tick` time (254,016,000,000 per second).
  - "Everything is a command".
  - Gates before every commit: fmt, clippy `-D warnings`, test, layers, assets, wasm.
  - One task id per commit.
  - Large binary fixtures are banned; corpora are fetched pinned by sha256.

  — [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md), [CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md)
- **ArtCraft trademark clause:** the brand marks in `docs/brand/` are not open source, and "forks and modified versions must remove them". — [AGENTS.md §1.8](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)
- **CI.** `.github/workflows/` has only four workflows:
  - `freebsd.yml`: nightly plus packaging-path PRs.
  - `packaging-lint.yml`: packaging paths only.
  - `release.yml`: pushes to the `release` branch.
  - `windows-arm64.yml`: Cargo.toml/lock changes and weekly.

  None of them runs `cargo test` or `cargo xtask ci`. The windows-arm64 file says a native ARM64 `cargo test` job "is left" out. The ROADMAP lists "CI that runs the test suite … on both [Windows and Linux]" as a to-do (item 5). (Local analysis of clone @7b6c134, .github/workflows; [ROADMAP.md](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md))
- **`cargo xtask ci`** runs `fmt --check`, `clippy --workspace --all-targets --release -D warnings`, `test --workspace --release`, layers, assets and wasm. — [xtask/src/main.rs](https://github.com/storytold/filmcraft/blob/main/xtask/src/main.rs)
- **Commit cadence.** The public history begins 2026-09-30 (first commit: "first commit", 2 files). Commits per day:

  | Date | Commits |
  |---|---|
  | 09-30 | 94 |
  | 10-01 | 111 |
  | 10-02 | 55 |
  | 10-03 | 65 |
  | 10-04 | 56 |
  | 10-05 | 36 |
  | 10-06 | 97 |
  | 10-07 | 131 |
  | 10-08 | 62 |
  | 10-09 | 134 |

  That is 841 commits in total, of which 302 are merges (173 "Merge pull request") and 539 are non-merge. 202 non-merge commits landed since 2026-10-06. Tags: v0.1.0, v0.1.1, v0.2.0, v0.2.1, v0.4.0 (2026-10-08). (Local analysis of clone @7b6c134)
- **Authors.** 41 distinct author names. Brandon Thomas (maintainer) has 632 commits including merges, and 339 non-merge. Next are justjohn12345 (39) and punkhop (19). (Local analysis of clone @7b6c134)
- **AI authorship:**
  - 504 of 539 non-merge commits (93.5%) carry a `Co-Authored-By` trailer naming Claude, Codex or OpenAI.
  - 335 of the maintainer's 339 non-merge commits carry a Claude trailer.
  - Trailer tally: "Claude Opus 5.5" 501 (+6 lower-case, +6 "(1M context)"), "Claude Sonnet 5.5" 18, Codex/OpenAI Codex 11, "Claude Fable 5.1" 5, "Claude Opus 4.8" 1.
  - CLAUDE.md describes an "autonomous operation protocol" that works from a maintainer-local `plan/` directory, which is gitignored.

  (Local analysis of clone @7b6c134; [CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md))
- **History is implausibly compressed for human work.** Example: commits 936b28e→37193b6 on 2026-09-30 between 05:33 and 06:05 add the workspace skeleton, the isobmff mux, the CPU renderer and "~110 commands" (+1.5k, +5.4k and +8.4k lines). (Local analysis of clone @7b6c134)
- **Repo size:**
  - Full bare clone: **24 MB**. History has 4,956 blobs, ~152 MiB uncompressed. The working tree is 36 MB including .git.
  - Largest historical blobs: README PNGs of 0.5–1.6 MB, a 1.04 MB SVG app icon, a 0.74 MB .icns, `av1/src/spec_tables.rs` at 724 KB, and **an accidentally committed test fixture `crates/h264enc/target/agent-h264enc/fixtures/h264enc/oracle/psnr1080.h264` (1.34 MB) and `cbr.h264` (0.57 MB)**.
  - No media corpora are in git.

  (Local analysis of full bare clone)
- **Build scripts:** `crates/text/build.rs` (embeds craft-fonts if `CRAFT_FONTS_DIR` is set), `crates/ui-egui/build.rs` and `apps/filmcraft/build.rs`. (Local analysis of clone @7b6c134)
- **The maintainers' own self-assessment** (2026-10-05):
  - Feature checklist ~87%, "Ready for real work" ~50–60%.
  - Correctness in depth ~60–70%. A first-time contributor found four real bugs in core paths within hours.
  - Performance ~35–40%.
  - Stability "improving, unproven": the never-crash pass only landed 2026-10-04.
  - "Development and testing happen on macOS. Windows and Linux are … barely exercised at runtime."
  - Partly "agent-reported" percentages, and "No automated parity check runs in CI".

  — [ROADMAP.md](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)

### Inferences (judgment)
- Engineering intent and guardrails are unusually good: layering lint, never-crash lint, spec-sourced codecs and oracle tests. Code produced this way is plausible as a base.
- **But the codebase is only 10 days old.** It is changing at over 100 commits a day with no server-side CI gate, and is written almost entirely by AI agents under one maintainer. Internal APIs will churn heavily. Any downstream patch against `engine` or `project` will conflict often: commands.rs was touched by 27 non-merge commits since 2026-10-06, and the `project/src/lib.rs` data model by 9.
- Bus factor ≈ 1 human (the maintainer).

### Gaps
- I could not see whether the public history was replayed from an earlier private repo. The dates suggest either extreme agent throughput or a rebased import; nothing in the repo says which.
- I did not measure test pass rate (no build allowed).

---

## Q3. Project file format, migrations and API stability

### Takeaway
`.fcproj` is a versioned JSON envelope with a disciplined forward-only migration chain. Newer files are refused, and a backup is written before upgrading. But the schema is moving very fast: it reached **v13 in 10 days**. The Rust APIs carry no stability promises: crates are not published to crates.io, all are at 0.4.0, command params are free-text strings, and there is no protocol version.

### Cited Findings
- **Format:** `.fcproj` is "UTF-8 JSON with an explicit schema version" (`"schema_version": 13`). The build writes `filmcraft_format::SCHEMA_VERSION` and reads every version from 1 up to it. — [docs/project-files.md](https://github.com/storytold/filmcraft/blob/main/docs/project-files.md)
- **Migrations:**
  - `MIGRATIONS[i]` upgrades schema i+1 to i+2, and `SCHEMA_VERSION = MIGRATIONS.len() + 1`.
  - "never edit a migration that has shipped".
  - `#[serde(default)]` fields need no migration. New required shapes get a no-op bump so older builds refuse the file.
  - A newer file is refused rather than half-read.
  - A `(schema vN backup).fcproj` is kept on upgrade.

  — [crates/format/src/lib.rs](https://github.com/storytold/filmcraft/blob/main/crates/format/src/lib.rs), [docs/project-files.md](https://github.com/storytold/filmcraft/blob/main/docs/project-files.md)
- **Schema history:** v1 (M0) through v13 ("Multi-stream audio"). Bumps include graphics (v3), Essential Sound (v4), colour management (v5), masks (v7), multicam (v8), transcripts (v9) and graphics design data (v12). — [docs/project-files.md](https://github.com/storytold/filmcraft/blob/main/docs/project-files.md)
- **Saves are atomic:** write a temp sibling, then rename. — [docs/architecture.md §3](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md)
- **Separate versioned formats:** preferences (`version` 2) and effect presets (`"format": "filmcraft.effect-presets", "version": 1`). — [docs/project-files.md](https://github.com/storytold/filmcraft/blob/main/docs/project-files.md)
- **Publishing:** all crates are `version.workspace = 0.4.0`. Only golden and testkit set `publish = false`, but nothing is published. Issue #232 (2026-10-08) asks for the crates, "especially codecs", on crates.io; the user currently consumes them "via Cargo's git support". No maintainer reply. (Local analysis; [issue #232](https://github.com/storytold/filmcraft/issues/232))
- **Command params:** documented as a string per command (for example `r#"{"name":str,"width":u32=1920,…}"#`). Open issue #347 asks for "optional strict validation for command params and export settings". — [commands.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/commands.rs); [issues page 3](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=3)
- **Control-protocol stability:** the docs promise only that UI "Element ids are stable, e.g. `timeline.clip.<id>`…". I found no versioning statement for command ids or the protocol. — [docs/control-protocol.md](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md)

### Inferences
- The file format is the most stable contract FilmCraft has. Resolution should treat `.fcproj` plus command ids as the interface, and pin to a commit for everything else.
- Expect Rust-level breaking changes on every merge.
- Resolution needs its own adapter layer: a `ResolutionEngine` trait over `Session` that isolates `CommandSpec`, `Project` and `EditorState` types.

### Gaps
- No documented deprecation policy for command ids.

---

## Q4. The MCP server

### Takeaway
`filmcraft-automation` is a small (1.4k LOC) rmcp 3.5 server. It runs over **stdio only** and has two modes: headless (in-process `Session`) and bridge (JSON-lines TCP to the running app's control channel). It exposes **generic** tools (`command_run`, `command_batch`, `command_list`), not one MCP tool per command, plus inspection, rendering and UI-automation tools, and two resources.

### Cited Findings
- **Modes:** "official `rmcp` SDK, stdio transport". Headless drives an in-process `filmcraft_engine::Session`. Bridge forwards to `filmcraft --control <port>` so agents can also "inspect, screenshot, click, drag, scroll and type in the live UI". — [crates/automation/src/lib.rs](https://github.com/storytold/filmcraft/blob/main/crates/automation/src/lib.rs)
- **Dependencies:** `rmcp = { version = "3.5", features = ["server","macros","transport-io"] }`. Only internal dependencies are engine and time. — [crates/automation/Cargo.toml](https://github.com/storytold/filmcraft/blob/main/crates/automation/Cargo.toml)
- **Tools** (`#[tool]` in server.rs):
  - `command_list`: every command with id, label, menu, shortcut, param doc and enabled state.
  - `command_run`: `{id, params}`.
  - `command_batch`: `{steps:[{id,params?}], stop_on_error?}`; each edit is its own undo step.
  - `doc_inspect`, `project_inspect`, `sequence_inspect`, `media_import`.
  - `render_frame` and `render_preview`: return PNG.
  - Live app only: `ui_inspect`, `ui_elements`, `ui_click`, `ui_drag`, `ui_key`, `ui_type`, `ui_screenshot`, and `ui_control` (calls any control-channel method).

  — [crates/automation/src/server.rs](https://github.com/storytold/filmcraft/blob/main/crates/automation/src/server.rs)
- **Resources:** `filmcraft://document` (project tree plus active sequence) and `filmcraft://commands` (catalogue), with cache hints. — [server.rs](https://github.com/storytold/filmcraft/blob/main/crates/automation/src/server.rs)
- **Long jobs:** exports get progress notifications and partial-file cleanup (`long_job.rs`). — [crates/automation/src/long_job.rs](https://github.com/storytold/filmcraft/blob/main/crates/automation/src/long_job.rs)
- **Wiring:** `.mcp.json` registers `filmcraft-cli mcp --bridge 127.0.0.1:9876` and `filmcraft-cli mcp --demo`. — [.mcp.json](https://github.com/storytold/filmcraft/blob/main/.mcp.json)
- **Command mapping:** every engine command is reachable through `command_run` by id. Params are free-form JSON validated inside each command; the params doc is a string. — [docs/architecture.md §4](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md)
- **Open MCP bug:** #459, "MCP render_frame: max_side 0 fails…". — [open issues](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen)

### Inferences
- The MCP design is "one generic dispatcher plus a catalogue". That works well for Claude, but there are no per-command JSON Schemas, so tool-level validation is weak.
- For an MCP-first Resolution, the automation crate is a useful reference but too thin to be the product's MCP surface. Resolution likely wants:
  - its own MCP server with streamable-HTTP as well as stdio;
  - typed per-domain tools (edit, color, fairlight, deliver);
  - resources per timeline or bin;
  - the generic `command_run` kept as an escape hatch.
- Bridge mode depends on ui-egui's `control.rs`, so Resolution must implement its own control channel.

### Gaps
- I did not check MCP spec-version negotiation or the rmcp 3.5 features used, beyond tools and resources (no prompts, no sampling).

---

## Q5. Known issues

### Takeaway
At 2026-10-09 there were 140 open and 70 closed issues, filed in about 9 days. The themes, in order, are: real-world media and import robustness (multi-audio-stream, VFR/large MKV, capitalised extensions, alpha, timecode), Windows and Linux install, launch and runtime failures, performance on macOS (playback lag, choppy audio) and with large files, interchange fidelity, and UI and feature gaps. The issue numbers match the maintainers' own ROADMAP admissions.

### Cited Findings
Counts: 140 open, 70 closed. — [open](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen), [closed](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aclosed)

- **Codec, import and media bugs:**
  - Multi-audio-track handling:
    - Open: #397 "Only the first audio track of multi-track video files is imported", #416, #403.
    - Closed: #315 and #312 (the MKV case).
    - Schema v13 "Multi-stream audio" was the response.
  - #288 "H.264 frame cropping (SPS crop offsets) is ignored on import".
  - #432 "WEBM is showing Black".
  - #314 "VP9 import alpha channel rendered black".
  - #402 "lacks support of transparent video files".
  - #460 "Sony FX3 MP4: start timecode in the rtmd metadata track is not read".
  - #262 "Cannot import files where the format is in capitals".
  - #447 "HybridDecoder fallback loses pictures when the software decoder rejects a replayed sample".
  - #414 "LOG & 4K FOOTAGE".
  - Camera RAW requests: #290 BRAW, #383, #345 ProRes RAW.

  — [open p1](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen), [p2](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=2), [p3](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=3), [p4](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=4), [closed](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aclosed)
- **Crashes and freezes:**
  - #439 "filmcraft crashes when creating a new project and importing a video".
  - #330 "The software crashes when running on macOS".
  - #266 "Recover project causes full system reboot".
  - #309 "App freezes and consumes ~12 GB RAM when importing a large (~15 GB, 3.5 h) .mkv file on Windows".
  - #279 "Import reads long WAV files whole on the UI thread: ~1 GB per 90-minute mono WAV".
  - #285 "a closed stdout makes filmcraft-cli panic".
  - Closed #230: "Browser proxy creation and mask tracking crash on unsupported std::time::Instant".

  — same issue pages
- **Performance:**
  - #226 "Virtually unusable on mac": "Media files are laggy and audio is choppy and distorted"; a contributor says #41 may fix it, and there is no maintainer reply.
  - #461 "Importing a Premiere FCP7 XML / OTIO sequence takes 97 s / 66 s for 3 media files".
  - #291 "Slow Warp Stabilizer".
  - #336 "Audio randomly cuts out when multiple video clips overlap".
  - Closed #236 "Audio crackling…".
  - #371 "0.4.0 macOS export: 40 Mbps resolved target produces 6.1 Mbps".

  — [issue #226](https://github.com/storytold/filmcraft/issues/226); issue pages
- **Platform and install:**
  - #380 "does not even run on win11 x64".
  - #413 "Filmcraft does not open after installation".
  - #310 "v4 not installing on windows 10".
  - #268 "Windows portable silently fails to run in Windows 10".
  - #377 "error code 2503 and 2502" (MSI).
  - #457 "UI scale too big on Linux (x11)".
  - #433 GNOME Wayland title bar.
  - #260 Linux "not responding" during a file dialog.
  - #258 "HDR support is missing on linux".
  - #320 "Flickering when utilizing Intel Integrated Graphics".
  - The README documents an AMD `atio6axx.dll` crash workaround: OpenGL is disabled by default.

  — issue pages; [README](https://github.com/storytold/filmcraft/blob/main/README.md)
- **Interchange fidelity:**
  - #463 "FCP7 XML export writes every audio clip as source channel 1".
  - #323 "Completed AAF export fails independent pyaaf2 parsing".
  - Closed #340: FCP7 XML sub-frame spans.
  - The ROADMAP says AAF/OMF are "never … validated in Avid Media Composer or Pro Tools".

  — issue pages; [ROADMAP.md](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- **Text and i18n:** #455 Persian, #395 Arabic, #441 Chinese, #243 Vietnamese diacritics. — issue pages
- **Engine and API bugs found by agents/contributors:**
  - #443 "Test failure: shortcuts_tests::assign_reassigns_conflicts_and_undoes" (closed).
  - #244 "project.delete rejects explicit items unless the Project panel has a selection".
  - #245 "ui.key Ctrl+ chords do not trigger shortcuts on Windows/Linux".
  - #426 "AV1 oracle tests fail instead of skipping when ffmpeg has no libsvtav1/libdav1d".

  — issue pages
- **Resolve-related requests:** #334 "Improving Color Grading Tools and Introducing Node-Based Workflows". — [p3](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=3)

### Inferences
- Decoders that are bit-exact on conformance streams do not yet translate into robust real-world import. Large-file and long-audio memory and threading problems (#309, #279) are architectural, in media/codecs and the UI-thread import path. Resolution would inherit them.
- Windows and Linux are the weakest platforms. That matters if Resolution targets them.

### Gaps
- I did not read individual issue bodies beyond #226 and #232, or labels: the listing showed none, and API access was denied. Closed-issue pages 2–3 were not fetched.

---

## Q6. Fork mechanics

### Takeaway
**My recommendation (judgment): (d) a private mirror repo with an `upstream` remote and a vendor branch, used as a Cargo git dependency, combined with (c)-style pinning in Resolution's own workspace.** Concretely:
- Resolution's private app repo depends on the private mirror's L0–L4 crates by `git = …, rev = …`.
- Resolution's few engine patches live as commits on a `resolution/patches` branch in the mirror, rebased onto each upstream sync.

A git subtree is the main alternative: it is fine because the repo is tiny (24 MB, no big binaries). But at more than 100 upstream commits a day, subtree merges into the app repo become noisy, and patches are harder to keep minimal and visible. Submodules work but add friction for little gain over Cargo git deps.

### Cited Findings (facts that drive the choice)
- **Size:** the full history is 24 MB and has no large media. The largest blobs are 1–1.6 MB PNGs and two stray .h264 fixtures (≤1.34 MB). — local analysis of full bare clone
- **Velocity:** 36–134 commits per day; commands.rs had 27 non-merge touches in 4 days. — local analysis
- **Packaging:**
  - Crates are not on crates.io. They use workspace inheritance (`version.workspace`, `[workspace.dependencies]` path deps, `[lints] workspace = true`).
  - Cargo resolves git dependencies on a workspace member and honours inheritance when it packages them from git. Other users already consume them "via Cargo's git support" (#232).

  — [Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml); [issue #232](https://github.com/storytold/filmcraft/issues/232)
- **Build-time inputs:** `crates/text/build.rs` reads an optional `CRAFT_FONTS_DIR`. Codec crates need `opt-level = 3` in dev for tolerable test speed. That is set in FilmCraft's root `Cargo.toml` profile, which **does not apply when consumed as a dependency**; Resolution must replicate it in its own workspace profile. — [Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml); [AGENTS.md §1.9](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)
- **Forks must remove the ArtCraft brand marks** (`docs/brand/`). — [AGENTS.md §1.8](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)

### Comparison (judgment)

| Option | Upstream merges | Carrying patches | Engine swappability | Notes |
|---|---|---|---|---|
| **(a) git subtree** of filmcraft into the private Resolution repo (`vendor/filmcraft`) | `git subtree pull --squash` per sync. With `--squash`, history stays small but each sync is one big blob commit; without it, 841+ upstream commits enter the app history. | Patches are ordinary commits under `vendor/`. Conflicts are resolved in subtree merges, and the patches are hard to list ("what do we differ by?" needs a diff against an upstream tag). | Fine via path deps | Viable given 24 MB. Tooling is clunky. AI agents in Resolution can edit vendor code freely, which risks unbounded drift. |
| **(b) submodule → private mirror** | Sync the mirror, then bump the submodule SHA. | Patches live in the mirror's branch; their visibility is good. | Path deps into the submodule | Submodule UX friction (recursive clones, CI checkout, detached HEADs). No real advantage over (c) or (d) for Rust. |
| **(c) Cargo git deps pinned to a rev, with `[patch]`** | Bump `rev` in one place. Cargo handles the 40+ crates. `[patch."https://github.com/storytold/filmcraft"] filmcraft-engine = { path = "../patched/engine" }` overrides individual crates. | Patching one crate means copying the whole crate. This duplicates files and makes merging the copied crate manual, so it is bad for ongoing patches and fine for rare hotfixes. | Best: app code sees normal crates; easy to put behind a trait | Pointing directly at public upstream needs no mirror, but leaves you exposed to force-pushes or deletion. |
| **(d) private mirror with `upstream` remote, `vendor` branch tracking upstream, `resolution` branch carrying patches** | `git fetch upstream; git rebase` (or merge) the patch branch onto the new upstream; the vendor branch mirrors it untouched. | Patches stay a small, reviewable stack (`git log upstream/main..resolution`), and each patch can be proposed upstream. | Combined with (c): the app depends on `git = "<private mirror>", rev = …` | **Recommended.** It also gives a private place to run the missing CI (`cargo xtask ci` on Linux, Windows and macOS) before bumping the rev. |

Recommended concrete setup (judgment):
1. **`resolution-filmcraft` (private) mirror.**
   - `main` is a mirror of `storytold/filmcraft:main`, synced by a scheduled job.
   - `resolution` is the patch stack, kept to: a command-registration hook, any visibility (`pub`) changes, removal of ArtCraft branding, and bug fixes destined for upstream PRs.
   - Tag each tested sync point `fc-sync-YYYYMMDD`.
2. **The Resolution app repo (private) has its own workspace.** It depends on `filmcraft-engine`, `-project`, `-render`, `-gpu`, `-export`, `-codecs`, `-scopes`, `-platform` and the others via `{ git = "ssh://…/resolution-filmcraft", rev = "<sync tag sha>" }`. It also copies FilmCraft's dev profile `opt-level` overrides for the codec crates.
3. **`resolution-engine-api` crate.** This is Resolution's own trait boundary: `trait EditEngine { execute, inspect, render_frame, … }` plus Resolution's own DTOs. A `filmcraft-adapter` crate implements it. Resolve-UI and AI/MCP code depend only on the trait crate, so the engine stays swappable and upstream type churn stays inside one adapter crate.
4. **Upstream contributions.** Upstream the generic patches (a command-registry extension point, typed param schemas per #347, publishing crates per #232) to shrink the patch stack.
5. **Sync cadence.** Given the velocity, sync weekly or bi-weekly rather than daily. Gate each sync on the mirror's CI plus Resolution's adapter tests and a `.fcproj` round-trip suite.

### Inferences
- Because the playback and frame-pool code lives in `ui-egui`, Resolution has two options. It can upstream a refactor moving `frames.rs`, `play_ahead.rs` and audio-clock logic into a UI-free crate (for example `filmcraft-playback` at L4/L5), which is the best long-term fix. Or it can copy them into Resolution, where they will drift.
- Avoid depending on `ui-egui` at all.

### Gaps
- I did not test whether `cargo` resolves the git dependency (no build allowed). The `[patch]` semantics described are standard Cargo behaviour, not verified against this repo.

---

## Q7. Resolve-relevant features already in FilmCraft

### Takeaway
Several Resolve-adjacent systems exist but all are modelled on Premiere: a keymap preset system with Premiere, FCP and Avid presets but **no Resolve preset**, a dockable workspace/panel system, scopes, proxies, multicam, interchange (FCP7 XML is "the format Premiere and DaVinci Resolve exchange") and colour management. **There is no node-based colour graph.** Colour is a Lumetri-style layered effect stack, and node workflows are only a feature request (#334). There is no Fairlight-style or Fusion-style page model and no Cut or Media page concept.

### Cited Findings
- **Keymap presets (engine-level):**
  - `crates/engine/src/shortcuts.rs` covers bindings, presets, conflict detection and the `shortcuts.*` commands.
  - Presets are "FilmCraft Default", "Premiere Pro Compatible", "Final Cut Pro Compatible" and "Avid Media Composer Compatible".
  - Tables are in `shortcut_presets.rs` (448 lines: `PREMIERE`, `PREMIERE_PANEL`, `FINAL_CUT`, `AVID`).
  - Panel-scoped shortcuts and Windows preset import are supported.
  - A grep for "Resolve"/"DaVinci" found no preset.

  — [shortcuts.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/shortcuts.rs), [shortcut_presets.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/shortcut_presets.rs)
- **Workspaces and docking (UI-level, egui):**
  - `dock.rs` provides "Premiere-style docking: a tree of splits and tab groups… workspaces are serialized trees".
  - `WORKSPACES = ["Editing","Assembly","Color","Effects","Audio","Captions and Graphics","Learning","Review","All Panels"]`.
  - Saved layouts go in `workspaces.json`.
  - These live in ui-egui and would not be reused by a non-egui front end.

  — [crates/ui-egui/src/dock.rs](https://github.com/storytold/filmcraft/blob/main/crates/ui-egui/src/dock.rs), [panels/workspaces.rs](https://github.com/storytold/filmcraft/blob/main/crates/ui-egui/src/panels/workspaces.rs)
- **Colour:**
  - The Lumetri-style panel covers Basic Correction, Creative looks, Curves including hue-vs curves, three-way wheels, HSL Secondary and Vignette. Grading runs in linear light, f32.
  - Colour management: rec709, rec2100-pq and rec2100-hlg working spaces, plus a LUT library.
  - ASC CDL is a GPU effect.
  - Lumetri itself still runs on the CPU.

  — [README](https://github.com/storytold/filmcraft/blob/main/README.md), [crates/engine/src/color.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/color.rs), [docs/architecture.md](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md), [ROADMAP.md](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- **No node graph:** a grep for "node graph"/"NodeGraph"/"color node" in crates returned nothing. Open #334 requests "Node-Based Workflows". — local grep; [issues p3](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=3)
- **Scopes:** `filmcraft-scopes` (L3, no UI) provides waveform, parade, histogram, YUV and HLS vectorscopes, and numeric summaries for agents, in BT.601, 709 and 2020. — [crates/scopes/src/lib.rs](https://github.com/storytold/filmcraft/blob/main/crates/scopes/src/lib.rs)
- **Proxies and ingest:** `engine/src/proxies.rs` does background transcode to ProRes 422 Proxy/LT or H.264 at ¼ or ½ size, attach and reconnect, toggle proxies, and ingest with fingerprint-verified copy. Export always uses full resolution. — [proxies.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/proxies.rs)
- **Multicam:**
  - `engine/src/multicam.rs`: sync by In, Out, timecode, marker or audio; merge clips; multicam source sequences; enable and flatten.
  - `multicam.switchAngle` and live record/cut with keys 1–9.
  - Schema v8.

  — [multicam.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/multicam.rs)
- **Other Resolve-relevant engine features:**
  - Trim modes: ripple, roll, slip, slide and dynamic JKL trimming.
  - Render-preview cache.
  - Audio mixer with automation modes, plus EBU R128 loudness.
  - Speech-to-text (optional `whisper` feature, candle).
  - Scene detection (`scene_detect.rs`), transcript-based editing.
  - Interchange: EDL, FCP7 XML, FCPXML, OTIO, AAF, OMF.

  — [README](https://github.com/storytold/filmcraft/blob/main/README.md), [docs/architecture.md](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md)

### Inferences
- A Resolve keymap could be added as one more preset table in `shortcut_presets.rs`. That is a small, upstreamable patch, but its ids would be FilmCraft/Premiere command ids.
- Resolve concepts with no engine equivalent would need new engine-level data and commands, which means schema bumps and a divergent `.fcproj`, the hardest kind of fork patch:
  - colour node graph (serial, parallel and layer nodes; versions; stills/gallery);
  - Cut page source tape;
  - Fusion compositions;
  - Fairlight buses.

  Best practice: keep Resolution-specific data in a sidecar or extension namespace rather than modifying `filmcraft-project` types.
- The Resolve UI clean-room obligations are similar to FilmCraft's Adobe rules: no Blackmagic assets, icons or screenshots.

### Gaps
- I did not examine whether `project` has an extension or metadata map that could hold Resolution-specific data without schema patches. OTIO import mentions `metadata.filmcraft`, but I did not check a generic `metadata` field on Project or TrackItem.
