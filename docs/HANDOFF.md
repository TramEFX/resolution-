# Picking up locally

Where things stand and how to continue on your own machine. Last updated 2026-10-12.

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

Measured on Windows 11 (Rust 1.99, 2026-10-09): the first `cargo test` took 3 min 12 s, and
`target/` held 3.0 GB after tests and clippy.

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
Media Pool to open it in the source, then F9 / F10 to insert or overwrite at the playhead. Click a clip to
select it; click or drag the ruler to move the playhead. While playing, the viewer's top right shows the
frames per second reaching the screen. Put local test clips in `test media/`, which git ignores.

Video is decoded by the GPU where the operating system can (LOG D-014); `--software-decode` turns that
off, `--mute` plays without sound. Two self-tests: `--play 6` plays six seconds and prints what was shown
and heard; `--screenshot out.png` saves the window. To measure playback without a window:
`cargo run -p resolution-playback --example play_bench -- <clip> [--hardware] [--side 1920]`; add
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

## 4. Where things stand

| Area | State |
|---|---|
| Engine | FilmCraft **v0.6.0** (`2ff3f5e`) via the private mirror, pinned at `sync/fc-20261012` (`9acf816`), synced 2026-10-12 (LOG D-011) |
| Mirror CI | **Green** for the pin: run [38216382326](https://github.com/TramEFX/resolution-filmcraft/actions/runs/38216382326) (2026-10-12, Linux, ~46 min) on the sync point: 2,808 tests passed, 0 failed, 0 skipped; layers, assets, wasm ok. The mixer speed test stays outside the gate (LOG B-007) |
| Engine boundary | `engine-api` trait + vocabulary (71 Resolve-named commands; 46 run headless on FilmCraft today, the rest are front-end or not yet supported), FilmCraft adapter, drift test |
| Keymap | Resolve defaults as data (`keymaps/resolve.toml`); **not yet verified** against a real Resolve install |
| MCP | 10 tools: search by page/Resolve key, run with risk tiers and dry runs, atomic batches, checkpoints, render frame. 30 tests across the workspace (passing on Windows), 5 of them end to end over MCP stdio against the built binary (`apps/resolution-cli/tests/mcp_stdio.rs`) |
| GPU parity, real hardware | Local, 2026-10-12, at the previous pin (`2ba0629`): FilmCraft's 33 golden tests pass, the 4 GPU-vs-CPU ones on an AMD Radeon RX 9070 XT through Vulkan (the adapter wgpu's default request picks, checked with a probe). CI covers the same tests on software Vulkan (LOG B-005) |
| UI | **First slice of the Edit page** (`apps/resolution-app`, LOG D-012): page bar, Media Pool, viewer, timeline, inspector, Resolve keys. 13 tests, including UI tests that press keys and click clips |
| Playback | **Real time, with sound** (`crates/playback`, LOG D-013): frames rendered ahead, sound mixed ahead, the audio device as the clock, GPU video decoding (D-014). Measured on this PC: phone 4K30, Sony α7R II 4K 23.976 (100 Mbit/s) and 1080p60 H.264 play at full rate with no dropouts, the sound sample-exact against ffmpeg and checked by ear (2026-10-13); a sideways (portrait) phone 4K clip at 25 to 26 of 30 fps; CPU-heavy composites at about 10 fps. Footage decoded in software can freeze (B-017, open). 4K60 HEVC from ffmpeg-muxed files falls back to software (B-013, reported upstream as storytold/filmcraft#1483) |
| CI for this repo | None yet: needs a token that can read the private mirror |

Gates before pushing: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.
Bugs and design decisions go in [LOG.md](LOG.md) as they come up.

## 5. Next steps (pick any)

1. **Follow the upstream reports** from mirror CI (filed 2026-10-12): storytold/filmcraft#1249 (APV
   oracle needs ffmpeg 8.0+), #1250 (GPU tests vs adapters that cannot render `Rgba32Float`),
   #1251 (Japanese-UI test needs a font). When one is fixed and synced, the matching CI
   workaround can go (LOG B-004 to B-006). B-007 was already fixed upstream.
   The fixes are small (skip or fail clearly when the tool lacks a capability), and they shrink
   what our CI has to work around. A failure in FilmCraft's own tests is upstream's bug, not ours.
2. **Real footage:** first pass done on 2026-10-12 ([report](research/real-footage-2026-10.md)). 33 profiles
   from 127 local files all import and render, including HEVC 4K60, 5-track audio, rotation and
   variable frame rate. A phone 4K clip decodes correctly. Two timing defects were found (LOG B-010,
   reported as storytold/filmcraft#1276; B-011). Still needed: **camera and phone
   originals** (iPhone HEVC/HDR, 10-bit, GoPro, mirrorless, ProRes, MTS). The local files are mostly
   downloads. Third pass, 2026-10-12: a Sony α7R II clip and a Samsung portrait clip decode, play and
   sound correctly; the rotated clip exposed a playback deadlock (B-015, fixed) and a software-decoding
   freeze (B-017). This decides how soon we need a fallback decoder (research risk #6).
3. **Verify the keymap** on a machine with Resolve: open Keyboard Customization, compare each line
   of `keymaps/resolve.toml`, set `verified = true`, fix any wrong keys.
4. **Draft timelines for agents** (agents edit a copy you accept), the next MCP milestone.
5. **CI for this repo:** a GitHub Actions workflow with a fine-grained token (read access to
   `resolution-filmcraft`) stored as a secret.
6. **Grow the Edit page** (M2): a source viewer, track headers, trimming by drag. Then the app
   hosts the MCP server so agents edit the timeline you are looking at.
7. **Faster playback where it is slow** (LOG D-013 "Known limits"): composite frames on the GPU as
   FilmCraft's GPU frame plans do, for effects-heavy timelines; proxies for footage too slow to
   decode; B-013 upstream for ffmpeg-muxed HEVC; B-017, footage decoded in software freezing instead of
   playing slowly. Measure with `play_bench` before and after.

Weekly: `scripts/upstream-status.sh` to see what FilmCraft changed; `scripts/sync-mirror.sh` to
take it (docs/UPSTREAM.md).
