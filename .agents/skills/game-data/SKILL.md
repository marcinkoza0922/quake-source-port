---
name: game-data
description: Look things up in the installed Quake game data for this port — which mods/gamedirs exist, which maps a mod ships, which PAK/PK3 supplies a file, and a map's entities (spawn points, coordinates, targetnames, worldspawn keys like fog/sky, secrets, keys, monsters). Use whenever you need coordinates for a screenshot or setpos, want to know what a map contains, need to find where an asset or progs.dat comes from, are triaging a mod-specific bug, investigating a qbot failure, or picking a map for a test — and before reaching for tools/qinspect, which is still a stub.
---

# Game data lookup

Game data lives under `QUAKE_BASEDIR` (default: the gitignored symlink
`./vkQuake directory with maps`). It is the user's own install: **read only**,
never commit any of it, never write into it (the engine itself never does;
user data goes to `~/.local/share/quake-source-port/<game>/`).

`tools/qinspect` was planned as the data dumper, but its `main` still only
prints "not yet implemented". Use `tools/qdata.py` instead. It needs no
build and mirrors the engine's search order from `crates/qcore/src/fs.rs`.
If you extend it, keep it read-only and keep its Tooling line in DESIGN.md
up to date.

## qdata.py

```sh
Q=tools/qdata.py
python3 $Q mods                                # gamedirs + what each has (pak/pk3/progs/maps)
python3 $Q maps -g ad                          # maps a game itself ships (--all adds base dirs)
python3 $Q which progs.dat -g ad               # which archive wins for a path
python3 $Q ents e1m1 -c info_player_start      # entities; -c is a classname substring
python3 $Q ents e1m1 -k targetname==t1         # -k KEY==EXACT or KEY=SUBSTR, repeatable
python3 $Q ents ad_sepulcher -g ad -c worldspawn --all   # every key (fog, sky, _wateralpha...)
python3 $Q ents hip1m1 -g +hipnotic -c item_ --json
```

- `-g` takes a comma list; `+hipnotic`/`+rogue`/`+quoth` mount a mission pack
  under a mod the way `-hipnotic -game mod` does. id1 is always mounted.
- `ents` reads the BSP entity lump (BSP29/BSP2/2PSB/Quake64), replaced by a
  `maps/<map>.ent` from the same or a higher-priority gamedir, like the engine.
- Default output shows the keys that usually matter (origin, targetname,
  target, killtarget, model, spawnflags, angle, mangle, message). Brush
  entities (doors, triggers, `trigger_secret`) have `model "*N"` and no
  origin; their position is the submodel's bounds, which the script does not
  compute. For those, use a nearby point entity or the qbot verbose log.

## Common uses

- **A camera spot**: `info_player_start` origin + `angle` → `setpos x y z 0 <angle> 0`
  (see the visual-check skill). Monsters and items give spots deeper in a map.
- **Map facts for a bug**: worldspawn `fog`, `sky`, `wateralpha`, `_texpart_*`
  (FTE particles), `message` (title); count `monster_*` / `trigger_secret`.
- **Wiring**: follow `target` → `targetname` chains to see what a button or
  trigger fires (qbot door gating works the same way).
- **Which mod overrides what**: `which` answers "is this id1's file or the mod's?",
  e.g. a mod's `gfx.wad`, `progs.dat`, `default.cfg`, or a `.lit`.

## The installed set (2026-11-12)

id1, hipnotic, rogue, ad, mg1, mg3, q64 plus about 25 community mods and jams
(copper, dwellv2p2, qbj/qbj2/qbj3, func_mapjam3/5, xmasjam2017–2021, udob,
rubicon2, …), `ironwail-*-linux-x64.7z` and `vkquake.AppImage`. Run `mods` for
the current list rather than trusting this one. Tests and tools must not
assume a given mod is present: skip when its directory is missing.
