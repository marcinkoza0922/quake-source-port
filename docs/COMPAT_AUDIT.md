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

## Fixed after the audit (2026-11-17)

The audit's open list, in its order:

1. **Sky wind.** `gfx/env/<sky>wind.cfg`, `r_skywind` and the `skywind`,
   `skywind_lookdir`, `skywind_rotate`, `skywind_load`, `skywind_save`
   commands, with ironwail's two-layer drift in both renderers
   (`src/skywind.rs`). dmd, qbj2 and qbj3 skies now drift.
2. **AD's vkQuake-only paths.** Only the parity runs (`sv_fteparticles 0`)
   look like ironwail. In normal single-player play we answer AD's particle
   checks like vkQuake, and ad_sepulcher matches it: both report "Advanced
   Engine", both keep 312 of the map's 1703 `misc_gtkmodel`s (AD removes the
   1391 with spawnflags 32 in both), 4439 edicts here against 4434 there
   after the same 300 + 250 server frames (two `DelayedUse` timers differ),
   and the FTE still (rain, embers, decorations) matches vkQuake within the
   ironwail-vkQuake band. `DP_QC_GETSURFACE` (+ `DP_QC_GETSURFACETRIANGLE` /
   `DP_QC_GETSURFACEPOINTATTRIBUTE`), `FTE_STRINGS`, `FRIK_FILE` (sandboxed
   under `<basedir>/<game>/data/`), and `DP_QC_SPRINTF` are now implemented
   and advertised (`qvm::EXTENSIONS` + `qserver` host extensions, gated on
   `pr_checkextension`). Every AD
   map, dev-kit test maps included (104, `tools/vkparity.py`, 720 server
   frames at skill 1), spawns the same content in both engines: "Advanced
   Engine" everywhere, no errors, no class present in only one engine. The
   only count differences are randomly timed temporary entities (bubbles,
   rubble, gibs, sparks); spark counts on ad_crucial/ad_tfuma swing either
   way between 600, 700 and 800 frames. Not covered: what those extensions
   do later in play (e.g. surface queries at run time). Scripting
   vkQuake with `map` + `wait`s doesn't work: its server stuffs `cmd pext`
   during signon and waits for the reply, which lands behind the `wait`s.
3. **Reference commands**: `fly`, `maps`, `path`, `mapname`, `find`/`apropos`,
   `v_idlescale` and the `v_i*` cycles/levels (plus the always-on
   intermission sway, also missing before), `cl_nolerp`, `gl_polyblend`,
   `gl_cshiftpercent` (and QuakeSpasm's per-shift `_contents/_damage/_bonus/
   _powerup`, with intermissions showing only the contents shift),
   `scr_conalpha` (default 1: our console stays opaque), `r_fullbright`
   (single player), `edict`/`edicts`/`edictcount`, `entities`, `pointfile`.
   `edict` no longer prints the null field first.
4. **Saves from other engines are listed.** Load shows vkQuake/QuakeSpasm
   (`<basedir>/<game>`), ironwail (`~/.ironwail/<game>`) and KEX saves of
   the running game after ours, labelled by source, with no Delete. A name
   we also have stays ours, as `load` resolves it.
5. **One-frame start offset.** The real cause was in our startup: the second
   command-buffer pass after `stuffcmds` ran past a `wait` the first pass had
   stopped at, so a script's first `wait` cost no server frame. The command
   buffer now runs once per frame, and a frame that spawns a server also runs
   one, as Host_Frame does. `map; save a; wait; save b` now gives 1.2 /
   1.2139 like C, and `spawnparity.py --frames 216` matches ironwail with the
   same script for both engines (the `+1 wait` workaround is gone).
6. **Mods built on other mods.** `modinfo.txt` `base` takes any installed
   game dir (mounted `-game ad -game <mod>`; the engine now stacks several
   `-game` like both references). A map pack without progs whose readme says
   "Mod Needed : Arcane Dimension 1.7" is mounted on AD, so Oxyblack Fortress
   launches from the mod menu. The installer maps dependencies such as
   `ad_v1_80p1final` to the installed dir.
7. **MG3 `ogre_rocket.md5mesh`.** Its root's parent is an armature joint
   listed last that the animation doesn't drive. Mesh parents may now point
   forward, and pose parents follow the `.md5anim` hierarchy as in vkQuake.
   All 64 MD5 models in id1/hipnotic/rogue/mg1/mg3/dopa load.
8. **Headless timedemo speed.** Not reproducible on a working GPU: headless
   timedemo is uncapped (demo1: 2245 fps at 320x240, 485 at 1280x720,
   232 at 1080p). The ~90 fps came from the sweep's runs while the GPU
   needed a reset (most of them failed to create a Vulkan device).

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

Our half, rerun after the GPU reset (`demosweep.py --engines ours`, 53
demos): every demo plays at its expected frame count, at 480-750 fps
headless. dm4jam and smej2 report 0 frames because their `quake.rc` ends
in `startdemos ...` followed by `demos`, and Host_Demos_f cuts the timedemo
there in ironwail too. Demos also stuff cvars that neither reference has
(`sv_gameplayfix_*` in Quoth's, `r_maxedges`/`r_maxsurfs`, `gl_flashblend`);
`gl_zfix`, which both references register, is now accepted quietly.

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

Never filled in for 2026-11-17; see the second audit below.

# Second audit (2026-11-23)

HEAD `2515e22`, about 80 commits after the first audit (CSQC, HDR lighting,
BSPX, IQM, PEXT2_PREDINFO, bloom/dlight shadows/RT, the perf rewrites).
1555 playable maps in 113 game dirs. References: ironwail `qsp-diag`, vkQuake
source. KEX and vkQuake were not run. The ironwail half of the demo sweep was
not run either: `demosweep.py` SIGKILLs the references, which once cost a GPU
reset.

| Check | Result |
|---|---|
| `qsmoke`, 1555 maps, skill 3 | 4 maps with errors: 2 renderer aborts and `swjam_nolcoz` (both ours, fixed); `hwjam2_thulsa` (map bug, fails in ironwail and vkQuake too; below) |
| `spawnparity`, skill 1 | 1527 identical, 24 ironwail file-name-case failures, 1 fails in both, 3 explained below |
| `spawnparity --frames 216`, skill 3 | 1525 identical (HEAD, before the fixes); 25 ironwail file-name-case failures; `swjam_nolcoz`, the 2 `BRUSHLIST` maps (all 3 fixed, now identical); `hwjam2_thulsa`; `modjam_adaya/test` (`transwater`: dedicated ironwail has no `r_wateralpha` cvar for its QC to probe) |
| `savecompat`, 27 saves | ours and ironwail read every save both can load identically |
| Demos, ours only, 53 | 47 play at full length; dm4jam and smej2 stop at 0 frames (known: `demos` in their quake.rc) |
| `qbot`, 6 games with baselines | no engine or QC error, every exit reached; baselines stale (below) |
| `cargo nextest run` | 929 passed (with the fixes below) |

## Fixed (2026-11-23)

- **Crash with dynamic-light shadows on.** A face with fewer than 3 edges keeps
  inverted bounds, and `f32::clamp` panics on them (`box_touches_sphere`, in
  both renderers; since 8849bd9). 28 installed maps have such faces, among
  them five of mjolnir's `mj4*` and its hub `start2`.
- **`swjam_nolcoz` couldn't start.** Its QC precaches `/progs/smoke1.spr`; C
  joins `dir/` + path, ours refused a leading slash.
- **`//st` and `//sts` stufftext** (ironwail's extended, float and string
  stats, in its demos and from its servers, and in our own demos recorded
  mid-game) reached the console as an unknown command and the stats were
  lost. The recorder now keeps fractions.
- **A failed `load` left the half-restored level running** (ironwail:
  Host_Error). On a dedicated server it could be saved. `retrojam6/a0`.
- `menu_credits` (stuffed by 9 games' progs; a no-op in both references) was
  an unknown command. `cl_minpitch` above `cl_maxpitch` no longer panics.
- **`BRUSHLIST` collision removed.** Added in a976286, it traced against the
  lump's brushes for odd-sized boxes, so monsters settled differently on the
  two installed maps with the lump (qbj3_annihilazor, mc_hj26_dh). Neither
  ironwail nor vkQuake reads it and both maps have complete clip hulls. It was
  dropped from customized games too: those change only what the player picks.
  The parser stays for the collision hull view.
- **Trailing spaces in file names** are dropped when the name as written is
  nowhere, for loose files only (`hwjam2_thulsa` now loads, below).
- `spawnparity.py`: free UDP ports (fixed ports clashed and looked like
  failures) and the `ext_*` globals no longer count as differences.

## hwjam2_thulsa

The map names its model `"progs/pumpkin.mdl "`, with a trailing space;
`progs/pumpkin.mdl` is installed. On Linux ironwail's server stops at the
precache. vkQuake 1.34.1's server carries on (a missing model is fatal only
for the world), but its client then stops with `Host_Error: Model
progs/pumpkin.mdl  not found` (real client run in headless gamescope; control
map hwjam2_burnham ran). The author presumably never saw it because Windows
drops trailing spaces in file names (not tested here). It loads in ours now.
Over all installed data this is the only model, sound or texture reference
with edge whitespace, and no file name has any, so nothing else resolves
differently.

## Fixed after the second audit (2026-11-23)

Its open list, in order, plus what turned up on the way:

1. **`-dedicated` needs no GPU.** The host draws through `src/gfx.rs`, which
   forwards to `qrender` or, on a dedicated server, to nothing.
2. **qbot drift explained**, baselines re-saved (9). They held
   until c3facad and moved at 6405023, the first audit's parity commit. Since
   then only two things move them: PEXT2_PREDINFO (3f73e71; vkQuake also skips
   the spawn clientdata with deltas, so `fixangle` stays set for one frame) and
   `cl_autoswitch` (8b3a164). With both off, HEAD is identical to 3dca905 on
   id1, hipnotic and rogue.
3. `ambient_level` and `ambient_fade` are cvars (srj1's QC sets one).
4. Unknown `//` stufftext is dropped, as in the references; clients without
   deltas get custom stats as `//st` / `//sts`.
5. `inc`, `cycle`, `cycleback`, `reset`, `resetall`, `resetcfg`, `unaliasall`,
   `condump`, `randmap`, `skies`, `mods`, `games`, `music_pause`,
   `music_resume`; `serverinfo` and the four `scr_*` cvars load quietly.
6. DP_TE_STANDARDEFFECTBUILTINS (#418-431). AD calls `te_explosion2` where it
   sees scripted particles, which we advertise; unimplemented, it was a
   Host_Error that vkQuake doesn't have. Not reachable with installed data:
   only `monster_mammoth` calls it and AD 1.7 doesn't ship its model.
7. **A Host_Error was printed every frame, forever,** and left the client
   attached. It now prints once, shuts the server down and disconnects.
8. **Weapon auto-switch is a modifier.** `cl_autoswitch` (default 1) undid
   pickup switches server-side in vanilla games; no value left them to the
   progs. Now `modifier_autoswitch` in customized games only, off by default.
9. **qbot baselines re-saved** after looking at each map that dropped (id1,
   hipnotic, rogue, mg1, mg3). Eight of fifteen finish everything that exists
   and boss2 kills all 312: the totals moved (monsters spawned in play; hip2m3's fifth secret had been
   counted twice). The rest are goals the bot misses depending on order: two
   zombies on e1m7, a button on e3m4 and r1m6, horde6, boss and map8. No
   engine or QC error, every exit reached. mg1 and mg3 also held at c3facad
   and moved at 6405023.

Full `qsmoke` on merged master (5bbd6c0): 1555 maps, 0 errors.

## Last of the second audit (2026-11-23)

- **vkQuake's server builtins.** The missing ones had been counted by number;
  against what the server implements there were 92, `tracebox`, `randomvec`
  and `pointsound` among them. Ported (`qvm` `builtins/dp.rs`, `qserver`
  `host/dp.rs`) with their extensions, the `spawnfunc_` prefix,
  `drawonlytoclient` / `nodrawtoclient` and `vectoangles` with an up vector.
- **Late precaches.** hwjam3_shades sets a model it never precached: Host_Error
  in ironwail, a warning in vkQuake. Now as vkQuake (`svcdp_precache`, to
  clients with deltas); brush models still load with the map only.
- **`v_water`**: a mod's PlayerPostThink putting the player under water (liquid
  entities) warps the view and muffles sound, as in ironwail.
- `music_loop`, `music_jump`, `viewmodel`, `viewframe`, `viewnext`, `viewprev`.
- **qbot over every mod that has flagged an issue**: `-game flagged`,
  `tools/qbot/flagged.txt` (66 game dirs, 1101 maps, about 7 minutes), a
  baseline each. A second run matched the first exactly. Two maps keep QC
  errors that both references raise too (`assignment to world entity`):
  alkjam_plaw and jam5_mfx. 17 maps hit `-maxwall 90` and aren't compared. A
  `-game all` run (1555 maps) found those two and hwjam3_shades.
- **ironwail against our server.** A real ironwail client played qbj3 on our
  GPU-less dedicated server (1004 datagrams) and was sent `//st 33 17`.
- **Demos in ironwail.** 11, one per game up to 3600 frames: played to the end
  without errors. Its frame count is one lower than ours in each.
- Full `qsmoke`: 1555 maps, 0 errors, on acdfe2c and again with all of this.

## Open (2026-11-23)

1. Not ported from vkQuake: `getlight` (the server has no lightmaps), bot
   clients (`spawnclient` finds no slot), 64-bit writes, `pointerstat`,
   `misc_model` without a spawn function, user info keys beyond name and
   colours. `strftime`'s local time is UTC.
2. Not run: vkQuake demo playback, KEX.
