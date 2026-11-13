---
name: qbot
description: Run, interpret, debug and improve qbot (tools/qbot), the automated level-breaking test that plays every map at skill 3 with god mode, noclip and full knowledge of the edicts — killing every monster, finding every secret, setting off every trigger, button and shootable, taking every key, then touching the exit — to see whether a level or the engine breaks. Use whenever the task mentions qbot, the test bot, "kill every enemy / find every secret / hit every trigger", baselines or REGRESSION lines, failed goals in report.json, or after changing server physics, the QuakeC VM, triggers, doors/plats, spawning or anything else that can change how a level's scripting runs.
---

# qbot

**What it's for** (the user's words): not traversing levels the way a player
would — that was "a fool's errand" and the old nav-graph bot was dropped on
2026-11-12. qbot tries to **break the level**: kill every enemy, find every
secret, hit every trigger and scripted event. It has god mode, noclip and full
knowledge of where everything is. Keep it fast on every game (about a second
of wall time per game on all cores).

It runs `qserver::Server` in-process and headless at 20 Hz, skill 3, one
client driven by `set_move`. Runs are deterministic. A release build has
`panic = "abort"`, so a panic on one map ends the whole run.

## Running

```sh
cargo build --release -p qbot
target/release/qbot                       # every playable id1 map, all cores
target/release/qbot -game hipnotic        # baselines exist for id1, hipnotic, rogue
target/release/qbot e1m3 e2m5 -v          # specific maps, verbose log
target/release/qbot -game ad ad_sepulcher -time 1200
```

Flags: `-basedir D -game G -skill N -time S -j N -out DIR -baseline F
-save-baseline -v`.

Output `target/qbot/report.json` per map: kills, secrets, triggers, buttons
(incl. shootable brushes), pickups (keys, runes) as done/total, `has_exit`,
`exited`, `failed` (one line per goal given up on: `Kind classname #ent at
pos: why`), sim/wall time, `errors`, `unusual` (developer-log lines, see
below). `cargo nextest run -p qbot` checks e1m1
and e1m3 (every monster, every secret, exit); skips without data.

## Baselines

`tools/qbot/baseline-<game>.json`: `{kills, secrets, events, exited}` per map
(`events` = triggers + buttons + pickups). Fewer of anything, a lost exit, or
any engine error prints `REGRESSION` and exits 1. Each entry also has
`log`: the map's known developer-log lines (qbot runs `developer 1`;
`bot/devlog.rs` drops lines every level prints). A line not in `log` prints
`NEW-LOG map: text` and is a regression: read it before re-saving. Most
known lines are vanilla spawn complaints (`"property 1" is not a field`,
`walkmonster in wall`, `Bonus item fell out of level`, OBJECT ERROR dumps
for missing targets); add a pattern to `EXPECTED` only for noise every
level prints. Engine errors and QC errors
are the point of the tool: treat them as real bugs.

Any change to the bot can reorder goals and move a count by one or two
(failed goals are order-dependent). Re-save (`-save-baseline`, merged per
map) only after you've looked at every map that dropped and can explain it;
never just to make a regression go away.

## How it works (tools/qbot/src)

- `bot/goals.rs`: goals from edicts, rescanned every 2 s (triggers spawn
  monsters). Monsters not solid and not damageable are dormant until they
  wake. Exits are inert (touch cleared) until everything else is done.
- `bot.rs`: main loop picks the nearest open goal; 20 s timeout per goal,
  failed goals get one retry at the end. Teleports (a jump further than
  flight) count trigger goals at the departure point; a bot pinned in place
  for 1 s (teleporter whose destination is inside itself) counts the triggers
  there and `setpos`es out. `impulse 9` every 5 s; fire is held while
  impulses are queued (QC ignores them until `attack_finished`).
- `bot/fly.rs`: classic noclip flight (`sv_altnoclip 0`, `sv_maxspeed
  1000`); touch goals fly to a point in the box clear of other triggers;
  after 5 s it `setpos`es there. Shootables: nearest open firing spot with a
  clear shot at the centre or a face. Buttons: noclip never touches, so it
  stands just off a face (or on top), turns noclip off and walks/swims in.

## Debugging a failed goal

1. `target/release/qbot <map> -v`: goals, done, warps, pushes, pins, FAILED
   lines with positions.
2. Look up the entity with the game-data skill (`tools/qdata.py ents <map>
   -c <class>`); edict numbers differ from entity-lump indices, match by
   position or model.
3. Look at the spot: headless screenshot with `setpos x y z` (visual-check).
4. Decide: a bot gap (fix generally in `fly.rs`/`goals.rs`, rerun all three
   games) or a real level/engine problem (that's a finding: report it).

hipnotic `func_spawn` keeps dormant template monsters (`spawnfunction` set)
that never wake; they are skipped. A spawner fired by `item_hornofconjuring`
makes a charmed ally and leaves `total_monsters` one higher (hip2m1 62/63 is
that, not a bot gap). Doors that fire a `target` when touched open are goals
pressed like buttons. No QC source for a mod? Disassemble the function from
`vm.progs` (statements + `global_at_ofs` names): that's how func_spawn and the
horn were worked out.

Known residuals (2026-11-12): e1m7 closet zombies that get knocked down where
no open spot sees them; a few buttons in id1/rogue/hipnotic; hip1m5 shootable
door; Shub-Niggurath in `end` (killed by a telefrag the bot doesn't perform).
