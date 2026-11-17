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
references can play them (step R3).

| Demo | Map | Size | What it stresses |
|---|---|---|---|
| `spin-e1m1` | id1 e1m1 | small | baseline, md5 grunts/dogs |
| `spin-e2m2` | id1 e2m2 | medium | more monsters, water |
| `spin-mg1` | mg1 start hub | medium | fog, KEX-era content |
| `spin-sepulcher` | ad ad_sepulcher | huge | world size, 1703 decorations if FTE is on |
| `spin-tears` | ad ad_tears | huge | 217k BSP nodes, PVS/culling |
| `combat-e1m3` | id1 e1m3 | small | rockets, explosions, gibs, dynamic lights, many md5 models on screen |

The spin demos are qbench's existing recording (god, 30°/s spin at the
spawn point). The combat demo comes from qprof's combat script (god,
`impulse 9`, rockets → super nailgun → super shotgun) with `record`
running.

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
  `r_fteparticles` is on. R3 therefore records AD demos with a stated
  `r_fteparticles` (with decorations for the FTE suite; decide for the
  main suite) and writes it into the demo's name, and every engine then
  plays back the same entities.
- **FTE, ours and vkQuake:** `spin-sepulcher` (AD's sky rain from
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

New items go here once the first baseline exists.

## Results

(date, commit, scene, resolution, engine, median fps / frame µs / p99 µs,
spread)

None yet. The last recorded parity numbers (1280×720, MSAA off, 8×
anisotropy, no md5, 2026-11-11) are in HISTORY.md and DESIGN.md (M7); they
are not comparable with this suite.
