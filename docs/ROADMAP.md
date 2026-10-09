# Roadmap

FilmCraft supplies the plumbing (codecs, timeline edits, interchange, proxies, multicam, export).
Resolution's own work sits where FilmCraft is thinnest and where Resolve is most itself.

## M0 — Foundation (in progress)

- [x] Engine boundary (`engine-api`), FilmCraft adapter, upstream-drift test
- [x] Resolve keymap as data (seed table)
- [x] Headless CLI
- [x] Private mirror `resolution-filmcraft` with Linux CI on sync points; pin switched to it
- [ ] Windows/macOS mirror CI runs (manual; minute cost)
- [ ] CI here: test, clippy, `cargo-deny` licence scan, "only the adapter imports filmcraft" check
- [ ] Verify every keymap entry against Resolve's default map (`verified = true`)
- [ ] Hands-on evaluation: real camera footage through FilmCraft decode, export, interchange
      round-trips with Resolve (FCPXML, FCP7 XML, OTIO, EDL); results in `docs/research/`

## M1 — AI-native core

- [x] `resolution-mcp`: page-organised discovery, risk tiers, dry runs with timeline diffs,
      atomic batches, checkpoints, `render_frame`, stdio (`resolution mcp`, `.mcp.json`)
- [ ] Draft timelines for agents, accepted by the user; provenance receipts
- [ ] Open real projects/media from MCP end to end (import → assemble → render) as an acceptance test
- [ ] Per-command JSON Schemas for the vocabulary
- [ ] Transcription on by default; transcript-based editing; silence removal
- [ ] Media search (embeddings) over the media pool

## M2 — Edit page

- [ ] Resolve-style shell: page bar, Media Pool, source/record viewers, timeline, inspector
- [ ] Playback outside the UI (upstream a `filmcraft-playback` crate, or own a port)
- [ ] Resolve edit types the engine lacks: place on top, ripple overwrite, fit to fill,
      append at end, replace; Trim mode semantics; dynamic trim
- [ ] Proxies / optimized media workflow with Resolve's naming

## M3 — Cut page and Media page

## M4 — Color page

- [ ] Node graph (serial, parallel, layer), primaries wheels, curves, qualifiers, windows
- [ ] GPU colour pipeline (FilmCraft's Lumetri path is CPU); scopes from `filmcraft-scopes`

## Later

Fairlight-style bus model, Deliver page presets, Fusion page (watch EffectCraft), OpenFX hosting.
