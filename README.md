# Resolution

An AI-native video editor in Rust that feels like home to DaVinci Resolve users.

*"Resolution" is an internal codename. Run a trademark clearance before any public release
(see [docs/research](docs/research/) — risk #14).*

## Two invariants

1. **Home for Resolve users.** Resolve's pages (Media, Cut, Edit, Fusion, Color, Fairlight,
   Deliver), its edit types, its names for things and its default keyboard. A Resolve editor
   should be productive in the first minute. We copy workflow and keys (functional); we never copy
   Blackmagic's icons, artwork, styling or files.
2. **AI-native from the ground up.** Every action, by a person or an agent, is a named command on
   one engine, through one undo history. The MCP server is a first-class client, not a plugin;
   the UI is just another client of the same command API.

## How it is built

- **Engine:** [FilmCraft](https://github.com/storytold/filmcraft) (MIT OR Apache-2.0), consumed from
  our private mirror [`resolution-filmcraft`](https://github.com/TramEFX/resolution-filmcraft) as
  pinned git dependencies. Upstream improvements are merged by moving one pin;
  see [docs/UPSTREAM.md](docs/UPSTREAM.md).
- **Boundary:** everything above the engine speaks Resolution's own command vocabulary through the
  `Engine` trait in `crates/engine-api`. Only `crates/filmcraft-adapter` knows FilmCraft exists.
  Swapping engines means writing one new adapter. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

| Path | What |
|---|---|
| `crates/engine-api` | `Engine` trait, command vocabulary (Resolve names, grouped by page), result and frame types, the `Playback` snapshot |
| `crates/filmcraft-adapter` | `Engine` on FilmCraft's headless `Session`; the Resolution→FilmCraft command map and the upstream-drift test |
| `crates/keymap` | Keymaps as data; validates `keymaps/*.toml` against the vocabulary |
| `keymaps/resolve.toml` | DaVinci Resolve default keyboard (seed; being verified) |
| `crates/mcp` | MCP server: agents find, run, preview, check and undo edits through the same commands as people |
| `crates/playback` | Real-time playback, engine-neutral: frames rendered ahead, sound mixed ahead, the audio device as the clock (ported from FilmCraft) |
| `crates/grade` | Colour grades as Resolution's own model: nodes, Primaries, Custom Curves, hue curves, windows, qualifiers with matte finesse, and their math |
| `crates/renderer` | Resolution's GPU renderer (wgpu): draws the viewers' and Deliver's frames with grades by their own math |
| `apps/resolution-cli` | Headless CLI: `resolution commands`, `resolution keys`, `resolution run …`, `resolution mcp` |
| `apps/resolution-app` | The editor (egui): Resolve's page bar, Edit, Color and Deliver pages over the same commands; `cargo run -p resolution-app` |

## Try it

```sh
cargo test
cargo run -p resolution-cli -- commands
cargo run -p resolution-cli -- keys
cargo run -p resolution-cli -- run project.inspect
cargo run -p resolution-cli -- mcp --demo     # MCP over stdio, on FilmCraft's demo project
cargo run -p resolution-app                   # the editor (see docs/HANDOFF.md for keys)
```

In Claude Code, this repo's `.mcp.json` starts the MCP server automatically (approve it once).
The engine lives in a private mirror; Cargo fetches it with your git credentials.

## Docs

- **[Picking up locally](docs/HANDOFF.md)**: setup, MCP in Claude Code, status, next steps
- [The one-minute edit, in Resolution](docs/ONE-MINUTE-EDIT.md): what works in the editor today, step by step
- [Architecture and invariants](docs/ARCHITECTURE.md)
- [Decision & issue log](docs/LOG.md): every design decision (`D-nnn`) and bug (`B-nnn`), as it happened
- [Roadmap](docs/ROADMAP.md)
- [Tracking FilmCraft upstream](docs/UPSTREAM.md)
- [Watch list: FilmCraft coverage and progress](docs/WATCHLIST.md)
- [Engine research report (Oct 2026)](docs/research/FilmCraft%20engine%20alternatives.md)

## Licence

Not chosen yet. MIT OR Apache-2.0 (matching FilmCraft) is the default candidate. FilmCraft's
licence and NOTICE apply to the engine code we depend on; see [NOTICE](NOTICE).
