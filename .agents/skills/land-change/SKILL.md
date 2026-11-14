---
name: land-change
description: Finish a change in this Quake port properly — run the right verification for what was touched (nextest, clippy, screenshots, qbot, qsmoke, qbench), keep files under the 500-line guideline by splitting along real seams, update docs/DESIGN.md, and, when the user asks, commit on a topic branch and open a PR in the repo's style. Use before saying a task is done, before committing or opening a PR, when the user says "wrap up", "ship it", "commit this", "make a PR", "clean up", or when a file has grown past 500 lines.
---

# Landing a change

## 1. Verify in proportion to what changed

Always:

```sh
cargo clippy --message-format=short 2>&1 | tail -30   # must be clean (warnings count)
cargo nextest run 2>&1 | tail -15                     # with game data present
```

Workspace lints (`Cargo.toml`): clippy `correctness` is deny, and
`suspicious`, `perf`, `dbg_macro`, `todo`, `unimplemented`, `mem_forget`,
`undocumented_unsafe_blocks` (write `// SAFETY:`) and `large_stack_arrays`
warn. `unsafe_op_in_unsafe_fn` is deny. Fix the warnings rather than
`#[allow]`ing them, unless the existing code does the same for the same reason.

Then add the checks that match the change:

| Touched | Also run |
|---|---|
| Anything visible (render, HUD, menus, effects) | headless screenshots, looked at (visual-check) |
| qvm, qworld, qserver, triggers, doors, physics | `target/release/qbot` on id1, hipnotic and rogue, baselines hold (qbot) |
| fs, BSP/model loading, VM, protocol | `target/release/qsmoke` for the affected mods or all of them (mod-compat) |
| Hot paths (renderer, client frame, cull) | headless timedemo A/B or qbench (perf-profiling) |
| Vulkan code | `QRENDER_VALIDATION=1` run is clean (vulkan-renderer) |

Report honestly: what you ran, what passed, what you skipped and why. Count
data tests that skipped as skipped.

## 2. File length (AGENTS.md)

```sh
find src crates tools -name '*.rs' | xargs wc -l | awk '$2!="total" && $1>500' | sort -rn
```

Keep Rust files under 500 lines, and split only at a logical seam: a
cohesive group of types, a subsystem, or a separate `impl` concern. The
established pattern is `foo.rs` + `foo/*.rs` submodules declared from the
parent (`src/host.rs` + `src/host/`, `src/menu.rs` + `src/menu/`). A crate's
`lib.rs` uses `#[path = "lib/x.rs"] mod x;`. Moved items become `pub(super)`,
and behaviour must not change in a split. A single function over 500 lines
that can't be split logically stays as it is (`qrender/src/frame.rs` is the
current example). Don't split just to hit the number.

## 3. docs/DESIGN.md

DESIGN.md says *how* (PROPOSAL.md says *what*), and AGENTS.md requires
keeping it current and under 500 lines. It is at about 450 lines of very
long paragraphs, so:

- Edit the section that owns the topic (architecture, the crate's
  "Implemented APIs" entry, the milestone status, the Tooling list for new
  `tools/` helpers). Don't append a changelog at the end.
- Replace superseded numbers and status instead of stacking new ones under
  old ones. Keep dates on measurements (e.g. "2026-11-12").
- Record decisions the user made, cvar names and defaults, what was ported
  from which reference, and what was deliberately left out.
- Check with `wc -l docs/DESIGN.md`.

## 4. Commit and PR (only when the user asks)

- Never commit game data, `ironwail/`, `vkQuake/`, `refs/`, `target/`
  outputs, screenshots or `.pak`/`.pk3` files (most of these are
  gitignored; check `git status` anyway).
- If you're on `master`, branch first: `feature/<topic>`, `fix/<topic>`,
  `improve/<topic>` or `refactor/<topic>` (the existing names).
- Commit subject: `Area: what changed`, imperative and specific, about 72
  characters at most, e.g. `Debris: optional Kill Lift (r_debris_killlift)`
  or `qbot: gate locked doors; earn keys; safer returns to the graph`. The
  body is a few wrapped lines on *why* and on behaviour the reader can't see
  in the diff (defaults, gating, numbers). End with the attribution trailer
  the harness specifies.
- PR to `master` with `gh pr create`: title = the change in one line; body
  = `## Summary` bullets naming each commit (short hash in backticks), plus
  validation numbers when there are any (tests, qbot totals, fps), then the
  generated-with line the harness specifies.
- Push or open the PR only when asked, and confirm before force-pushing or
  rewriting anything already pushed.
