# Watch list: FilmCraft and the ArtCraft suite

Where to follow progress, in priority order. Compiled October 2026; see the research report for
sources. Press coverage was a one-time launch spike: the repo itself is the signal.

| # | Source | URL | Watch for | Cadence |
|---|---|---|---|---|
| 1 | ROADMAP.md — "Log", "Honest assessment", "Where we are lacking" | https://raw.githubusercontent.com/storytold/filmcraft/main/ROADMAP.md | Schema bumps, command renames, playback / hardware-acceleration work, parity numbers. `scripts/upstream-status.sh` diffs it against our pin. | Daily |
| 2 | Commits feed | https://github.com/storytold/filmcraft/commits/main.atom | Churn in `crates/engine/src/commands.rs`, `project`, `format`, `platform` | Daily |
| 3 | Releases and tags | https://github.com/storytold/filmcraft/releases.atom · https://github.com/storytold/filmcraft/tags.atom | Sync bases (latest: v0.4.0, 2026-10-08) | On release |
| 4 | Key issues | #30 hardware acceleration · #232 crates.io · #347 typed params · #334 node colour — https://github.com/storytold/filmcraft/issues | Decisions that shrink or grow our patch stack | Weekly |
| 5 | craftrules (shared rules for all Craft apps) | https://github.com/storytold/craftrules | Never-crash, font, corpus rules that ripple into every app | Weekly |
| 6 | EffectCraft (After Effects clone, same engine family) | https://github.com/storytold/effectcraft | Compositing and effects; a possible Fusion-page source | Weekly |
| 7 | PhotoCraft parity tooling and corpus | https://github.com/storytold/photocraft · https://github.com/storytold/photocraft-corpus | `cargo xtask parity` coming to FilmCraft; shared real-media corpora | Monthly |
| 8 | ArtCraft Discord | https://discord.gg/artcraft | Informal roadmap talk, bug triage | Weekly skim |
| 9 | Maintainer (Brandon Thomas) | https://x.com/echelon · https://github.com/echelon | Strategy, licensing, commercial plans, governance | Weekly |
| 10 | VideoHelp release tracker | https://www.videohelp.com/software/FilmCraft | Passive release alerts | On notification |
| 11 | Competitors | Concat https://github.com/jub0t/concat (AGPL; hardware decode, HDR) · OpenCut Rust rewrite https://github.com/OpenCut-app/OpenCut · Ravel https://github.com/narusenia/ravel (node graph, OCIO) | A permissive challenger shipping; design references | Monthly |
| 12 | Forks of FilmCraft | e.g. https://github.com/pwilkin/filmcraft · https://github.com/mweinbach/filmcraft | Fixes worth cherry-picking; who else builds on the engine | Monthly |

Not to be confused: `storytold/artcraft` is ArtCraft's separate, older AI image/video tool,
not part of the Craft suite.

## Known gotchas to stay ahead of

Full register (20 items, with evidence) in the research report. The ones that bite first:

- **Churn:** ~100 commits/day; `.fcproj` schema went v7→v13 in days; command ids carry no
  stability promise. → Pin, sync on purpose, keep FilmCraft behind the adapter.
- **No upstream CI running tests;** the maintainers develop on macOS; Windows/Linux are "barely
  exercised at runtime". → Our mirror runs CI on all three.
- **Real camera/phone media is far less tested** than conformance streams (multi-audio-track,
  WebM alpha, timecode, large MKV issues open). → Build our own real-footage corpus early; keep
  decode swappable (FFmpeg/GStreamer backend as a fallback).
- **Playback lives in the UI crate**, so a new front end must rebuild or extract it.
- **CPU-bound colour, keys, masks and export.** A Resolve-class Color page is our own GPU work.
- **Security:** custom parsers for untrusted media, a TCP control port, light fuzzing.
  → Continuous fuzzing; loopback + token on every port.
- **Bus factor ≈ 1; ~93% of commits AI co-authored.** → Be ready to own the engine outright
  (the licence allows it).
- **Distribution-time issues:** strip ArtCraft branding; H.264/HEVC patent pools; Blackmagic
  trade dress; trademark check on the name.
