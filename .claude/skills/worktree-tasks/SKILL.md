---
name: worktree-tasks
description: Split work in this Quake port into independent tasks and run them in separate git worktrees (for parallel subagents, or to keep a long experiment off the main checkout) without losing access to the gitignored game data and reference engines. Use whenever the user asks to parallelise work, "give tasks to subagents", run things side by side on different branches, use a worktree, or when a subagent's tests mysteriously skip or can't find ironwail/vkQuake/refs in a worktree.
---

# Parallel tasks in worktrees

PROPOSAL.md asks for discrete tasks that don't interfere with each other to
go to subagents. The user still has to ask before you spawn any. Worktrees
keep each task's build, branch and outputs apart. Earlier sessions used
`/tmp/quake-<topic>` on a `feature/<topic>` branch.

## What a fresh worktree is missing

Only tracked files are checked out. These are gitignored and won't be there:

- `vkQuake directory with maps` (the game data symlink). Without it,
  **every game-data test silently skips and passes**, and qbot, qsmoke and
  the engine find no `id1`.
- `ironwail/`, `vkQuake/`, `refs/` (reference sources).
- `target/` (no build, no qbench demos).

## Setup

```sh
main=$(git rev-parse --show-toplevel)          # run from the main checkout
topic=my-topic
git worktree add -b feature/$topic /tmp/quake-$topic master
for d in "vkQuake directory with maps" ironwail vkQuake refs; do
  [ -e "$main/$d" ] && ln -sfn "$(realpath "$main/$d")" "/tmp/quake-$topic/$d"
done
# .gitignore has /ironwail/ /vkQuake/ /refs/ — trailing-slash patterns match
# directories only, so symlinks with those names would show up as untracked.
ex="$(git -C "$main" rev-parse --path-format=absolute --git-common-dir)/info/exclude"
for p in /ironwail /vkQuake /refs; do grep -qxF "$p" "$ex" 2>/dev/null || echo "$p" >> "$ex"; done
ls "/tmp/quake-$topic/vkQuake directory with maps/id1" >/dev/null && echo "game data OK"
git -C "/tmp/quake-$topic" status --porcelain    # should print nothing
```

The Agent tool's `isolation: "worktree"` gives you a worktree automatically,
but it is missing the same links. Make creating them the subagent's first
step, or set `QUAKE_BASEDIR="$(realpath "$main/vkQuake directory with maps")"`
in its commands.

The local `info/exclude` (shared by all worktrees, never committed) keeps the
reference symlinks out of `git add -A`. The data symlink is already covered
by `.gitignore`. Still check `git status` before committing in a worktree.

## Builds

Each worktree has its own `target/`, so its first build is a full one (in
release with `lto = "thin"` and `codegen-units = 1` this takes a while).
Don't point several worktrees at one `CARGO_TARGET_DIR` while they build at
the same time: cargo's lock serialises them, and they overwrite each other's
binaries.

## Splitting tasks so they don't collide

- Split along crate boundaries with frozen interfaces (DESIGN.md: qformats,
  the qvm interpreter, the qsound mixer, the qui toolkit and qbot nav were
  meant as independent tasks). One crate or subsystem per task.
- Hotspot files that several features touch: `docs/DESIGN.md`,
  `src/host/cvars.rs`, `src/menu/pages.rs`, `src/menu/page_info.rs`,
  `src/host/render.rs`, `crates/qrender/src/frame.rs`, `Cargo.toml`. Either
  give one task ownership of each, or have tasks return their snippets
  (DESIGN.md paragraph, cvar rows, menu rows) for you to merge serially.
- Freeze any shared interface (types in `qrender::types`, `qclient` state,
  `qserver` APIs) before fanning out. Change it first on master, then branch.
- Give each task its own acceptance check (tests, qbot game, screenshot set)
  so results can be judged independently.
- Several worktrees share the user's data dir
  (`~/.local/share/quake-source-port`). Headless runs need
  `+sv_autosave 0`, and windowed runs must be SIGKILLed (visual-check).

## Bringing it back

Review each branch's diff yourself. Merge or rebase one at a time, run the
land-change checks on the combined result, then clean up:

```sh
git worktree remove /tmp/quake-$topic     # or `git worktree prune` for ones already deleted
```

If it refuses because of untracked files, look at them first (unsaved work?),
then use `--force`. Removing a worktree deletes the data and reference
symlinks, not what they point to.

`git worktree list` currently shows three stale `prunable` entries in /tmp
(autosaves, qbot-navigation, see-through-portals). Prune them only if the
user agrees: `feature/see-through-portals` may hold unmerged work.
