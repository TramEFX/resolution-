# Picking up locally

Where things stand and how to continue on your own machine. Last updated 2026-10-09.

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

## 2. Use the MCP server from Claude Code

Open Claude Code in the repo folder. `.mcp.json` registers a `resolution` server
(`cargo run -q -p resolution-cli -- mcp --demo`, FilmCraft's demo project); approve it when asked,
or check it with `/mcp`. The first start compiles the app, so give it a minute.

Then just ask, for example: *"Look at the timeline, split the timeline at 2 s and at 4 s as one
batch, dry-run it first, then render the frame at 2.5 s."*

To work on a real project instead of the demo, edit `.mcp.json` locally (`"mcp", "--project",
"C:/path/to/edit.fcproj"`) or run `cargo run -p resolution-cli -- mcp --project <path>` from any MCP
client. Destructive commands (save/open/delete) ask the agent to confirm with you first.

## 3. Where things stand

| Area | State |
|---|---|
| Engine | FilmCraft v0.4.0+ (`7b6c134`) via the private mirror, pinned at `sync/fc-20261009` |
| Mirror CI | **Green** for the pin: run [38019701912](https://github.com/TramEFX/resolution-filmcraft/actions/runs/38019701912) (2026-10-10, Linux), engine code identical to `2ba0629`: 2,294 tests passed, 0 failed; layers, assets, wasm ok; ~23 min. Getting there took six fixes to the CI environment (LOG B-003 to B-007, D-007 to D-010) |
| Engine boundary | `engine-api` trait + vocabulary (71 Resolve-named commands; 46 run headless on FilmCraft today, the rest are front-end or not yet supported), FilmCraft adapter, drift test |
| Keymap | Resolve defaults as data (`keymaps/resolve.toml`); **not yet verified** against a real Resolve install |
| MCP | 10 tools: search by page/Resolve key, run with risk tiers and dry runs, atomic batches, checkpoints, render frame. 30 tests across the workspace (passing on Windows), 5 of them end to end over MCP stdio against the built binary (`apps/resolution-cli/tests/mcp_stdio.rs`) |
| UI | Not started (M2) |
| CI for this repo | None yet: needs a token that can read the private mirror |

Gates before pushing: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.
Bugs and design decisions go in [LOG.md](LOG.md) as they come up.

## 4. Next steps (pick any)

1. **Report upstream** what mirror CI found (LOG B-004 to B-007): FilmCraft's tests assume a
   recent ffmpeg, a real GPU, a Japanese system font, and an unloaded CPU for one speed test.
   The fixes are small (skip or fail clearly when the tool lacks a capability), and they shrink
   what our CI has to work around. A failure in FilmCraft's own tests is upstream's bug, not ours.
2. **Real footage:** run `resolution mcp --project …` or `resolution run raw.filmcraft/file.import
   '{"paths":["clip.mp4"]}' …` on your own camera and phone files; note what fails to import or
   play (research risk #6). This decides how soon we need a fallback decoder.
3. **Verify the keymap** on a machine with Resolve: open Keyboard Customization, compare each line
   of `keymaps/resolve.toml`, set `verified = true`, fix any wrong keys.
4. **Draft timelines for agents** (agents edit a copy you accept), the next MCP milestone.
5. **CI for this repo:** a GitHub Actions workflow with a fine-grained token (read access to
   `resolution-filmcraft`) stored as a secret.
6. **Start the Resolve-style UI shell** (M2): page bar, Media Pool, viewers, timeline.

Weekly: `scripts/upstream-status.sh` to see what FilmCraft changed; `scripts/sync-mirror.sh` to
take it (docs/UPSTREAM.md).
