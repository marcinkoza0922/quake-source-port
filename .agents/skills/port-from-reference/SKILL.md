---
name: port-from-reference
description: Find and faithfully port behaviour from the reference engines (ironwail/, vkQuake/, refs/quake-id, refs/mvdsv) into this Rust Quake port. Use whenever a task says "like ironwail/vkQuake/QuakeSpasm does", "match the reference", "port X", or adds or fixes engine behaviour that the C engines already have — QuakeC builtins and extensions, physics quirks, protocol messages, cvars and commands, rendering parity, menus/HUD, demos, sound, QuakeWorld — and whenever you need to know how vanilla Quake handled something before deciding how ours should.
---

# Porting from the reference engines

Porting logic from these trees is allowed: they are GPL-2.0-or-later like
this project. They are gitignored, so never commit them or copy them
wholesale. Port the logic into idiomatic Rust in the crate that owns that
concern.

| Tree | Use it for |
|---|---|
| `ironwail/Quake/` | Default reference for gameplay, client/server, menus/HUD behaviour, rendering parity and perf techniques (GPU culling `cull_mark`, `lightcache_t`, postprocessed palette). |
| `vkQuake/Quake/` + `vkQuake/Shaders/` | FTE scripted particles (`r_part_fte.c`, ported as `crates/qpart`), QSS/FTE extensions (`pr_ext.c`), SSAO (`r_ssao.c`), `gl_zfix` depth-bias values, `in_sdl3.c` / `snd_sdl3.c`. |
| `refs/quake-id/WinQuake`, `refs/quake-id/QW` | Vanilla id behaviour, plus the original QuakeWorld client (`QW/client`) and `pmove.c`. |
| `refs/mvdsv/` | QuakeWorld server behaviour and protocol details. |

ironwail and vkQuake are both QuakeSpasm forks and often diverge. When they
disagree, read both. Prefer the behaviour that maps depend on, and say in
DESIGN.md which one you followed.

## Where things live

| C source | Here |
|---|---|
| `common.c` (COM_*, search paths, `COM_WordWrap`) | `qcore` (fs, reader); word wrap in `src/console.rs` |
| `cvar.c`, `cmd.c` (`q_atof` lives in `qcore::cvar`) | `qcore::cvar`, `qcore::cmd`; host commands in `src/host/commands.rs`, server commands in `crates/qserver/src/commands.rs` |
| `bspfile.h`, `gl_model.c` (loading), `wad.c`, `image.c` | `qformats` (parsing only, no GPU) |
| `pr_exec.c`, `pr_edict.c`, `progs.h` | `qvm` (`exec.rs`, `edict.rs`, `progs.rs`, `strings.rs`) |
| `pr_cmds.c`, `pr_ext.c` | `qvm/src/builtins*.rs` (table by number and name) + `qserver/src/host/builtins.rs` |
| `world.c`, `sv_move.c`, `sv_phys.c`, `sv_user.c` | `qworld` (`world.rs`, `hull.rs`, `phys/`, `monster.rs`, `user.rs`) |
| `sv_main.c`, `host_cmd.c` (server side), save games | `qserver` (`server/`, `send/`, `save.rs`, `commands.rs`) |
| `net_*.c`, `protocol.h` | `qnet`; UDP server/client glue in `src/net.rs`, `src/host/net.rs` |
| `cl_parse.c`, `cl_main.c`, `cl_tent.c`, `cl_input.c` | `qclient` (`parse/`, `relink.rs`, `effects.rs`, `input.rs`) |
| `cl_demo.c` | `src/demo.rs` |
| `view.c`, `chase.c` | `src/view.rs` |
| `sbar.c` | `src/sbar.rs` + `src/sbar/` |
| `menu.c` | `src/menu.rs` + `src/menu/` |
| `console.c`, `keys.c` line editing, `screen.c` centerprint | `src/console.rs` |
| `gl_*.c`, `r_*.c`, GLSL in `gl_shaders.c` / `vkQuake/Shaders` | `qrender` (`world/`, `frame.rs`, `pipelines/`, `shaders/`) |
| `r_part.c` (classic particles) | `qclient` effects + `qecs::ParticlePool` |
| `r_part_fte.c` | `qpart` |
| `snd_*.c`, `bgmusic.c` | `qsound` |
| `in_sdl*.c`, `keys.c` | `qinput` + `qplatform` (SDL3 via raw `sdl3-sys`) |
| QW `cl_*.c`, `pmove.c` | `qclient::qw`, `qworld::pmove`, `src/qw/` |

## Finding the code

```sh
grep -rn 'Host_SetPos_f\|"setpos"' ironwail/Quake vkQuake/Quake
grep -rn 'Cvar_RegisterVariable (&r_lerpmodels' ironwail/Quake   # cvar definition
grep -rn 'Cmd_AddCommand.*"fog"' ironwail/Quake vkQuake/Quake      # command → handler
```

Read the whole function and the helpers it calls before porting. Quirks often
hide in callers, e.g. a `key_dest == key_game` check or a `cls.demoplayback`
branch.

## Porting rules

- **Keep the quirks maps depend on.** Opcode semantics, runaway-loop limits,
  physics edge cases (step-up, `FL_ONGROUND` handling, pusher blocking),
  string-table behaviour and protocol limits all count. Changing one breaks
  real mods, and qbot and qsmoke will catch it late.
- **Name the origin in a doc comment**, as the codebase already does:
  `/// Port of PR_LoadProgs`, `//! V_CalcRefdef (view.c): ...`,
  `(ironwail lightcache_t/InterpolateLightmap)`. Later readers can then diff
  against the source.
- **C semantics that differ in Rust:**
  - `(int)f` truncates and is UB on overflow, while Rust `as` saturates and
    maps NaN to 0. Match C where QC values can be out of range.
  - C signed overflow and wrapping: use `wrapping_*` where the C code relies
    on 32-bit wrap (hashes, CRC, md4).
  - `atof`/`atoi` accept prefixes like `"12abc"`. Use `q_atof` and the
    `qpart::num` C-style helpers, not `str::parse`.
  - Quake strings are bytes in the Quake charset, not UTF-8. Decode lossily
    for display, compare bytes, and keep high-bit characters.
  - `rand()`: the VM has its own 15-bit generator (`Vm::rand15`, used by
    `pf_random` with `sv_gameplayfix_random` behaviour). Reuse it so runs stay
    deterministic; qbot depends on that.
  - Fixed `MAX_*` arrays: keep the protocol-visible limits and relax
    internal ones only where protocol 999 / BSP2 maps need more.
- **Parsers never panic**: `qformats` returns `Err { format, msg }` with the
  byte offset (`crates/qformats/tests/robust.rs` checks this).
- **Server stays headless**: nothing in `qserver`, `qworld` or `qvm` may
  depend on SDL, Vulkan or the host binary (qbot and dedicated servers need
  this).
- **Don't port the platform layer**: use SDL3 through `qplatform` and Vulkan
  through `qrender`. Third-party crates only for solved problems (PROPOSAL).

## Verifying parity

- Behaviour: write a test against the real progs (write-tests skill, headless
  `Server` harness), or check qsmoke and qbot numbers for server-side changes.
- Visuals: compare screenshots with ironwail or vkQuake (visual-check skill).
- Record what you ported and anything you deliberately left out ("Not
  ported: ...") in the relevant `docs/DESIGN.md` section. That is where the
  existing parity notes live.
