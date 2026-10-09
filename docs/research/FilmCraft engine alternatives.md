# Fork FilmCraft behind a boundary Resolution owns

**Build Resolution on FilmCraft, but treat it as a pinned, fast-moving vendor dependency that sits behind Resolution's own engine trait. Do not treat it as a codebase to merge into.** No other open engine combines everything Resolution needs. FilmCraft has a permissive licence, which the repo's own `Cargo.toml` gives as **MIT OR Apache-2.0**. Its codecs are pure Rust with no FFmpeg. Its commands are typed: about 549 command ids by static count, against a README claim of "more than 650". It also has a built-in MCP server, a wgpu renderer, and pro interchange covering EDL, FCP7 XML, FCPXML, OTIO, AAF and OMF ([Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml), [README](https://github.com/storytold/filmcraft/blob/main/README.md)). The only Rust project with a comparable architecture is Concat, and its AGPL-3.0 licence rules it out for a private derivative unless Resolution buys a commercial licence ([Concat LICENSE-EXCEPTIONS](https://github.com/jub0t/concat/blob/main/LICENSE-EXCEPTIONS.md)). The non-Rust engines (MLT, GStreamer Editing Services) are more mature. Neither, however, has a command/undo API, MCP, a wgpu compositor or a modern colour pipeline, so Resolution would have to rebuild most of what FilmCraft already has. The verdict comes with four caveats:

- **FilmCraft is a Premiere Pro clone, not a Resolve clone.** Colour is a Lumetri-style stack with no node graph.
- **Its public history is ten days old:** 841 commits, 93.5% of non-merge commits co-authored by AI, one human maintainer, and no server-side test CI.
- **Its playback engine lives inside the egui UI crate.** Resolution would have to rebuild it.
- **Its own maintainers rate it ~50–60% "ready for real work"** ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)).

The fork design below is built so those caveats cost Resolution as little as possible: a private mirror, a small patch stack, a trait-isolated adapter, and Resolve-specific data kept in Resolution's own namespace. If FilmCraft stalls, Resolution keeps working because it owns a permissively licensed copy.

## FilmCraft wins because no rival is both permissive and deep

The Rust video-editor field is wide but shallow. GitHub's `video-editor` topic lists 58 Rust repositories, and only two have real traction: FilmCraft (★6.7k) and Concat (★4.4k). Everything else has a few hundred stars at most. Most of the rest are personal or AI-scaffolded projects, and the list even includes a repository named "davince-resolve-crack-download", which is likely a malware lure ([GitHub topic, Rust](https://github.com/topics/video-editor?l=rust), [page 2](https://github.com/topics/video-editor?l=rust&page=2)).

**MCP no longer sets anyone apart.** At least ten Rust editors already ship it: Concat, OpenReelio, UltimateSlice, chukcut, kerf, ShellX Cut, Verbreel, Montagent, wayshot and feathercut. FilmCraft's real moat is the depth of the command surface, its interchange support, and its FFmpeg-free codecs ([openreelio](https://github.com/openreelio/openreelio), [kerf](https://raw.githubusercontent.com/OrellBuehler/kerf/main/README.md), [ShellX Cut](https://raw.githubusercontent.com/martinsbrezauckis/shellx-cut/main/README.md)).

Concat deserves a fair hearing, because it is technically the closest peer and is ahead of FilmCraft in places:

- **Architecture:** windowing-free engine crates with commands and undo, an FFmpeg boundary confined to one crate, and a wgpu compositor running WGSL effect packages.
- **Hardware decode:** VideoToolbox, D3D11VA, MediaCodec and VAAPI.
- **Colour and HDR:** an ACEScct effect space with HLG/PQ input and 10-bit BT.2020 output.
- **APIs:** JSON-RPC (with token auth), gRPC and MCP.

([Concat ARCHITECTURE.md](https://github.com/jub0t/concat/blob/main/ARCHITECTURE.md)). Its licence is the blocker. Concat is **AGPL-3.0-or-later**, and its plugin exception frees only independent modules that talk to it over its API. A modified fork stays AGPL, including the network source-offer clause, and a commercial licence is available only through Concat's CLA ([LICENSE-EXCEPTIONS](https://github.com/jub0t/concat/blob/main/LICENSE-EXCEPTIONS.md)). Concat also documents no interchange formats, which matter to Resolve users.

The permissive alternatives are much weaker. OpenReelio (MIT, ★84, pre-alpha) and avio (MIT/Apache, ★12, a library-only FFmpeg+wgpu timeline with undo) are both small and depend on FFmpeg. Ravel (Apache/MIT open core, ★11) has a node-graph NLE design with OpenColorIO that suits a Color or Fusion page, but it has no releases and no MCP. OpenCut has ★93k and an MIT licence, but its "Rust core" so far contains only `media/setup`, and it does not accept outside contributions ([avio](https://github.com/itsakeyfut/avio), [ravel](https://github.com/narusenia/ravel), [OpenCut](https://github.com/OpenCut-app/OpenCut), [OpenCut crates](https://github.com/OpenCut-app/OpenCut/tree/main/crates)).

Several projects are disqualified by licence:

- **Copyleft:** UltimateSlice, chukcut and Miniter are GPL-3.0.
- **Source-available, not open source:** kerf (PolyForm Noncommercial) and Tooscut (Elastic 2.0).
- **Custom licence:** Smelter requires a paid licence to bundle it into products.

OxiMedia calls itself "production-grade". Its own README, however, admits that its AV1 and VP9 encoders "do not emit a valid bitstream" ([smelter LICENSE](https://raw.githubusercontent.com/software-mansion/smelter/master/LICENSE), [oximedia](https://github.com/cool-japan/oximedia)).

Among non-Rust engines, MLT is the strongest fallback on maturity. Its core is LGPL-2.1, its 7.36.x releases are current, and it powers Kdenlive and Shotcut. GES has official generated Rust bindings in gstreamer-rs. Both still lack Resolution's core requirements: no typed command or undo layer, no MCP, no wgpu compositor and no OCIO/ACES pipeline. Wrapping either one means rebuilding most of FilmCraft's work on top of a C/FFmpeg stack that has to be packaged for three operating systems ([MLT copyright policy](https://mltframework.org/docs/copyrightpolicy/), [Wikipedia: MLT](https://en.wikipedia.org/wiki/Media_Lovin%27_Toolkit), [gstreamer-rs GES docs](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer_editing_services/)). The rest are ruled out:

- **Olive** has the most Resolve-like design (nodes and OCIO), but it is GPL-3.0 and its README still calls it "highly unstable" alpha software ([olive](https://github.com/olive-editor/olive)).
- **Blender VSE, Natron and Cinelerra-GG** are GPL applications. Resolution could only drive them as separate processes, never link them.
- **Remotion and Diffusion Studio** carry company or commercial licences that conflict with a later MIT/Apache release ([Remotion licence](https://remotion.dev/license), [Diffusion Studio v3](https://docs.diffusion.studio/docs/version-3)).

### Ranked recommendation and fallbacks

| Rank | Option | When to use it | Why it ranks here |
|---|---|---|---|
| **1 (recommended)** | **FilmCraft L0–L4 crates via a private mirror, behind a Resolution-owned `EditEngine` trait** | Now | Permissive licence. Pure-Rust codecs. ~549+ commands. MCP. wgpu. EDL/FCP7/FCPXML/OTIO/AAF/OMF. Proxies, multicam, scopes and trims already exist. |
| 2 | **Own the FilmCraft fork outright** (stop syncing upstream) | Upstream stalls, pivots or relicenses future work | MIT/Apache means the code you already hold is yours to keep. Funding depends on ArtCraft's AI-token business ([PetaPixel](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/)). |
| 3 | **FilmCraft plus a pluggable I/O backend**: dynamically linked LGPL FFmpeg via `rsmpeg`/`ffmpeg-next`, or GStreamer via gstreamer-rs | Pure-Rust decoders fail on real camera media | Keeps the timeline, command and MCP layers and swaps only decode. Avoids a wholesale engine change ([rsmpeg](https://crates.io/crates/rsmpeg), [gstreamer](https://crates.io/crates/gstreamer)). |
| 4 | **Custom engine on a permissive Rust stack**: avio's model, FilmCraft's codec crates, wgpu, vello/parley, cpal/rubato/symphonia | FilmCraft's engine layer proves unusable but its codecs hold up | EffectCraft already consumes FilmCraft's codecs, which shows they are reusable ([effectcraft](https://github.com/storytold/effectcraft)). No maintained Rust OTIO or OCIO exists, so Resolution would write those ([crates.io ocio](https://crates.io/crates/ocio)). |
| 5 | **MLT via a hand-written bindgen crate** (dynamic LGPL; exclude the `plusgpl`, Movit and `melt` GPL parts) | Maturity matters more than Rust purity | The most production-proven open NLE engine, but Resolution still has to build the command/undo/MCP layer, the GPU compositor and OCIO ([MLT policy](https://mltframework.org/docs/copyrightpolicy/), [OpenBSD mlt7-gpl2](https://openports.pl/path/multimedia/mlt7,-gpl2)). |
| 6 | **GES via gstreamer-rs** | Hardware decode breadth and official bindings matter most | LGPL with official bindings. The API is not thread-safe, the NLE community is small, and Pitivi moves slowly ([GES docs](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer_editing_services/)). |
| 7 (conditional) | **Concat, under a negotiated commercial licence** | Resolution accepts paying for a licence, or an AGPL open-source future | The best architecture peer, with stronger HDR and hardware decode. AGPL and the CLA constrain the licensing options ([Concat](https://github.com/jub0t/concat)). |

Some projects are worth watching but not adopting. **EffectCraft** (MIT/Apache, the After Effects sibling, which shares FilmCraft's time model and codecs) is the natural complement for a Fusion-style page. **Ravel** is a design reference for a node-based Color page with OCIO. **OpenCut's Rust rewrite** becomes the most credible permissive challenger if it ships ([effectcraft](https://github.com/storytold/effectcraft), [ravel](https://github.com/narusenia/ravel), [OpenCut](https://github.com/OpenCut-app/OpenCut)).

### Licence discrepancy resolved: the repo files say MIT OR Apache-2.0

The storytold organisation page lists FilmCraft as "Apache-2.0" ([storytold org](https://github.com/storytold)). The primary sources disagree. The workspace `Cargo.toml` declares `license = "MIT OR Apache-2.0"`, the NOTICE file requires that notice to be preserved, and CONTRIBUTING.md accepts contributions under the same dual licence ([Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml), [NOTICE](https://github.com/storytold/filmcraft/blob/main/NOTICE), [CONTRIBUTING.md](https://github.com/storytold/filmcraft/blob/main/CONTRIBUTING.md)). The org-page label is most likely GitHub's licence detector picking up only one of the two licence files. Treat **MIT OR Apache-2.0** as authoritative.

Two further licence notes apply:

- **Brand marks.** The ArtCraft name and logos are covered by a separate brand licence. Distributed modified versions must remove them, though they may say they are "based on FilmCraft by the ArtCraft team" ([LICENSE-brand.txt](https://github.com/storytold/filmcraft/blob/main/docs/brand/LICENSE-brand.txt)).
- **Third-party licences inside the tree.** symphonia is MPL-2.0, which is file-level copyleft. The bundled fonts are OFL-1.1 ([NOTICE](https://github.com/storytold/filmcraft/blob/main/NOTICE)).

## What you inherit: a Premiere-shaped engine that is ten days old

A local audit of `main` at commit `7b6c134` (2026-10-09) supports the engine's headline claims. It also shows where Resolution's work actually lies.

The workspace contains about **322k lines of Rust in 47 packages**. Layering is machine-enforced by `cargo xtask layers`: dependencies may only point downward, egui/winit/cpal are banned below L5, and every L0–L4 crate must build for wasm32. The workspace forbids `unsafe` everywhere except the FFI crate `platform` ([docs/architecture.md](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md), [xtask](https://github.com/storytold/filmcraft/blob/main/xtask/src/main.rs), [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)). The code is test-dense, with **2,230 `#[test]`s**, 27 golden PNG renders and ffmpeg-oracle comparisons ([docs/testing.md](https://github.com/storytold/filmcraft/blob/main/docs/testing.md)). The `.fcproj` project format is the most stable contract FilmCraft offers:

- versioned JSON;
- forward-only migrations;
- refusal of files newer than the build;
- a backup written before any upgrade;
- atomic saves.

([docs/project-files.md](https://github.com/storytold/filmcraft/blob/main/docs/project-files.md), [crates/format](https://github.com/storytold/filmcraft/blob/main/crates/format/src/lib.rs)).

The boundary between engine and UI holds for edits but leaks for playback. Every mutation goes through `Session::execute`, as the README promises when it says "a different front end can replace it without touching editing logic". However, playback-critical code lives in `ui-egui`:

- `frames.rs`, the 1,053-line background frame worker pool and cache;
- `play_ahead.rs`, the audio ring-buffer mixer;
- `source_playback.rs`;
- `control.rs`, the 555-line control-channel handler that MCP bridge mode depends on.

The cpal audio master clock sits in `apps/filmcraft`. UI-only commands such as tools, playback, zoom and workspaces are in `ui-egui/src/menus.rs` ([README](https://github.com/storytold/filmcraft/blob/main/README.md), [docs/architecture.md §4](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md)). Two more structural limits matter:

- **The command registry is closed.** It is a static `OnceLock<Vec<CommandSpec>>` with no external registration API, and parameter schemas are free-form strings such as `{"time":ticks?}` rather than JSON Schema ([commands.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/commands.rs)).
- **The engine is coarse.** `filmcraft-engine` is a single 46k-line crate with 18 internal dependencies, so Resolution takes all of it or none of it.

**The product model is Premiere's, which matters most for a Resolve clone.** Shortcut presets cover FilmCraft Default, Premiere, Final Cut Pro and Avid, with no Resolve preset. The docking workspaces are Premiere's ("Editing", "Assembly", "Color"…) and are written in egui. Colour is a Lumetri-style layered stack: Basic, Creative, Curves, Wheels, HSL Secondary and Vignette, running in linear-light f32 on the CPU, with ACES-style working spaces and a GPU ASC CDL effect. **There is no node graph**. Node workflows exist only as feature request #334 ([shortcut_presets.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/shortcut_presets.rs), [dock.rs](https://github.com/storytold/filmcraft/blob/main/crates/ui-egui/src/dock.rs), [engine color.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/color.rs), [issue #334 listing](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=3)). Resolve concepts that need new engine-level data have no FilmCraft equivalent:

- serial, parallel and layer colour nodes with versions and a stills gallery;
- the Cut page's source tape;
- Fusion compositions;
- Fairlight buses.

Adding these is the most expensive kind of fork patch, because each one means a schema bump and a `.fcproj` that diverges from upstream. Several pieces carry over directly and form a solid head start:

- proxies (ProRes Proxy/LT, H.264 at ¼ or ½ size);
- multicam with audio and timecode sync;
- ripple, roll, slip, slide and JKL trims;
- a scopes crate with numeric summaries for agents;
- an EBU R128 audio mixer;
- optional Whisper speech-to-text.

([proxies.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/proxies.rs), [multicam.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/multicam.rs), [scopes](https://github.com/storytold/filmcraft/blob/main/crates/scopes/src/lib.rs)).

**Velocity and provenance are the biggest structural risks.** Public history starts on 2026-09-30, and since then:

- **841 commits** have landed at 36–134 per day;
- 504 of 539 non-merge commits (93.5%) carry Claude, Codex or OpenAI co-author trailers;
- the maintainer, Brandon Thomas, authored 632 of the commits;
- `commands.rs` was touched by 27 non-merge commits in four days;
- the project schema went from v1 to **v13 in ten days**.

([commit history](https://github.com/storytold/filmcraft/commits/main), [CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md), [docs/project-files.md](https://github.com/storytold/filmcraft/blob/main/docs/project-files.md)). None of the four GitHub workflows runs `cargo test`. The `cargo xtask ci` gate runs only on the maintainer's machine, and "CI that runs the test suite" on Windows and Linux is still a to-do ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)).

The maintainers' own self-assessment is unusually candid:

- feature checklist ~87%;
- ready for real work ~50–60%;
- correctness in depth ~60–70%;
- performance ~35–40%.

The same ROADMAP says the percentages are "partly agent-reported" with "no automated parity check" in CI. It also says development happens on macOS, while "Windows and Linux are … barely exercised at runtime" ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)). The issue tracker, with 140 open and 70 closed issues in about nine days, backs this up ([open issues](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen)):

- **Media import:** multi-audio-track import (#397), black WebM (#432), ignored SPS cropping (#288).
- **Memory:** about 12 GB of RAM on a 15 GB MKV (#309).
- **Playback:** "virtually unusable on mac" (#226).
- **Install and launch:** Windows launch failures (#380, #268).

One discrepancy is worth carrying forward. The README claims more than 650 commands, while a static count finds 549 distinct ids, with macros generating more. Read the claims as directionally true but self-reported.

## Three repositories and one trait keep upstream mergeable

A FilmCraft fork has to absorb roughly 100 upstream commits a day without drift, keep Resolution's patches small and visible, and let the engine be replaced later. The repo's size makes any mechanism feasible: a full bare clone is 24 MB with no media corpora. The two stray `.h264` fixtures under `crates/h264enc/target/` are the only accidental binaries ([commit history](https://github.com/storytold/filmcraft/commits/main)). Velocity is what decides the mechanism.

| Option | Verdict | Reason |
|---|---|---|
| git subtree into the app repo | Viable, not preferred | With `--squash`, each sync arrives as one large blob commit; without it, more than 841 upstream commits flood the app history. Asking "how do we differ from upstream?" requires a diff against a tag. AI agents working in the app repo can edit vendored code freely, so drift has no bound. |
| git submodule | Rejected | Recursive clones, CI checkouts and detached HEADs add friction, with no advantage over Cargo git dependencies for Rust. |
| Cargo git dependency pinned to `rev`, plus `[patch]` | Use for consumption and hotfixes | Bumping one `rev` moves all 40+ crates together. `[patch]` copies a whole crate, so it suits only rare emergency fixes. |
| **Private mirror with an upstream-tracking branch and a patch branch, consumed through a Cargo git dependency** | **Recommended** | Patches stay a small stack that can be reviewed with `git log upstream/main..resolution`, and each patch can be proposed upstream. The mirror also provides a place to run the CI that upstream lacks. |

The recommended setup has three repositories and one trait.

**First, the private mirror, `resolution-filmcraft`.** It has three parts:

- **`main`** is an untouched mirror of `storytold/filmcraft:main`, updated by a scheduled job.
- **`resolution`** is the patch branch, rebased onto each sync. It should hold only four kinds of patch: a command-registration hook (`register_commands(Vec<CommandSpec>)`), `pub` visibility changes, removal of ArtCraft branding for distributed builds, and bug fixes that are on their way upstream.
- **Tested sync points** are tagged `fc-sync-YYYYMMDD`.

The mirror's CI runs `cargo xtask ci` on Linux, Windows and macOS, the cross-platform coverage upstream lacks, before any tag is cut. Prefer upstream release tags (v0.4.0 shipped on 2026-10-08) as sync bases when they are recent. Otherwise sync weekly or every two weeks, not daily.

**Second, the Resolution app workspace.** It depends on the L0–L4 crates with `{ git = "…/resolution-filmcraft", rev = "<fc-sync sha>" }`. Those crates are the codec and container crates plus time, geom, color, frame, media, project, audio-dsp, text, edit, codecs, interchange, captions, speech, format, render, gpu, scopes, export and engine, and optionally `platform` for hardware codecs. The workspace has two jobs here:

- **Copy FilmCraft's dev-profile `opt-level` overrides for the codec crates.** A dependency's root profile does not apply to its consumers, so without them decoding in tests is slow ([Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml), [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)).
- **Never depend on `ui-egui`, `apps/*` or `automation`.** Depending on them pulls in egui, and they are the parts Resolution replaces.

**Third, the boundary: a `resolution-engine-api` crate.** It defines `trait EditEngine` (execute, inspect, render_frame, playback hooks) together with Resolution-owned data types. A `filmcraft-adapter` crate is the only code that imports FilmCraft types such as `Session`, `CommandSpec`, `Project` and `EditorState`. The Resolve UI, the MCP server and the AI features depend only on the trait crate. Upstream churn then stays inside one crate, and an alternative engine (fallbacks 3–6) means writing one new adapter rather than rewriting the product.

Resolve-specific state belongs in a Resolution namespace, not in `filmcraft-project` types. That covers the colour node graph, page layouts and Fairlight bus routing, kept either as a sidecar file or an extension map. One open question remains: whether `project` exposes a generic metadata map. OTIO import writes `metadata.filmcraft`, but a general field on Project or TrackItem was not confirmed. Check this first, because the answer decides whether the hardest Resolve features need upstream schema changes at all.

**Playback is the one place to spend upstream goodwill early.** Resolution can copy the frame pool, play-ahead mixer and audio clock (about 1.5k lines) out of `ui-egui`, where the copy will drift. The better option is to propose a refactor that moves them into a UI-free `filmcraft-playback` crate at L4/L5. That patch is generic, benefits upstream's own web and CLI front ends, and removes Resolution's largest maintenance burden. Other patches are worth upstreaming for the same reason:

- the command-registry hook;
- a Resolve keymap table in `shortcut_presets.rs`, though its ids remain FilmCraft/Premiere ids;
- typed parameter validation, which issue #347 already requests;
- publishing the crates to crates.io, which issue #232 requests and which would let Resolution swap git `rev`s for semver ([issue #232](https://github.com/storytold/filmcraft/issues/232), [issues page 3](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen&page=3)).

Upstream merged 21 community PRs in a single day on 2026-10-06, so it accepts outside contributions quickly and does not require a CLA ([ROADMAP log](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md), [CONTRIBUTING.md](https://github.com/storytold/filmcraft/blob/main/CONTRIBUTING.md)). Keep a separate, pristine fork for upstream PRs. The brand licence lets a fork that exists only to send changes upstream keep the ArtCraft marks ([LICENSE-brand.txt](https://github.com/storytold/filmcraft/blob/main/docs/brand/LICENSE-brand.txt)).

Each sync should pass a gate before the `rev` moves:

1. Mirror CI passes on all three operating systems.
2. The adapter's contract tests pass.
3. A `.fcproj` round-trip suite passes, including migration from the previous schema.
4. Command ids are diffed against the last sync. Upstream promises stability only for UI element ids, so a renamed command breaks MCP clients without warning ([docs/control-protocol.md](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md)).

Because Resolution is AI-native, add one more guard: a CI check that fails if the app repo's lockfile points at anything but a tagged sync point, or if the mirror's `resolution` branch touches files outside an allow-listed set. Agents will otherwise "fix" engine code in place. This guard is a judgement call, not something the sources prescribe.

## Expose Resolve's seven pages, not 549 commands

FilmCraft's `filmcraft-automation` crate is a 1.4k-line rmcp 3.5 server that runs over **stdio only**. It works in two modes:

- **headless:** an in-process `Session`;
- **bridge:** a JSON-lines TCP connection to the running app's `--control 9876` channel.

It exposes generic tools rather than one tool per command:

- `command_list`, `command_run` and `command_batch`;
- inspection tools;
- `render_frame` and `render_preview`, which return PNGs;
- `ui_click`, `ui_drag`, `ui_type` and `ui_screenshot` for driving the live UI.

Two resources round it out: `filmcraft://document` and `filmcraft://commands` ([server.rs](https://github.com/storytold/filmcraft/blob/main/crates/automation/src/server.rs), [lib.rs](https://github.com/storytold/filmcraft/blob/main/crates/automation/src/lib.rs), [.mcp.json](https://github.com/storytold/filmcraft/blob/main/.mcp.json)). It is a sound reference but too thin to be the product's MCP surface. There are no per-command schemas, no HTTP transport and no draft model, and bridge mode depends on egui's `control.rs`.

The most mature video MCP servers have converged on patterns that Resolution should copy:

- **Compound tools.** samuelgursky/davinci-resolve-mcp (MIT, about 3.4k stars) ships a compound server of 37 tools covering 136 guarded actions, alongside a granular 389-tool server. Every compound response uses a shared `_operation` envelope ([samuelgursky](https://github.com/samuelgursky/davinci-resolve-mcp)).
- **Lazy loading.** hoyt-harness/davinci-mcp-professional keeps six always-loaded "kernel" tools and loads 289 domain tools on demand through `notifications/tools/list_changed`, so "context cost at session start is bounded by the kernel alone" ([hoyt-harness](https://github.com/hoyt-harness/davinci-mcp-professional)).
- **Discovery and preflight.** Kinocut (Apache-2.0, 203 tools) adds a `search_tools` discovery tool, preflight validation before rendering, hash-chained "receipts" with resume cursors, and fail-closed human approval ([Kinocut](https://github.com/KyaniteLabs/mcp-video)).
- **A cautionary gap.** OpenChatCut admits it has no per-session draft or version layer for external agents ([Product Hunt](https://www.producthunt.com/products/openchatcut)).

Every Resolve MCP server is also hostage to Blackmagic's scripting API. They need Studio and Python 3.10/3.11, and the free-edition bridge breaks in Resolve 21.1, where the Scripts menu no longer lists `.py` files ([samuelgursky](https://github.com/samuelgursky/davinci-resolve-mcp), [tooflex](https://github.com/tooflex/davinci-resolve-mcp)). That is the strongest market argument for Resolution's thesis that the engine itself should be the API.

| Design decision | Resolution's choice | Lesson source |
|---|---|---|
| Tool surface | About 7–9 page-scoped domains (Media, Cut, Edit, Fusion, Color, Fairlight, Deliver, plus Project/System), loaded lazily. A small kernel (`list_domains`, `activate_domain`, `switch_page`, `search_commands`, `describe_command`). `command_run` kept as the escape hatch for the long tail. | hoyt-harness kernel and domains; samuelgursky compound tools; FilmCraft `command_run` |
| Schemas | Per-command JSON Schema generated from typed parameters, replacing FilmCraft's free-text param strings. Upstream this as #347. | FilmCraft [commands.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/commands.rs) weakness |
| Result envelope | One envelope with operation id, diff summary, undo-group id and `known_limitation` objects | samuelgursky `_operation` |
| Safety | Dry-run on every mutating tool. Risk tiers with `acknowledge` flags for destructive operations. `inspect_operation` returns blast radius. The timeline is snapshotted before mutation. Source media is immutable, and analysis output goes to sidecars. | samuelgursky; Kinocut preflight |
| Agent isolation | Agents work on a draft timeline or branch that the user reviews and merges | OpenChatCut's admitted gap |
| Provenance | Hash-chained receipts per operation; traces that omit parameters and paths | Kinocut receipts; samuelgursky traces |
| Transport and auth | stdio by default; streamable HTTP bound to loopback with a per-launch bearer token; HMAC plus nonce for any in-app bridge. Never expose an unauthenticated TCP control port. | samuelgursky; Concat token-authenticated socket ([ARCHITECTURE.md](https://github.com/jub0t/concat/blob/main/ARCHITECTURE.md)) |
| Offline operation | The headless engine owns the API and the GUI is just another client; project-file tools work without the app running | samuelgursky's 18 offline `.drp` tools |
| Visual feedback | `render_frame`, plus numeric scope summaries, so agents can grade without pixels | FilmCraft `render_frame` and `filmcraft-scopes` |

Open bug #459 ("MCP render_frame: max_side 0 fails…") shows that FilmCraft's MCP surface is as young as the rest of the engine ([open issues](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen)).

## Watch the ROADMAP log, not the press

The highest-signal source is FilmCraft's own **ROADMAP.md**. Its dated "Log", "Honest assessment" and priority-ordered "Where we are lacking" sections are written for agents as well as humans, and they work as the public plan of record. The real plan sits in a gitignored `plan/` directory ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md), [CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md)). The repo has no CHANGELOG.md, SECURITY.md or governance file ([raw check](https://raw.githubusercontent.com/storytold/filmcraft/main/CHANGELOG.md)). Press coverage was a launch spike that focused mostly on PhotoCraft. No specific Hacker News or Reddit thread could be confirmed.

| Priority | Source | URL | What to watch | Cadence |
|---|---|---|---|---|
| 1 | ROADMAP raw file (automate a diff of "Log" and "Honest assessment") | https://raw.githubusercontent.com/storytold/filmcraft/main/ROADMAP.md | Breaking changes: schema bumps, command renames, playback and hardware-acceleration work, parity numbers | Daily, automated |
| 2 | Commits feed | https://github.com/storytold/filmcraft/commits/main.atom | Churn in `engine/src/commands.rs`, `project`, `format`, `platform` | Daily, filtered by path |
| 3 | Releases and tags | https://github.com/storytold/filmcraft/releases.atom, https://github.com/storytold/filmcraft/tags.atom | Candidate sync bases (current: v0.4.0, 2026-10-08) | On push |
| 4 | Issues, especially #30 (hardware acceleration), #232 (crates.io), #347 (typed params), #334 (node colour) | https://github.com/storytold/filmcraft/issues, https://github.com/storytold/filmcraft/issues/30 | Decisions that shrink or grow Resolution's patch stack | Weekly |
| 5 | craftrules (cross-app standards) | https://github.com/storytold/craftrules | Rule changes (never-crash, fonts, corpora) that ripple into every app | Weekly |
| 6 | EffectCraft | https://github.com/storytold/effectcraft | Compositing and effects work, and a possible Fusion-page source | Weekly |
| 7 | PhotoCraft (parity tooling) and photocraft-corpus | https://github.com/storytold/photocraft, https://github.com/storytold/photocraft-corpus | `cargo xtask parity` being ported to FilmCraft; shared real-media corpora | Monthly |
| 8 | Discord | https://discord.gg/artcraft | Informal roadmap talk and bug triage | Weekly skim |
| 9 | Maintainer on X / GitHub | https://x.com/echelon, https://github.com/echelon | Strategy, commercial and licensing statements | Weekly |
| 10 | VideoHelp release tracker | https://www.videohelp.com/software/FilmCraft | Passive release alerts | On notification |
| 11 | Competitors: Concat, OpenCut Rust rewrite, Ravel | https://github.com/jub0t/concat, https://github.com/OpenCut-app/OpenCut, https://github.com/narusenia/ravel | A permissive challenger shipping; design references | Monthly |
| 12 | Forks building on the engine | e.g. https://github.com/pwilkin/filmcraft, https://github.com/mweinbach/filmcraft | Who else is building on the engine, and fixes worth cherry-picking | Monthly |

The launch coverage is useful once, for sentiment and legal framing, and not for tracking. Design Week's hostile take on security and AI trust is the most useful critical piece ([Design Week](https://www.designweek.co.uk/first-ai-came-and-scooped-up-your-art-now-its-scooped-up-adobe/)). The useful outlets are:

- **Hype and the walk-back:** PetaPixel on the claim of "100% parity in a month", later revised to "months not years" ([PetaPixel](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/)).
- **Trade dress:** BigGo Finance and Notebookcheck ([BigGo](https://finance.biggo.com/news/3969ffe0-e181-451e-8cc7-f21136f246ed), [Notebookcheck](https://www.notebookcheck.net/Photoshop-Premiere-more-Experienced-Rust-developer-recreates-Adobe-suite-with-AI-assistance-as-open-source.1419230.0.html)).
- **Launch framing:** TechSpot, Gizmodo and Futurism ([TechSpot](https://www.techspot.com/news/114136-software-over-developer-uses-ai-build-free-adobe.html), [Gizmodo](https://gizmodo.com/someone-vibe-coded-a-free-knockoff-of-adobe-creative-suite-2000823322), [Futurism](https://futurism.com/future-society/adobe-creative-suite-software-open-source-artcraft)).

Do not confuse `storytold/artcraft` with the suite. It is the separate, older AI image and video "crafting engine" with about 10.8k stars ([storytold org](https://github.com/storytold)).

## Risk register: churn, codecs and the word "Resolve"

Severity reflects Resolution's position: a private repo now, possibly distributed or open-sourced later. Legal entries are inferences from public precedent, not legal advice.

| # | Risk / gotcha | Evidence | Severity | Mitigation |
|---|---|---|---|---|
| 1 | **Upstream churn breaks the adapter on every sync** | 841 commits in 10 days; schema v13 in 10 days; `commands.rs` touched 27 times in 4 days; no stability promise for command ids ([commits](https://github.com/storytold/filmcraft/commits/main), [control-protocol.md](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md)) | High | Pin to tested sync tags; keep all FilmCraft types inside `filmcraft-adapter`; diff command ids per sync; automate rebases |
| 2 | **No server-side CI; unverified test pass rate** | No workflow runs `cargo test`; the gate is local, and takes 20–60 minutes under load ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)) | High | Run `cargo xtask ci` on all three operating systems in the mirror before tagging; offer the CI config upstream |
| 3 | **Bus factor ≈ 1, AI-authored, private plan** | One owner and integrator; 93.5% of non-merge commits carry AI trailers; `plan/` is gitignored; funding tied to ArtCraft's token sales ([CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md), [PetaPixel](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/)) | High | Budget to own the engine (fallback #2); build in-house understanding of `engine`, `render` and `codecs`; keep the trait boundary |
| 4 | **Premiere-shaped data model** | No node graph; Lumetri colour; Premiere workspaces and ids; no Resolve keymap ([shortcut_presets.rs](https://github.com/storytold/filmcraft/blob/main/crates/engine/src/shortcut_presets.rs)) | High | Keep Resolve-specific state in Resolution's namespace; map Resolve commands onto FilmCraft ids in the adapter; upstream a Resolve keymap |
| 5 | **Playback lives in the UI crate** | `frames.rs`, `play_ahead.rs`, `control.rs` and the audio clock sit outside the engine ([architecture.md](https://github.com/storytold/filmcraft/blob/main/docs/architecture.md)) | High | Upstream a `filmcraft-playback` crate, or own a port; never depend on `ui-egui` |
| 6 | **Real-world media robustness** | Decoders are bit-exact on conformance streams, but "camera and phone files in the wild are less tested"; no camera RAW or E-AC-3; open issues #397, #432, #288, #309, #279 ([README](https://github.com/storytold/filmcraft/blob/main/README.md), [issues](https://github.com/storytold/filmcraft/issues?q=is%3Aissue+is%3Aopen)) | High | Build a private camera and phone corpus now; put decode behind a trait so FFmpeg or GStreamer can be added (fallback #3) |
| 7 | **Performance is CPU-bound** | Lumetri, keys, masks and export run on the CPU; 31 effects run on the GPU, but the UI badges more as GPU-accelerated; "8K isn't real-time"; Mac playback lag (#226) ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md), [#226](https://github.com/storytold/filmcraft/issues/226)) | High for a Resolve-class colour page | Plan a GPU colour pipeline as Resolution's own work; track upstream #30 |
| 8 | **Export and encoder gaps** | H.264 is the only software encoder; HEVC export is hardware-only; no AV1 export; ProRes 422 HQ only; ProRes output is validated only against ffmpeg/ffprobe ([README](https://github.com/storytold/filmcraft/blob/main/README.md), [testing.md](https://github.com/storytold/filmcraft/blob/main/docs/testing.md)) | Medium | Test deliverables in Resolve, Premiere and QuickTime; prefer OS encoders for HEVC |
| 9 | **Windows and Linux are the weakest platforms** | Development is on macOS; Windows launch failures (#380, #268, #439); AMD OpenGL crash workaround; Linux ALSA problems and UI scaling (#457) ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md), [README](https://github.com/storytold/filmcraft/blob/main/README.md)) | Medium–High | Make Resolution's CI run smoke tests on all three operating systems |
| 10 | **Security of hostile media and control surfaces** | Seeded mutation fuzzing only, with no cargo-fuzz or OSS-Fuzz; `unsafe` FFI in `platform` passes bitstreams to drivers; a JSON-lines TCP control port and UI-automation tools; no SECURITY.md ([AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md), [testing.md](https://github.com/storytold/filmcraft/blob/main/docs/testing.md), [README](https://github.com/storytold/filmcraft/blob/main/README.md)) | High | Continuous coverage-guided fuzzing of codec and container crates; loopback plus token auth on every port; sandbox decode; publish Resolution's own SECURITY.md |
| 11 | **Codec patents on distribution** | FilmCraft ships its own H.264 encoder and decoder and an HEVC decoder. Via LA AVC is $0 for the first 100k units a year per entity, then $0.20 per unit. HEVC Advance (29,000+ patents) shows no comparable free tier. FilmCraft documents no patent position ([Via LA AVC fees](https://www.via-la.com/wp-content/uploads/avcweb.pdf), [Access Advance](https://accessadvance.com/wp-content/uploads/2025/07/HEVC-Advance-Program-Overview-July-2026.pdf)) | Low while private; High once distributed | Before distribution, route HEVC (and optionally H.264) through OS codecs such as VideoToolbox and Media Foundation; get licensing counsel; make software codecs a build feature |
| 12 | **Licence and brand obligations** | MIT OR Apache-2.0, with NOTICE preserved; the ArtCraft marks must be removed from distributed modified builds; symphonia is MPL-2.0 and slated "to be replaced by our own"; fonts are OFL-1.1 ([NOTICE](https://github.com/storytold/filmcraft/blob/main/NOTICE), [LICENSE-brand.txt](https://github.com/storytold/filmcraft/blob/main/docs/brand/LICENSE-brand.txt)) | Medium | Brand-removal patch in the mirror; NOTICE carried into Resolution; licence-scan CI with cargo-deny |
| 13 | **Blackmagic trade dress** | Press flagged trade dress as ArtCraft's main legal exposure. Lotus v. Borland makes menu hierarchies uncopyrightable "methods of operation", but Lotus v. Paperback held direct copying of screen displays infringing ([BigGo](https://finance.biggo.com/news/3969ffe0-e181-451e-8cc7-f21136f246ed), [Look and feel](https://en.wikipedia.org/wiki/Look_and_feel), [computerlaw.com](https://computerlaw.com/articles/look-and-feel-in-computer-software/)) | Low while private; Medium–High once distributed | Copy workflow, page structure and hotkeys, which are functional. Use original icons, colours and styling; no Blackmagic assets or screenshots in the repo; adopt FilmCraft's clean-room rules for Resolve |
| 14 | **The name "Resolution" next to "DaVinci Resolve"** | "DAVINCI RESOLVE" is a registered or applied-for mark in the same goods class ([Justia](https://trademark.justia.com/877/32/davinci-87732717.html)) | Medium, rising on public release | Run a trademark clearance before any public release; keep "Resolution" as an internal codename until then |
| 15 | **Clean-room claims cannot be verified** | Clean-room is the maintainer's own characterisation; the agents are trained LLMs and the reference material is private ([Yahoo Tech](https://tech.yahoo.com/ai/claude/articles/software-over-developer-uses-ai-174550981.html), [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)) | Low–Medium | Record provenance of the inherited code; keep codec crates swappable |
| 16 | **Self-reported metrics** | Parity percentages are "partly agent-reported"; the README claims 650+ commands against 549 counted; star counts vary by date (4,463 around Oct 8 vs ★6.7k on Oct 9) ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md), [Pillitteri](https://pasqualepillitteri.it/en/news/21619/free-adobe-clones-artcraft-claude-opus-5-5)) | Medium | Benchmark independently before committing to features; trust code, not claims |
| 17 | **Unvalidated interchange** | AAF and OMF have "never been validated in Avid Media Composer or Pro Tools"; FCP7 audio-channel bug #463; AAF fails parsing by pyaaf2 (#323) ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)) | Medium (Resolve round-trips) | Test FCP7 XML, FCPXML and OTIO round-trips against real Resolve exports |
| 18 | **No plugin hosting** | No OpenFX, VST3 or AU; an "owner decision" is needed because it requires `unsafe` FFI ([ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)) | Medium for pros | Treat OFX hosting as Resolution-owned work in its own crate |
| 19 | **Build and tooling gotchas** | Codec `opt-level` overrides are not inherited by consumers; each worktree build takes 6–7 GB; MSRV 1.95, edition 2024; stray `.h264` fixtures in history ([Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml), [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)) | Low | Copy the profile overrides; size CI runners for the build; pin the toolchain |
| 20 | **Supply-chain lures** | A "davince-resolve-crack-download" repo appears under the Rust video-editor topic ([GitHub topic p2](https://github.com/topics/video-editor?l=rust&page=2)) | Low | Allow-list dependency sources; vet any repo before cloning |

Several questions remain open: Concat's commercial licence terms, how good the pure-Rust decoders are on real camera footage compared with FFmpeg, whether `project` already has an extension map, and whether the patent pools treat free open-source distribution differently. Answer the second and third before committing to the roadmap.

## Conclusion

The fact that FilmCraft is a Premiere clone shapes Resolution more than the engine choice does. Choosing FilmCraft is close to forced, because no other permissively licensed engine comes near its breadth. But it means Resolution's real product work sits exactly where FilmCraft is thinnest: a node-based GPU colour page, a Fairlight-style bus model, a playback engine outside the UI, and Resolve's page structure. The engine supplies the plumbing (codecs, timeline edits, interchange, proxies, multicam, export). Resolution's own code owns its identity. That is why the `EditEngine` trait and a Resolution-owned data namespace matter more than the git mechanics. They let Resolution use FilmCraft's speed without being bound to its product shape, and they turn "swap the engine" from a rewrite into one new adapter.

The second takeaway is that FilmCraft's velocity is both its value and its main hazard, and the response is automation rather than caution. Upstream lands about 100 agent-written commits a day with no server CI. The fork therefore has to be the place where cross-platform CI, fuzzing, `.fcproj` round-trips and command-id diffing run on every sync. Resolution should also put its upstream effort into the few generic patches that shrink its own patch stack: playback extraction, command registration, typed parameters and crates.io publishing. If those land, Resolution becomes one of the main downstream consumers of a fast-moving permissive engine. If they don't, the MIT/Apache licence still means Resolution can take the code and own it outright.
