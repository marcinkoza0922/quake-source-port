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
| our enhancements and add-ons off | `r_ssao 0`, `r_motionblur 0`, `r_speedblur 0`, `r_enhancedwater 0`, `r_teleportwindows 0`, `r_enhancedfx 0`, `r_enhancedparticles 0`, `r_particle_quality 1`, `r_impact_fx 0` (+ bullets, nails, scorches, tracers), `r_blood 0`, `r_debris 0`, `r_softemu 0`, `r_dither 0`, `r_colorblind 0`, `custom_enable 0`, `addon_hitstop 0` | – | – |
| particles, `ultra` | `r_fteparticles 0`, `r_particles 2` | `r_particles 2` | `r_fteparticles 0`, `r_particles 2` |
| particles, `ultra-fte` | `r_fteparticles 1`, `r_particledesc classic`, `r_softparticles 0`, `r_quadparticles 1`, `r_particles 2` | `r_particles 2` | same without `r_softparticles` (1.34.1 has none) |

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

How they're recorded: our engine headless in the sandbox (`XDG_DATA_HOME`
there, `+sv_autosave 0`, 320×240, `-fixedstep 250`), so nothing outside
`target/` is touched; all seven take about 4 minutes. The server ticks at
72 Hz, which gives ~62.5 demo frames per game second whatever the step:
~15000 frames per spin, ~7500 for combat. Spins are qbench's script (god,
30°/s spin at the spawn point); combat is qprof's (god, `impulse 9`
refilled every 20 s, 40°/s spin firing rockets → super nailgun → super
shotgun). `sv_fteparticles` and `r_fteparticles` are pinned while
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
  `_texpart_*` emitters) and `combat-e1m3`, both with `r_fteparticles 1`,
  `r_particledesc classic`. Before comparing, check with `r_partinfo` (both
  engines have it) that live particle, beam and decal counts are within a
  few percent of each other. Otherwise the workloads differ and the fps
  comparison is meaningless. ironwail is listed as "classic fallback, not
  comparable".

If the combat demo turns out too light to stress particles, record a
denser one (more monsters, more explosions) in a vanilla map rather than
using `sv_horde`, which is a gameplay modifier.

## Visual parity

Fixed camera positions, 2–3 per map, chosen for what can go wrong: an md5
monster close-up, fog with sky, water/lava/slime, coloured lighting, a
fence texture, and, for the FTE suite, AD rain.

- Capture each position in all three engines at 1920×1080 (step R5). The
  capture method has to be settled first. DESIGN.md notes that `+map` is
  ignored by vkQuake on AD (no `stuffcmds` in AD's quake.rc; work around
  it with an overlay `quake.rc`). Scripted waits don't work in vkQuake.
  Candidates: a per-run autoexec in the sandbox overlay dir that runs
  `setpos` + `screenshot`, or window capture with `spectacle`.
- Mask the HUD; prefer views without moving entities, and for model
  close-ups freeze time if every engine supports it.
- Metric: SSIM (ImageMagick `compare`). The references differ from each
  other too, so the allowed band is set by them: our distance to each
  reference must not exceed the ironwail↔vkQuake distance by more than a
  margin set from the first baseline. Every run also writes a side-by-side
  HTML gallery for a human look.
- Inside our engine the old rule still applies: an optimisation keeps
  `compare -metric AE` = 0 on these stills.

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

- **V1**: `spin-sepulcher` at 540p is the one scene where we aren't ahead
  (ironwail +2%, within noise). It is the scene where AD sends classic
  particle messages every frame, so this is H4 in its classic form, and it
  is CPU work: at 4K we lead by 45%.
- **V2**: one 49.7 ms cold-start hitch on `spin-sepulcher-fte`, the only
  frame over 33 ms in our cold runs. It is probably the FTE particle
  pipelines or textures created on first use; find out and create them at
  load. `combat-e1m3`'s worst cold frame (6.5 ms) is also twice the
  references'.
- **V3**: the r_partinfo particle-count check (Particle suite) is still to
  do before the FTE numbers count as a like-for-like comparison.

## Results

**2026-11-17, commit 6fcd456, demo set `e53154d-dirty`.** qbench
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
