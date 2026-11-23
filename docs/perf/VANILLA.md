# Part 1: Vanilla ultra against ironwail and vkQuake

The base game at the best settings all three engines share, measured with
the same demos in each engine. Shared rules, resolutions, metrics and the
retooling steps are in [README.md](README.md).

Goal: at both resolutions, on every scene, our median frame time and p99
are no worse than ironwail's and, where comparable, vkQuake's. Any scene
where a reference is more than 5% faster gets an item below. Visual parity
is a gate, not a report: a scene whose stills fail parity doesn't count as
a win.

## Settings

"Vanilla ultra": 8× MSAA, no supersampling, 16× anisotropic filtering,
remaster md5 models, everything else the classic look. The table is code:
`tools/perfkit/src/settings.rs`, printed by `perfkit settings [ultra|
ultra-fte]` (step R2, done 2026-11-17). Benchmarked binaries: ironwail
0.8.1, vkQuake **1.34.1** (the AppImage; the `vkQuake/` source checkout is
newer).

| Setting | ours | ironwail | vkQuake |
|---|---|---|---|
| 8× MSAA | `r_msaa 8` | `vid_fsaa 8` | `vid_fsaa 8` |
| no supersampling | `vid_fsaamode 0` | `vid_fsaamode 0` | `vid_fsaamode 0` |
| 16× anisotropic | `r_anisotropy 16` | `gl_texture_anisotropy 16` | `vid_anisotropic 1` (device max, 16×) |
| classic texture filter | `r_texfilter 0` | `gl_texturemode GL_NEAREST_MIPMAP_LINEAR` | `vid_filter 1` |
| render scale 1 | `r_renderscale 1` | `r_scale 1` | – (vkQuake has no render scale) |
| remaster md5 models | `r_enhancedmodels 1` | `r_enhancedmodels 1` | `r_allow_replacement_md5models 1` |
| model/move lerping | `r_lerpmodels 1`, `r_lerpmove 1` | same | same |
| dynamic lights | `r_dynamic 1` | same | same |
| coloured light (`.lit`) | `r_coloredlight 1` | always on | always on |
| fog | `r_fog 1`, `r_skyfog 0.5` | `r_skyfog 0.5` | `r_skyfog 0.5` |
| gamma / contrast | `gamma 1`, `contrast 1` | same | same (defaults are 0.9 / 1.4) |
| no vsync | `vid_vsync 0` | same | same |
| vkQuake extras off | – | – | `r_rtshadows 0`, `vid_palettize 0` |
| our enhancements and add-ons off | `r_ssao 0`, `r_motionblur 0`, `r_speedblur 0`, `r_enhancedwater 0`, `r_teleportwindows 0`, `r_enhancedfx 0`, `r_enhancedparticles 0`, `r_impact_fx 0` (+ bullets, nails, scorches, tracers), `r_blood 0`, `r_debris 0`, `r_softemu 0`, `r_dither 0`, `r_colorblind 0`, `custom_enable 0`, `addon_hitstop 0` | – | – |
| particles, `ultra` | `r_fteparticles 0`, `r_particle_quality 1`, `r_particles 2` | `r_particles 2` | `r_fteparticles 0`, `r_particles 2` |
| particles, `ultra-fte` | `r_fteparticles 1`, `r_particledesc classic`, `r_particle_quality 0` (soft particles off), `r_part_density 2` (undoes quality 0's half density), `r_quadparticles 1`, `r_particles 2` | `r_particles 2` | `r_fteparticles 1`, `r_particledesc classic`, `r_quadparticles 1`, `r_particles 2` (1.34.1 has no soft particles) |

How the pins reach each engine (perfkit runner):

- `vid_*` cvars go on the command line, since the references read video
  cvars from there before creating the window; everything else goes into
  a generated cfg in the sandbox, run with `+exec` before `+timedemo`.
  ironwail cuts off a long command line: with every pin on it, it stopped
  at `+r_sca…` and never started the demo.
- Every engine starts from its defaults: the sandbox leaves out the game
  data's configs, and ours gets `XDG_DATA_HOME` pointed at a sandbox
  folder with an empty `config.cfg`, so it never reads (or writes) the
  user's settings and saves.
- A run fails if the engine answers any pinned cvar with `Unknown
  command`. That caught `r_scale`, which vkQuake doesn't have (qbench's
  vkQuake pin has been a no-op), and `r_softparticles`, which the 1.34.1
  binary lacks. `perfkit`'s tests check every pin against each engine's
  source as well.
- `r_particles 2` (square, software look) is both references' default;
  ours defaults to 1 (round, GLQuake look). vkQuake's ray-traced shadows
  are off because we have no equivalent; its `r_waterquality` and
  `r_sky_quality` stay at their defaults.

Particles: the main suite uses each engine's **classic** particles
(`ultra`) so the three engines draw the same thing. FTE particles have
their own suite below (`ultra-fte`).

## Scenes

Demos recorded by our engine in a vanilla (not customized) game, so the
references can play them. `perfkit record [-only a,b] [-verify]` (step R3)
records the catalogue in `tools/perfkit/src/scenes.rs` into
`target/perf/demos/<commit>/` with a `manifest.json`, and points
`target/perf/demos/latest` at it.

| Demo | Map | Game s | Size | What it stresses |
|---|---|--:|---|---|
| `spin-e1m1` | id1 e1m1 | 240 | small | baseline, md5 grunts/dogs |
| `spin-e2m2` | id1 e2m2 | 240 | medium | more monsters, water |
| `spin-mg1` | mg1 start hub | 240 | medium | fog, KEX-era content |
| `spin-sepulcher` | ad ad_sepulcher | 240 | huge | world size; AD without FTE: no decorations, classic particle messages every frame (134 MB) |
| `spin-sepulcher-fte` | ad ad_sepulcher | 240 | huge | the same with FTE: 1703 decorations, point particles (FTE suite) |
| `spin-tears` | ad ad_tears | 240 | huge | 217k BSP nodes, PVS/culling |
| `combat-e1m3` | id1 e1m3 | 120 | small | skill 3; rockets, explosions, gibs, dynamic lights, many md5 models on screen |
| `combat-grendel-fte` | ad ad_grendel | 120 | medium | the same fight with FTE: AD's `effectinfo` rocket trails, explosions, blood and item effects (FTE suite) |

How they're recorded: our engine headless in the sandbox (`XDG_DATA_HOME`
there, `+sv_autosave 0`, 320×240, `-fixedstep 250`), so nothing outside
`target/` is touched; all eight take about 5 minutes. The server ticks at
72 Hz, which gives ~62.5 demo frames per game second whatever the step:
~15000 frames per spin, ~7500 for combat. Spins are qbench's script (god,
30°/s spin at the spawn point); combat is qprof's (god, `impulse 9`
refilled every 20 s, 40°/s spin firing rockets → super nailgun → super
shotgun). `god` and the first `impulse 9` wait half a second and the
rocket launcher one second: sent with `+record` they ran before the
player spawned, so until the 2026-11-17 set `god` never took (AD's combat
player killed itself) and combat started with the shotgun ("No weapon")
until the 40 s switch. `sv_fteparticles` and `r_fteparticles` are pinned while
recording: the *server* cvar decides whether AD sees FTE support. Our
engine writes a plain `.dem` every engine plays and, when that conversion
drops FTE/DP particle messages, keeps the original as `.qsd`; in the FTE
suite ours and vkQuake play the `.qsd`, ironwail the `.dem` (installed in
the sandbox as `<scene>-full.dem` / `<scene>-plain.dem`).

Timedemo playback runs no server or QuakeC. Server cost (HISTORY items 8
and 10) is measured separately, in our engine only, with a live
`qprof -window -fixedstep 250` vanilla-ultra run of the combat script.

## Particle suite

ironwail has classic particles only; vkQuake and we have FTE scripted
particles. So there are two comparisons:

- **Classic, three engines:** `combat-e1m3` and `spin-sepulcher` with
  classic particles. In a timedemo the entities come from the demo, so the
  engine that *records* decides what AD shows: AD's progs drops its 1703
  `misc_gtkmodel` decorations on ad_sepulcher when the server doesn't
  advertise FTE particles, and ours advertises them only while
  `sv_fteparticles` is on. So the main suite plays `spin-sepulcher`
  (recorded without FTE, as ironwail would see AD), the FTE suite
  `spin-sepulcher-fte`, and every engine plays back the same entities.
- **FTE, ours and vkQuake:** `spin-sepulcher-fte` (AD's sky rain from
  `_texpart_*` emitters) and `combat-grendel-fte` (AD's `effectinfo`
  combat effects), both from their `.qsd`, with `r_fteparticles 1`,
  `r_particledesc classic`. id1 maps get no FTE effects under
  `r_particledesc classic` (V3), so `combat-e1m3` stays classic only. Before comparing, check with `r_partinfo` (both
  engines have it) that live particle, beam and decal counts are within a
  few percent of each other. Otherwise the workloads differ and the fps
  comparison is meaningless. `perfkit partinfo` does this (V3). ironwail
  is listed as "classic fallback, not comparable".

If the combat demo turns out too light to stress particles, record a
denser one (more monsters, more explosions) in a vanilla map rather than
using `sv_horde`, which is a gameplay modifier.

## Visual parity

`perfkit stills` (R5) captures fixed views in all three engines and
compares them. The views (`perfkit::stills::VIEWS`) are chosen for what
can go wrong:

| view | map | checks |
|---|---|---|
| e1m1-start, e1m1-back | e1m1 | world, lightmaps, flame models |
| e1m1-grunts | e1m1 | md5 grunt and dog close-up |
| e2m2-start | e2m2 | world, lightmaps |
| mg1-start, mg1-back | mg1 start | fog, fence textures |
| sepulcher-start | ad_sepulcher | coloured light, fog, sky |
| tears-waterfall | ad_tears | coloured light, translucent water, waterfall |
| sepulcher-fte-start | ad_sepulcher, FTE | AD sky rain, `misc_gtkmodel` decorations |

How the capture works, and why:

- **One demo per map, recorded by our engine.** The demo runs noclip and
  notarget, then `setpos` to each view, holding it 2 s. Before each view,
  a quarter second at the same spot turned 45° works as a separator, so
  two views with the same angles are still told apart. Console scripts
  can't do this: vkQuake runs a startup `map` only once its command buffer
  is empty, so a `wait` chain after it never sees the map.
- **`screenshot` patched into the demo.** A `svc_stufftext "screenshot"`
  goes three quarters into each hold (`perfkit::demofile`), and the demo
  echoes a marker before its final `svc_disconnect`. Each engine plays it
  with `playdemo` in a 1920×1080 window, with the view's profile and the
  HUD, crosshair, weapon and notify lines off. So all three shoot the same
  moment of the same demo, with the same entity state.
- **`playdemo` with vsync on, not `timedemo`.** vkQuake 1.34.1's
  screenshot copies `swapchain_images[current_cb_index]`: the swapchain
  image matching the command buffer index, not the image it just drew.
  With vsync off one image can stay untouched since startup, and shots
  showed the startup console; with FIFO the images rotate and the copy is
  at most a few frames old. At timedemo speed vkQuake also reached the
  first view before its console had slid away.
- **The metric is DSSIM** (ImageMagick 7, 0 = identical), for every pair
  of engines. The references set the band: ours is OFF when its distance
  to either reference exceeds 1.5 × the ironwail↔vkQuake distance +
  0.005. A flat +0.02 margin let ad_tears' opaque water through
  (0.029 against a band of 0.0135). FTE views compare ours with vkQuake
  only and get no verdict.
- **Gallery.** `target/perf/stills/<commit>/index.html` shows each view
  side by side, with ours' difference images (×4) against both
  references. Inside our engine the old rule still applies: an
  optimisation keeps `compare -metric AE` = 0 on these stills.

## Items

Open items from the old plan, to re-baseline under this suite before
acting (numbers in [HISTORY.md](HISTORY.md)):

- **H1 (old item 1)**: resolve cvar ids at startup; about 280 name lookups
  per frame remain. Small; check at 540p.
- **H4 (old item 4)**: particle cost on AD. Now measured against vkQuake's
  FTE implementation, which tells us whether the remaining cost is ours or
  inherent.
- **H8 (old item 8)**: server spikes in large fights. Live run only.
- **H10 (old item 10)**: repeated contents checks per frame.
- **H12 (old item 12)**: driver and synchronisation overhead; 540p shows
  it best.

From the first baseline (Results, 2026-11-17):

- **V1 — done 2026-11-23.** `spin-sepulcher` at 540p was the sole cell where
  ironwail led (2831 vs 2187 fps, ref +29%, 457 vs 353 µs/frame; 4K led +40%).
  `perf` + `+prof_timedemo` showed the CPU gap was NOT primarily `SVC_PARTICLE`
  parsing (client p50 36 µs) but CPU render time (~430 µs/frame): unculled
  `cl.static_entities` (no efrag PVS culling, unlike ironwail's `R_AddStaticModels`),
  per-frame `Vec::new()` reallocs (`ents`, `parts`, `dls`, `world_draws`,
  `alias_draws`, `brush_surfs`, `demo_msg_buf`), `entity_rotation` (3× `sin_cos`
  + `mul3`) running before frustum culling, `build_world_draw` scanning all 865
  buckets per brush entity, `World::light_point_dir` reading cold `surfaces[]`
  on every cache hit, per-leaf `Planes::clip`, uncached per-frame `Cvars::find`
  lookups, and `Debris::track_shooters`/`Tracers::frame` scanning all active
  entities when off. Fixing these (with `compare -metric AE` = 0 on `ad_sepulcher`)
  lifted `spin-sepulcher` 540p from **2187 → 4073 fps (+86%)**, beating ironwail
  (2795 fps) by **+46%** and vkQuake (703 fps) by **+479%**, and 4K from
  **747 → 802 fps (+50% over ironwail)**. Every Part 1 cell across both suites
  and resolutions now beats the fastest reference by **+25% to +321%**.
- **V2**: one 49.7 ms cold-start hitch on `spin-sepulcher-fte`, the only
  frame over 33 ms in our cold runs. It is probably the FTE particle
  pipelines or textures created on first use; find out and create them at
  load. `combat-e1m3`'s worst cold frame (6.5 ms) is also twice the
  references'.
- **V3 — done 2026-11-17.** `perfkit partinfo` (DESIGN.md, Tooling)
  stuffs `r_partinfo` into 12 blocks of each FTE-suite demo and timedemos
  it in ours and vkQuake. `spin-sepulcher-fte` is like for like: summed
  live particles ours 428978, vkQuake 428354 (+0.1%), every sample within
  0.6%; every AD `effectinfo` type matches exactly, only the rain splashes
  (+3.0%) and lit sky rain (+2.2%) differ; decals agree within sampling
  noise (265–528 per sample in both). `combat-e1m3` has **no** FTE
  particles in either engine: `r_particledesc classic` loads no scripts,
  so id1's effects stay classic, and its `ultra-fte` cell measures the
  same thing as the main-suite combat cell (vkQuake 861 fps in both; ours
  4763 vs 4796). See V6.
- **V6 — done 2026-11-17.** The FTE suite's combat scene is now
  `combat-grendel-fte`: ad_grendel (most monsters near the spawn of any
  AD map, no sky rain) recorded with `sv_fteparticles 1`. `perfkit
  partinfo`: like for like, summed particles ours 82990, vkQuake 83063
  (−0.1%), every sample within 0.2%; rocket trails −2%, every other type
  exact. `-verify` (one run, 540p): ours 2240 fps, vkQuake 714 (ironwail
  3204 on the plain `.dem`), against 4847 / 857 on `combat-e1m3`. The
  re-record also fixed the combat script (Scenes).
- **V7**: decals on `combat-grendel-fte`: from about 40 s in, ours holds
  ~110 live against vkQuake's ~70 (summed over the samples 2134 vs 1912,
  +12%; the first three samples agree). Small next to ~7000 particles, but
  find out whether lifetime, fade or the decal cap differs.
- **V8 — done 2026-11-18.** The `ultra-fte` pins didn't turn our soft
  particles off. Since Particle Quality (0ead02c, 2026-11-16),
  `r_softparticles` is a compat cvar that does nothing and the soft level
  follows `r_particle_quality` (`soft_particles()` in
  `src/host/render.rs`), so `r_particle_quality 1` ran ours with soft
  particles **low**: in the FTE suite ours split the scene pass for AD's
  soft effects and read depth, against vkQuake 1.34.1, which has none.
  Quality 0 turns them off but halves scripted particle density
  (`particle_density()`), so `ultra-fte` now pins `r_particle_quality 0`
  with `r_part_density 2` (density 0.5 × 2 = 1; impacts and enhanced
  particles are off anyway). `r_particle_quality` moved from the shared
  row into the particle rows (`ultra` keeps 1: classic particles aren't
  soft, so the main suite's cells are unchanged). New perfkit tests: no
  pin is a cvar ours only registers for old configs, and every cvar is
  pinned once per profile. `perfkit partinfo` afterwards: still like for
  like, `spin-sepulcher-fte` ours 429322 vs vkQuake 429082 (+0.1%),
  `combat-grendel-fte` 82990 vs 83063 (−0.1%), the same as V3/V6. The
  2026-11-17 FTE cells (void anyway, V4) ran with soft particles on; the
  rerun is the first with them off.
- **V9 (new 2026-11-18)**: steady-state heap allocations and cvar name
  lookups per frame (H1 grew in 3dca905). The first census (Results,
  2026-11-18) counts ~114 allocations per steady-state frame on
  `spin-e1m1`, about a fifth of them in the status bar and HUD text.
  The target is zero allocations per frame in a timedemo, and `CvarId`s
  for everything `view_cvars` and the render settings read each frame.
  Pixel-identical by construction; A/B with the headless timedemo (or
  callgrind, R9).
- **V10 (new 2026-11-18)**: sky wind changed `sky.frag` for every map.
  On the suite's maps (no `<sky>wind.cfg`) the output must be
  pixel-identical to 56238f2's, and the sky stage in `QSP_GPUTIMES` no
  slower at 4K. Then measure one map whose sky drifts (dmd, qbj2 or qbj3)
  against ironwail, since both draw the same two layers.
- **V4**: the first results below are void. R5 found that ours drew its
  main menu over every `+timedemo` (the startup menu opened because a demo
  isn't a running server), and that AD's `quake.rc` runs `menu_main` after
  `stuffcmds`, which put AD's menu over every AD timedemo in all three
  engines. Both are fixed (`src/host/frame.rs`; the sandbox's `quake.rc`
  copy). Rerun qbench before acting on V1/V2.
- **V5 — done 2026-11-17.** ad_tears' waterfall looked whiter and more
  streaky in ours than the references' blue glow (DSSIM 0.019 against
  0.0135). AD ships 32-bit `textures/+Nbmk_wfall01*.tga` replacements,
  which both references load for every world texture (Mod_LoadTextures);
  ours drew the 8-bit miptex and its fullbright streaks. Ours now loads
  external world textures the same way (DESIGN.md, Renderer: Loading),
  which also moved `sepulcher-start` closer to both references.

## Results

**2026-11-23, commit `1a1e77d` (post P5 optimisations), demo set `1a1e77d` (`1a1e77d-dirty` for FTE with `+sv_protocol_pext2 8` so vkQuake 1.34.1 plays `.qsd`).** RTX 3060 Ti (driver 620.70.06), i7-12700K, `qbench` defaults (3 fps runs + 1 MangoHud run + cold start per cell, 960×540 windowed and 3840×2160 8× MSAA in gamescope scaled to 2560×1440, vanilla `ultra` / `ultra-fte`, no menu over demos, no config changed). Reports archived in `target/perf/part1-1a1e77d/`.

| scene | 540p ours | ironwail | vkQuake | ours vs best | 4K ours | ironwail | vkQuake | ours vs best |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| spin-e1m1 | **6137** (6041–6176) | 3757 | 855 | **+63%** | **987** (976–1066) | 645 | 630 | **+53%** |
| spin-e2m2 | **6152** (5924–6159) | 3752 | 852 | **+64%** | **1051** (1046–1062) | 642 | 544 | **+64%** |
| spin-mg1 | **3576** (3487–3610) | 2872 | 801 | **+25%** | **664** (651–699) | 517 | 292 | **+28%** |
| spin-sepulcher | **2187** (2140–2189) | 2831 | 709 | **ref +29%** | **747** (744–783) | 533 | 524 | **+40%** |
| spin-tears | **3464** (3445–3483) | 2514 | 732 | **+38%** | **620** (575–622) | 418 | 303 | **+48%** |
| combat-e1m3 | **5400** (5305–5426) | 3498 | 819 | **+54%** | **975** (967–977) | 610 | 584 | **+60%** |
| spin-sepulcher-fte ᶠ | **668** (666–678) | (2928) | 341 ᶠ | **+96%** | **532** (529–536) | (540) | 221 ᶠ | **+141%** |
| combat-grendel-fte ᶠ | **2703** (2634–2755) | (3135) | 641 ᶠ | **+321%** | **496** (495–501) | (513) | 270 ᶠ | **+84%** |

ᶠ ours and vkQuake play the `.qsd` with AD's FTE particle messages (exact same frame counts: 15002/15001 and 7501/7500). ironwail plays the plain `.dem` without FTE particles/decorations, so its bracketed numbers aren't comparable.

- **p99 frame time (ms, ours / ironwail / vkQuake)**:
  - 540p: `spin-e1m1` **0.61** / 0.71 / 1.28, `spin-e2m2` **0.43** / 0.63 / 1.28, `spin-mg1` 0.76 / **0.72** / 1.46, `spin-sepulcher` 0.90 / **0.73** / 2.36, `combat-e1m3` **0.52** / 0.75 / 1.34; FTE: `spin-sepulcher-fte` **2.34** / (0.70) / 4.76, `combat-grendel-fte` **0.71** / (0.67) / 1.99 (`spin-tears` MangoHud started 15 frames late on our fast run so p99 omitted).
  - 4K 8× MSAA: `spin-e1m1` **2.68** / 3.56 / 7.35, `spin-e2m2` **2.72** / 3.29 / 7.37, `spin-mg1` **4.57** / 4.67 / 10.11, `spin-sepulcher` **2.47** / 4.23 / 7.74, `combat-e1m3` **1.85** / 3.60 / 7.37; FTE: `spin-sepulcher-fte` **4.23** / (4.31) / 9.22, `combat-grendel-fte` **5.69** / (4.19) / 9.70.
- **Cold start (960×540, shader cache off)**: **zero >33 ms hitches** across every completed scene for ours (`spin-e1m1` 5.6 ms, `spin-e2m2` 1.9 ms, `spin-mg1` 2.4 ms, `spin-sepulcher` 4.1 ms, `combat-e1m3` 15.5 ms, `spin-sepulcher-fte` 32.8 ms [down from 49.7 ms, V2], `combat-grendel-fte` 25.9 ms), while ironwail hitched on `spin-sepulcher` (38.4 ms), `spin-tears` (111.2 ms), and `spin-sepulcher-fte` (37.5 ms) and vkQuake hitched on `spin-tears` (210.7 ms).
- **Follow-up (2026-11-23, V1 `spin-sepulcher` CPU optimisations, verified pixel-identical `compare -metric AE` = 0)**: `qbench -suite main -only spin-sepulcher -res 540,4k -runs 3 -mhruns 1 -nocold`:
  - **960×540**: ours **4073** (4001–4081) vs ironwail 2795 (2777–2818) vs vkQuake 703 (699–706) → **+46% over ironwail** (up from 2187 fps / ref +29%, **+86% gain**); p99 **0.65** / 0.87 / 2.58 ms.
  - **3840×2160 8× MSAA**: ours **802** (771–825) vs ironwail 534 (534–535) vs vkQuake 505 (497–511) → **+50% over ironwail** (up from 747 fps); p99 **2.42** / 4.23 / 7.76 ms.
  - Every cell across both Part 1 suites (8 scenes) and both resolutions (540p & 4K) now beats the fastest reference by **+25% to +321%**.

---

**2026-11-17, commit 6fcd456, demo set `e53154d-dirty`. Void (V4): ours
ran with its main menu over every demo, and every engine with AD's menu
over the AD demos.** qbench
defaults: 3 fps runs + 1 MangoHud run per cell, median fps (min–max), p99
from the MangoHud run. 540p is in a window, 4K in gamescope. No errors, and
no config changed. A copy of the report is in `target/perf/part1-6fcd456/`.

| scene | 540p ours | ironwail | vkQuake | ours vs best | 4K ours | ironwail | vkQuake | ours vs best |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| spin-e1m1 | 5277 | 3827 | 887 | +38% | 970 | 657 | 683 | +42% |
| spin-e2m2 | 5262 | 3780 | 891 | +39% | 962 | 656 | 655 | +47% |
| spin-mg1 | 3415 | 2927 | 854 | +17% | 634 (604–681) | 525 | 349 | +21% |
| spin-sepulcher | 2777 | 2840 | 777 | −2% | 723 | 498 | 482 | +45% |
| spin-tears | 3338 | 2500 | 796 | +34% | 583 | 396 | 295 | +47% |
| combat-e1m3 | 4763 | 3545 | 861 | +34% | 882 | 621 | 630 | +40% |
| spin-sepulcher-fte ᶠ | 772 | (2948) | 372 | +108% | 442 | (503) | 261 | +69% |
| combat-e1m3, ultra-fte | 4796 | – | 861 | +457% | 855 | – | 634 | +35% |

ᶠ ours and vkQuake play the `.qsd` with AD's FTE particles. ironwail plays
the plain `.dem` without them, so its numbers in brackets aren't
comparable. Run spread is under 1% except where shown (and for vkQuake at
4K, up to 13%).

- **p99 frame time** is lowest for us in every main-suite cell: 0.46–0.71 ms
  at 540p (ironwail 0.51–1.35, vkQuake 1.19–1.40) and 2.4–3.0 ms at 4K
  (ironwail 3.3–5.9, vkQuake 7.2–9.7). With FTE on sepulcher it is 2.25 ms
  at 540p and 6.83 ms at 4K, against vkQuake's 4.64 and 9.28.
- **FTE particle cost** on sepulcher (frame time with FTE minus classic) is
  0.94 ms for us and 1.40 ms for vkQuake at 540p, and 0.88 ms against 1.76 ms
  at 4K. So the per-frame cost of the particles is smaller than vkQuake's
  (H4), but it is the largest single cost in the suite (×3.6 frame time).
- **Cold start** (driver shader cache off, 540p): our worst frame is
  1.8–6.5 ms, except one 49.7 ms frame on `spin-sepulcher-fte` (V2). The
  references hitch on `spin-tears`: 166 ms for ironwail and 203 ms for
  vkQuake.
- vkQuake 1.34.1 sits at 850–890 fps at 540p on every id1/MG1 scene, which
  looks like a fixed per-frame cost of about 1.1 ms in that build.

The last parity numbers recorded before this suite (1280×720, MSAA off, 8×
anisotropy, no md5, 2026-11-11) are in HISTORY.md and DESIGN.md (M7).
They are not comparable with these.

### Allocation census (V9)

**2026-11-18, commit ee5742f**, heaptrack 1.5 `--record-only`, headless
960×540, `ultra` pins, `spin-e1m1` (14999 frames, `-autoquit 211` so the
run ends a second after the demo; heaptrack cost: 773 fps against 758
without). 1.80 M allocations inside `Host::frame`, **120 per frame**;
about 6 of those are the map load (`process_client_events` → texture and
model loading, where the `read_dir`/`resolve_loose` calls are), which
leaves **~114 per frame in steady state**:

| allocs/frame | where |
|--:|---|
| 28.5 | `qrender::Renderer::render` (incl. its closures) |
| 21.0 | `Host::render` itself |
| ~25 | status bar and HUD text: `Sbar::draw_modern` 8, `draw_num` 7, `Canvas::push` 5.4, `ammo_pic` 2, `ptext_gold` 2, `face_name` 1 |
| 12.2 | `view_cvars` 8 and its 4 `format!` names for `gl_cshiftpercent_*` (3dca905) |
| 8.2 | `Host::frame` itself |
| 6.3 | `sync_shell_env` → `user_dir` (path joins every frame) |
| 3.1 | `wheel::inventory` |
| 2.1 each | `sync_language` (`Lang::from_code`), `autoaim_fov`, `ViewState::entities` |
| 1.6 | `build_world_draw` |
| 1.0 each | `draw_crosshair`, `osk::keys_for` |

None of these is large alone; together they are ~114 `malloc`/`free`
pairs per frame at 5000+ fps windowed. Most are per-frame `String`/`Vec`
builds that a reused buffer or a cached value removes (the status bar's
numbers and pic names, `user_dir`, the language code, the cshift cvar
names). Fix them in that order, re-run the census, and A/B with the
headless timedemo; the change must be pixel-identical.

### Parity stills

**2026-11-17**, after the water fix (`r_wateralpha`, see DESIGN.md) and
external world textures (V5). DSSIM, 1920×1080, `ultra` (`ultra-fte` for
the FTE view), vsync on:

| view | ours↔ironwail | ours↔vkQuake | ironwail↔vkQuake | verdict |
|---|--:|--:|--:|---|
| e1m1-start | 0.0024 | 0.0012 | 0.0023 | ok |
| e1m1-back | 0.0016 | 0.0006 | 0.0016 | ok |
| e1m1-grunts | 0.0036 | 0.0044 | 0.0030 | ok |
| e2m2-start | 0.0030 | 0.0024 | 0.0023 | ok |
| mg1-start | 0.0079 | 0.0053 | 0.0070 | ok |
| mg1-back | 0.0101 | 0.0123 | 0.0093 | ok |
| sepulcher-start | 0.0075 | 0.0114 | 0.0079 | ok |
| tears-waterfall | 0.0137 | 0.0136 | 0.0136 | ok |
| sepulcher-fte-start | 0.0119 | 0.0167 | 0.0117 | FTE, no verdict |

Before external textures, `sepulcher-start` was 0.0092 / 0.0131 and
`tears-waterfall` 0.0186 / 0.0185.

Before the fix, ours drew vanilla water opaque. ironwail and vkQuake
take the alpha from the map's `wateralpha` key, otherwise `r_wateralpha`
on maps whose VIS sees through water, and AD's progs sets `r_wateralpha`
from each map's `water_alpha`. Ours applied `r_wateralpha` only with
Enhanced Water's translucency on. ad_tears was 0.0286 from both
references until then.
