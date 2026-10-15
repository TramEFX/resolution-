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
- [x] Draft timelines for agents, accepted by the user (built in the editor, M3; D-027)
- [ ] Provenance receipts per operation (today a draft lists the agent's edits by undo label)
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
      F11 to F12 and the Shift edits are below (D-059).
- [ ] **Step 2's bar:** Truman cuts a one-minute piece from the Sony and Samsung clips
- [x] **Step 3:** minimal Deliver page: presets, a render queue and Render All over the engine's
      export (`deliver.render`, D-025)
- [x] Resolve's Replace (F11), Place on Top (F12), Ripple Overwrite (Shift+F10) and Append at End
      (Shift+F12), composed from FilmCraft's commands (D-059)
- [x] Fit to Fill (Shift+F11; D-060)
- [ ] Trim mode semantics; dynamic trim
- [ ] Faster playback where it is slow: frames are composited on the GPU since D-036 to D-038;
      standard effects on the GPU (D-041), proxies for footage too slow to decode (D-013 "Known
      limits")
- [ ] Proxies / optimized media workflow with Resolve's naming

## M3 — Agents in the editor (step 4)

- [x] Agents edit the open timeline through the editor's engine thread: the agent link and
      `resolution mcp --editor` (D-026)
- [x] Draft timelines: an agent's edits go to a copy of the timeline that the user accepts or
      rejects from a banner; the copy follows the one undo history (D-027)
- [ ] Provenance receipts: what each agent edit did, where and why, beyond its undo label

## M4 — Color page, first slice (step 5)

- [x] Grades in Resolution's own types, compiled to Lumetri on each clip: serial nodes, the
      Primaries controls and Custom Curves, and the `color.*` vocabulary (D-028)
- [x] The Color page: viewer, Node Editor, clip strip with thumbnails, Primaries wheels, Custom
      Curves, Resolve's node keys (D-029)
- [x] Primaries: Shadows, Highlights, Midtone Detail (D-055)
- [ ] The rest of the Color page's first look: Gallery, Mini-Timeline, Bars and Log modes, Lum Mix
- [x] Scopes from `filmcraft-scopes`: Waveform, Parade, Vectorscope, Histogram on the Color page;
      `scopes.read` for agents (D-030)
- [x] Windows: Linear and Circle, as FilmCraft effect masks, with an on-screen control (D-031)
- [x] Qualifiers: an HSL key limiting the whole node (D-032, D-033): a real key on the GPU (D-036,
      D-044), baked into a 3D LUT in the Lumetri compile
- [x] Hue curves: Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat (D-043), Sat vs Lum
      (D-047)
- [ ] Picking a hue curve's points from the viewer
- [x] Matte finesse: Denoise, Clean Black, Clean White, Blur Radius (D-046)
- [x] The Highlight view: Highlight and Highlight B/W of the current node (D-048)
- [x] More matte finesse: Black Clip, White Clip, Morph (D-058)
- [ ] In/Out Ratio, Highlight Difference, RGB and Luma qualifier modes
- [x] Gradient window (D-052)
- [x] Polygon window: corners dragged, added and removed on screen (D-053)
- [x] Window handles on screen: size, rotation, softness (D-057)
- [x] Curve window, smooth through its points (D-061)
- [ ] Curve windows' own Bézier handles; tracking
- [ ] Parallel and layer nodes
- [ ] **Our own GPU renderer** (D-035, Truman's choice), replacing the Lumetri compile for grades:
  - [x] G1: the viewers' frames on the GPU, with the grade stage (tone, saturation, real key masks,
        windows) exact to `crates/grade`; FilmCraft's CPU renderer for what it does not cover yet
        (D-036; graded 4K plays at 24 fps on the GPU where the CPU managed 14)
  - [x] G2: the rest of the viewers' frames on the GPU (to the screen without a copy back: off)
    - [x] Dissolves, dips, titles, captions and clips FilmCraft renders itself; graded clips known
          by their pictures (D-037)
    - [x] Blend modes, Push, colour management of HDR and wide-gamut media (D-038)
    - [ ] Standard effects on the GPU: waits for an Effects panel (D-041); they run on the CPU per
          layer meanwhile
    - [ ] Frames to the screen without a copy back (D-039): built, then turned off since it
          crashed the editor on a resize (B-036); needs the GPU work on the UI thread
    - [x] Chroma siting; shrinking by more than 4× (D-040)
  - [x] G3: Deliver through the same renderer, so viewer and file are identical (D-042; 2 to 3×
        faster on graded 4K)
  - [x] Grades by their own math on every path, graded at their source where FilmCraft draws a
        frame itself; the compile only for FilmCraft and renders without a GPU (D-044, D-045)
  - [x] A grade stage for mattes that need a pixel's neighbours (D-046)
  - [ ] Later: parallel and layer nodes, no clamping between nodes

## M5 — Cut page and Media page

## Later

Fairlight-style bus model, full Deliver page presets, Fusion page (watch EffectCraft), OpenFX hosting.
Skins (D-018): alternative chrome over the same layouts; the step-1 Graphite and Soft Light
directions are the first candidates.

## Tools

- [x] **Action journal, its core** (D-034, D-054): every command, who sent it, how long it ran and
      how it ended, noted by the engine thread; `--journal` writes it to a file as it goes
- [ ] The journal live in a panel, and readable by agents (D-054 sets out the options)
