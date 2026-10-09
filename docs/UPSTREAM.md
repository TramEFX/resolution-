# Tracking FilmCraft upstream

FilmCraft lands ~100 commits a day with no server-side CI, and makes no stability promise for
command ids or the `.fcproj` schema. We take its improvements on purpose, at tested points.

## Repositories

| Repo | Visibility | Role |
|---|---|---|
| `resolution` (this) | private | The app. Depends on FilmCraft crates by git URL pinned to a tested revision. |
| `resolution-filmcraft` | private | Mirror. `main` = untouched copy of `storytold/filmcraft:main`. `resolution` branch = our small patch stack, rebased each sync. Tested points tagged `fc-sync-YYYYMMDD`. CI runs upstream's `cargo xtask ci` on Linux, Windows and macOS before tagging. |
| `<you>/filmcraft` | public fork | Pristine fork used only to open PRs upstream. |

**Current state:** the mirror does not exist yet, so `Cargo.toml` pins
`https://github.com/storytold/filmcraft` at `7b6c134` (2026-10-09, after v0.4.0) directly.
Once the mirror exists, change the URL in `[workspace.dependencies]` to the mirror and pin to an
`fc-sync-*` tag.

The patch stack may hold only: a command-registration hook, `pub` visibility changes, ArtCraft
brand removal for distributed builds, and fixes on their way upstream. Anything else belongs in
Resolution's own crates.

## Sync procedure (weekly or fortnightly; prefer upstream release tags as bases)

1. `scripts/upstream-status.sh` — what changed since our pin (commits, ROADMAP log, command ids).
2. In the mirror: fetch upstream, fast-forward `main`, rebase `resolution` onto it, run
   `cargo xtask ci` on all three OSes, tag `fc-sync-YYYYMMDD`.
3. Here: move the `rev` in `Cargo.toml`, `cargo update -p filmcraft-engine`, then run the gates:
   - `cargo test` — includes `every_mapped_id_exists_upstream` (command renames/removals) and the
     adapter contract tests;
   - `.fcproj` round-trip and migration tests (to add);
   - review the command-id diff from step 1 for new commands worth adding to the vocabulary.
4. Fix the adapter, never the engine in place. Commit the pin bump and adapter fixes together.

## Patches worth sending upstream

They shrink our patch stack and help FilmCraft:

- Move playback (frame pool, play-ahead mixer, audio clock) out of `ui-egui` into a UI-free crate.
- A command-registration hook so downstream crates can add commands.
- Typed, schema-checked command parameters (upstream issue #347).
- Publishing the crates to crates.io (#232), so we can pin semver instead of git revs.
- A DaVinci Resolve shortcut preset in `shortcut_presets.rs`.
- Missing Resolve edits: place on top, ripple overwrite, fit to fill, append at end, replace.
