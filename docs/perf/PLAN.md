# Performance plan, lean version (2026-11-21)

Replaces the two-part plan in [README.md](README.md) (Part 1
[VANILLA.md](VANILLA.md), Part 2 [ADDITIONS.md](ADDITIONS.md)) as the plan
we work from. Those files are kept for their tooling notes, settings
table, demo-recording details and open items. [HISTORY.md](HISTORY.md)
stays the results log from before 2026-11-17.

Why it changed: the renderer has gained a lot since 2026-11-17 (bloom,
real-time dynamic lights, planar and ray-traced water reflections,
HDR/deluxe lightmaps, sky wind), and the old plan's full runs took too
long. The full qbench pass alone took about 70 minutes, and Part 2 swept
every quality tier at two resolutions. This plan keeps the measurement
rules but cuts the matrix down.

## Shape

1. **Vanilla baseline**: a few demos of the base game, ours only.
2. **One scene per feature**: each enhancement (audio included) and each
   costly add-on, on its own, measured as the cost over the same scene
   with everything off.
3. **Everything on**: one split-screen co-op scene with every feature on,
   to see how they interact.
4. **Parity, after optimising**: ironwail and vkQuake comparisons
   (qbench, parity stills) only once the optimisation pass is done, to
   confirm we still match the references and haven't fallen behind them.

No reference engines run before step 4. That removes gamescope at 540p,
the per-engine settings pinning and most of the run time.

## Run rules

- **Two tiers of runs:**
  - **Quick** (during a change, target under 5 minutes): the vanilla
    demos plus the scene for the feature being changed, 3 runs each.
  - **Full** (before merging a perf PR, and at milestones, target under
    30 minutes): every scene below, 3 runs each.
- **Fixed length**: every scene is 10–20 s or a fixed number of frames.
  Leave out a warm-up (first 2 s) so pipeline compiles don't count, but
  report hitches over 33 ms separately.
- **Two resolutions**: 960×540 (CPU-bound) and 3840×2160 (GPU-bound,
  inside gamescope). Quick runs can use 540p alone when the change is
  CPU-only.
- **Medians of 3**, with the spread. A result only counts if it differs
  by more than 3% **and** by more than the spread (as in README.md).
- **Settings**: tiers are not swept. Each feature runs at its **default**
  tier and its **top** tier. More tiers only once a feature is shown to be
  expensive.
- **Hygiene**: perfkit's sandbox (no user config), `+sv_autosave 0`,
  windowed runs killed so `config.cfg` is never written, one engine
  process on the GPU at a time, governor `performance` if the user has set
  it (record it either way), no other GPU load.

## Metrics

For every run: mean and p50/p99/max frame time, hitches over 33 ms, CPU
and GPU split (host phases from `prof_start`/`prof_stop`, GPU stages from
`QSP_GPUTIMES`, in a separate attribution run). For feature scenes, the
**cost** = feature on minus everything off, in µs per frame and in % of
frame time. For the everything-on scene, add VRAM and resident memory.

## 1. Vanilla baseline

Ours only, vanilla ultra settings (VANILLA.md, Settings), timedemo of
demos recorded by `perfkit record`. Proposed set (4 demos, from the
existing 56238f2 set):

| Demo | Why |
|---|---|
| `combat-e1m3` | typical gameplay: fighting, weapons, monsters |
| `spin-e1m1` | typical id1 map, light load |
| `spin-sepulcher` | stress: big AD map, many entities |
| `spin-sepulcher-fte` | stress: FTE particle rain |

Timedemo runs no server or QuakeC, so server cost shows up only in the
live scenes below.

## 2. Feature scenes

Each feature runs alone, on top of the vanilla baseline, in a scene built
to stress it. The "off" run uses the same scene with the feature off.

Where the feature is client-side and replays from a demo, use a demo:
it's the most repeatable. Where it doesn't (add-on state, audio,
split-screen), use a **scripted live scene**: fixed map, `setpos` or
`freecam x y z pitch yaw` for the camera, `portal_set` and spawned
entities for state, `-fixedstep`, fixed frame count.

### Enhancements

| Feature | Enable | Scene | Stresses |
|---|---|---|---|
| Bloom | `r_bloom 1` | `spin-sepulcher` | full-screen passes (4K) |
| Real-time dynamic lights | `r_shadow_realtime_dlight 1` | `combat-e1m3` + rocket/plasma spam | shadow passes per light |
| Water reflections, planar | `r_enhancedwater 1`, `r_water_reflections 1` | e2m2 or AD water view | extra scene pass |
| Water reflections, ray traced | `r_water_reflections 2` | same view | ray queries, BLAS/TLAS updates |
| Enhanced water (rest) | translucency, ripples, splashes, caustics | same view + entities in water | |
| SSAO | `r_ssao` | `spin-sepulcher` | 4K |
| Motion + speed blur | `r_motionblur 1`, `r_speedblur 1` | `combat-e1m3` | |
| Blood, impacts, enhanced particles | `r_enhancedfx 1` + each toggle | `combat-e1m3` | particle counts, CPU |
| Ragdolls & debris | `r_debris 1` | `combat-e1m3`, explosions | CPU physics |
| Teleporter windows | `r_teleportwindows 1` | `start` | extra views |
| HDR / deluxe lightmaps | `r_hdr_lightmaps`, `r_deluxemapping` | a map with `.lit2`/BSPX E5BGR9 | |
| Sky wind | `r_skywind 1` | a map with `<sky>wind.cfg` | |

Pin every master toggle (`r_enhancedfx`, `r_enhancedwater`, ...) in the
baseline, so a feature can't be on or off by accident (ADDITIONS.md
found `r_enhancedfx` unpinned).

### Audio

Timedemo runs faster than real time and doesn't model sound load, so
audio has its own **real-time** scene: a live, scripted fight (many
monsters, explosions, ambient sounds) in a room with echo, played at
normal speed for 20 s.

| Feature | Enable |
|---|---|
| Reverb | `s_reverb 1` |
| HRTF | `s_hrtf 1` |
| Occlusion | `s_occlusion 1` |
| Underwater filter | `snd_waterfx 1`, scene under water |

Metrics: mixer CPU time per audio buffer (mean and max), main-thread time
in the acoustic probes (`src/audio/acoustic.rs`), and underrun count. If
the mixer doesn't expose its time per buffer yet, adding that counter is
the first step (P2 below).

### Add-ons

Customized game, one add-on at a time. Proposed as the costly ones; the
rest (railgun, hookshot, sticky, boomerang, bullet time, dodge, hitstop)
only show up in the everything-on scene:

| Add-on | Scene |
|---|---|
| Portal gun | e1m1, fixed portals via `portal_set`: 2 portals at default depth, then 4 at depth 6 |
| Gel sprayer | e1m1, spraying both gels for 20 s; report p99 and max (coat rebuild spikes) |
| Cable | rope tether wrapped around several corners, yanking |
| Powerwasher | spraying walls and monsters |
| Goggles | night-vision and thermal on, `spin-sepulcher` view |

## 3. Everything on

One scene: **split-screen co-op, 4 players**, ad_sepulcher (or the
heaviest map from step 1), every enhancement at its default tier, the
add-ons above in use (portals placed, gel sprayed, cable out), the audio
features on, a scripted fight in progress, 20 s live, fixed step.

Report its cost over the vanilla 4-player run, and the **excess**: that
cost minus the sum of the single-feature costs. A large excess means
features interact.

When the result is bad, run **leave-one-out**: everything on minus one
feature, one run per feature. That points at the interaction without
testing every pair. A pair that stands out gets its own scene.

(A 2-player run is optional. Run it only if 4 players can't be made
stable.)

## 4. Parity (after optimising)

qbench's main and FTE suites and `perfkit stills`, as described in
VANILLA.md, on the same demos as step 1. Kept unchanged until then, so
the comparison runs on the same inputs.

## Steps

- **P0. Scene list.** One file (`tools/perfkit`, for example
  `scenes.toml`) listing each scene: kind (demo or live), map, camera,
  cvars, setup commands, frame count, group (`vanilla`, `enhancement`,
  `audio`, `addon`, `all`). Adding a feature then means adding one entry.
- **P1. One runner.** `perfkit run [-quick|-full] [-only a,b] [-res
  540|2160]`: plays each scene's off and on runs interleaved, writes
  `target/perf/runs/<commit>/report.{json,md}` with the cost table.
  Builds on qprof's live-scene code (`tools/qprof/src/scenarios.rs`,
  `ultimate.rs`) and perfkit's sandbox. qprof's old scenarios go away.
- **P2. Audio counters.** Mixer time per buffer, probe time, underruns,
  readable at the end of a run.
- **P3. Demo check.** Confirm which enhancement scenes render the same
  from a demo and live (ADDITIONS.md, A0). The rest stay live.
- **P4. First full run**, saved as the baseline
  (`-save-baseline`). Each feature gets a budget from it (for example,
  default tier at most X% of the 4K frame).
- **P5. Optimise**, items in order of measured cost. Open items carried
  over: V1/V2/V7/V9/V10 (VANILLA.md), H5/H7/H11 (ADDITIONS.md). Vanilla
  optimisations stay pixel-identical (AE = 0).
- **P6. Parity** (step 4), then update DESIGN.md Tooling and the
  perf-profiling skill.

## Results

(date, commit, scene, resolution, off / on / cost µs and %, p99, spread)

None yet.
