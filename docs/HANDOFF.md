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
| Mirror CI | First run of FilmCraft's own gate (`cargo xtask ci`, Linux) was in progress at hand-off: https://github.com/TramEFX/resolution-filmcraft/actions/runs/37984421765 |
| Engine boundary | `engine-api` trait + vocabulary (71 Resolve-named commands; 46 run headless on FilmCraft today, the rest are front-end or not yet supported), FilmCraft adapter, drift test |
| Keymap | Resolve defaults as data (`keymaps/resolve.toml`); **not yet verified** against a real Resolve install |
| MCP | 10 tools: search by page/Resolve key, run with risk tiers and dry runs, atomic batches, checkpoints, render frame. 25 tests across the workspace (passing on Windows); the stdio end-to-end check is not in the repo |
| UI | Not started (M2) |
| CI for this repo | None yet: needs a token that can read the private mirror |

Gates before pushing: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.
Bugs and design decisions go in [LOG.md](LOG.md) as they come up.

## 4. Next steps (pick any)

1. **If mirror CI failed:** read the log at the link above. A failure in FilmCraft's own tests is
   upstream's bug, not ours: note it, report it upstream if new, and keep the pin (our adapter
   tests passed against this commit).
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
