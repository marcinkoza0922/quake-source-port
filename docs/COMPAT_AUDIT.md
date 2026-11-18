# Compatibility and parity audit (2026-11-17)

References: **ironwail** master (08d57813, built locally on branch `qsp-diag`
with diagnostics, see DESIGN.md › Tooling), **vkQuake** 1.34.1 (AppImage) and
the **2021 re-release** (KEX). Its data is the installed id1/hipnotic/rogue/
mg1/mg3/dopa/ctf (byte-identical to the Steam `rerelease/` paks), and its
saves were tested. The KEX executable itself was not run: it is a Windows
binary behind Steam DRM.

Game data at the start: 38 game dirs, 877 BSPs (559 BSP29, 268 BSP2, 32
Quake64; BSPX `FACENORMALS`, `BRUSHLIST`), 773 `.lit`, 53 `.ent`, 8178 MDL,
127 md5mesh, 38 MD3, 809+8 sprites (10 SPR32), 23 shipped demos (protocols
15, 666 and 999 with RMQ flags), 42 savegames (ours, vkQuake, KEX). During
the session quoffee, smej, smej2, mc_hj26 and func_mapjam9_2 were added,
followed by every Quaddicted jam not yet installed (see Map packs).

## How it was tested

| Check | Tool | Result |
|---|---|---|
| Static inventory: file types, configs, progs builtins/strings, BSP formats, entity keys | `tools/compat_audit.py` | `target/compat/inventory.md` |
| Every map loads and runs 8 s (skill 3) | `qsmoke` | 680 + 128 maps; 1 content error (jjj2_dfl objerror, non-fatal in all engines) |
| Spawn state = ironwail (save right after `map`) | `tools/spawnparity.py` | 876 maps: identical except ironwail failures below |
| 3 s of simulation = ironwail (216 frames, `host_framerate`) | `spawnparity.py --frames 216` | identical on every map ironwail can load, skill 1 and 3 |
| Savegames: ours and ironwail load + re-save the same corpus | `tools/savecompat.py` | 45 saves (ours, vkQuake, KEX v6): both engines read every save identically |
| Demos in all three engines | `tools/demosweep.py` | inconclusive, see Demos |
| Behaviour under play | `qbot -game all` | see qbot |

The parity sweeps run both engines as one-slot dedicated servers with the
same RNG (ironwail `qsp_rng 1`: our LCG and seed for `random()` and
`movetogoal`). A difference is therefore a real difference in spawn
functions, builtins, physics, edict allocation or the save format.

"Except ironwail failures": func_mapjam3 ships `PAK0.PAK`/`PROGS.DAT` and
qbj `qbj_Mr_M.bsp` in upper/mixed case. ironwail on Linux looks names up
case-sensitively and misses them; ours matches case-insensitively, as a
Windows engine does, which these releases were made with.

## Fixed in this audit

Gameplay and simulation (all match ironwail now, pinned by `crates/qserver/tests/compat.rs`):

- **Edict reuse order.** ironwail (since 27f3b9d5) and vkQuake reuse the
  *oldest* freed edict (a FIFO list, head only); ours effectively picked an
  arbitrary free slot. This changes which edict number every mid-game spawn
  gets, so entity processing order, saves and demos all followed. The rule
  also compares `time - freetime > 0.5` in double, like C.
- **New entities move a frame late.** `SV_Physics` fixes its loop bound at
  the start of the frame in both references. Ours ran entities spawned during
  the frame (rockets fired in PlayerPostThink, sparks) in that same frame,
  one 1/72 s step early.
- **`setmodel` bounds.** Alias models get their real bounds over every pose,
  and sprites ±w/2,±h/2, not vanilla's ±16. Mods that don't `setsize`
  (MG1/MG3 corpses, many mods' decorations) collided and were PVS-culled
  differently.
- **Engine fields for every progs** (`PR_MergeEngineFieldDefs`): `alpha`,
  `scale`, `colormod`, `emiteffectnum`, `traileffectnum`, `tag_entity`,
  `tag_index`, `modelflags`. qbj3's `misc_model` with `scale 5` was drawn at
  scale 1, and saves lacked these fields.
- **C float/double rounding** in AngleVectors (QC `makevectors`, player
  movement, QW pmove), `vectoangles`, `vectoyaw` and `walkmove`. f32 math
  flipped an `(int)` truncation in AD's `misc_model`.
- `SV_Impact` sets the `time` global; settle frames leave `frametime`
  alone; `PR_SetEngineString("")` is non-null (`OP_IF` is true for it);
  entity keys keep 1023 bytes (`COM_Parse`).
- **Dedicated servers** run `autoexec.cfg` and the command line only, never
  `quake.rc` (AD's sets `temp1 1024`).

Saves:

- **KEX (2021 re-release) saves load** (version 6: a game-dir line after the
  version). `load` also finds them in the re-release's save folder (the
  Proton prefix of Steam app 2310, or Windows `Saved Games\Nightdive
  Studios\Quake`), plus saves vkQuake/QuakeSpasm left in `<basedir>/<game>`
  and ironwail's `~/.ironwail/<game>`. Tested with your honey `s0`, id1
  `s1` and mg3 `s2` KEX saves: they load exactly as ironwail loads them.
- Strings are written raw, as C does (newlines were escaped).
- The dead-player check uses active clients' edicts, as ironwail does, so a
  dedicated server or a client still in signon can save.

Console commands mods use (QC `stuffcmd` or their configs):

- `v_cshift` (AD, Alkaline, alkjam, jjj2, rubicon2, the xmasjams, Mjolnir):
  screen tints were ignored. Sky contents also no longer get the water tint.
- `sky <name>` (AD, Alkaline, sewerjam2, xmasjams): runtime skybox changes
  were ignored.
- `hudstyle` (ironwail's name; qbj3's HUD-cycle impulse), `play`, `playvol`,
  `stopsound`, `chase_active` + `chase_back/up/right` (Mjolnir's rally car
  third person), `pause` as in the references (`pausable`, broadcast,
  `svc_setpause`, demo pause), `host_framerate`, `host_timescale`,
  `sys_ticrate`.

Tools and hygiene:

- `qsmoke` passes `+sv_autosave 0` (sweeps had been rotating your real
  autosaves), uses the mod menu's mission-pack flags and new rules.
- Basedir discovery: a flat-pak directory counts as the game only when its
  `pak0.pak` holds `gfx/palette.lmp`. The Alkaline dev kit unpacked in the
  repo root had become the basedir for every run without `-basedir`. The kits
  now live in `refs/devkits/`.
- Quaddicted: `quaddicted_list` / `quaddicted_install <filter | =stem ...>`.
  The installer now puts a base pack (Quoth) into `quoth/`, not a dir of its
  own, and gives mods that depend on one a `modinfo.txt` with `base`.
- `qbot -game all|a,b`: every installed pack, with summary.

## Open (not fixed)

Ordered by impact.

1. **Sky wind** (`skywind`, `gfx/env/<sky>_wind.cfg`, in both references;
   dmd, qbj2, qbj3 ship wind configs). A renderer feature.
2. **AD's vkQuake-only paths** (FTE particles, misc_gtkmodel) are by design.
   AD calls `getsurfacetexture`/`getsurfacenearpoint`, `strncasecmp`,
   `strstrofs`, `fopen`/`fputs` and `te_explosion2` only behind extension
   checks we and ironwail answer "no".
3. **Remaining reference commands** that mods or players use: `fly`,
   `maps`/`path`/`mapname`/`find`, `v_idlescale` and the `v_i*` family,
   `cl_nolerp`, `gl_polyblend`, `gl_cshiftpercent` (qbj3 reads it),
   `scr_conalpha`, `r_fullbright`, `edicts`/`edict`/`entities`, `pointfile`.
4. **Saves are found but not listed.** The load menu doesn't list KEX,
   vkQuake or ironwail saves yet (only `load <name>` finds them).
5. **One-frame start offset.** The references run a server frame in the same
   host frame as `map`; ours runs it a frame later. This only matters for
   `wait`-counting scripts.
6. **Oxyblack Fortress** is an AD map without progs. `modinfo.txt` `base`
   only knows hipnotic/rogue/quoth, so the mod menu can't launch it on AD.
7. MG3's `progs/ogre_rocket.md5mesh` is rejected by our md5 parser ("bad
   parent 54"), so the MDL is used.
8. **Headless timedemo** runs at about 90 fps, not uncapped (long demos take
   minutes in sweeps).

## Map packs added

Every Quaddicted release with "Jam" in its title that wasn't installed yet,
via the new `quaddicted_install` (headless, on lavapipe while the GPU needed a
reset): 61 releases plus Quoth 2.2 as the dependency of Blue Monday Jam and
Func Map Jam 7 (and Func Map Jam 9). The rest:

- Doom Tintin Jam failed first: its zip writes Zip64 records, which our
  zip/pk3 reader didn't parse. Fixed, then installed.
- Map-Center Retrojam 1 returns 404 on Quaddicted; installed by hand from
  elsewhere.
- Coppertone Summer Jam 2 v1.0 skipped in favour of v1.2. SMEJ Jam was
  already here.

Installer bugs found this way and fixed: Quoth went into `quoth2pt2full_2/`
(with a circular `base quoth`) instead of `quoth/`; Blue Monday's own mod dir
got no `modinfo.txt`, so the mod menu would have launched it without
`-quoth`. Both installs were repaired in place.

## Reference engine issues found

- ironwail `-dedicated 1`: `Host_CheckAutosave` dereferences a NULL
  `sv_player`, and `Con_LinkPrintf` crashes when a dedicated server saves.
- ironwail aborts at startup for *any* game when *any* game dir has an
  unreadable pak (your `rrp/pak0.pak` is truncated at exactly 128 MiB; its
  directory points past the end). Ours skips it.
- On Linux both references miss upper/mixed-case file names
  (func_mapjam3, qbj_Mr_M).

## Demos

The cross-engine demo sweep is inconclusive. Ours played every shipped demo
it reached: id1, hipnotic, func_mapjam5 (protocol 666) and bbelief, at the
expected frame counts. The reference half failed: through a pipe the
references block-buffer stdout, and three parallel headless gamescope
sessions hung vkQuake. Killing the hung run left the NVIDIA driver needing a
GPU reset (Xid 51, 16:31). The harness now runs references one at a time
under `stdbuf -oL` and stops rather than killing a hung reference. Visual
stills (new views for hipnotic, rogue, MG3, dopa, qbj3, Alkaline, sewerjam2
and Q64 are in `perfkit stills`) wait for the reset as well.

## qbot

(filled in below)
