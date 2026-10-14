# Tracking FilmCraft upstream

FilmCraft lands ~100 commits a day with no server-side CI, and makes no stability promise for
command ids or the `.fcproj` schema. We take its improvements on purpose, at tested points.

## Repositories

| Repo | Visibility | Role |
|---|---|---|
| `TramEFX/resolution-` (this) | private | The app. Depends on FilmCraft crates from the mirror, pinned by commit `rev`. |
| [`TramEFX/resolution-filmcraft`](https://github.com/TramEFX/resolution-filmcraft) | private | The engine mirror; layout in its `RESOLUTION.md`. |
| `<you>/filmcraft` | public fork | Optional, later: a pristine fork used only to open PRs upstream. |

Mirror branches:

- `upstream` — untouched copy of `storytold/filmcraft:main`. (Not `main`: upstream's workflows
  run on pushes to `main` and would spend the private repo's Actions minutes.)
- `resolution` (default) — `upstream` + our patch stack, rebased at each sync. Today the stack is
  mirror plumbing only (mirror CI, upstream workflows removed); engine code is untouched.
- `sync/fc-YYYYMMDD` — one branch per sync point, never moved or deleted. Pushing one runs
  `mirror-ci` (upstream's own test gate) on Linux; Windows and macOS are manual runs because
  they cost 2x and 10x Actions minutes on a private repo. Branches rather than tags because agent
  sessions can push branches but not tags. Mirror CI runs the phases of upstream's
  `cargo xtask ci` as separate steps, leaving one speed test out of the result, and fails with a
  diff if upstream changes that gate (LOG D-010). It builds with cargo's stock release settings
  rather than upstream's LTO profile, and keeps its dependency cache when a run fails (D-007,
  B-003). Its Linux job installs what upstream's tests assume a developer machine has: a
  pinned ffmpeg 9 (D-008), software Vulkan (B-005), and a Japanese font (B-006). To try a
  workflow change without a new sync point, run `mirror-ci` by hand on `resolution`.

**Current pin:** `sync/fc-20261013` = `29d01cf` = storytold/filmcraft@`cee1feb` (main, 2026-10-13;
not a release tag) + mirror plumbing (LOG D-015). Mirror CI run 38413841087 on the sync point
failed three tests in FilmCraft's UI crate and desktop app, outside our graph (B-020); the pin was
taken anyway by decision (D-015).
Previous: `sync/fc-20261012` = `9acf816` (`2ff3f5e`, v0.6.0).

**Private-repo access:** `.cargo/config.toml` sets `net.git-fetch-with-cli`, so Cargo fetches the
mirror with your normal git credentials. CI for this repo will need a token that can read the
mirror.

The patch stack may hold only: mirror plumbing, a command-registration hook, `pub` visibility
changes, ArtCraft brand removal for distributed builds, and fixes on their way upstream. Anything
else belongs in Resolution's own crates.

## Sync procedure (weekly or fortnightly; prefer upstream release tags as bases)

1. `scripts/upstream-status.sh [ref]` — what changed since our pin: commits, engine churn, command
   ids added/removed, ROADMAP log.
2. `scripts/sync-mirror.sh [ref]` — fast-forwards the mirror's `upstream`, rebases `resolution`
   onto it (removed upstream workflows stay removed; any other conflict stops for a human), and
   pushes a new `sync/fc-*` branch, which starts mirror CI.
3. When mirror CI is green: `scripts/sync-mirror.sh [ref] --apply` moves the `rev` here and runs
   `cargo test`. The gates:
   - `every_mapped_id_exists_upstream` (command renames/removals) and the adapter contract tests;
   - `.fcproj` round-trip and migration tests (to add);
   - the command-id diff from step 1, for new commands worth adding to the vocabulary.

   Then `cargo clean`: Cargo keeps the old pin's build beside the new one, and they add up
   (LOG B-032; a clean build of everything is 8.3 GB and about 6.5 minutes). It also deletes the
   manual's text in `target/resolve-manual/`; `python scripts/resolve-manual.py` makes it again in
   about 2.5 minutes (it needs `pip install pypdf fonttools`).
4. Fix the adapter, never the engine in place. Compare upstream's `[profile.dev.package.*]`
   overrides at the new base with ours in `Cargo.toml`, and mirror any for crates in our graph
   (`cargo tree -i <crate>`). Commit the pin bump and adapter fixes together.

## Patches worth sending upstream

They shrink our patch stack and help FilmCraft:

- Move playback (frame pool, play-ahead mixer, audio clock) out of `ui-egui` into a UI-free crate.
- A command-registration hook so downstream crates can add commands.
- Typed, schema-checked command parameters (upstream issue #347).
- Publishing the crates to crates.io (#232), so we can pin semver instead of git revs.
- A DaVinci Resolve shortcut preset in `shortcut_presets.rs`.
- Missing Resolve edits: place on top, ripple overwrite, fit to fill, append at end, replace.
