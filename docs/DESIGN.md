# DESIGN

Living design doc. `PROPOSAL.md` says *what*; this says *how*. Update when decisions change.

## Decisions (interview, 2026-11-10)

| Topic | Decision |
|---|---|
| License | GPL-2.0-or-later. Porting algorithms/tables from `ironwail/` and `vkQuake/` is allowed. |
| Networking | Keep the client/server split. Loopback for SP; NetQuake protocols 15 / 666 (FitzQuake) / 999 (RMQ) for LAN/internet, after SP is solid. |
| System deps | `ash` (Vulkan, dynamic loader), SDL3 via raw `sdl3-sys` (window, input, gamepad DB + rumble, audio device). |
| Platforms | Linux first; keep code portable, Windows later. |
| Aim assist | On in SP/co-op; in DM controlled by a server-side cvar (`sv_aimassist`, default 0 in DM). |
| Mod menu | Browse installed mods + metadata, per-mod map browser (with completion/secret stats), per-mod config profiles. No online downloads. |
| Test bot | Legal movement with god mode; noclip fallback when stuck, flagged in report. |
| Game data | `QUAKE_BASEDIR` env var (default: the `vkQuake directory with maps` symlink). Data-dependent tests skip when absent. |

## Compatibility targets

Everything ironwail/vkQuake load: id1, hipnotic/rogue (`-hipnotic`/`-rogue` builtins quirks), MG1/MG3 (Machine Games re-release `progs`, `mg1`/`mg3` dirs, KEX extensions where the references handle them), Arcane Dimensions (fog, skyfog, `alpha`, BSP2, `colormod`-free, large entity counts, particle-heavy, func_illusionary etc.). Formats:

- Archives: PAK, PK3/zip (store + deflate).
- BSP29, BSP2, 2PSB, lit/colored (`.lit`), external `.ent` overrides, `.vis` not required.
- MDL (incl. `QuakeForge` flags, MD5 replacement models from re-release optional later), SPR/SPR32, WAD2, LMP, TGA/PCX external textures, `.skybox` (`sky` worldspawn key).
- QuakeC progs.dat v6, FTE/QSS extensions used by AD/MG (checkextension, string builtins, `sv_protocol 999` limits).
- Audio: WAV, OGG/FLAC/MP3 music in `music/` (decoders: `lewton` ok as a "solved" crate; mp3 via `minimp3`-class crate).

## Architecture

Cargo workspace. Binary crate at root (`src/main.rs`) wires crates together.

```
crates/
  qcore      math (Vec3, plane, bbox), byte reader, hashing, cvar + cmd + console text, filesystem (pak/pk3 search paths, gamedir stack)
  qformats   BSP29/BSP2/2PSB, MDL, SPR, WAD2, LMP, palette, PCX/TGA, WAV parsing → plain Rust structs, no GPU
  qecs       custom ECS (see below)
  qvm        QuakeC VM: progs loader, interpreter, builtin dispatch trait, edict field layout mapping onto ECS
  qworld     collision (hull traces, BSP2 large hulls), SV_Move, physics (SV_Physics_*), area grid
  qnet       message buffers, protocols 15/666/999, loopback + UDP datagram transport
  qserver    server frame, builtins impl, entity delta encoding
  qclient    client state, parse, prediction-free NetQuake model, interpolation, effects/particles (CPU sim)
  qrender    Vulkan renderer (ash)
  qsound     mixer + spatialization, music streaming
  qplatform  SDL3 wrapper: window, events, gamepad, audio callback, timers
  qinput     binds, mouse, gamepad curves, aim assist
  qui        immediate-mode UI toolkit custom-built for console/menus/HUD; mod menu
tools/
  qbot       automated level traversal test harness (headless server + bot client)
  qinspect   CLI: dump bsp/mdl/progs/pak contents, entity lists, secret/monster counts
  qbench     timedemo runner comparing against ironwail/vkQuake numbers
```

Dependency direction: qcore ← qformats ← {qworld, qvm} ← qserver; qnet ← {qserver, qclient}; qrender/qsound/qinput/qui depend on qcore/qformats and client state, never on qserver. The server must run headless (no SDL/Vulkan) — required by `qbot` and dedicated servers.

### ECS (`qecs`)

Fit-to-purpose, not general. Quake's entity model is a fixed field layout defined by progs, so:

- **Server**: edicts are stored as a dense SoA-ish block of `f32/i32` slots exactly as progs expects (QC accesses fields by offset; this must stay O(1) and branch-free). The ECS adds *engine-side* components (link state in area grid, cached hull, interpolation, bot nav data) in sparse-set storages keyed by edict index. Generational `Entity` handles for engine code; raw indices for QC.
- **Client**: entities, temp entities, beams, dlights, particles live in archetype-free typed pools (`Pool<T>` with free list + dense iteration). Systems are plain functions run in a fixed schedule; no dynamic scheduler, no reflection.
- Particles are a separate SoA pool (pos/vel/color/die/type) for SIMD-friendly update.

### Renderer (`qrender`)

- Vulkan 1.2 baseline + dynamic rendering when available; ash, hand-written allocator (big-block suballocation; few resource types).
- World: BSP surfaces batched into one big vertex buffer at load; per-frame visible face set from PVS + frustum culled on CPU (multithreaded mark-leaves), drawn via indirect indexed draws grouped by texture+lightmap atlas. GPU lightmap update for lightstyles (compute shader, like ironwail's approach) to avoid CPU uploads.
- Alias models: GPU vertex lerp from packed frame buffers; instanced by model.
- Water warp, sky (scrolling + skybox), fog (AD `fog` / worldspawn), alpha surfaces sorted, r_wateralpha / per-entity alpha, fullbrights, colored lighting, palette-accurate shading option (`r_softemu`-style).
- Shaders: GLSL compiled with `glslc` in `build.rs` to SPIR-V, embedded.
- Target: ≥ ironwail fps on AD large maps (ad_sepulcher, ad_tears) measured by `qbench`.

### Server / QuakeC

Interpreter ported faithfully from the references (opcode semantics, runaway loop limit, string table handling incl. dynamic strings for QSS extensions). Builtins table indexed by number, with extension builtins looked up by name like QSS. Physics ported from sv_phys/sv_move/world.c behaviors including known quirks maps depend on.

### Input and aim assist

Gamepad via SDL3 gamepad API + SDL DB. Look: radial deadzone, response curve (exponent), acceleration ramp on sustained full deflection. Aim assist (gamepad only, never mouse):
- **Slowdown (friction)**: reduce look sensitivity when crosshair within an angular cone of a valid target's bbox.
- **Rotational/magnetism**: when player is moving and target is in cone, add a portion of the target's relative angular velocity to view.
- **Snap**: none (not industry standard for this genre); optional `aim_snap` cvar off by default.
Target selection: live monsters (`FL_MONSTER`, health > 0) and DM opponents if allowed, visible via trace, within range; weighted by angle and distance. Client-side, using interpolated entity positions; server cvar gating via serverinfo/stufftext.

### UI / mod menu

Custom immediate-mode UI (`qui`) drawing through the renderer's 2D batch, conchars + scalable bitmap font. Mod menu: scans basedir for gamedirs (has pak/pk3/progs/maps), reads metadata (`modinfo.txt`, readme, `maps/*.bsp` names, worldspawn `message`), thumbnail from screenshot cache, launches with correct base game flags. Per-mod profile = `config.cfg` in `<basedir>/<mod>/`, with per-mod stats file (`qsp_stats.json`) storing kills/secrets/completion per map.

### Test bot (`tools/qbot`)

In-process headless `qserver::Server` (no engine binary), `skill 3`, `god`, one local client driven by `set_move`. Per map:
1. Scan edicts: goals are `trigger_secret`, `func_button`, `item_*key*` and FL_MONSTER edicts. `trigger_changelevel` gets its `touch` cleared so the run can't end early. Exits and `trigger_teleport` boxes are kept out of the nav graph.
2. Nav graph (`nav.rs`): flood fill over a 32-unit XY grid from the spawn point using world-only hull 1 traces, so doors count as open and lift shafts as drops. Edges are a walk (step 18), a jump (rise ≤ 40, pressed as +jump) or a drop (≤ 640). Nodes are never in lava, slime, sky or solid. When the bot ends up off the graph (teleport, lift, noclip), the graph is flood-filled again from where it stands (`Nav::extend`); standing on a brush entity, it paths from the world floor node below.
3. Goal loop: Dijkstra from the nearest node and take the cheapest reachable goal; replan every 1 s for monsters, 3 s otherwise. Shoot the nearest visible monster or shootable button (world hull 0 line of sight) while walking. Weapons and ammo come from impulse 9: the super nailgun, or rockets for zombies (they only die when gibbed). In water it swims up towards the next node.
4. Stuck (no 16-unit progress for 2 s): jump and strafe; after 4 s noclip to the goal node, then `setpos` exactly onto it. A monster still out of sight at the end of its path triggers a noclip to 72 units from it. A goal is given up after 3 noclips or 30 s. Unreachable goals (no node) are noclipped to directly. If the bot can't get back onto the graph after 8 tries, the run stops and is marked `stranded`.
5. Report `<out>/report.json`, one entry per map: kills/total, secrets/total, noclip uses, failed goals, nav nodes, exited, stranded, sim/wall time, errors (QC errors, spawn failures, panics). Sim time is capped at 900 s or 15 s per goal, whichever is larger (`-time` overrides). Runs are deterministic, so `tools/qbot/baseline-<game>.json` (kills and secrets per map) is compared exactly: fewer kills or secrets, or any error, exits 1. Changes to the bot itself shift the numbers, so re-save the baseline (`-save-baseline`, merged per map) after changing it.

## Tooling

- `cargo nextest run` — quiet default profile; `--profile verbose` for debugging.
- `cargo clippy --message-format=short`.
- `tools/qinspect`, `tools/qbot`, `tools/qbench` as above.
- `tools/qbot` (M7): `cargo build --release`, then `target/release/qbot [-game G] [-skill N] [-time S] [-j N] [-out DIR] [-baseline F] [-save-baseline] [-v] [map ...]`. With no maps it runs every playable map the game dir owns, in parallel threads (`-j`, default all cores). `-v` logs goals, noclips and the goals left at the end. Output: `target/qbot/report.json`. Baselines exist for id1, hipnotic and rogue. `cargo nextest run -p qbot` runs e1m1 (all secrets, ≥ 80% kills).
- `tools/qbench`: timedemo fps vs ironwail/vkQuake (M7). `cargo build --release`, then `target/release/qbench [-game ad] [-runs 3] [-secs 24] [-width W -height H] [-engines ours,ironwail,vkquake] [-rerecord] [map ...]` (default ad_sepulcher ad_tears). The demo is recorded by our engine headless (god, 30°/s spin at the spawn point), cached in `target/qbench/demos`. Engines run from `target/qbench/base` (symlinks to id1 + the game; the game data dir is never written) with `-basedir . -window +vid_vsync 0 <pinned settings> +timedemo qbench/<map>` (MSAA off, 8x anisotropy, render scale 1 in each engine's cvar names), under `stdbuf -oL` because the C engines block-buffer stdout. qbench reads stdout+stderr for the fps line, then kills the process group (the references don't quit after a timedemo). ironwail comes from `<basedir>/ironwail-*-linux-x64.7z`, extracted into `target/refengines`; vkQuake is `<basedir>/vkquake.AppImage`. Medians go to `target/qbench/report.json`.
- `tools/qsmoke`: the every-map smoke sweep (M5). Build with `cargo build --release`, then `target/release/qsmoke [-j N] [-time S] [mod[:map] ...]`. It runs each playable map (one with an `info_player_*` entity) of each gamedir in its own headless engine process at skill 3 and classifies console output: Host_Error, QC errors, panic/crash, timeout, unknown commands, missing assets. Output goes to `target/qsmoke/report.json` (per map) and `summary.txt` (issues grouped by message). Exit code 1 if any map has an error. A full run over the local data takes about 4 minutes.

## Work plan (milestones)

1. qcore + qformats + qinspect, with tests against real data (skip if absent).
2. qvm + qworld + qserver headless: load start.bsp, run frames, no errors on all installed maps (smoke test across every mod).
3. qnet loopback + qclient.
4. qplatform + qrender (world, models, 2D) + qsound + qinput → playable id1.
5. AD / MG1 / MG3 / hipnotic / rogue compat pass. Status (2026-11-11): `qsmoke` loads all 611 playable maps across 32 installed gamedirs with no errors (Quake64 BSP support and the miptex tolerance fix were needed). Hipnotic/rogue status bars are done. `scr_usekfont` works as in ironwail: it only word-wraps centerprints (`console::word_wrap`, a port of COM_WordWrap, wrapping to `center_wrap_cols`), and it is set to 1 at startup when mg3 is mounted. ironwail doesn't draw the KEX font either, and localization (`qserver::loc`) was already in place. Visual spot checks against vkQuake 1.34.1 (2026-11-11) on e1m1, q64 start, mg1 start and ad_sepulcher: e1m1 matches apart from texture filtering. Q64 needed the `.lit` priority fix (it was picking up id1's start.lit and losing its RGB lighting) and now matches. MG1 needed the `fog` command. MG1's grates glowed because we used the mips embedded in the BSP: the compiler filled the see-through texels of mips 1–3 with index 240, a fullbright orange. Fixed: like ironwail, only mip 0 is used and `convert::downsample_rgba` builds the rest (alpha-weighted for fences). AD comparison still open: vkQuake shows its main menu over AD because AD's quake.rc has no `stuffcmds`, and it can't be closed from the command line. To capture vkQuake: it only runs `map` once its command buffer is empty, so scripted waits never work. Launch `vkquake.AppImage -basedir . -game G -window +map M` in the background, then grab the window with `spectacle -b -n -a -o out.png`.
6. qui console/menus/mod menu; aim assist. Status (2026-11-11): mod menu done. `qui::mods` scans the basedir (a game dir has paks, pk3s, progs or `maps/`), titles come from `modinfo.txt` (`title`, `base`, `description`), then a built-in table, then the dir name. The blurb is the first lines of the readme. `qui::mods::maps` lists a mod's own playable maps (those with an `info_player_*`) with worldspawn titles, reading only the entity lump, lossily, since AD has non-UTF-8 bytes. `qui::stats` keeps `qsp_stats.json` in the mod's *user dir* (never the game dir), with the best kills/secrets/time per map, written at intermission/finale (not in deathmatch or headless). `qui::json` is a minimal JSON parser/writer. UI is in `src/modmenu.rs`: Main menu → Mods (list + info panel); Enter launches the mod (the running one just closes the menu). The level selector (map page: Launch/Start + maps with K/S/time; gold = 100%) is hidden unless `ui_levelselect 1` (Options → Gameplay → Level Select), then Right opens it for any mod including id1. Console: `menu_mods [dir]`. A mod with no user `config.cfg` yet inherits the nearest base game's user config (pack below it, then id1; `Host::inherited_config`), run after any config.cfg the mod ships and saved as its own on quit. `game <dir> [map]` switches mods by saving the config and re-executing the engine with the mod's flags (`-hipnotic`/`-rogue`/`-quoth`, `-game`, `+map`), dropping the old `-game`/pack flags and `+commands` (`relaunch_args`), because renderer, sound and fs are bound to the gamedir at startup. Not done: screenshot thumbnails.
7. qbot; qbench perf parity. Status (2026-11-11): qbench done (see Tooling). The server now keeps reliable output as separate messages (`take_reliable` returns `Vec<Vec<u8>>`, one per flush); it used to merge the signon flushes, which wrote an 81 KB demo block that ironwail and vkQuake reject (> MAX_MSGLEN). First numbers, 1280×720, RTX 3060 Ti, median of 3 runs: ad_sepulcher ours 1832 / ironwail 3273 / vkQuake 270 fps; ad_tears 1750 / 2822 / 476. Not yet at ironwail parity. Profiling (`perf record --call-graph dwarf`, needs `kernel.perf_event_paranoid<=1`; we are CPU-bound, fps barely changes with resolution) led to: a light-point cache keyed by the floored sample point plus bilinear lightmap sampling (ironwail lightcache_t/InterpolateLightmap), and a cached PVS leaf list per view leaf with frustum clip bits (a per-frame BSP node walk was slower on ad_tears: 217k nodes). Then world culling moved to the GPU (port of ironwail's cull_mark, `shaders/cull.comp`): at load, `build_world_cull` uploads every (leaf, surface) pair of the world, one `GpuCullSurf` per surface (facing plane, box as center/extent, draw command, lightmap) and the source indices. Draw commands are one per batch texture (`World::slot_texture`: random animations pick per surface) used by world leaves, ordered opaque, sky, liquids by type, each owning a region of a per-frame output index buffer sized for all its surfaces. Per frame the CPU only finds the PVS leaves of the view leaf (cached) and tests their boxes against the frustum (`World::leaf_bits`), stages the zeroed commands (resolved animated texture as first instance), lightmap dispatch args and leaf bits in dynbuf, and the GPU copies them into a device-local `cmdbuf` (atomics on host memory were slow). cull.comp tests the leaf bit, facing and frustum, dedupes surfaces with a per-surface frame stamp, appends indices and the surface's lightmap to the list the lightmap pass reads, which then runs as an indirect dispatch. Brush entities keep the CPU path (`build_world_draw`). Output is pixel-identical to the CPU path on e1m1, e1m3, e2m2, start, ad_sepulcher, ad_tears and mg1 start. qbench now pins MSAA off, 8x anisotropy and render scale 1 in every engine (our runs had used the saved config's 8x MSAA). Then GPU timestamps (`QSP_GPUTIMES=1` prints average µs per stage on exit: cull, lightmap, targets, world, models, translucent, particles+vm, post, ui+present) showed the GPU busy ~180 µs of a ~310 µs frame on ad_sepulcher at 1280x720 (world 52, cull 30, post 23, lightmap 22), so frames are CPU-bound; 3 frames in flight gave nothing. A cheap hash for the light cache and reused cull scratch followed. Result at 1280x720: ad_sepulcher 3252 vs ironwail 3354, ad_tears ~3050–3150 vs 2816. qbot done (2026-11-11, see Test bot): totals at skill 3 are id1 42 maps 1529/1834 kills, 119/124 secrets; hipnotic 1039/1380, 70/77; rogue 1103/1337, 50/59. AD runs too, but takes minutes per map (ad_sepulcher 349/503, 42/50). No QC errors on any of these. Known weak spots: e1m7, e2m5, hip3m1 and r1m4 end stranded; lifts and locked doors are only handled by noclip.
8. Network play protocols 15/666/999. Status (2026-11-11): done. `src/net.rs`: `NetServer` listens on one UDP socket (`-port`/`port`, default 26000) whenever the running server has `maxplayers` > 1 (`-listen [n]`, default 8; `-dedicated [n]`, default 16: headless, no local client, no rendering, real-time pacing at 72 Hz, every server log line echoed). CCREQ_CONNECT reserves a free slot (`Server::connect_client`, `max_datagram` = DATAGRAM_MTU) and CCREP_ACCEPT names the same port, so clients are told apart by address. CCREQ_SERVER_INFO/PLAYER_INFO/RULE_INFO are answered. Per server tic `receive` parses clc_nop/disconnect/move/stringcmd into the slot (clc_move angles are 8-bit for protocol 15, 16-bit otherwise), and `send` passes the slot's reliable messages to the `qnet::Connection` and sends only the newest datagram (a nop every 5 s otherwise); 300 s silence drops the client, and dropped slots linger until their svc_disconnect is acked. `NetClient` (`connect host[:port]`, `reconnect`) retries the request 3× at 2.5 s, then feeds messages to the same `ClientState` the loopback game uses; the host sends its clc_move unreliably each tic and its reliable bytes (signon replies, forwarded commands) as they appear. Unknown console commands are forwarded to the remote server (Cmd_ForwardToServer). New server commands: `maxplayers` (sets deathmatch like Host_MaxPlayers_f), `say`, `say_team`, `tell`, `status`, `kick`, `ping` (no ping times). Replies go to the asking client's console, or to the local player's on a listen server. `-realtime` makes `-headless` use wall-clock time (needed for two headless engines to talk). Tests: `net::tests::udp_play_protocol_{15,666,999}` run two clients over real UDP sockets against an in-process e1m1 server (signon, movement, chat, disconnect). Checked by hand against ironwail 0.8.1: our client on an ironwail dedicated server with sv_protocol 15, 666 and 999, and the ironwail client on our dedicated server. Not done: a multiplayer menu (join/host), `slist`, ping times, QuakeWorld.

Subagent split: crates with frozen interfaces (qformats, qvm interpreter, qsound mixer, qui toolkit, qbot nav) are independent tasks given to separate agents once qcore's API is set.

## Implemented APIs

### qecs
- `Entity` (index = QC edict number + generation); `EntityAllocator` with `alloc`, `alloc_with(|idx, freed_at| bool)` for ED_Alloc-style reuse policies, `alloc_at(idx)`, `free(e, now)`, `free_time`.
- `SparseSet<T>` with generation-checked access plus `*_raw` index access for QC; `join2`/`join3` and `join_mut`/`join2_mut` all iterate the smallest storage.
- `Pool<T>`/`PoolHandle`: preallocated with `with_max`, swap-remove `retain`.
- `ParticlePool`: SoA, `spawn`, `kill_expired`, `integrate(dt)`, `update_with(time, dt, closure)` for per-kind ramps/gravity.

### qformats
All parsers take `&[u8]` and return `qformats::Result<T>`; bad input gives an `Error { format, msg }` with the byte offset, never a panic. Vectors are `[f32;3]`.
- `Bsp::parse`: BSP29/BSP2/2PSB/Quake64 normalised into one set of structs (i32 children: a node child < 0 is leaf `-(i+1)`, a clipnode child < 0 is a contents value). Also `decompress_vis`, `leaf_pvs`, `parse_lit`.
- Miptex texel data that runs past the end of the lump (seen in qbj, retrojam2) is tolerated: mip 0 is clamped and zero-filled, as in ironwail, and smaller mips that are out of bounds are point-sampled from the previous level.
- Quake64 (`q64`): `miptex64_t` adds `MipTex::shift` (the renderer divides st by `2*shift`), only mip 0 is stored, and the 16-bit RGB lightmap is expanded into `Bsp::rgb_lighting` (used like a `.lit` when no `.lit` file is present) plus mono `lighting`. `lightofs` is halved to index the expanded data. Skies are 32×64 with the front layer on top at alpha 128 (`convert::sky_layers(.., q64)`).
- `parse_entities` keeps key order and duplicate keys.
- `Mdl`, `Spr` (v1 and SPR32), `Wad` (WAD2/WAD3), `parse_pic`/`parse_palette`/`parse_colormap`/`parse_pcx`/`parse_tga`, `Wav` (with loop points as in GetWavinfo).
- Known gaps: maps inside .pk3 are not covered by tests yet (qcore's zip reader will fix that). For stereo WAV, the frame count is length / (width × channels), whereas Quake divides by width only; the mixer must take this into account.

### qcore
- `math`: `Vec3`, `angle_vectors`, `vec_to_angles`, `Plane` (with `box_on_side`), `Bounds`.
- `reader`: `Reader<'a>`, a little-endian cursor that returns `ReadError{pos,need,len}`.
- `fs`: `Fs::new(basedir)` / `Fs::default_basedir()`, `add_game_dir`, `truncate_game_dirs`, `open -> Option<Vec<u8>>`, `exists`, `origin`, `list(prefix, ext)`. Search order within a gamedir: loose files < pak0..N < *.pk3 (sorted, QSS order); later gamedirs override earlier ones. Names in archives are case-insensitive. Loose files and `pakN.pak` fall back to a case-insensitive match when the exact name is missing (beyond the references, by user decision 2026-11-11). `..` is rejected. Each `open` reopens the archive; mmap is a future optimisation.
- `cvar`: `Cvars` registry (`register` keeps a value already set by `set`; the usual flags).
- `cmd`: `com_parse`, `Args::tokenize`, `Cbuf` (`wait`), `Commands<C: CmdContext>` with built-in alias/set/seta/toggle/echo. `exec` and `cvarlist` must be registered by the engine.
- Dependency: `miniz_oxide` (pk3 deflate).

### qworld
- `ents::Edicts`: the trait the server implements. Fields are read and written by enum (`V`/`F`/`E`). The `touch`/`blocked`/`think` callbacks receive `&mut World` so QC can run re-entrantly.
- `hull::CollisionModel::from_bsp`: hulls 0–2 and per-submodel heads. `Hull::trace` matches `SV_RecursiveHullCheck` exactly, including the epsilon, the 0.1 backup loop and doubles on non-axial planes.
- `world::World`: depth-4 area tree with C-identical link order. `trace` is SV_Move; also `link_edict`/`unlink`/`touch_links`/`point_contents`. Traces don't allocate.
- `phys`: per-movetype physics and push movers (`PhysParams` holds the cvars and f64 time). `physics_client_move` sits between PlayerPreThink and PlayerPostThink, which the server calls.
- `monster`: movestep / check_bottom / move_to_goal. `user`: SV_User movement.
- Where C would Host_Error, this crate carries on instead (bad movetype, SOLID_BSP without a model, and similar). Rotating pushers (`SV_PushRotate`) are deliberately absent, matching ironwail and vkQuake: hipnotic rotation is done entirely in QC.

### qvm
- `Progs::parse`: v6 progs. Every operand, jump and range is validated at load, so running code can't panic.
- `Vm::new/load(progs, max_edicts, maxclients)`: `execute(&mut dyn Host, fnum)` is re-entrant, with the reference limits (runaway, stack depth). Errors come back as `VmError { msg, trace }` and the VM stays usable afterwards.
- Edicts: one dense u32 slot array. `ed_alloc` follows the ED_Alloc reuse rules via `qecs::EntityAllocator::alloc_with`. The host must unlink an edict before `ed_free`.
- Strings: a handle ≥ 0 is an offset into the progs string table; a negative handle is an engine slot (1024 rotating temp strings, zone strings, interned constants).
- `trait Host { builtin, ext_builtin, has_extension, dprint, print }`. `Builtin` is an enum of the id1 numbers 1–80. Re-release ex_* names and numbers 90–92 are remapped. #0 builtins are resolved by name at load. Math and string builtins run inside the VM.
- `spawn::load_from_file(host, ents, SpawnFilter)`: ED_LoadFromFile, including the key hacks.
- Approximations: sprintf %g, strconv modes 0–2 only, `random()` uses its own generator rather than C rand(), and tokenize uses com_parse.

### qserver
- `Server::new(fs, cvars, maxclients)` holds `core` (fs, cvars, cbuf, clients, level state) and `level` (VM + World). `spawn_server(map)` follows SV_SpawnServer (including `.ent` overrides). `frame(dt)` runs cbuf, then client input, then SV_Physics, then sends.
- Clients are in-memory slots. `connect_local()` runs the full signon and redoes it after changelevel. `set_move(i, ClientMove)`, `client_command`. Output via `take_reliable(i)` / `take_datagrams(i)`, which are protocol 15/666/999 byte streams. `sv_protocol` defaults to 999 (INT32COORD, like ironwail/vkQuake): 666 coords wrap past ±4096, which breaks large maps (e.g. Mjolnir start2).
- `stats() -> LevelStats { total/found secrets, total/killed monsters }`, used by qbot.
- qworld `CollisionModel::submodels` mins/maxs are spread by ±1 as in Mod_LoadSubmodels; precache dispatches on file magic (alias models named `.bsp` load as alias).
- Localization (`loc.rs`, `Core::loc`): `localization/loc_english*.txt` loaded in `Server::new`. bprint/sprint/centerprint use PF_VarString with LOC_GetString + LOC_Format (`{}`/`{N}` filled from the following args, each translated), WriteString translates `$key` as ironwail. `Localization::translate` is the client-side LOC_GetString for whole print strings.
- Differences from ironwail: alias and sprite models get ±16 bounds, no QSS custom stats, no `sv_netsort`, and the `alpha` key is kept only when progs define an `alpha` field.
- Smoke tests: `smoke_fast` runs by default. `cargo nextest run --release -p qserver --run-ignored only smoke_all` loads every installed map; the summary goes to `target/qserver-smoke-all.txt`.

### qnet
- `msg`: `SizeBuf` (a copy of `qserver::msg::SizeBuf`, so qserver can switch to it later) and `MsgReader` (MSG_Read*: reading past the end sets `badread` and never panics). Both have coord/angle/angle16 for the PRFL_* flags of protocols 15/666/999. `protocol`: svc/clc/TE/U_/SU_/B_/SND_/STAT_ constants.
- `dgram::Connection` is the net_dgrm.c state machine and has no I/O: reliable messages are split into fragments (`fragment`, 1024 by default), each ends with EOM and must be acked before the next goes out. A fragment is resent after 1 s. Unreliable messages carry a sequence number and stale ones are dropped. Queued reliable messages are merged up to 64000 bytes. Use it through `send_reliable`/`send_unreliable`/`handle_packet`/`poll`/`take_outgoing`. `NetSocket<T: Transport>` ties a connection to a peer.
- `control`: `Ccreq`/`Ccrep` encode/decode for CCREQ_CONNECT/SERVER_INFO/PLAYER_INFO/RULE_INFO and CCREP_ACCEPT/REJECT/SERVER_INFO/PLAYER_INFO/RULE_INFO.
- `transport`: `Transport` trait, `UdpTransport` (non-blocking std UdpSocket), `LoopbackTransport::pair()`.

### qclient
- `ClientState`: protocol/flags, signon, precache name lists, `model_flags` (MF_*, which the host fills in after loading models), `entities` (with baselines, `msg_origins`/`msg_angles` [0]=new [1]=old, interpolated `origin`/`angles`), statics, stats/items, scores, lightstyles, viewangles/punch/velocity, intermission/centerprint, fog/skybox. Effects live in qecs pools: `beams`, `dlights` (`Pool`) and `particles` (`ParticlePool`, PT_* kinds).
- `parse_server_message(&[u8]) -> Result<(), ParseError>` handles every svc_* that ironwail's CL_ParseServerMessage handles (FitzQuake/RMQ bits, Nehahra U_TRANS, temp entities). Side effects come out as `ClientEvent`s (sounds, prints, stufftext lines, signon stages) through `take_events()`. Signon replies and string commands are queued as clc bytes, drained with `take_reliable_out()`.
- `frame(dt)`: time advance, CL_RelinkEntities (CL_LerpPoint; `nolerp` for a local server), beam temp entities, dlight decay, CL_RunParticles. The particle spawners (run_particle_effect, rocket_trail, explosions, splashes, entity_particles) are public. `QRand` stands in for C rand().
- Input: `base_move(&InputState, &MoveSpeeds, frametime) -> UserCmd`, `encode_move`/`build_move` (clc_move; the first two moves of a level are dropped).
- The test `tests/loopback.rs` connects to qserver on start.bsp for protocols 15/666/999 and checks signon, entities, the player origin and gunshot particles.

### qrender
- Vulkan 1.3 (dynamic rendering, descriptor indexing, multiDrawIndirect) via `ash` loaded at runtime; no SDL. `Renderer::new(&dyn SurfaceProvider, &palette)` or `Renderer::headless(w, h, &palette)` + `read_pixels()` (RGBA8). Windowed: `request_capture()` before `render` copies the presented swapchain image (swapchain gets TRANSFER_SRC usage when supported) into a host buffer; `read_pixels()` then returns it as RGBA8 (BGRA swizzled). `resize(w, h)` must be called by the host on window resize (vsync toggles recreate the swapchain).
- Loading: `load_world(&Bsp, lit) -> Vec<ModelId>` (submodels `*1..`; frees the previous level); `load_level(&Bsp, lit, &[(&Bsp, lit)]) -> (subs, brush ids)` also loads external brush models (`maps/b_*.bsp`): their faces, textures (appended to the material table) and lightmaps (same atlas) are appended to the world buffers and drawn like submodels, `load_alias(&Mdl)`, `load_sprite(&Spr)`, `set_skybox(Option<&[Rgba;6]>)` + `load_skybox(&Fs, name)`, `register_pic(&Pic)`/`register_pic_rgba`, `set_conchars(128×128)`. Pics persist across maps. `light_point(p, styles)` = R_LightPoint.
- Per frame: `render(&RenderFrame)` with `View` (fov_x is for 4:3, adapted to aspect), `RenderEntity` (frame/prev_frame/lerp, skin, colormap, alpha, viewmodel, fullbright, player), `Particle` (palette colour), `DLight`, d_lightstylevalue ints (`lightstyle_value`), `Fog` (`parse_fog`), polyblend, `Draw2D` list (pixels), `Settings` (gamma, contrast, wateralpha + per-type, skyfog, skyalpha, dynamic, vsync, `msaa`, `render_scale`, `texture_linear`, `anisotropy`; changes are applied at the start of `render`: MSAA rebuilds the scene pipelines via `Pipelines::build` and the targets, scale rebuilds targets, filtering recreates the `Sampling::World` sampler and rewrites those descriptors). `View::reserve_bottom` keeps the bottom rows free of 3D (classic status bar). These mirror qclient fields; the binary converts.
- Internals: own block allocator (64 MB blocks per memory type/linearity, first fit + merge). All textures are one bindless `sampler2D[4096]`; world materials SSBO is indexed by `firstInstance`. World: static vertex buffer; per frame CPU PVS + frustum + backface marking, indices copied per texture into a per-frame host buffer, one `vkCmdDrawIndexedIndirect` per pass (opaque/sky/liquid/translucent liquid). Brush entities are drawn with a dynamic depth bias (vkQuake `gl_zfix` values for D32F: constant -4, slope -0.125) so they win over coplanar world faces (e.g. e1m1's lowered floor door). Lightmaps: raw samples (mono or .lit) in an SSBO; a compute shader rebuilds visible surfaces' texels in an RGBA16F atlas (2048², layers) from styles + dlights every frame. Alias: all poses in one SSBO, lerp in the vertex shader, anorm shadedots as vkQuake, colormap via ramp masks + palette SSBO: base skins keep their original colours (blacking out shirt/pants texels broke linear filtering on non-player models such as the nailgun), and only translated entities add (translated − default) ramp colour where the nearest-sampled mask is set. Scene renders to RGBA8 at output size × render scale (multisampled + resolved when MSAA is on), then the post pass applies polyblend, contrast, gamma at output resolution and the 2D list is drawn on top in the same output pass (so UI is never scaled or gamma-adjusted). Texture sampling classes: `World` (user filter/anisotropy), `Nearest` (UI pics, colormap index textures), `Clamp` (sprites, skybox).
- Filtering and 8-bit emulation (ironwail/vkQuake parity, 2026-11-11):
  - Alias pipelines cull back faces (clockwise front, ironwail's `glFrontFace(GL_CW)`); other pipelines don't cull.
  - Texture filtering: `gl_texturemode` (ironwail's six `GL_*` names or 1–6; empty = `r_texfilter` picks nearest/linear with linear mips), `r_anisotropy` (default 16 like vkQuake's `vid_anisotropic 1`, clamped to the device), `gl_lodbias` (`auto` = -log2(samples)/2 when supersampling). They all go into the one `Sampling::World` sampler. Non-mipmapped modes use `max_lod 0`.
  - `vid_fsaamode 1` turns on per-sample shading (`sampleRateShading`) in the scene pipelines.
  - `r_dither` (default 1) with `r_softemu 0`: Bayer banding suppression of ±0.5/255 in the world and alias shaders and the post pass.
  - `r_softemu` 1 fine / 2 coarse / 3 banded palettizes in the post pass. A 128³ palette-index LUT is built on the CPU (`convert::palette_lut`, multithreaded) and stored in the palette SSBO after the 256 colours, packed 4 per uint. It is rebuilt when the metric changes: `r_softemu_metric` <0 auto = OKLab, or naive RGB in banded mode on maps without `.lit` (`World::lit`).
  - Polyblend, contrast and gamma are applied to the looked-up palette colour, like ironwail's postprocessed palette.
  - Per-mode values go in the Frame UBO (`soft`, `softf`), worked out in `frame::softemu_params`. World dither level 1 (coarse) snaps lightmap UVs to 1/16 texel, biases texture LOD by -0.5, and adds near (texture-space) / far (screen-space, fog only) noise. Level 2 (`r_softemu_lightmap_banding`) quantizes light to 64 levels with a -1 bias. Alias mode 1 dithers only in fog; mode 2 (`r_softemu_mdl_warp`) samples a `noperspective` texcoord at LOD 0 (software affine warp).
  - Coarse mode forces nearest + linear mips with at least 8× anisotropy. Fine mode dithers in post with noise per scene pixel.
  - Not ported: dithering for water, sky, sprites and particles.
- Example: `cargo run -p qrender --example offscreen [map] [w h] [game]` writes `target/qrender-<map>.ppm`. `QRENDER_DEVICE=<index>` picks a GPU; `QRENDER_VALIDATION=1` enables the Khronos layer.

## Milestone 4 crate boundaries (set before parallel work)
- `qplatform` owns SDL3, via raw `sdl3-sys` and linking the system libSDL3. Window creation has a Vulkan flag. It provides `vulkan_instance_extensions() -> Vec<CString>` and `create_vulkan_surface(instance: u64 /*VkInstance*/) -> u64 /*VkSurfaceKHR*/`. Events are a plain Rust enum (key, mouse motion/button/wheel, text, gamepad axis/button, added/removed, resize, quit). Also gamepad rumble, relative mouse, an audio stream fed by a callback `FnMut(&mut [i16])` in stereo at 44100/48000 Hz, and a high-resolution timer.
- `qrender` does not depend on SDL. It takes `trait SurfaceProvider { fn instance_extensions(&self) -> Vec<CString>; fn create_surface(&self, instance: u64) -> u64; fn drawable_size(&self) -> (u32, u32); }`. Inputs are `qformats` data plus a per-frame `RenderFrame` view built from `qclient::ClientState` (refdef, entities, particles, dlights, lightstyles, 2D draw list).
- `qsound` has no device code. `Mixer::paint(&mut [i16])` is called from the platform audio callback, sounds are fed from qclient `ClientEvent`s, and music is streamed (OGG/MP3/FLAC decoders allowed as solved-problem crates).
- `qinput` has no SDL dependency. It consumes `qplatform`-style events through its own small event enum mirrored in the binary, and produces `qclient::InputState`/view angle deltas. It owns binds, gamepad curves and aim assist.
- The root binary (`src/main.rs`) is the host glue: frame loop, cvars/commands, loopback server/client, and the console.

### qsound
- `SoundSystem::new(rate, loader)` returns `(SoundSystem, Mixer)`. `loader: FnMut(&str) -> Option<Vec<u8>>` fetches bytes by game path. `SoundSystem` stays on the game thread: `precache` (under `sound/`, resampled to mono i16 at the output rate, stereo downmixed), `event(SoundEvent)`, `update(Listener)`, `set_volume`/`set_bgm_volume`/`set_water_fx`, `play_music`/`play_cd_track` (`music/trackNN.ogg|mp3|flac|wav`, opened on the game thread)/`stop_music`/`pause_music`.
- `Mixer` is `Send` and lives on the audio thread. `paint(&mut [i16])` (interleaved stereo) first swaps out the `Mutex<Vec<Cmd>>` queue (a short lock), then mixes. Channel layout follows snd_dma.c: 4 ambients, then 128 dynamic, then statics up to 1024. Spatialization, pick-channel and ambient fade follow the C code. Music is decoded by symphonia inside the callback and linearly resampled. The underwater effect is a 1 kHz one-pole lowpass that fades in and out.
- `SoundEvent` mirrors the qclient events; the binary does the conversion (attenuation as a float, volume 0..1).

### qplatform
- `sdl3-sys` 0.7.3 links the system libSDL3 through pkg-config.
- `Platform` (`!Send`): `init` / `init_with_hints`, `create_window(title, w, h, vulkan)`, `poll_event(s)` (gamepads hotplug automatically), `rumble`, gyro (`has_gyro` / `set_gyro_enabled`), `open_audio(freq, FnMut(&mut [i16]) + Send)` (stereo S16; starts paused, so call `resume()`), `ticks_ns`.
- `Window`: `drawable_size`, `window_size`, `display_modes`, `desktop_size`, `set_display(DisplayMode::{Windowed, Fullscreen, Borderless}, size)`, `set_relative_mouse`, `set_text_input`, `set_fullscreen`, `vulkan_instance_extensions`, `create_vulkan_surface(instance) -> Result<u64>`. The binary implements `qrender::SurfaceProvider` on top of `Window`.
- The `Event` enum carries raw SDL scancodes and keycodes; `axis::*` and `button::*` hold the SDL gamepad enum values.

### qinput
- Keys use ironwail numbering (gamepad keys 207–248, plus `_ALT` variants). `KButton` behaves like `kbutton_t`. `InputEvent` is the crate's own enum; the binary converts from qplatform events.
- `InputHost: CmdContext`. `register_commands` adds bind/unbind/unbindall/bindlist/impulse and the +/- buttons. `handle_event(ctx, ev) -> Option<Routed>` returns what the console or menu still has to handle. `set_key_dest`.
- `Input::new(&mut Cvars)`. `Input::frame(&cvars, viewangles, dt, Option<&AimContext>) -> qclient::InputState`, where `look_delta` holds [yaw, pitch] degrees.
- Aim assist works only with the gamepad, never with mouse input in the same frame. Its parts: a cone of `aim_cone` plus the target's angular radius, linear falloff, look slowdown, and rotational assist while a stick is deflected. Snap is off. Targets need a visibility check through `trait Visibility`. `aim_assist_allowed(deathmatch, sv_aimassist)` gates it; the binary registers `sv_aimassist`.
- Cvars are listed in `qinput::CVAR_LIST` (mouse, joy_*, flick stick, gyro, aim_*).
- Differs from ironwail: flick stick works without gyro, and `gyro_mode` uses its own numbering (0 = always on, 1 = `+gyroaction` disables, 2 = `+gyroaction` enables).

### Host binary (`src/`)
- User data (config.cfg, saves, default screenshots) lives in `$XDG_DATA_HOME/quake-source-port/<game>/` (default `~/.local/share`), which is added as a top-priority search path. The game data dir is never written (user decision 2026-11-11).
- Modules: `main` (args, SDL/renderer/audio setup, frame pacing), `host` (cbuf/commands/cvars, loopback server+client, frame), `assets` (precache → qrender/qsound incl. `b_*.bsp` brush models via `load_level`, worldspawn fog/sky), `view` (V_CalcRefdef, colour shifts, viewmodel, entity/particle/dlight lists, alias frame lerp (`r_lerpmodels`) and MOVESTEP move lerp (`r_lerpmove`, R_SetupEntityTransform) tracked per entity; the view consumes `LERP_RESETANIM`/`LERP_RESETMOVE`), `sbar` (`scr_hudstyle` 1 = modern corner HUD, default; 0 = classic bar drawn in a full-width backtile strip below the 3D view via `View::reserve_bottom`), `console`, `menu` (basic menus only; the mod menu goes elsewhere; Options is a hub of Gameplay/Input/Graphics/Audio pages built from typed `Item`s (slider, toggle, choice, display mode, resolution, apply); display mode/resolution are pending until Apply runs `vid_apply`; `menu_options <page>` opens a page; every option cvar is archived; menus are laid out in a 16:9 640×360 virtual area (`Canvas::menu_layout`, integer scale `floor(min(h/360, w/640))`, 3x at 1080p) with the classic 320×200 screens centred in it and the options pages drawn as a wide panel with a highlight bar; option values with more than two choices (choice cvars, display mode, resolution) open a scrolling dropdown on Enter/click, while Left/Right still cycle; the quit prompt has clickable (Y)es / (N)o buttons (Left/Right + Enter, Y/N keys, Escape); mouse hover/click/wheel via `HostEvent::MousePos` mapped into that 640×360 space, click-to-set on option sliders), `draw` (2D canvas in 320-wide virtual units, `scale = floor(h/240)`), `platform` (SurfaceProvider, SDL→`InputEvent`), `png`, `default.cfg` (Quakespasm's, used when `default.cfg` is missing as in the 2021 re-release).
- One cvar registry: the host's `CmdContext::cvars` is `server.core.cvars`. Lines go to host commands/aliases/cvars, then to server commands (`Server::has_command`), else to the local client as a string command.
- Frame: input every frame (`base_move` updates view angles), server tic at 72 Hz when `host_maxfps` > 72 (else every frame); attack/jump/impulse are latched until a tic sends them. `connect_local` signs on directly, so client signon string commands (and name/color) aren't forwarded.
- Options: `-basedir`, `-game`, `-hipnotic/-rogue/-quoth`, `-width/-height`, `-fullscreen`, `-headless` (offscreen renderer, simulated `1/host_maxfps` steps, no audio), `-autoquit <s>`, `-nosound`, `+cmd` (an argument may contain spaces: `"+delay 2 +forward"`). Extra host command `delay <s> <cmd>` for scripting. `screenshot [path]` writes PNG (headless offscreen image, or windowed via `request_capture` + `read_pixels`). Prints are passed through `server.core.loc.translate` (server already formatted them).
- Example: `quake-source-port -headless -game ad -autoquit 6 +map ad_sepulcher "+delay 4 screenshot target/shots/ad"`.
- Savegames: `Server::save_game`/`load_game` (qserver `save.rs`, version 5 text format as ironwail); `Sv::loadgame` makes connect/spawn keep the saved edicts and parms.
- One shared command buffer, as in Quake: the server's Cbuf text (QC `localcmd`, changelevel) is moved into the host Cbuf before each server frame and at each `exec_cbuf`, so it reaches host commands and cvars too. `name`/`color` are host commands that set `_cl_name`/`_cl_color` and forward to the server, which has server-side `name`/`color` (svc_updatename/updatecolors).
- `.lit` and `.ent` files load through `Fs::open_companion`, which only accepts them from the same or a higher-priority game dir than the map (ironwail's `path_id` check), so id1's `start.lit` never lights another game's `start.bsp`.
- Fog has a single current state, `cl.fog`. Worldspawn sets it at map load (unless an svc_fog arrived first), and svc_fog or the `fog` console command (Fog_FogCommand_f) change it; MG1 sets fog through QC. Fade times apply instantly.
- Liquid alpha (GL_WaterAlphaForTextureType): worldspawn `[_]wateralpha/lavaalpha/slimealpha/telealpha` override `r_wateralpha` and `r_lavaalpha/r_slimealpha/r_telealpha`; a per-type value of 0 falls back to water alpha. Not ported: ironwail's check that the map was vised for transparent water (`contentstransparent`).
- `r_drawentities 0` hides all entities except the view model (for debugging).
- `COMPAT_CVARS` (host.rs) registers cvars from other engines without effect (gl_*, r_*_list, scr_usekfont, ...), so vkQuake.cfg and mod configs load quietly. `gamecfg`/`savedgamecfg` are real: QC reads them.
- `sbar::Pack` (Id/Hipnotic/Rogue, from the mounted game dirs as in ironwail: rogue, hipnotic or quoth) selects the mission-pack status bar rules in both HUD styles: hipnotic laser/Mjolnir/proximity weapons, keys on the main bar, wetsuit/empathy shield items; rogue RIT_* ammo and armor bits, r_invbar1/2, powered-up weapon pics and shield/antigrav items, and no sigils. Rogue's CTF team-colour face is not done.
- Menu pause: in single player the host calls `Server::frame_held(dt, true)` while the menu is open, which skips physics like ironwail's `key_dest == key_game` check, without setting `sv.paused`.
- Demo playback (`src/demo.rs`, host `read_demo`): `playdemo`, `startdemos` (gated by `cl_startdemos`), `demos`, `stopdemo`, ported from ironwail's CL_PlayDemo_f/CL_GetMessage/CL_NextDemo. Messages go through the normal client parser, which is fed until `cl.time` catches up with `mtime[0]`. View angles are interpolated from `cl.mviewangles`. With no server world, point contents and ambient sound levels come from the client's BSP (`bsp_leaf`). The startup menu stays open over the demo; a key pressed in game mode during a demo opens the menu, and starting a map ends the demo loop.
- Demo recording (`DemoWriter`): `record <name> [<map> [track]]` writes `<user dir>/<name>.dem` from the messages in `feed_client`; `stop` ends it with svc_disconnect, and leaving the game or starting a map stops it too. Recording mid-game follows ProQuake/ironwail. The messages that reached signon 2 (`demo_head`) are written first, then scores, lightstyles, stats, setview and signon 3. Signon 3 is skipped when loopback already batched it into the head. With a map argument, recording starts when that map connects. `playdemo` also looks in the user dir.
- `timedemo <name>` reads one message per frame without a frame cap and prints frames/seconds/fps from wall-clock time, so the result is real even headless.
- Gaps: multiplayer menu, `sizeup/down` are stubs.
