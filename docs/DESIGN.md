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

Headless server, `skill 3`, `god` on. For each map:
1. Parse entities: monsters, secret triggers (`trigger_secret`), doors/buttons/keys.
2. Build a nav graph from BSP walkable surfaces (hull 1 traces), plus edges discovered by simulated jumps.
3. Goal loop: nearest unvisited secret trigger / live monster / required key/button; path, move via usercmds, shoot monsters (auto-aim from server side state), use buttons by touching/shooting.
4. Stuck detection → noclip to goal, recorded as "noclip used".
5. Report JSON per map: kills/total, secrets/total, noclip uses, time, errors (QC runtime errors, crashes). Exit code nonzero on regressions vs a stored baseline.

## Tooling

- `cargo nextest run` — quiet default profile; `--profile verbose` for debugging.
- `cargo clippy --message-format=short`.
- `tools/qinspect`, `tools/qbot`, `tools/qbench` as above.

## Work plan (milestones)

1. qcore + qformats + qinspect, with tests against real data (skip if absent).
2. qvm + qworld + qserver headless: load start.bsp, run frames, no errors on all installed maps (smoke test across every mod).
3. qnet loopback + qclient.
4. qplatform + qrender (world, models, 2D) + qsound + qinput → playable id1.
5. AD / MG1 / MG3 / hipnotic / rogue compat pass.
6. qui console/menus/mod menu; aim assist.
7. qbot; qbench perf parity.
8. Network play protocols 15/666/999.

Subagent split: crates with frozen interfaces (qformats, qvm interpreter, qsound mixer, qui toolkit, qbot nav) are independent tasks given to separate agents once qcore's API is set.

## Implemented APIs

### qecs
- `Entity` (index = QC edict number + generation); `EntityAllocator` with `alloc`, `alloc_with(|idx, freed_at| bool)` for ED_Alloc-style reuse policies, `alloc_at(idx)`, `free(e, now)`, `free_time`.
- `SparseSet<T>` with generation-checked access plus `*_raw` index access for QC; `join2`/`join3` and `join_mut`/`join2_mut` all iterate the smallest storage.
- `Pool<T>`/`PoolHandle`: preallocated with `with_max`, swap-remove `retain`.
- `ParticlePool`: SoA, `spawn`, `kill_expired`, `integrate(dt)`, `update_with(time, dt, closure)` for per-kind ramps/gravity.

### qformats
All parsers take `&[u8]` and return `qformats::Result<T>`; bad input gives an `Error { format, msg }` with the byte offset, never a panic. Vectors are `[f32;3]`.
- `Bsp::parse`: BSP29/BSP2/2PSB normalised into one set of structs (i32 children: a node child < 0 is leaf `-(i+1)`, a clipnode child < 0 is a contents value). Also `decompress_vis`, `leaf_pvs`, `parse_lit`.
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
- Clients are in-memory slots. `connect_local()` runs the full signon and redoes it after changelevel. `set_move(i, ClientMove)`, `client_command`. Output via `take_reliable(i)` / `take_datagrams(i)`, which are protocol 15/666/999 byte streams.
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
- Per frame: `render(&RenderFrame)` with `View` (fov_x is for 4:3, adapted to aspect), `RenderEntity` (frame/prev_frame/lerp, skin, colormap, alpha, viewmodel, fullbright, player), `Particle` (palette colour), `DLight`, d_lightstylevalue ints (`lightstyle_value`), `Fog` (`parse_fog`), polyblend, `Draw2D` list (pixels), `Settings` (gamma, contrast, wateralpha + per-type, skyfog, skyalpha, dynamic, vsync). These mirror qclient fields; the binary converts.
- Internals: own block allocator (64 MB blocks per memory type/linearity, first fit + merge). All textures are one bindless `sampler2D[4096]`; world materials SSBO is indexed by `firstInstance`. World: static vertex buffer; per frame CPU PVS + frustum + backface marking, indices copied per texture into a per-frame host buffer, one `vkCmdDrawIndexedIndirect` per pass (opaque/sky/liquid/translucent liquid). Lightmaps: raw samples (mono or .lit) in an SSBO; a compute shader rebuilds visible surfaces' texels in an RGBA16F atlas (2048², layers) from styles + dlights every frame. Alias: all poses in one SSBO, lerp in the vertex shader, anorm shadedots as vkQuake, colormap applied exactly via ramp masks + palette SSBO. Scene renders to RGBA8 then a post pass applies polyblend, contrast, gamma.
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
- `Window`: `drawable_size`, `set_relative_mouse`, `set_text_input`, `set_fullscreen`, `vulkan_instance_extensions`, `create_vulkan_surface(instance) -> Result<u64>`. The binary implements `qrender::SurfaceProvider` on top of `Window`.
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
- Modules: `main` (args, SDL/renderer/audio setup, frame pacing), `host` (cbuf/commands/cvars, loopback server+client, frame), `assets` (precache → qrender/qsound incl. `b_*.bsp` brush models via `load_level`, worldspawn fog/sky), `view` (V_CalcRefdef, colour shifts, viewmodel, entity/particle/dlight lists, alias frame lerp tracked per entity), `sbar`, `console`, `menu` (basic menus only; the mod menu goes elsewhere), `draw` (2D canvas in 320-wide virtual units, `scale = floor(h/240)`), `platform` (SurfaceProvider, SDL→`InputEvent`), `png`, `default.cfg` (Quakespasm's, used when `default.cfg` is missing as in the 2021 re-release).
- One cvar registry: the host's `CmdContext::cvars` is `server.core.cvars`. Lines go to host commands/aliases/cvars, then to server commands (`Server::has_command`), else to the local client as a string command.
- Frame: input every frame (`base_move` updates view angles), server tic at 72 Hz when `host_maxfps` > 72 (else every frame); attack/jump/impulse are latched until a tic sends them. `connect_local` signs on directly, so client signon string commands (and name/color) aren't forwarded.
- Options: `-basedir`, `-game`, `-hipnotic/-rogue/-quoth`, `-width/-height`, `-fullscreen`, `-headless` (offscreen renderer, simulated `1/host_maxfps` steps, no audio), `-autoquit <s>`, `-nosound`, `+cmd` (an argument may contain spaces: `"+delay 2 +forward"`). Extra host command `delay <s> <cmd>` for scripting. `screenshot [path]` writes PNG (headless offscreen image, or windowed via `request_capture` + `read_pixels`). Prints are passed through `server.core.loc.translate` (server already formatted them).
- Example: `quake-source-port -headless -game ad -autoquit 6 +map ad_sepulcher "+delay 4 screenshot target/shots/ad"`.
- Savegames: `Server::save_game`/`load_game` (qserver `save.rs`, version 5 text format as ironwail); `Sv::loadgame` makes connect/spawn keep the saved edicts and parms.
- Gaps: hipnotic/rogue status bars, demos, multiplayer menu, `sizeup/down` are stubs.
