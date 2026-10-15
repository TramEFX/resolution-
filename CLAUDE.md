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

## Unattended runs (first tried 2026-10-14; to refine)

Truman may hand over a whole usage window, such as the five hours after a reset, while he
rests. His PC stays on, and a Claude Code session can stay open for days, so session-scheduled
jobs (CronCreate) fire through the night. The hardware is ours to use. Visual test windows go on
display 2 (X=-1920, Y=165, 1920×1080), not his main screen.

How it runs:

1. **A queue file** holds the night's goal, its rules, the work items in order, and the wrap-up.
   Every job reads it, and items are ticked off there as they finish. It sat in the session's
   scratchpad the first time.
2. **Scheduled jobs,** each one-shot:
   - a start a few minutes after the reset;
   - hourly check-ins, which resume from the file if a turn ended early (a job waits while a
     turn is still running);
   - a last check-in about 30 minutes before the end, which only finishes what can still be
     merged;
   - a wrap-up.
3. **Rules while he's away:**
   - Ask nothing; a question would block the night.
   - A choice that can be undone later: make it, and log its D-entry as "provisional, for
     Truman's review".
   - A choice that is hard to undo (vocabulary ids, the grade JSON schema, MCP parameters, the
     node-graph model): don't build it. Write up the options with a recommendation instead.
   - Post nothing outside this repo.
   - Every PR passes the checks above, and new tests get a mutation check.
   - Unfinished work stays on its branch, unmerged.
4. **The morning report,** as the final message and at the top of the queue file:
   - the PRs merged;
   - provisional decisions, for review;
   - options waiting on Truman;
   - what was seen by eye;
   - what failed or was skipped.

   HANDOFF, the LOG index and memory are brought up to date before it.

Learned on the first run (2026-10-14/15):
- **The usage cap, not the clock, ended it.** It ran out about 1.5 hours into a five-hour window,
  after 11 PRs.
- **Check-ins don't help once the cap is out.** They queued, and with them the wrap-up, and all
  arrived together at the next prompt the next day. So the morning report did not exist until
  then.
- **So keep the report current after every PR**, in the queue file and the memory note. Write
  the log and HANDOFF as each PR merges, never only at the end, and leave nothing half-done on
  main at any moment.

To refine:
- Keep the queue file in the repo (`docs/runs/`), so it outlives the session.
- Pace the work to the cap, or not: a quick burn finishes as much, but leaves no one to answer
  the check-ins.
- How big an item should be, and how often to check in.
- Whether subagents in worktrees are worth their cost for parallel items.
- How to measure what a night achieved.
