---
name: mod-compat
description: Check and fix compatibility with Quake mods and map packs in this port — Arcane Dimensions, Machine Games MG1/MG3, hipnotic/rogue/quoth, Quake64, and the ~30 installed community mods and jams. Covers the qsmoke every-map sweep, reproducing a single map headless, triaging Host_Error/QC errors/missing assets/unknown commands, QuakeC extension builtins, mission-pack flags, and comparing behaviour with ironwail/vkQuake. Use whenever a mod or map crashes, errors, looks or plays wrong, a QC "runaway loop"/"server error" shows up, the user mentions a specific mod by name, or after changes to the VM, server, filesystem, BSP/model loading or protocol that could break some mod.
---

# Mod compatibility

The goal (PROPOSAL): run everything ironwail and vkQuake run, with special
attention to Arcane Dimensions and the official MG1/MG3 episodes. As of
2026-11-11, qsmoke loaded all 611 playable maps across 32 gamedirs without
errors. Keep it that way.

## Sweep everything: qsmoke

```sh
cargo build --release                   # qsmoke drives target/release/quake-source-port
target/release/qsmoke                   # every playable map of every gamedir (~4 min)
target/release/qsmoke ad mg1:start      # filter: mod or mod:map
target/release/qsmoke -j 4 -time 8 -timeout 120 -out target/qsmoke
```

Each map runs in its own headless engine at skill 3 (`-hipnotic`/`-rogue` are
added for those dirs). Output: `target/qsmoke/report.json` (per map) and
`summary.txt` (issues grouped by message). Exit code 1 if any map errors.
Lines are classified by substring (`RULES` in `tools/qsmoke/src/main.rs`):

- **Errors**: `panicked at`, `host_error`, QC `server error`/`object error`,
  `couldn't spawn server`, `cannot find map`, `render error`, `bad server message`.
- **Warnings**: `unknown command`, `couldn't load model`, `not found`, and
  `couldn't` / `bad ` / `warning`. "couldn't exec autoexec.cfg" is normal.

qsmoke does not pass `+sv_autosave 0`, so a sweep can push the user's real
per-mod autosaves out of rotation. Check whether that's been fixed; if not,
fix it, or warn the user before running a full sweep.

## Reproduce one map

```sh
target/release/quake-source-port -headless -autoquit 10 +sv_autosave 0 -game ad +skill 3 \
  +map ad_tears "+delay 8 screenshot target/shots/ad_tears" 2>&1 | tee target/shots/ad_tears.log
```

- Mission-pack bases: `-hipnotic`, `-rogue`, `-quoth` (combinable with
  `-game`). A mod built on a pack needs the flag. Check its readme or
  `modinfo.txt` (`base`), and see how `qui::mods` picks base flags.
- Console output goes to stdout. A QC error prints the message plus a QC
  stack trace, innermost first (`Vm::stack_trace`, PR_StackTrace).
- Use the game-data skill to find which archive supplies `progs.dat`,
  `maps/x.bsp` or a `.lit`, and to dump worldspawn keys and entities.

## Triage by symptom

| Symptom | Look at |
|---|---|
| QC "runaway loop error", bad opcode, string errors | `crates/qvm` (`exec.rs`, `strings.rs`, dynamic strings for QSS extensions) against `pr_exec.c` / `pr_edict.c` |
| Missing or misbehaving builtin, `checkextension` | `crates/qvm/src/builtins*.rs` (by number, extensions by name) + `crates/qserver/src/host/builtins.rs`; reference `vkQuake/Quake/pr_ext.c`. The extensions we advertise change what mods like AD do. |
| Map won't load, bad lump, odd BSP | `qformats::Bsp` (BSP29/BSP2/2PSB/Quake64, tolerant miptex). Parsers must return `Err` with the offset and never panic. |
| Wrong lighting on a mod's map | `.lit` / `.ent` via `Fs::open_companion`: only from the same or a higher-priority gamedir (ironwail `path_id` check) |
| Physics or trigger behaviour | `qworld` against `sv_phys.c` / `sv_move.c` / `world.c` quirks; confirm with qbot on that game |
| Wrong HUD for a pack | `sbar::Pack` (Id/Hipnotic/Rogue), `src/save_inventory.rs` |
| Fog, sky, alpha, particles | worldspawn keys, `fog` command / `svc_fog`, `r_part_*` / `qpart` (FTE particles), `_texpart_*` keys |
| Unknown commands from a mod's cfg | add a no-op to `COMPAT_CVARS` (add-setting skill) or implement the real command |
| Text / localisation (MG/KEX) | `qserver::loc`, `scr_usekfont` (centerprint word wrap only, like ironwail) |

Known fixes worth knowing (DESIGN.md, M5): Q64 `miptex64_t`, 16-bit RGB
lightmaps and 32×64 skies; MG1 needed the `fog` command, and its grates
glowed until only mip 0 was used and the rest rebuilt with
`convert::downsample_rgba`; the `.lit` priority fix for Q64; AD's
engine-detection paths differ between engines by design.

## Inventory and save-card issues across mods

`python3 tools/audit_save_inventory.py "$QUAKE_BASEDIR" [mod ...] > target/inv.json`
runs a read-only audit of each mod's progs, inventory constants and fields,
HUD art, and QC source hints. See `docs/SAVE_INVENTORY_AUDIT.md`.

## Confirming the fix

- Compare with a reference engine when unsure what "correct" is
  (visual-check skill: sandboxed ironwail/vkQuake runs; port-from-reference
  skill for the C code).
- Rerun qsmoke for the affected mods, or for everything if the change was in
  shared code (VM, fs, BSP, protocol).
- Server-side changes: run qbot on id1, hipnotic and rogue (and AD where it
  matters) and check the baselines (qbot skill).
- If you fixed a quirk, add a game-data test that skips when that mod is
  absent (write-tests skill), and note it in DESIGN.md's compatibility notes.
