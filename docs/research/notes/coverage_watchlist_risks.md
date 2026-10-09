# FilmCraft / ArtCraft "Crafting Apps": coverage, watch list, and gotchas/risks (as of 2026-10-09)

Method note: the session's egress policy blocked github.com HTML pages, the GitHub API for storytold/*, hn.algolia.com, reddit.com, news.ycombinator.com and getartcraft.com, and WebFetch failed DNS for every news site. **raw.githubusercontent.com worked**, so FilmCraft's README.md, ROADMAP.md, AGENTS.md, CLAUDE.md, CONTRIBUTING.md, NOTICE, Cargo.toml, docs/testing.md, docs/agents.md and docs/brand/LICENSE-brand.txt were read in full from `main` (primary source). Press and community material comes from search-engine result summaries, not full-article reads. Treat press quotes as "as reported by" and check them before quoting anything externally.

Primary-source URLs used throughout:
- README: https://github.com/storytold/filmcraft/blob/main/README.md (raw: https://raw.githubusercontent.com/storytold/filmcraft/main/README.md)
- ROADMAP: https://github.com/storytold/filmcraft/blob/main/ROADMAP.md (raw: https://raw.githubusercontent.com/storytold/filmcraft/main/ROADMAP.md)
- AGENTS.md: https://github.com/storytold/filmcraft/blob/main/AGENTS.md
- CLAUDE.md: https://github.com/storytold/filmcraft/blob/main/CLAUDE.md
- Brand license: https://github.com/storytold/filmcraft/blob/main/docs/brand/LICENSE-brand.txt
- docs/testing.md: https://github.com/storytold/filmcraft/blob/main/docs/testing.md

---

## Q1. Watch list: the best places to follow FilmCraft/ArtCraft progress

### Takeaway
The single best tracking source is the repo's **ROADMAP.md**, specifically its dated "Log" and "Honest assessment" sections. It is updated with each landing, names the issue/PR numbers, and states when parity percentages did or did not change. Use the GitHub Atom feeds (releases, commits, tags) for push notifications, and Discord (discord.gg/artcraft) for informal and roadmap chatter. Press coverage is broad but shallow and mostly about PhotoCraft. I found no dedicated FilmCraft video review, HN thread ID, Reddit thread, or This Week in Rust/Phoronix item that I could verify.

### Cited Findings

**Tier 1: primary, high signal (subscribe to these)**
- **Repo:** https://github.com/storytold/filmcraft. v0.4.0, workspace `version = "0.4.0"`, MSRV `rust-version = "1.95"`, `unsafe_code = "forbid"` at the workspace level — [Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml)
- **Releases:** https://github.com/storytold/filmcraft/releases (v0.4.0 tag: https://github.com/storytold/filmcraft/releases/tag/v0.4.0; v0.2.1: https://github.com/storytold/filmcraft/releases/tag/v0.2.1). Each release ships SHA256SUMS.txt plus Windows (x64/arm64/x86, MSI and portable), macOS universal (signed and notarized), Linux (AppImage with zsync self-update, Flatpak, deb, rpm, tarball; x86_64 and aarch64), FreeBSD x86_64, and `filmcraft-web-<ver>.zip` — [README Downloads](https://github.com/storytold/filmcraft/blob/main/README.md#downloads); [v0.4.0 release](https://github.com/storytold/filmcraft/releases/tag/v0.4.0)
- **Atom feeds** follow standard GitHub URL patterns. They could not be fetched from this sandbox, so verify them in a browser:
  - https://github.com/storytold/filmcraft/releases.atom (releases)
  - https://github.com/storytold/filmcraft/tags.atom (tags)
  - https://github.com/storytold/filmcraft/commits/main.atom (every commit on main)
  - Repeat the same patterns for photocraft, effectcraft and the rest if you want to watch shared-standard changes.
- **ROADMAP.md** has "Last updated", the two headline numbers, "Honest assessment", "Where we are lacking" (a priority-ordered work queue), "Running now" and a dated "Log" (entries 2026-09-30 → 2026-10-08). It is written for agents as well as humans ("Agents: read Where we are lacking before choosing work"), so it is the de facto plan of record — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md). Cheap automated watch: poll the raw file and diff the "Log" and "Honest assessment" sections. That URL was reachable from this sandbox: https://raw.githubusercontent.com/storytold/filmcraft/main/ROADMAP.md
- **No CHANGELOG.md, SECURITY.md, CODE_OF_CONDUCT.md or GOVERNANCE.md** exist on main. All four returned 404 on raw.githubusercontent.com when checked 2026-10-09. The ROADMAP "Log" plus release notes stand in for a changelog — [raw repo check](https://raw.githubusercontent.com/storytold/filmcraft/main/CHANGELOG.md)
- **Issues:** https://github.com/storytold/filmcraft/issues. Numbers had reached at least #439 by about 2026-10-08, which means very high issue/PR velocity in roughly 9 days — [Issue #439](https://github.com/storytold/filmcraft/issues/439). The README says "Bug reports with real footage are the most useful thing you can send us" — [README Status](https://github.com/storytold/filmcraft/blob/main/README.md#status). Key tracking issue: **#30 hardware acceleration** — https://github.com/storytold/filmcraft/issues/30
- **Sibling and shared-standards repos.** Engine-wide rule changes land here first, so watch them:
  - https://github.com/storytold/craftrules: cross-app standards (`standards/never-crash.md`, `standards/fonts.md`, `standards/test-corpora.md`, and the `scripts/contributors.py` credit tooling) — [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)
  - https://github.com/storytold/craft-fonts: optional build input via `CRAFT_FONTS_DIR`, and all official releases embed it — [README](https://github.com/storytold/filmcraft/blob/main/README.md)
  - https://github.com/storytold/photocraft-corpus: shared real-file test corpora — [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)
  - https://github.com/storytold/photocraft: has `cargo xtask parity`, which FilmCraft lacks. Watch for the tool being ported — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
  - https://github.com/storytold/effectcraft: the After Effects clone. Compositing and effects work here may flow back into FilmCraft's effects stack. This is an inference from the shared suite, not stated in the docs.
  - Other suite repos: https://github.com/storytold/vectorcraft, https://github.com/storytold/lightcraft, https://github.com/storytold/pdfcraft (the README calls it "PdfCraft"; press calls it "PrintCraft"), https://github.com/storytold/designcraft — [README Crafting Apps table](https://github.com/storytold/filmcraft/blob/main/README.md)
  - https://github.com/storytold/artcraft: a separate, older project (the AI image/video "crafting engine", ~10.8k stars). Don't confuse it with the suite — [search summary citing github.com/storytold/artcraft](https://github.com/storytold/artcraft)
- **Official site:** https://getartcraft.com/apps/filmcraft (FilmCraft page and recommended installer), https://getartcraft.com/apps (all Crafting Apps), https://getartcraft.com/ (ArtCraft, the paid AI studio) — [README](https://github.com/storytold/filmcraft/blob/main/README.md). Could not be fetched (blocked).
- **Discord:** https://discord.gg/artcraft. The README names it as the place to report what's broken and ask for help ("Stuck, or want to show what you made? Ask in Discord") — [README](https://github.com/storytold/filmcraft/blob/main/README.md)

**Tier 2: creator socials**
- GitHub: https://github.com/echelon (Brandon Thomas). Bio says "Founder: ArtCraft, Storyteller.ai, FakeYou… Engineer, filmmaker", based in Atlanta, works mostly in Rust — [GitHub echelon](https://github.com/echelon)
- X/Twitter: https://x.com/echelon ("Brandon. ArtCraft…", ~1,400 posts per the search snippet) — [X @echelon](https://x.com/echelon?lang=en). The original "software is over" post was not located. A widely shared repost thread is https://x.com/Rainmaker1973/status/2108189742219096223 — [Massimo on X](https://x.com/Rainmaker1973/status/2108189742219096223)
- Bluesky: https://bsky.app/profile/echelon.bsky.social. Profile metadata looked stale (2023 era). Don't confuse it with the unrelated @dbrandonthomas.bsky.social — [Bluesky](https://bsky.app/profile/echelon.bsky.social)
- LinkedIn and YouTube channel: not found (gap).

**Tier 3: press (one-off launch coverage; useful for sentiment and legal framing)**
- PetaPixel, 2026-10-07: https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/
- Ars Technica, "'Software is over': Bold AI developer takes aim at Adobe with open source clones" (seen only as a Google News link): https://news.google.com/read/CBMiswFBVV95cUxNbnlQeEF0azJTVURrX1dtX3JUVjFRd0R0Z0t1S0hjQ1ZwSXh0TlhaVnFndEJHbzQtVU1PUFZVbmlzNGlCZmEyMGl6Wnc3NDFsMTdLVFQ0M0R0cHJDeWRGakJFUm93Vjh6ODVxTzR3MTJadGQyYnhXSnVwaUhsTzk1ZzFlVHJqa29rRlRmR3lOZUFWQ2p2Ymt5U3p6Ti1OQW5pclI5d3lycXBUZGYxSk53cWdiNA
- Gizmodo, "Someone Vibe Coded a Free Knockoff of Adobe Creative Suite": https://gizmodo.com/someone-vibe-coded-a-free-knockoff-of-adobe-creative-suite-2000823322
- TechSpot: https://www.techspot.com/news/114136-software-over-developer-uses-ai-build-free-adobe.html
- TweakTown: https://www.tweaktown.com/news/113989/developer-says-software-is-over-after-using-ai-to-recreate-adobes-entire-creative-cloud-suite-for-free/index.html
- Futurism: https://futurism.com/future-society/adobe-creative-suite-software-open-source-artcraft
- Design Week (opinion, hostile): https://www.designweek.co.uk/first-ai-came-and-scooped-up-your-art-now-its-scooped-up-adobe/
- OMG! Ubuntu (PhotoCraft on Linux): https://www.omgubuntu.co.uk/2026/10/artcraft-free-photoshop-ai-clone
- Notebookcheck: https://www.notebookcheck.net/Photoshop-Premiere-more-Experienced-Rust-developer-recreates-Adobe-suite-with-AI-assistance-as-open-source.1419230.0.html
- Also: VideoCardz https://videocardz.com/newz/adobe-photoshop-premiere-and-illustrator-now-have-ai-built-open-source-clones ; AfterDawn https://www.afterdawn.com/news/article.cfm/2026/10/07/vibe-coded-clones-for-adobe-photoshop-lightroom ; GIGAZINE https://gigazine.net/gsc_news/en/20261005-artcraft-crafting-apps/ ; Techaeris https://techaeris.com/2026/10/07/artcraft-launches-7-open-source-creative/ ; Yahoo Tech https://tech.yahoo.com/ai/claude/articles/software-over-developer-uses-ai-174550981.html ; BigGo Finance (trade-dress angle) https://finance.biggo.com/news/3969ffe0-e181-451e-8cc7-f21136f246ed ; PhotographyTalk https://www.photographytalk.com/free-adobe-alternatives-artcraft/ ; Ground News aggregation https://ground.news/article/artcraft-crafting-apps-is-a-collection-of-free-adobe-alternative-apps-aiming-to-be-an-open-source-alternative-to-photoshop-illustrator-premi

**Tier 3b: version trackers and directories (good passive release alerts)**
- VideoHelp lists FilmCraft 0.4.0 (2026-10-08). VideoHelp has per-software update notifications — https://www.videohelp.com/software/FilmCraft
- free-codecs.com (lagging, still on 0.2.1): https://www.free-codecs.com/download/filmcraft.htm
- Portable Freeware forum thread (a Windows-portable launch failure was reported here): https://www.portablefreeware.com/forums/viewtopic.php?p=111510
- GitDiagram architecture view: https://gitdiagram.com/storytold/filmcraft
- Community packaging: the Gentoo `::snakebyte` overlay `media-video/filmcraft-bin` (community-maintained) — https://github.com/switch87/snakebyte-overlay ([README](https://github.com/storytold/filmcraft/blob/main/README.md))

**Tier 4: community discussion**
- Hacker News: the suite launch thread reportedly had "more than 120 points and 180 comments by October 7", and Thomas walked back his timeline in the comments there — [search summary of press coverage; HN item id not recovered](https://tech.yahoo.com/ai/claude/articles/software-over-developer-uses-ai-174550981.html). A Daily Commit aggregator item (2026-10-06) cites HN as the source for storytold/filmcraft — https://thedailycommit.in/story/2026-10-06/02-github-storytold-filmcraft. Use https://hn.algolia.com/?q=filmcraft and https://hn.algolia.com/?q=artcraft to find thread IDs (blocked here).
- ResetEra: "Adobe's Creative Suite has been reverse-engineered with Claude Opus 5.5 – Open Source clones are now available". Page 2 links the YouTube video **youtu.be/eFB79TYI-Vw** ("should help answer some questions"). The video's title and creator could not be identified — https://www.resetera.com/threads/adobes-creative-suite-has-been-reverse-engineered-with-claude-opus-5-5-open-source-clones-are-now-available.1654903/page-2
- Graphic Design Forum: "ArtCraft – has anyone seen this?" — https://www.graphicdesignforum.com/t/artcraft-has-anyone-seen-this/34853
- Reddit: Futurism quotes r/FuckAdobe commenters, and PetaPixel cites a Thomas Reddit comment about Adobe's cancellation fee. No specific thread URLs were recovered — [Futurism](https://futurism.com/future-society/adobe-creative-suite-software-open-source-artcraft); [PetaPixel](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/)
- X (Chinese-language dev community share of the suite): https://x.com/lencx_/status/2107640838939652150

**Forks to monitor (signals of who else builds on the engine):** pwilkin, mweinbach, jabsch, GuintherKovalski, justjohn12345, punkhop, iBobX, EthanSuquet, txiki739, demon820308 and ebsmapps all have `/filmcraft` forks indexed by search — e.g. https://github.com/pwilkin/filmcraft, https://github.com/mweinbach/filmcraft ([search results](https://github.com/pwilkin/filmcraft))

### Inferences
- Watch-list priority for an upstream-merging fork:
  1. ROADMAP.md raw diff (daily)
  2. commits/main.atom
  3. releases.atom
  4. Issue #30 plus issues labelled with platform or codec terms
  5. craftrules repo (rule changes ripple into every app)
  6. Discord #filmcraft
  7. @echelon on X for strategy and commercial statements
  8. VideoHelp for release alerts
- Press is launch-spike coverage and unlikely to track progress; deprioritize it after this week.
- The repo is effectively agent-developed with a single integrator (see Q2). Commit velocity will be extreme, so a fork must automate rebasing and conflict detection rather than review upstream by hand.

### Gaps
- Exact HN item IDs, Reddit thread URLs (r/rust, r/linux, r/opensource, r/VideoEditing, r/editors, r/premiere, r/davinciresolve) and the LinkedIn profile were not retrievable because of egress blocks. None surfaced in search.
- No coverage of FilmCraft specifically was found in Phoronix, It's FOSS, DistroWatch, The Verge or This Week in Rust. OMG! Ubuntu covered PhotoCraft only.
- The content and creator of YouTube video eFB79TYI-Vw are unknown. It was found only as a link in the ResetEra thread.
- Whether GitHub Discussions is enabled on storytold/filmcraft could not be checked.
- Live star and fork counts could not be pulled (GitHub API blocked).

---

## Q2. Catalog of reported criticisms, gotchas and risks

### Takeaway
The project's own ROADMAP is the most candid source:
- About 87% "feature checklist" but only about 50–60% "ready for real work".
- The parity numbers are partly agent-reported with no CI check.
- Development and testing happen on macOS; Windows and Linux are "barely exercised at runtime".
- Real-world camera and phone media is under-tested. Export and most effects (Lumetri included) are CPU-only.
- No plugin hosting.
- A first-time contributor found four real core bugs within hours.

External critics add AI-code trust and security concerns, hype and walk-backs on parity timelines, trade-dress exposure, and a business model tied to a paid AI service. For a DaVinci-Resolve-UI fork, the biggest gotchas are:
- upstream churn
- unvalidated codecs on hostile media
- codec patent licensing for H.264/HEVC encode and decode
- Blackmagic trade dress and trademark risk, including the "Resolution" name's proximity to "Resolve"
- the ArtCraft brand-mark removal obligation

### Cited Findings

**A. Self-reported metrics and their reliability**
- Two headline numbers: "Feature checklist: ~87%" and "Ready for real work: ~50–60%" (ROADMAP last updated 2026-10-05) — [README Status](https://github.com/storytold/filmcraft/blob/main/README.md#status); [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- The ROADMAP flags its own numbers: "**Partly agent-reported.** The other areas' percentages are written by the agents doing the work. No automated parity check runs in CI… so nothing stops a number drifting upward." It also notes "**Presence, not quality.** An item counts once it exists… Nothing compares our rendering with Premiere's." — [ROADMAP Honest assessment](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- UI fidelity (~80%) is judged by agents comparing against Premiere reference screenshots and "hasn't been checked independently" — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- **Unverified performance claims (self-reported, no independent benchmark found):**
  - H.264 decode at 500–600 fps at 1080p; HEVC at about 225 fps
  - AV1 at about 4 fps at 1080p
  - 24 audio tracks × 3 effects mix about 6× realtime on one core
  - A 6-second 960×540 export renders in 1.3 s
  - VA-API 4K H.264 drops from 417 ms to 11 ms of CPU per frame
  - Sources: [README](https://github.com/storytold/filmcraft/blob/main/README.md); [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- **Unverified test-scale claims (self-reported):** "35 crates, 1605+ tests". Codecs are claimed "bit-exact" on 37+ H.264, 41 HEVC and 50+ VP9 conformance streams, and AV1 is claimed bit-exact versus libdav1d on 19 SVT plus 22 libaom vectors — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Parity-timeline hype and walk-back: Thomas initially promised "100% feature parity" within a month, then amended to "99% parity will take a while, but measured in months and not years" — [PetaPixel via search summary](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/); [Massimo on X](https://x.com/Rainmaker1973/status/2108189742219096223). The ROADMAP estimates "~50–82 h (≈2–3.5 days, 24/7)" of wall-clock agent time to "100% and better". That is an agent-hours extrapolation, not a measured outcome — [ROADMAP Estimate to parity](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- "Software is over" framing, and Thomas reportedly said he "one-shotted Photoshop" — [TweakTown](https://www.tweaktown.com/news/113989/developer-says-software-is-over-after-using-ai-to-recreate-adobes-entire-creative-cloud-suite-for-free/index.html)
- **Star counts (single-source, point-in-time):** PhotoCraft 23,303, FilmCraft 4,463, LightCraft 4,448 (around Oct 8). Suite total was over 42,000 stars in eight days — [search summary of press; e.g. Pasquale Pillitteri](https://pasqualepillitteri.it/en/news/21619/free-adobe-clones-artcraft-claude-opus-5-5). The model attribution conflicts: most outlets say Claude Opus 5.5, one repost says Opus 4.5 — [Massimo on X](https://x.com/Rainmaker1973/status/2108189742219096223) vs [TweakTown](https://www.tweaktown.com/news/113989/developer-says-software-is-over-after-using-ai-to-recreate-adobes-entire-creative-cloud-suite-for-free/index.html). The ROADMAP itself says "five Opus 5.5 agents in parallel" — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)

**B. AI-generated code at scale, code quality and process**
- The development process as the ROADMAP describes it:
  - Parallel agents ("five Opus 5.5 agents in parallel for ~4.5 wall-clock hours… moved parity by ~4 points")
  - "a single integrator merging, resolving conflicts… and re-running CI"
  - A machine "heavily overloaded (load 150–300 on 14 cores)" that "once ran out of disk"
  - Source: [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- The agent operating protocol says "Don't stop to ask unless it lists the decision as the user's". Planning docs (`plan/STATUS.md`, `plan/execution-plan.md`, `plan/architecture.md`, `plan/premiere/`) are maintainer-local and gitignored, so the real plan is not public — [CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md)
- Design Week: "you can't really trust AI generated code that hasn't been fully audited by experts". The writer wouldn't run "vibe coded anything" with disk access. It also notes the GitHub repo appears to be a small, two-person project — [Design Week](https://www.designweek.co.uk/first-ai-came-and-scooped-up-your-art-now-its-scooped-up-adobe/)
- Futurism quotes an r/FuckAdobe commenter: "very impressive", but AI makes fast prototypes that aren't necessarily polished or stable — [Futurism](https://futurism.com/future-society/adobe-creative-suite-software-open-source-artcraft)
- HN and press criticism centred on PhotoCraft (lag, frozen tools, unresponsive shortcuts). Thomas himself called it "super early alpha" — [search summary of Yahoo/TechSpot coverage](https://tech.yahoo.com/ai/claude/articles/software-over-developer-uses-ai-174550981.html)
- PetaPixel: Thomas says he is "not a casual vibe coder", and early testers report serious bugs — [PetaPixel](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/)
- Example of an AI-integration bug: hardware decoding was "actually on" only from #41, because "`register()` sat inside a `log::info!` that never ran". VideoToolbox therefore worked only in the CLI and benchmarks, and "releases up to 0.2.1 decoded in software" — [ROADMAP Log 2026-10-06](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- "The never-crash pass (no panics in product code, last-resort guards) landed on 2026-10-04. There's no field record from real users' projects yet." Stability is "improving, unproven" — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Correctness in depth is estimated at ~60–70%: "A first-time contributor found four real bugs in core paths within hours (#5–#8: AIFF import failed, Slide left linked audio behind, transitions didn't follow ripple trims, export ignored start/end times)". Settings are "most wired", not all — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- A 2026-10-06 batch of "21 community PRs landed (#19, #28, #38, #39, #41–#60)". This shows external contributions are accepted and merged quickly — [ROADMAP Log](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)

**C. Codec correctness (from-scratch H.264/HEVC/ProRes/etc.)**
- "Our decoders are bit-exact on conformance streams, but camera and phone files in the wild are less tested" — [README Status](https://github.com/storytold/filmcraft/blob/main/README.md#status)
- Codecs and media are estimated at ~75% versus a checklist ~90%: "Real camera and phone media (variable frame rate, damaged files, unusual containers) is far less tested. No camera RAW (RED, BRAW, ARRIRAW), no E-AC-3." — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- A "Real-world media corpus" is priority #3 in "Where we are lacking", i.e. not done yet — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Encoder and export gaps:
  - H.264 is the only *software* delivery encoder.
  - HEVC export is hardware-only (NVENC on Windows; VideoToolbox 8-bit SDR on macOS). There is no 10-bit HEVC on macOS and no HEVC export on Linux beyond the NVENC H.264 path.
  - No AV1 export.
  - ProRes export writes only 422 HQ.
  - Source: [README](https://github.com/storytold/filmcraft/blob/main/README.md); [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- AAF and OMF "have never been validated in Avid Media Composer or Pro Tools" — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- MP3/FLAC/ALAC/Vorbis decoding still goes through the `symphonia` crate (MPL-2.0), "to be replaced by our own". That is a future churn point for anyone who depends on that API — [README](https://github.com/storytold/filmcraft/blob/main/README.md)
- Approximations count as present in the parity numbers: Warp Stabilizer (2-D only), Morph Cut, Auto Reframe; optical flow "renders as frame blending" — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- The "ProRes encoder outputs are accepted by Apple/Premiere/Resolve" question has no independent confirmation. The project validates against ffmpeg/ffprobe only — [docs/testing.md](https://github.com/storytold/filmcraft/blob/main/docs/testing.md)

**D. Performance (big footage, proxies)**
- "Speed on big footage" is listed as the #1 gap. Performance and hardware are rated at ~35–40% of Premiere:
  - Lumetri, keys, masks, the remaining effects and **export rendering are CPU**.
  - Only 31 effect ids run on the GPU, yet "the effect list shows Premiere's 'GPU accelerated' badge" on many more (a UI that overstates capability).
  - "8K isn't real-time."
  - Source: [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- 4K playback depends on machine load ("4K plays 192/0 at load 100–140"). There is opt-in "draft decoding" that skips deblocking — [ROADMAP Log 2026-10-03](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Proxies exist (ProRes Proxy/LT, H.264 ¼/½), and export stays full-res. A browser proxy creation crash was reported in #230 (unsupported `std::time::Instant` in wasm) — [ROADMAP M11](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md); [Issue #230](https://github.com/storytold/filmcraft/issues/230)
- Issue #329 (0.4.0, Windows) reports timeline editing, preview playback and panel customization problems — [Issue #329](https://github.com/storytold/filmcraft/issues/329)
- "Stutter on 5K 10-bit": **no source found**. See Gaps.

**E. Missing features and functional gaps**
- No plugin hosting (VST3/AU/OpenFX). "For many professional editors this alone rules FilmCraft out." Plugin hosting needs an "**Owner decision**" because it requires FFI/`unsafe` — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- AI features are thin: speech-to-text is off by default (the `whisper` feature), "Enhance Speech is a DSP chain, not a model", no Generative Extend — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Ripple delete **exists**. It was buggy with split edits until #56–#58 (2026-10-06), and "transitions didn't follow ripple trims" was a reported core bug (#5–#8) — [README](https://github.com/storytold/filmcraft/blob/main/README.md); [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Source monitor: forward playback only landed 2026-10-08. "Reverse/shuttle Source playback and source sequences remain unsupported" — [ROADMAP Log 2026-10-08](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Remaining editing gaps:
  - Multicam paging beyond 16 angles
  - Interlaced encoding, smart render and publishing destinations
  - 608/708 caption embedding in video streams
  - About 800 remaining commands and dialogs
  - Source: [ROADMAP Milestones](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Web build: "a compile target; the browser app shell isn't finished". Press lists FilmCraft as desktop-only — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md); [Pasquale Pillitteri summary](https://pasqualepillitteri.it/en/news/21619/free-adobe-clones-artcraft-claude-opus-5-5)
- Team Projects and Productions are "out of scope by design" — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)

**F. Platform issues (Linux and Windows least tested)**
- "Development and testing happen on macOS. Windows and Linux are built for releases but barely exercised at runtime." Item #5 in "Where we are lacking" is CI that runs tests and smoke tests on Windows and Linux, which does not exist yet — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Linux:
  - ALSA audio error (#23, "Fix the Linux audio path")
  - VA-API hardware decode covers H.264 and HEVC only (VP9/AV1 run in software)
  - Linux's only hardware encoder is NVENC H.264
  - Source: [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Windows:
  - **The portable build silently fails to launch on Windows 10 21H2** (#268), also reported on the Portable Freeware forum — [Issue #268](https://github.com/storytold/filmcraft/issues/268); [Portable Freeware forum](https://www.portablefreeware.com/forums/viewtopic.php?p=111510)
  - The portable build writes to the user profile rather than staying self-contained (#139) — [Issue #139](https://github.com/storytold/filmcraft/issues/139)
  - Crash on placing a .mov on the timeline on Windows 11 (#439) — [Issue #439](https://github.com/storytold/filmcraft/issues/439)
  - The README documents that AMD drivers (`atio6axx.dll`) crash when wgpu creates an OpenGL instance. The app now initializes only Vulkan and DX12; the workaround is `WGPU_BACKEND` — [README](https://github.com/storytold/filmcraft/blob/main/README.md)
- Other reported bugs:
  - Color Matte always grey with no picker (#29) — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
  - `filmcraft-cli` panicked on a broken pipe until #285 — [ROADMAP Log](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)

**G. Security (custom parsers on untrusted media)**
- The project explicitly treats media as hostile:
  - "Every input-derived number is hostile… cap allocations sized by input."
  - "Bound recursion."
  - "Parsers and decoders get mutation-fuzz tests (truncation, bit flips, corrupt sizes) run under `catch_unwind`."
  - Source: [AGENTS.md §0](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)
- The testing tiers include "Robustness / fuzz: `*/tests/robustness.rs`, `*/tests/fuzz.rs` — seeded mutation and truncation…; nothing may panic". This is seeded mutation inside `cargo test`, not continuous coverage-guided fuzzing. I found no evidence of cargo-fuzz or OSS-Fuzz — [docs/testing.md](https://github.com/storytold/filmcraft/blob/main/docs/testing.md)
- `unsafe_code = "forbid"` workspace-wide, with one exception: the `crates/platform` FFI crate for VideoToolbox, Media Foundation/D3D11, VA-API and NVENC (ADR 0001). Hardware decode paths therefore hand untrusted bitstreams to OS and driver decoders through `unsafe` FFI — [AGENTS.md §0.3](https://github.com/storytold/filmcraft/blob/main/AGENTS.md); [Cargo.toml](https://github.com/storytold/filmcraft/blob/main/Cargo.toml)
- `panic = "unwind"` is kept intentionally so a `catch_unwind` last-resort guard works — [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)
- **There is no SECURITY.md or vulnerability disclosure policy** (404 on main) — [raw check](https://raw.githubusercontent.com/storytold/filmcraft/main/SECURITY.md)
- **Attack surface beyond media files.** The app exposes:
  - a JSON-lines **TCP control channel** (`--control 9876`)
  - an MCP server
  - UI automation (click, drag, type, screenshot)
  - Source: [README Built for agents](https://github.com/storytold/filmcraft/blob/main/README.md); [docs/agents.md](https://github.com/storytold/filmcraft/blob/main/docs/agents.md)
- Design Week's writer would not run it with disk access — [Design Week](https://www.designweek.co.uk/first-ai-came-and-scooped-up-your-art-now-its-scooped-up-adobe/)

**H. Bus factor, governance, contributions**
- There is one human "owner" whose decisions gate key items ("**Owner decision needed** before starting" plugin hosting) and "a single integrator merging" — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- Design Week describes the repository as a small two-person project — [Design Week](https://www.designweek.co.uk/first-ai-came-and-scooped-up-your-art-now-its-scooped-up-adobe/)
- The real plan lives in gitignored `plan/` files, so outsiders see only ROADMAP.md — [CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md)
- Contributions are accepted under MIT OR Apache-2.0 with no CLA mentioned. Rules are "absolute": no Adobe assets, no GPL/LGPL code, ffmpeg only as an oracle. Every commit must pass `cargo xtask ci` (fmt, clippy -D warnings, tests, layers, assets, wasm). Commits use one task id each (e.g. `M2.4: …`). Contributors are credited in the About window via `contributors/people.toml` — [CONTRIBUTING.md](https://github.com/storytold/filmcraft/blob/main/CONTRIBUTING.md); [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md); [CLAUDE.md](https://github.com/storytold/filmcraft/blob/main/CLAUDE.md)
- A full `cargo xtask ci` takes "20–60 min under load". Each worktree build is 6–7 GB — [ROADMAP](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)
- The project schema version moves fast: v7 → v12 between 2026-10-01 and 2026-10-03, with migrations in `docs/project-files.md` — [ROADMAP Log](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)

**I. Sustainability and business model**
- Funding comes from ArtCraft's paid AI offering: "selling token-based access to the existing ArtCraft image model, which also appears inside the apps". Subscriptions with generative-AI credits exist, while the apps themselves are free — [PetaPixel via search summary](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/); [TechSpot](https://www.techspot.com/news/114136-software-over-developer-uses-ai-build-free-adobe.html)
- The README positions ArtCraft as "our AI image and video studio for artists who want real control" and funnels users to Discord and getartcraft.com — [README](https://github.com/storytold/filmcraft/blob/main/README.md)
- Background: Thomas founded ArtCraft about a year ago. He was previously CEO of Storyteller.ai (Atlanta, founded 2022), founded FakeYou, and was an engineer at Square — [GitHub echelon](https://github.com/echelon); [Tracxn](https://tracxn.com/d/companies/storyteller.ai/__mWn75HNsGH4F05zmy6ak4rB3JsliEF2B8YOcxQmZo9U); [PetaPixel](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/)
- His stated motivation is Adobe's "dark pattern" cancellation fee. Adobe agreed to a $150M settlement in March 2026 while denying wrongdoing — [atlabyte](https://atlabyte.com/en/news/software-is-over-developer-unveils-ai-crafted-open-source-adobe-clones); [Massimo on X](https://x.com/Rainmaker1973/status/2108189742219096223)

**J. License, clean-room and brand obligations (directly relevant to forking)**
- Code is MIT OR Apache-2.0. NOTICE must be preserved. Bundled fonts (Inter, Noto Serif, JetBrains Mono, plus craft-fonts in official builds) are OFL-1.1 — [NOTICE](https://github.com/storytold/filmcraft/blob/main/NOTICE)
- **The ArtCraft name and logos are trademarks and are not covered by the code license.** "If you distribute or publish a modified version… you must remove the ArtCraft Marks… and must not call it ArtCraft." You "may say, in plain text, that your work is based on FilmCraft by the ArtCraft team". A fork kept only to send changes upstream may keep the marks — [LICENSE-brand.txt](https://github.com/storytold/filmcraft/blob/main/docs/brand/LICENSE-brand.txt)
- symphonia (MPL-2.0) is a dependency. Its file-level copyleft applies only if you modify symphonia's files — [README](https://github.com/storytold/filmcraft/blob/main/README.md); [AGENTS.md §2](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)
- Clean-room process as stated:
  - Never read or disassemble Adobe bundles. Observe behaviour only.
  - Never copy GPL/LGPL/AGPL code (FFmpeg, x264, x265, MLT, Kdenlive, Shotcut, Olive…).
  - Implement codecs from specs without reading reference decoders (libvpx, dav1d, openh264…).
  - Premiere reference screenshots live only in gitignored `plan/premiere/`.
  - The README says FilmCraft contains "no Adobe code, icons, images, presets or LUTs".
  - Sources: [AGENTS.md §1–2](https://github.com/storytold/filmcraft/blob/main/AGENTS.md); [README](https://github.com/storytold/filmcraft/blob/main/README.md)
- **The clean-room claim cannot be externally verified.** The agents were trained LLMs, and the reference material is private. Press notes that "clean-room" is Thomas's own characterization, not a legal determination. Design Week floats (speculatively) that decompiling would breach Adobe's EULA — [Yahoo Tech](https://tech.yahoo.com/ai/claude/articles/software-over-developer-uses-ai-174550981.html); [Design Week](https://www.designweek.co.uk/first-ai-came-and-scooped-up-your-art-now-its-scooped-up-adobe/)
- The UI deliberately copies Premiere: "the same panels, workspaces, tools and shortcuts", "Premiere 26 look", and an effects bin naming Premiere's own effect list (Lumetri, Ultra Key, Warp Stabilizer…). Adobe trademarks are disclaimed in the README footer — [README](https://github.com/storytold/filmcraft/blob/main/README.md); [ROADMAP M0](https://github.com/storytold/filmcraft/blob/main/ROADMAP.md)

**K. Adobe legal exposure (trade dress, patents)**
- I found no public Adobe response and no reported lawsuit as of the coverage — [search summary across Futurism, Design Week, Ars](https://futurism.com/future-society/adobe-creative-suite-software-open-source-artcraft)
- The coverage's main stated legal exposure is **trade dress**: reimplementing features is generally lawful, "but the clones could face lawsuits if their 'trade dress' too closely replicates Adobe's recognizable interface" — [BigGo Finance](https://finance.biggo.com/news/3969ffe0-e181-451e-8cc7-f21136f246ed); [Notebookcheck](https://www.notebookcheck.net/Photoshop-Premiere-more-Experienced-Rust-developer-recreates-Adobe-suite-with-AI-assistance-as-open-source.1419230.0.html)
- One report says long-term viability hinges on whether ArtCraft infringes Adobe's "thousands of patents covering software appearance and functionality". No specific patent was analyzed — [search summary of PetaPixel/related coverage](https://petapixel.com/2026/10/07/someone-rebuilt-free-open-source-versions-of-photoshop-premiere-and-lightroom-using-ai/)

**L. Analogous risk for a DaVinci Resolve / Blackmagic UI clone (legal background)**
- *Lotus v. Borland*: the First Circuit (1995) held that a menu command hierarchy is an uncopyrightable "method of operation", reversing a 1992 district-court win for Lotus — [Look and feel (Wikipedia)](https://en.wikipedia.org/wiki/Look_and_feel); [Copyright of User Interfaces](https://www.cs.oberlin.edu/~rms/mmcc/a/mod11/mod113.html)
- *Apple v. Microsoft*: the court rejected a gestalt "look and feel" claim and required analytic dissection of UI elements. A prior license limited Apple's claim — [Apple v. Microsoft (Wikipedia)](https://en.wikipedia.org/wiki/Apple_Computer,_Inc._v._Microsoft_Corp.)
- *Lotus v. Paperback*: direct copying of 1-2-3's screen displays was held infringing — [Look And Feel In Computer Software](https://computerlaw.com/articles/look-and-feel-in-computer-software/)
- Trade dress (Lanham Act) protects source-identifying non-functional design. Functional features are barred — [Trade dress (Wikipedia)](https://en.wikipedia.org/wiki/Trade_dress)
- Trade dress is the usual fallback "knockoff" theory when no copyright or patent fits — [Venable, What Is Trade Dress Law](https://www.venable.com/files/Publication/f44938c2-3d01-4b08-8c86-b046e52531c6/Preview/PublicationAttachment/c351df88-a9ef-423a-bbf9-dea1a5b1bd93/What_Is_Trade_Dress_Law.pdf)
- Software look-and-feel trade dress has been pleaded before, e.g. Fidelity, 2009 — [Techdirt](https://archive.techdirt.com/articles/20090902/0123586074.shtml)
- "DAVINCI RESOLVE" is a registered or applied-for trademark — [Justia trademark record](https://trademark.justia.com/877/32/davinci-87732717.html)
- I found no reported Blackmagic Design lawsuit over Resolve UI clones — [search, no results](https://ndvlaw.com/can-tech-companies-sue-for-cloned-software-interfaces/)

**M. Codec patent licensing (H.264/HEVC) for an open-source distributor**
- FilmCraft ships its own H.264 decoder and encoder and an HEVC decoder in software, plus HEVC export via OS/GPU encoders — [README](https://github.com/storytold/filmcraft/blob/main/README.md)
- **HEVC:** Access Advance runs the HEVC Advance pool (29,000+ patents). It acquired Via LA's HEVC/VVC pool in December 2025, which became "VCL Advance". The pool licenses HEVC decoders and encoders "in devices, software or through cloud-based services", with per-unit royalties and special terms for software sold separately — [Access Advance HEVC Advance Program Overview (July 2026)](https://accessadvance.com/wp-content/uploads/2025/07/HEVC-Advance-Program-Overview-July-2026.pdf); [BusinessWire Jan 2026](https://www.businesswire.com/news/home/20260126799824/en/Access-Advance-Extends-HEVC-Advance-Rate-Increase-Deadline)
- **H.264/AVC:** Via LA (formerly MPEG LA) licenses AVC. The first 100,000 units per year per legal entity are $0. Above that, $0.20 per unit up to 5M and $0.10 per unit beyond, with an annual enterprise cap ($9.75M since 2017). A "unit" is a decoder, an encoder, or a product with one of each — [Via LA AVC fee PDF](https://www.via-la.com/wp-content/uploads/avcweb.pdf); [Via LA AVC page](https://www.via-la.com/licensing-2/avc-h%20264/)
- New AVC streaming terms for 2026 licensees replace the old $100k cap with tiers up to $4.5M — [TechSpot](https://www.techspot.com/news/111971-h264-licensing-fees-could-jump-100k-45m-streaming.html)
- No coverage or project doc addresses FilmCraft's own patent-license posture. The README and AGENTS.md discuss only copyright and clean-room questions — [README](https://github.com/storytold/filmcraft/blob/main/README.md); [AGENTS.md](https://github.com/storytold/filmcraft/blob/main/AGENTS.md)

### Inferences
- **Upstream-merge gotchas for "Resolution":**
  1. Extreme churn. Hundreds of PRs and issues in about 9 days, schema v7→v12 in 3 days, and parallel agents rewriting crates. Pin to release tags, not main, and track ROADMAP "Log" entries for breaking changes (project schema, command ids, `platform` crate API).
  2. Keep the engine boundary clean. FilmCraft already isolates the UI in the `ui-egui` crate, and "a different front end can replace it without touching editing logic". A Resolve-style UI should live in its own crate over `engine`, and the `ui-egui` crate should not be forked in place. This is the main lever for cheap upstream merges ([README Everywhere](https://github.com/storytold/filmcraft/blob/main/README.md)).
  3. Inherit and keep upstream's gates (`cargo xtask ci`, never-crash lints, layering), so upstream patches apply without fighting local style.
  4. Remove the ArtCraft marks in any distributed build (required), but keep a pristine "upstream-proposal" fork for contributing back (allowed).
  5. Premiere-specific command ids, shortcut presets, the "Lumetri" naming and effect-bin taxonomy are baked into the engine and command registry. A Resolve UI will need a mapping layer, and engine-level renames would cause merge conflicts.
- **Engineering gotchas to plan for:**
  - CPU-only Lumetri, effects and export. Resolve users expect GPU colour, which means substantial divergence or waiting on upstream #30.
  - Linux is the least exercised platform.
  - Windows launch and driver issues.
  - Unverified real-world media decode. Plan your own camera and phone corpus.
  - No OFX/VST hosting.
  - Unvalidated AAF and OMF.
- **Security.** Treat FilmCraft's pure-Rust decoders as memory-safe but not DoS-proof (allocation caps and recursion bounds are policy, not proven). Add cargo-fuzz or OSS-Fuzz-style continuous fuzzing on the codec and container crates. Disable or bind the TCP control channel and MCP to localhost or behind auth by default. Watch for a SECURITY.md upstream.
- **Legal.** Recreating DaVinci Resolve's distinctive page-based layout (Media/Cut/Edit/Fusion/Color/Fairlight/Deliver tabs), node-graph colour page look and visual styling carries the same trade-dress theory press raised against ArtCraft. Precedent (Lotus v. Borland, Apple v. Microsoft) makes menu structure and functional workflow weak copyright targets. Distinctive visual styling, icons and the "DaVinci Resolve" marks are the real exposure. The name **"Resolution" is close to "Resolve"** in the same goods class, which is a trademark-confusion risk worth a clearance check before any distribution. Because the product is private, exposure is lower until distributed. This is inference, not legal advice.
- **Codec patents.** Any distribution, even free, of software H.264 encoders and decoders and HEVC decoders can trigger pool-licensing questions. Via LA's AVC free tier (100k units/yr) may cover small distribution. HEVC Advance has no comparable free tier evident in the materials found. A private, non-distributed tool reduces but doesn't eliminate this; using OS-provided decoders/encoders (VideoToolbox, Media Foundation) shifts licensing to the OS vendor. This inference is based on general practice, not on a found source specific to FilmCraft.
- **Bus factor and sustainability.** The roadmap has a single owner and single integrator, depends on agent compute funded by an AI-token business, and the plan of record is private. If ArtCraft's commercial focus shifts, FilmCraft velocity could drop abruptly. A fork should be prepared to own the engine.

### Gaps
- **"Stutter on 5K 10-bit" footage:** no source found. It is plausible given the CPU-bound pipeline and VP9/AV1 software decode on Linux, but unverified.
- No independent benchmark, audit or code review of FilmCraft (or any Crafting App) codec correctness was found. All accuracy claims are self-reported against ffmpeg oracles.
- Full text of the Ars Technica, Gizmodo, PetaPixel, Futurism and TweakTown articles could not be read (fetch blocked). Quotes come from search-engine summaries and should be spot-checked.
- HN thread comments (beyond the reported 120+ points and 180+ comments, and Thomas's timeline walk-back) were not readable.
- No statement by Thomas was found on governance, a foundation, a CLA, accepting forks, or long-term commercial plans for FilmCraft specifically, beyond the token-funding model and the brand-license fork terms.
- No primary source covers whether Access Advance or Via LA treat free open-source software codecs differently. Cisco OpenH264's arrangement was not verified.
- No reported Blackmagic Design litigation over UI clones was found. Absence of results is not evidence of a policy.
- GitHub issue list, labels, open/closed counts and Discussions status could not be enumerated (GitHub API blocked for this repo in this session).
