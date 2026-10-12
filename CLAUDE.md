# Resolution — notes for agents

Read README.md and docs/ARCHITECTURE.md first. The two product invariants: **feels like home to
DaVinci Resolve users** and **AI-native from the ground up** (every action is a command on one
engine, one undo history, MCP is a first-class client).

Hard rules:

- Only `crates/filmcraft-adapter` may depend on `filmcraft-*` crates. No FilmCraft type crosses
  the `Engine` trait.
- Never depend on FilmCraft's `ui-egui`, `apps/*` or `automation`.
- Never edit FilmCraft code in place (it is a pinned git dependency). Engine fixes go to the
  mirror's patch branch or upstream; see docs/UPSTREAM.md.
- Vocabulary ids in `crates/engine-api/src/commands.rs` are stable API: add or deprecate, never
  rename or reuse. Every new id needs an entry in `crates/filmcraft-adapter/src/map.rs`.
- Resolve-specific state (colour nodes, page layouts, Fairlight routing) lives in Resolution's
  own types, not in FilmCraft's project schema.
- Copy Resolve's workflow and keys, never its icons, artwork, styling or files.
- Record every bug (`B-nnn`) and design decision (`D-nnn`) in docs/LOG.md when it comes up, not
  later. Entries are append-only and say what caught the bug (test, review, running it).

- The editor (`apps/resolution-app`) runs every action as a vocabulary id through the engine thread;
  only its `model.rs` may read the engine's inspect JSON.

Checks before pushing: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.
First builds compile ~45 FilmCraft crates; expect several minutes and several GB in `target/`.
