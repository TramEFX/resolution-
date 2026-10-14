# Roadmap

FilmCraft supplies the plumbing (codecs, timeline edits, interchange, proxies, multicam, export).
Resolution's own work sits where FilmCraft is thinnest and where Resolve is most itself.

## Order of work (agreed with Truman 2026-10-13, LOG D-016)

The Edit page is not finished before other pages start. It goes to a bar instead: a real edit, done
with Resolve habits, that ends in a file. Then agents, then colour; Cut and Media come later because
they mostly re-arrange panels built along the way.

1. **Shared look and layout** (M2). Truman chose **Black Box** from three mockups (D-018). Build
   it with a few colour themes, its own icons and panel chrome, and the page layout model: each
   page has a fixed layout with panels shown or hidden from its toolbar, as in Resolve, not free
   docking. Skins (alternative chrome) come later.
2. **The Edit page to the bar** (M2): a source viewer with In/Out and the F9–F12 edits, track headers,
   trimming by drag in Selection and Trim modes, snapping, zoom, waveforms, markers. The bar is met
   when Truman cuts a one-minute piece from the Sony and Samsung clips without reaching for
   anything un-Resolve-like.
3. **A minimal Deliver page** (M2): one H.264 preset and a render queue over the engine's
   `file.exportMedia`, so an edit leaves the app.
4. **Agents in the editor** (M3): the app hosts the MCP server, so an agent edits the timeline you
   are looking at; agent edits arrive as a draft you accept or reject.
5. **A first slice of the Color page** (M4): the node graph and GPU grading, the riskiest work and
   the most Resolve-defining.

After that: Cut and Media pages (M5), the rest of the Edit page, Fairlight.

**What Resolve does** is taken from the DaVinci Resolve Reference Manual for the version installed
here, then from the running install for anything the manual leaves out (LOG D-017). The manual is
read, never copied into the repo.

## M0 — Foundation (in progress)

- [x] Engine boundary (`engine-api`), FilmCraft adapter, upstream-drift test
- [x] Resolve keymap as data (seed table)
- [x] Headless CLI
- [x] Private mirror `resolution-filmcraft` with Linux CI on sync points; pin switched to it
- [ ] Windows/macOS mirror CI runs (manual; minute cost)
- [ ] CI here: test, clippy, `cargo-deny` licence scan, "only the adapter imports filmcraft" check
- [ ] Verify every keymap entry against Resolve's default map (`verified = true`): the manual first,
      then the install (D-017)
- [ ] Hands-on evaluation: real camera footage through FilmCraft decode, export, interchange
      round-trips with Resolve (FCPXML, FCP7 XML, OTIO, EDL); results in `docs/research/`

## M1 — AI-native core

- [x] `resolution-mcp`: page-organised discovery, risk tiers, dry runs with timeline diffs,
      atomic batches, checkpoints, `render_frame`, stdio (`resolution mcp`, `.mcp.json`)
- [ ] Draft timelines for agents, accepted by the user; provenance receipts (built in the editor, M3)
- [ ] Open real projects/media from MCP end to end (import → assemble → render) as an acceptance test
- [ ] Per-command JSON Schemas for the vocabulary
- [ ] Transcription on by default; transcript-based editing; silence removal
- [ ] Media search (embeddings) over the media pool

## M2 — Edit page, to the bar (steps 1–3)

- [x] First slice of the Edit page (`apps/resolution-app`, LOG D-012): page bar, Media Pool, viewer,
      timeline, inspector; Resolve keys through the keymap; stepping and scrubbing, preview playback
      without audio
- [x] Real-time playback with sound (`crates/playback`, a port of FilmCraft's, LOG D-013), with
      the operating system's GPU video decoders (D-014)
- [x] **Step 1 mockups:** three directions; Truman chose Black Box (D-018)
- [x] **Step 1:** Black Box in the editor: skin and theme tokens, five colour themes, panel chrome,
      own icons; fixed page layouts with panel toggles (Edit page; Effects, Sound Library, Metadata
      and Mixer are empty slots)
- [x] **Step 2:** source viewer next to the record viewer with In/Out and F9/F10 (D-021); track
      headers (D-019); trimming by drag in Selection and Trim modes (D-020); snapping; zoom and
      scrolling (D-023); audio waveforms (D-022); markers on the ruler; New Timeline (D-024).
      F11 (replace) and F12 (place on top) remain, under the edit types below.
- [ ] **Step 2's bar:** Truman cuts a one-minute piece from the Sony and Samsung clips
- [x] **Step 3:** minimal Deliver page: presets, a render queue and Render All over the engine's
      export (`deliver.render`, D-025)
- [ ] Resolve edit types the engine lacks: place on top, ripple overwrite, fit to fill,
      append at end, replace; Trim mode semantics; dynamic trim
- [ ] Faster playback where it is slow: GPU compositing for effects-heavy timelines, proxies for
      footage too slow to decode (D-013 "Known limits")
- [ ] Proxies / optimized media workflow with Resolve's naming

## M3 — Agents in the editor (step 4)

- [ ] The app hosts the MCP server: agents edit the open timeline through the same engine thread
- [ ] Draft timelines: agent edits shown as a draft the user accepts or rejects; provenance receipts

## M4 — Color page, first slice (step 5)

- [ ] Node graph (serial, parallel, layer), primaries wheels, curves, qualifiers, windows
- [ ] GPU colour pipeline (FilmCraft's Lumetri path is CPU); scopes from `filmcraft-scopes`

## M5 — Cut page and Media page

## Later

Fairlight-style bus model, full Deliver page presets, Fusion page (watch EffectCraft), OpenFX hosting.
Skins (D-018): alternative chrome over the same layouts; the step-1 Graphite and Soft Light
directions are the first candidates.
