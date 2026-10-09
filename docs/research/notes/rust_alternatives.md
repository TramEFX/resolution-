# Rust-native alternatives to FilmCraft as the engine for "Resolution" (as of 2026-10-09)

Method note: GitHub search API and HN Algolia were blocked from this sandbox, so discovery used the GitHub topic pages (`video-editor`, `nle`, filtered to Rust), the crates.io API (keyword searches plus per-crate metadata), web search, and READMEs/LICENSE files fetched from raw.githubusercontent.com. Star counts and dates come from those pages on 2026-10-09. Contributor counts were mostly not visible on fetched pages and are listed as gaps.

## Q1. Which Rust NLEs / video editing engines exist and are active as of Oct 2026?

### Takeaway
The GitHub `video-editor` topic filtered to Rust lists 58 repos. By stars, FilmCraft (6.7k) and Concat (4.4k) are far ahead of the rest. Every other Rust NLE is either a single-digit to low-hundreds-star personal or AI-scaffolded project, or an adjacent tool (live compositor, motion-graphics renderer, research engine) rather than a pro NLE engine. Since late 2025 there has been a burst of small "agent-native / MCP" Rust editors. None comes close to FilmCraft in scope.

### Cited Findings

**Landscape snapshot (GitHub topic `video-editor`, Rust filter)**
- The page reports 58 matching Rust repos. The top entries on 2026-10-09: storytold/filmcraft ★6.7k; jub0t/concat ★4.4k ("CapCut alternative… with MCP support", beta); mlm-games/Miniter ★90; openreelio/openreelio ★84 (pre-alpha); heng30/wayshot ★65; 1mrnewton/cutlass ★58; ChaosRealmsAI/NextFrame ★24 (last updated Apr 26 2026); waterblower/opencut ★21; iyxan23/dalang ★17 (MLT plus Actix, **last updated Jan 22 2024, stale**); Hugo4IT/Vide ★12; narusenia/ravel ★11; OrellBuehler/kerf ★8; martinsbrezauckis/shellx-cut ★6; geneva-render/geneva ★6; chuk-development/chukcut ★4; kmwallio/UltimateSlice ★4. — [GitHub topic video-editor (Rust)](https://github.com/topics/video-editor?l=rust)
- Page 2 of the topic lists only 0–3★ projects. Many are MCP/agent editors: MBehtemam/Montagent (CLI + MCP), rdh073/verbreel-engine ("agent-native video editor engine with CLI, MCP, HTTP, and WASM"), lwdot90/feathercut ("local MCP tools"), vfempire-hq/vf-media-engine (FFmpeg timeline engine), xXKuroiKenshiXx/OpenPremier, Gerbzz/Clipstrike (last updated Mar 22 2026). It also includes a repo named "davince-resolve-crack-download", a likely malware lure to avoid. — [GitHub topic page 2](https://github.com/topics/video-editor?l=rust&page=2)
- The `nle` topic (Rust) has 6 repos, all with 0–4★: UltimateSlice, Pixygon/Reel, blobbyofficial/Kite, wuwangzhang1216/agentcut ("headless, CLI-based NLE for coding agents… FFmpeg"), salom600/feathercut, Qamar2315/SiloCut. — [GitHub topic nle (Rust)](https://github.com/topics/nle?l=rust)

**Concat (jub0t/concat), the only other project with real traction**
- AGPL-3.0-or-later. ★4.4k, 413 forks, 880 commits. Version badge is inconsistent (0.2.5 vs 0.2.6) and the Releases section is empty. Self-described as "in beta: it works, and it still has edges." Offers "a JSON-RPC, gRPC and MCP API, so scripts and AI agents can cut video with it too". Export to H.264/HEVC/AV1 "up to 4K 60, 10-bit colour", plus "GPU compositor, proxies, hardware decode", 170+ effects, local Whisper captions, TTS/voice cloning, background removal. Targets macOS/Windows/Linux/Android/iOS. — [concat repo](https://github.com/jub0t/concat), [README](https://raw.githubusercontent.com/jub0t/concat/main/README.md)
- Architecture (ARCHITECTURE.md): the engine is windowing-free crates (`concat-core`, `concat-project` [document model, commands, undo], `concat-media` [FFmpeg; "the FFmpeg boundary is concat-media and nothing else links it"], `concat-effects`, `concat-render` [wgpu `WgpuCompositor`], `concat-export`, `concat-host`, `concat-api`, `concat-server`, `concat-cli`). The UI is **Slint**. Hardware decode uses VideoToolbox, D3D11VA, MediaCodec, and VAAPI (VAAPI only when explicitly named), with software fallback. Effects are WGSL packages validated with naga and can run in "linear, display, or ACEScct log space". The working space is linear Rec.709 primaries in half-float, with HDR (HLG/PQ) in and 10-bit HEVC/AV1 BT.2020 out. Undo is snapshot-based with depth 200. JSON-RPC runs over TCP/Unix socket with token auth, and gRPC is behind a feature flag. Interchange formats (OTIO/FCPXML/EDL) are **not mentioned**. — [concat ARCHITECTURE.md](https://github.com/jub0t/concat/blob/main/ARCHITECTURE.md)
- The license-exceptions file is a "Concat Plugin Exception" that only frees *independent modules* that talk to Concat via its API, IPC, or file formats. It says Concat cannot be conveyed "under terms other than the AGPL". A separate commercial license is available because contributions are under a CLA. — [LICENSE-EXCEPTIONS.md](https://github.com/jub0t/concat/blob/main/LICENSE-EXCEPTIONS.md)
- Third-party listings say Concat was formerly "WolfCut" and describe a React UI. That contradicts the repo's ARCHITECTURE.md, which says Slint, and the listings may be stale. They also report lower star counts (2.2k / 3.8k) taken at earlier dates. — [opensourcealternatives.to](https://www.opensourcealternatives.to/item/concat), [wpde.org thread](https://wpde.org/thread/203233-concat-die-freie-open-source-alternative-zu-capcut-leistungsf%C3%A4hige-videobearbeit/)

**Storytold siblings**
- The storytold org has 43 repos, all clean-room Adobe reimplementations in Rust: photocraft ★33.8k, artcraft ★10.8k, lightcraft ★7.4k, filmcraft ★6.7k, pdfcraft ★6.0k, vectorcraft ★4.9k, effectcraft ★3.7k, and craft-libs (★8, Apache-2.0, "Rust libraries for the Craft apps"). — [storytold org](https://github.com/storytold)
- EffectCraft is a clean-room After Effects reimplementation in pure Rust, MIT OR Apache-2.0, ★3.7k, 1.6k forks, 1,001 commits, first commit "on 1 October 2026". It is a stack of strictly layered crates with egui on top. It ships `effectcraft-cli mcp` (stdio, headless or bridged to the running app with `--bridge 9877`) and headless rendering. It "shares its time model and text engine design with FilmCraft, and gets its video and audio codecs from it" (H.264, ProRes, AAC), with "no FFmpeg inside". It says it "isn't yet a replacement for After Effects on client work". — [effectcraft repo](https://github.com/storytold/effectcraft)
- The org page lists filmcraft's license as Apache-2.0 only. The brief says MIT OR Apache-2.0, so the GitHub sidebar may be detecting just one of the two files. — [storytold org](https://github.com/storytold)

**Other notable engines and editors (one line each)**
- **OpenReelio**: MIT, ★84, 2,115 commits, Tauri 2 + React UI with a Rust core (`commands`, `ai`, `plugin`, `qc`), FFmpeg, event-sourced unlimited undo, headless CLI + MCP server ("read-only by default"), WASM (Wasmtime) plugins. Pre-alpha (<0.5.0). Interchange not mentioned. — [openreelio](https://github.com/openreelio/openreelio)
- **Gausian Native Editor**: core MPL-2.0 with commercial "Pro" features, ★1.0k but only 24 commits from essentially one contributor. Uses egui + wgpu, FFmpeg/ffprobe probing, GStreamer proxies/decode, and VideoToolbox on macOS. Exports FCPXML 1.9/1.10, FCP7 XML, and EDL. Has a ComfyUI integration and a headless CLI, but no MCP. Color management and LUTs are on the roadmap only. Web search reported the last push as Nov 19 2025 (**≈11 months, near-stale**). — [Gausian repo](https://github.com/gausian-AI/Gausian_native_editor), [gittrend](https://gittrend.io/repo/gausian-AI/Gausian_native_editor)
- **Ravel** (narusenia): Apache-2.0/MIT open core with commercial premium, ★11 but 2,539 commits. Node-graph (DAG) NLE with "timeline… syntactic sugar over sequence nodes". UI is GPUI-CE (Zed's GPUI community edition), with wgpu, FFmpeg (LGPL, dynamic) + native HW decoders, **OpenColorIO**, WGSL effects, Lua scripting, and an OpenFX shim. No MCP and no OTIO/EDL/AAF mentioned. macOS primary, Linux planned. — [ravel](https://github.com/narusenia/ravel)
- **OpenCut** (OpenCut-app): MIT, ★93.3k, but primarily a web/TypeScript editor. It "is being rewritten from the ground up" toward a "Rust core" for desktop, mobile, and browser. MCP server, headless mode, and Editor API are *planned*, not shipped. The README says "We're not set up to take outside contributions yet". The repo's `crates/` shows only `media/setup` so far. — [OpenCut](https://github.com/OpenCut-app/OpenCut), [crates dir](https://github.com/OpenCut-app/OpenCut/tree/main/crates)
- **UltimateSlice** (kmwallio): **GPL-3.0**, FCP-inspired, GTK4 + GStreamer, FFmpeg on PATH for export, FCPXML 1.10–1.14 import and 1.14 export, optional `--mcp` JSON-RPC server. The README says much of it was made with AI. — [UltimateSlice README](https://raw.githubusercontent.com/kmwallio/UltimateSlice/main/README.md)
- **Clipstrike** (Gerbzz): multi-crate workspace (core/audio/media/render/ui) with egui + wgpu and GStreamer. "usable as a development build", ★2, last updated Mar 2026. — [Clipstrike README](https://raw.githubusercontent.com/Gerbzz/Clipstrike/main/README.md)
- **chukcut**: **GPL-3.0**, Rust engine, GPU compositor, web UI, FFmpeg, 20 GPU effects, `chukcut-cli mcp`. — [chukcut README](https://raw.githubusercontent.com/chuk-development/chukcut/main/README.md)
- **kerf**: **PolyForm Noncommercial** (not OSS), Tauri 2, drives the `ffmpeg` CLI, 87 MCP tools with "staged edits with a reviewable diff" and edit history attributed to user/agent/system. — [kerf README](https://raw.githubusercontent.com/OrellBuehler/kerf/main/README.md)
- **ShellX Cut**: MIT, a schema-generated registry shared by REST, CLI, and MCP ("exact invalid-argument parity across direct dispatch, REST, CLI, and MCP"), `--headless` mode, depends on FFmpeg. ★6. — [shellx-cut README](https://raw.githubusercontent.com/martinsbrezauckis/shellx-cut/main/README.md)
- **Verbreel Engine**: agent-native engine with CLI, MCP (rmcp), and HTTP, `verbreel-render` (wgpu/WGSL), a WASM preview crate, and FFmpeg. ★0. — [verbreel README](https://raw.githubusercontent.com/rdh073/verbreel-engine/main/README.md)
- **Montagent**: MIT, a single binary that is both CLI and MCP server, needs `ffmpeg` ≥7.1 with libx264, statically links Skia. — [Montagent README](https://raw.githubusercontent.com/MBehtemam/Montagent/main/README.md)
- **cutlass** (1mrnewton): MIT/Apache-2.0, prompt-driven ("describe the edit"), "early alpha", has a mobile engine crate. ★58. — [cutlass README](https://raw.githubusercontent.com/1mrnewton/cutlass/main/README.md)
- **wayshot** (heng30): Apache-2.0 | GPLv3 (dual badge), Slint UI, ffmpeg-next/x264/openh264, H.264 MP4 export, and an MCP server with 50+ ops, though the author says "AI operation results are not great, currently incomplete". — [wayshot README](https://raw.githubusercontent.com/heng30/wayshot/main/README.md)
- **Miniter**: **GPL-3.0**, a prototype trim/concat editor limited by FFmpeg Android libs. — [Miniter README](https://raw.githubusercontent.com/mlm-games/Miniter/main/README.md)
- **Tooscut**: browser NLE with a Rust/WASM WebGPU compositor, **Elastic License 2.0** (source-available, not OSS). — [tooscut README](https://raw.githubusercontent.com/mohebifar/tooscut/main/README.md)
- **geneva**: MIT, JSON edits with HTML/CSS overlays rendered without a browser. The Rust planner/compositor/layout engine uses FFmpeg libs for codecs. A Remotion-style renderer, not an NLE. — [geneva README](https://raw.githubusercontent.com/geneva-render/geneva/main/README.md)
- **Smelter** (Software Mansion, formerly live_compositor): ★734, 1,527 commits, "real-time, low latency, programmable video and audio mixing", React-component scenes. Uses a **custom Software Mansion license**: offline processing is MIT-like, but live streaming has company-size/viewer/machine caps, and "embedding or bundling the Software into products for distribution or sale" needs a commercial license (from $1,000/month). — [smelter](https://github.com/software-mansion/smelter), [LICENSE](https://raw.githubusercontent.com/software-mansion/smelter/master/LICENSE)
- **Vidformer** (ixlab/OSU): Apache-2.0, ★68, a research "video data transformation" engine with a Rust core, Python cv2-compatible frontend, HLS serving, and a `misc/vidformer-mcp`. Built on FFmpeg. Last crate release v1.4.0 on 2026-02-02. — [vidformer](https://github.com/ixlab/vidformer), [crates.io](https://crates.io/crates/vidformer)
- **avio** (itsakeyfut): MIT OR Apache-2.0, ★12, 1,921 commits, crate v0.18.4 (2026-09-29). A library "editing engine": tracks, clips, keyframes, wgpu GPU compositing (`ff-render`), undo/redo, serde model, over safe FFmpeg wrappers (`ff-decode`/`ff-encode`/`ff-filter`…). HW decode/encode covers NVENC/NVDEC, QSV, AMF, VideoToolbox, and VAAPI. No interchange and no MCP. — [avio](https://github.com/itsakeyfut/avio)
- **OxiMedia** (cool-japan): Apache-2.0, ★262, only 12 commits but 115 workspace crates published. Includes `oximedia-edit`, `-timeline`, `-edl` (CMX3600/GVG/Sony), `-aaf` (ST 377-1 read/write), `-conform`, `-colormgmt` (ICC/ACES/LUT), and `-gpu` (wgpu). It **never supports H.264/H.265/AAC** ("patent encumbrance"). Its own README says AV1/VP9 encoders do "not emit a valid bitstream" and Opus is "untrustworthy", yet it calls itself "production-grade… 111 Stable". This looks AI-generated or overclaimed. — [oximedia](https://github.com/cool-japan/oximedia)
- **Lunaris** (node-based NLE on crates.io): `lunaris` 0.1.0, last updated 2025-11-21, and the app repo has 1 commit and 0★. **Stale placeholder.** — [crates.io search](https://crates.io/api/v1/crates?q=video%20editor), [lunaris_app](https://github.com/lunaris-systems/lunaris_app)
- **dalang** (MLT + Actix client-server editor): last updated Jan 22 2024. **Stale.** — [GitHub topic](https://github.com/topics/video-editor?l=rust)
- Motion-Canvas/Remotion-style Rust renderers found on crates.io: `motion-canvas-rs` 0.2.4 (2026-05-26, "vector animation engine inspired by Motion Canvas"), `rustmotion` 0.8.1 (2026-10-01, renders motion design videos from JSON), and Dioxuscut 0.2.0 (2026-09-23, Remotion-compatible animation primitives). These are not NLEs. — [crates.io API keyword searches](https://crates.io/api/v1/crates?q=video%20timeline)
- Search also surfaced a job ad for a commercial (closed) Rust + wgpu "video production engine" with an agent-driven timeline API and WASM/server parity. The ad is undated. — [djinni job](https://djinni.co/jobs/851902-senior-rust-graphics-engineer-wgpu-video-rend/)

### Inferences
- In this Rust space, being "AI-native/MCP-first" is no longer a differentiator. At least 10 Rust editors ship MCP: Concat, OpenReelio, UltimateSlice, chukcut, kerf, ShellX Cut, Verbreel, Montagent, wayshot, and feathercut. What sets FilmCraft apart is the depth of its typed command surface (650+), pro interchange, and FFmpeg-free codecs, not MCP itself.
- Many of the 2026 entrants show signs of heavy AI-assisted generation: huge commit counts against tiny star counts, self-audit prose, and READMEs that say so outright, as UltimateSlice's does. Treat their feature claims as unverified until tested.

### Gaps
- Contributor counts and exact last-commit dates for most repos were not visible on the fetched pages, and the GitHub API was blocked. Concat's contributor count and first-commit date are unknown.
- Concat's MCP tool count and command surface size are unknown because its docs site (concatenate.pages.dev) was unreachable from the sandbox.
- No Hacker News or r/rust threads could be retrieved (HN Algolia was blocked, and web search found none specific to 2026), so community reception of Concat and FilmCraft is unverified.

## Q2. How do the credible engines compare to FilmCraft on license, headless architecture, automation/MCP, codecs, GPU, color, and interchange?

### Takeaway
Only Concat matches FilmCraft's shape: a headless engine-crate workspace with a command/undo model, wgpu compositor, and a JSON-RPC/gRPC/MCP API. Concat even beats FilmCraft on hardware decode, HDR, and color-space handling. However, it is **AGPL-3.0**, which rules it out for a private derivative unless the project buys a commercial license. Every permissively licensed alternative is far less mature, relies on FFmpeg (bringing LGPL/GPL obligations), and lacks FilmCraft's breadth of interchange.

### Cited Findings
Comparison matrix (FilmCraft row from the task brief, not re-researched):

| Project | License | Headless engine vs UI | Automation / MCP | Codecs / HW decode | GPU compositing | Color | Interchange | Maturity / traction |
|---|---|---|---|---|---|---|---|---|
| FilmCraft (yardstick) | MIT OR Apache-2.0 | `engine` crate; egui in a replaceable `ui-egui` crate | 650+ typed commands, built-in MCP | Own Rust codecs (H.264, HEVC, ProRes, AAC, VP9, Opus, AV1, DNxHD), no FFmpeg | wgpu | (other researcher) | OTIO, FCPXML, FCP7, EDL, AAF, OMF | v0.4.0 pre-alpha, ★6.7k |
| Concat | **AGPL-3.0-or-later** + plugin exception, CLA, commercial license available | Yes: engine crates have no windowing; Slint UI on top | JSON-RPC (TCP/Unix, token auth), gRPC (feature), MCP, CLI | FFmpeg (isolated in `concat-media`); HW decode VideoToolbox/D3D11VA/MediaCodec/VAAPI | wgpu `WgpuCompositor`, WGSL effect packages | Linear Rec.709 half-float, ACEScct effect space, HLG/PQ in, 10-bit BT.2020 out | Not mentioned | Beta 0.2.x, ★4.4k, 880 commits |
| OpenReelio | MIT | Rust core + Tauri/React | Headless CLI + MCP (read-only default) | FFmpeg; NVENC/AMF/QSV/VT encode | Not mentioned | Not mentioned | Not mentioned | Pre-alpha, ★84 |
| Ravel | Apache/MIT open core | Layered core/media/GPU + GPUI-CE UI | Lua; no MCP | FFmpeg (LGPL dynamic) + native HW decoders | wgpu, WGSL | **OCIO** | Own `.ravprj` (RON); OpenFX shim | No releases, ★11 |
| Gausian | MPL-2.0 core + commercial Pro | Crates + egui app | Headless CLI, ComfyUI; no MCP | FFmpeg + GStreamer + VideoToolbox | wgpu preview | Roadmap only | FCPXML 1.9/1.10, FCP7 XML, EDL | 24 commits, ~stale since Nov 2025 |
| avio | MIT/Apache-2.0 | Library only (no UI) | Rust API only | FFmpeg; NVDEC/QSV/AMF/VT/VAAPI | wgpu | Not mentioned | None (serde) | 0.18.x, ★12 |
| UltimateSlice | **GPL-3.0** | GTK4 app | `--mcp` JSON-RPC | GStreamer + FFmpeg CLI | — | — | FCPXML 1.10–1.14 | ★4 |
| OxiMedia | Apache-2.0 | Library framework | Lua broadcast automation | No H.264/HEVC/AAC by policy; broken encoders | wgpu compute | ICC/ACES/LUT | EDL, AAF, conform | 12 commits, overclaimed |
| Smelter | Custom SWM license (bundling in sold products needs paid license) | Server + SDKs | HTTP/React SDK | gpu-video (Vulkan Video) | wgpu (per crate desc) | — | — | Live-only focus |

Sources: [concat ARCHITECTURE.md](https://github.com/jub0t/concat/blob/main/ARCHITECTURE.md), [concat](https://github.com/jub0t/concat), [openreelio](https://github.com/openreelio/openreelio), [ravel](https://github.com/narusenia/ravel), [Gausian](https://github.com/gausian-AI/Gausian_native_editor), [avio](https://github.com/itsakeyfut/avio), [UltimateSlice README](https://raw.githubusercontent.com/kmwallio/UltimateSlice/main/README.md), [oximedia](https://github.com/cool-japan/oximedia), [smelter LICENSE](https://raw.githubusercontent.com/software-mansion/smelter/master/LICENSE), [gpu-video crate](https://crates.io/crates/gpu-video)

- The Concat Plugin Exception covers only separate modules. A fork or modification stays AGPL, including the §13 network source-offer requirement. — [LICENSE-EXCEPTIONS.md](https://github.com/jub0t/concat/blob/main/LICENSE-EXCEPTIONS.md)
- UltimateSlice states that gstreamer-rs + GStreamer are LGPL-2.1-or-later and that its FFmpeg Flatpak build enables GPL options. This illustrates the copyleft baggage of GStreamer/FFmpeg-based editors, even though the gstreamer-rs binding crates themselves are MIT OR Apache-2.0. — [UltimateSlice README](https://raw.githubusercontent.com/kmwallio/UltimateSlice/main/README.md), [crates.io gstreamer](https://crates.io/crates/gstreamer)

### Inferences
- Concat is the only project that is technically a peer of FilmCraft. Its engine/UI split, command/undo layer, socket API, and wgpu compositor are architecturally the closest match, and its HW-decode and HDR/ACEScct story may be ahead. But the AGPL plus CLA model means a private Resolution derivative would need a negotiated commercial license, and an eventual open-source release would have to be AGPL. That conflicts with the user's licensing goal.
- Concat's FFmpeg dependency adds LGPL/GPL obligations of its own. FilmCraft's FFmpeg-free codec stack is a real licensing advantage for a private derivative, though patent licensing for H.264/HEVC/AAC encoders remains a separate issue regardless of who wrote the code.
- Ravel is the most interesting permissive design for a Resolve-like product, since it combines a node graph (like Resolve's Fusion/Color pages) with OCIO. But at ★11 with no releases, macOS-first, and no MCP, it is not a base. It is worth watching as a design reference for a Color/Fusion page.
- None of the alternatives document AAF/OMF. Only Gausian (FCPXML/FCP7/EDL), UltimateSlice (FCPXML), and OxiMedia (EDL/AAF, unproven) touch interchange at all.

### Gaps
- Concat's command count, undo semantics under API calls, and whether its MCP is bundled in the engine or the app were not confirmed.
- The color pipelines of OpenReelio, avio, and Clipstrike are undocumented.
- No benchmark comparisons between any of these and FilmCraft exist.

## Q3. Rust building blocks per layer, if Resolution ever had to swap away from FilmCraft

### Takeaway
A credible permissive fallback stack exists: FFmpeg bindings or pure-Rust decoders, wgpu/vello for compositing and titles, cpal/rubato/symphonia for audio, and gstreamer-rs as an alternative media layer. The weak layers are **OTIO** (no maintained Rust binding) and **OCIO** (only a brand-new pure-Rust port). FilmCraft's own codecs, and EffectCraft's reuse of them, may themselves be the best pure-Rust codec layer available.

### Cited Findings
All rows are from crates.io API metadata fetched 2026-10-09 (version | license | last update | total downloads).

**Decode/encode via FFmpeg**
- `ffmpeg-next` 9.0.0 | WTFPL bindings (FFmpeg itself is LGPL/GPL) | 2026-08-05 | 7.7M dl. — [crates.io](https://crates.io/crates/ffmpeg-next)
- `rsmpeg` 0.18.0+ffmpeg.8.0 | MIT | 2025-08-24 | 295k. — [crates.io](https://crates.io/crates/rsmpeg)
- `video-rs` 0.12.0 | MIT/Apache | 2026-09-09. — [crates.io](https://crates.io/crates/video-rs)
- `ffmpeg-sidecar` 2.6.0 | MIT | 2026-09-30 | 2.0M (drives the ffmpeg CLI). — [crates.io](https://crates.io/crates/ffmpeg-sidecar)
- avio's `ff-*` crates: MIT/Apache safe wrappers with HW accel, 0.18.4. — [avio](https://github.com/itsakeyfut/avio)

**Hardware codecs**
- `gpu-video` 0.4.0 | MIT | 2026-05-12 ("hardware video coding using Vulkan Video, with wgpu integration"; renamed from `vk-video`). — [crates.io](https://crates.io/crates/gpu-video)
- `cros-codecs` 0.0.6 | BSD-3 | 2025-06-18 (VAAPI/V4L2, Linux). — [crates.io](https://crates.io/crates/cros-codecs)
- `nvidia-video-codec-sdk` 0.4.0 | MIT | 2025-09-15. — [crates.io](https://crates.io/crates/nvidia-video-codec-sdk)

**Pure-Rust or permissive codecs**
- `rav1e` 0.8.1 | BSD-2 | 2025-06-16 (AV1 encode, 51M dl). — [crates.io](https://crates.io/crates/rav1e)
- `rav1d` 1.1.0 | BSD-2 | 2025-05-07 (Rust port of dav1d). — [crates.io](https://crates.io/crates/rav1d)
- `dav1d` 0.11.1 | MIT | 2025-11-25. — [crates.io](https://crates.io/crates/dav1d)
- `openh264` 0.9.8 | BSD-2 | 2026-08-08. — [crates.io](https://crates.io/crates/openh264)
- `rusty_h264` 0.16.0 (2026-09-07, "pure-Rust H.264 encoder and decoder… bit-exact vs Cisco openh264"). — [crates.io search](https://crates.io/api/v1/crates?q=h264%20decoder)
- `rust_h265` 0.1.0 (pure-Rust HEVC Main/Main10 decoder, 2026-04-22). — [crates.io search](https://crates.io/api/v1/crates?q=hevc%20decoder)

**OxideAV (pure-Rust FFmpeg-like framework)**
- MIT, ~95 crates, H.264 decode and CABAC/High10 encode, HEVC decode and partial encode, ProRes (all six RDD 36 profiles), AV1 decode, AAC-LC, and a macOS VideoToolbox bridge. Self-described as "still in heavy development", with no v1 tag. Workspace ★42. — [OxideAV org](https://github.com/OxideAV)

**Containers**
- `re_mp4` 0.5.1 | MIT | 2026-07-08. — [crates.io](https://crates.io/crates/re_mp4)
- `mp4-atom` 0.16.2 | MIT/Apache | 2026-10-06. — [crates.io](https://crates.io/crates/mp4-atom)
- `muxide` 0.2.5 | MIT/Apache (pure-Rust MP4 muxer). — [crates.io](https://crates.io/crates/muxide)
- `matroska-demuxer` 0.8.1 | Zlib/MIT/Apache. — [crates.io](https://crates.io/crates/matroska-demuxer)
- `mp4` 0.14.0 | MIT, last update **2023-08-01 (stale)**. — [crates.io](https://crates.io/crates/mp4)
- `re_video` 0.38.1 | MIT OR Apache-2.0 | 2026-09-16 (Rerun's container-parse and decode unifier that "wraps other crates"). — [crates.io](https://crates.io/crates/re_video), [README](https://raw.githubusercontent.com/rerun-io/rerun/main/crates/utils/re_video/README.md)

**Media framework alternative**
- `gstreamer` and `gstreamer-editing-services` 0.25.4 | MIT/Apache bindings | 2026-09-21. The underlying libraries are LGPL. — [crates.io gstreamer](https://crates.io/crates/gstreamer), [crates.io GES](https://crates.io/crates/gstreamer-editing-services)

**GPU, 2D, and text**
- `wgpu` 30.0.1 | MIT/Apache | 2026-08-22. — [crates.io](https://crates.io/crates/wgpu)
- `vello` 0.11.0 | Apache/MIT | 2026-10-02. — [crates.io](https://crates.io/crates/vello)
- `velato` 0.12.0 (Lottie on vello). — [crates.io](https://crates.io/crates/velato)
- `parley` 0.12.0, `cosmic-text` 0.19.0, `glyphon` 0.12.0 (text). — [parley](https://crates.io/crates/parley), [cosmic-text](https://crates.io/crates/cosmic-text), [glyphon](https://crates.io/crates/glyphon)

**Audio**
- `cpal` 0.18.2 | Apache-2.0. — [crates.io](https://crates.io/crates/cpal)
- `symphonia` 0.6.1 | **MPL-2.0** | 2026-08-13 (pure-Rust demux + audio decode). — [crates.io](https://crates.io/crates/symphonia)
- `rubato` 5.0.1 (resampling). — [crates.io](https://crates.io/crates/rubato)
- `fundsp` 0.23.0 (DSP). — [crates.io](https://crates.io/crates/fundsp)
- `kira` 0.12.5. — [crates.io](https://crates.io/crates/kira)

**Color**
- `ocio` 0.1.0 | MIT AND BSD-3 | first published 2026-10-04 ("pure Rust port of OpenColorIO"), 293 downloads. — [crates.io](https://crates.io/crates/ocio)
- `opencolorio` 0.1.0, a 2020 **placeholder (stale)**. — [crates.io](https://crates.io/crates/opencolorio)
- `colorutils-rs` 0.8.0 (BSD-3/Apache). — [crates.io](https://crates.io/crates/colorutils-rs)

**Interchange**
- `opentimelineio` / `opentimelineio-sys`: 2020 placeholders (0.1.0 / 0.0.0), **stale**. — [crates.io](https://crates.io/api/v1/crates?q=opentimelineio&per_page=20)
- `otio-types` 0.9.1 (2026-08-24) is project-specific ("Pulse OTIO profile"). — same
- `oximedia-edl` / `oximedia-aaf` exist but are unproven. — same, [oximedia](https://github.com/cool-japan/oximedia)

**Time model**
- `mediatime` 0.5.0 (2026-10-08, "Exact-integer rational time types… FFmpeg-style Timebase"). — [crates.io search](https://crates.io/api/v1/crates?q=nle)

**ML (for AI features)**
- `burn` 0.22.0, `candle-core` 0.11.0 (MIT/Apache). — [burn](https://crates.io/crates/burn), [candle-core](https://crates.io/crates/candle-core)
- `whisper-rs` 0.16.0 (Unlicense). — [whisper-rs](https://crates.io/crates/whisper-rs)

**UI alternatives to egui** (relevant to a Resolve-clone front end)
- GPUI via **Kael** (Apache-2.0 GPUI derivative, with optional `kael_media_engines` "timeline, compositing, audio, and export engines", crate 0.4.1 updated 2026-08-25). — [kael README](https://raw.githubusercontent.com/Augani/kael/main/README.md), [crates.io search](https://crates.io/api/v1/crates?q=nle)
- Slint (Concat, wayshot) and GPUI-CE (Ravel). — [concat ARCHITECTURE.md](https://github.com/jub0t/concat/blob/main/ARCHITECTURE.md), [ravel](https://github.com/narusenia/ravel)

### Inferences
- Recommended permissive fallback stack:
  - Media I/O: `rsmpeg` (MIT, tracks FFmpeg 8) or `ffmpeg-next` against an LGPL-only, dynamically linked FFmpeg build. `gpu-video` plus `cros-codecs`/NVIDIA SDK for HW paths.
  - GPU: `wgpu` + `vello`/`parley` for compositor and titles.
  - Audio: `cpal` + `rubato` + `symphonia` (note that symphonia is MPL-2.0, which is file-level copyleft and acceptable for private use).
  - Color: hand-rolled ACES/OCIO-config handling, since `ocio` is days old.
  - Interchange: write an OTIO JSON (de)serializer yourself or shell out to Python OTIO. No maintained Rust OTIO crate exists.
- avio is the closest permissive "engine library" to drop in under a custom UI. It has an FFmpeg + wgpu timeline with undo, but no interchange and no MCP, and only ★12.
- FilmCraft's codec crates are an asset even in a fallback scenario. EffectCraft already consumes them, which suggests they are reusable outside FilmCraft.

### Gaps
- `re_video` decoder specifics (which backends and whether HW decode is included) could not be fetched because docs.rs was unreachable.
- The real-world quality of pure-Rust H.264/HEVC decoders (`rusty_h264`, `rust_h265`, OxideAV) against FFmpeg was not verified.
- The new `ocio` crate's completeness is unknown.

## Q4. Verdict: does any Rust project beat FilmCraft as the engine base?

### Takeaway
No. Concat is the only serious peer and is stronger in places (HW decode, HDR/ACEScct, cross-platform including mobile, beta maturity). But its AGPL-3.0 license is incompatible with a private derivative that might later be open-sourced under the user's own terms, absent a commercial deal. Every permissively licensed alternative is a small, early, FFmpeg-dependent project with a fraction of FilmCraft's command surface and interchange. FilmCraft remains the best engine base. Concat is the main competitor to watch, and its architecture can serve as a design reference.

### Cited Findings
- Concat: AGPL, plugin exception for separate modules only, commercial license via CLA. Its engine/API/compositor details are as described in Q1 and Q2. — [LICENSE-EXCEPTIONS.md](https://github.com/jub0t/concat/blob/main/LICENSE-EXCEPTIONS.md), [ARCHITECTURE.md](https://github.com/jub0t/concat/blob/main/ARCHITECTURE.md)
- Next-largest permissive Rust editors by stars:
  - OpenReelio, ★84 pre-alpha. — [openreelio](https://github.com/openreelio/openreelio)
  - cutlass, ★58 early alpha. — [cutlass README](https://raw.githubusercontent.com/1mrnewton/cutlass/main/README.md)
  - Gausian, ★1.0k, MPL core, 24 commits, near-stale. — [Gausian](https://github.com/gausian-AI/Gausian_native_editor)
  - OpenCut, ★93k MIT, but the Rust rewrite has not shipped and it is closed to contributions. — [OpenCut](https://github.com/OpenCut-app/OpenCut)
- EffectCraft (MIT/Apache) shares FilmCraft's time model and codecs and has its own MCP. — [effectcraft](https://github.com/storytold/effectcraft)

### Inferences
- **EffectCraft is a complement, not a replacement.** It is a natural source for a Resolve-style "Fusion" page: a layer/expression compositor sharing FilmCraft's time model and codecs under the same permissive license. Its first commit was 2026-10-01, so it is days old.
- **Watch OpenCut's Rust rewrite.** If it ships an MIT Rust core with MCP/headless, it would become the most credible permissive challenger, given its 93k-star community. As of today it is a plan, not code.
- **Ravel is a design reference** for a node-based Color/Fusion page with OCIO.
- **Concat is a reference** for HW-decode and HDR pipeline design, and for socket API security (token auth, write-root confinement).
- **Risk note:** FilmCraft's main differentiators (own codecs, 650+ commands, AAF/OMF) are self-reported and pre-alpha. If the other researcher finds them hollow, the fallback is avio or a custom engine on the Q3 stack, not another existing Rust NLE.

### Gaps
- There are no head-to-head tests (render correctness, playback performance, MCP coverage) between FilmCraft and Concat.
- Concat's commercial license pricing and terms are unknown.
- OpenCut's Rust rewrite timeline is unknown.
