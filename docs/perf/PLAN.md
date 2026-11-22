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
| Underwater refraction (new 2026-11-21) | `r_water_refraction 1` | camera held under water (e2m2 or AD), `r_water_quality` 1 and 2 (2 adds the chromatic split) | full-screen post work (4K) |
| Screen droplets (new 2026-11-21) | `r_water_droplets 1` | scripted live: dip in and out of water every ~3 s, so the 2 s droplet window covers most frames | full-screen post work (4K); per-view state in split-screen |
| SSAO | `r_ssao` | `spin-sepulcher` | 4K |
| Motion + speed blur | `r_motionblur 1`, `r_speedblur 1` | `combat-e1m3` | |
| Blood, impacts, enhanced particles | `r_enhancedfx 1` + each toggle | `combat-e1m3` | particle counts, CPU |
| Ragdolls & debris | `r_debris 1` | `combat-e1m3`, explosions | CPU physics |
| Teleporter windows | `r_teleportwindows 1` | `start` | extra views |
| HDR / deluxe lightmaps | `r_hdr_lightmaps`, `r_deluxemapping` | a map with `.lit2`/BSPX E5BGR9 | |
| Sky wind | `r_skywind 1` | a map with `<sky>wind.cfg` | |

The two water screen effects (refraction, droplets) are passes inside the post shader (`water_screen.glsl`
/ `water_screen.wgsl`) and should cost little while inactive, so measure them
in frames where they are active, and check once what they add on a
dry view. They also belong in the everything-on scene with some players
under water.

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

- **P0. Scene list.** Done 2026-11-21 as `perfkit::plan` (Rust table, not TOML: perfkit has no dependencies and the tests check the cvars against the engine source). Planned as one file (`tools/perfkit`, for example
  `scenes.toml`) listing each scene: kind (demo or live), map, camera,
  cvars, setup commands, frame count, group (`vanilla`, `enhancement`,
  `audio`, `addon`, `all`). Adding a feature then means adding one entry.
- **P1. One runner.** Done 2026-11-21 (`perfkit run`, DESIGN.md Tooling): every scene above runs except `hdr-deluxe` (no installed map has HDR or deluxe lightmaps). Demo runs get GPU time from a profiled attribution run (`prof_timedemo`); live runs from their own profile. Left: the everything-on scene's excess column and leave-one-out runs (the single-feature costs come from other scenes, so their sum isn't comparable yet), and retiring qprof's scenarios (a decision for the user: qprof still has `-perf` per-function profiles).
- **P2. Audio counters.** Mixer time per buffer, probe time, underruns,
  readable at the end of a run.
- **P3. Demo check.** Confirm which enhancement scenes render the same
  from a demo and live (ADDITIONS.md, A0). The rest stay live.
- **P4. First full run**, saved as the baseline
  (`-save-baseline`). Each feature gets a budget from it (for example,
  default tier at most X% of the 4K frame). Run 2026-11-21 (Results); budgets still to be agreed.
- **P5. Optimise**, items in order of measured cost. Open items carried
  over: V1/V2/V7/V9/V10 (VANILLA.md), H5/H7/H11 (ADDITIONS.md). Vanilla
  optimisations stay pixel-identical (AE = 0).
- **P6. Parity** (step 4), then update DESIGN.md Tooling and the
  perf-profiling skill.

## Results

(date, commit, scene, resolution, off / on / cost µs and %, p99, spread)

- 2026-11-21, 07b9663-dirty, **one round only (no spread, preliminary)**,
  RTX 3060 Ti, i7-12700K. Vanilla demos, mean ms 540p / 4K: spin-e1m1
  0.219 / 1.050, combat-e1m3 0.237 / 1.073, spin-sepulcher 0.414 / 1.254,
  spin-sepulcher-fte 1.284 / 1.993 (FTE rain triples the 540p frame of the
  same map). Live scene costs, 540p / 4K: water-refraction +4 / +34 µs,
  water-droplets +15 / +218 µs (+17% at 4K, GPU 1223 → 1438 µs: about 6×
  refraction), teleporter-windows +126 / +315 µs (4K p99 1.07 → 2.43 ms),
  skywind +7 / +22 µs.
- The `spin-e2m2` demo showed no water, so the water reflection and
  "rest" scenes moved to a live camera over e2m1's pool (2026-11-21).
- 2026-11-21, same build, one round (preliminary): live costs 540p / 4K.
  water-planar +68 / +240 µs; **water-rt +208 / +1099 µs (+96% at 4K, p99
  1.99 → 4.67 ms)**; water-rest +6 / +16 µs; `r_water_quality 2` changes
  none of the water costs. portalgun +95 / +228 µs, recursion 6: +154 /
  +386 µs. gel +18 / +200 µs (4K p99 1.97 → 2.89 ms); washer −1 / +55 µs;
  cable +55 / +240 µs; goggles nv and thermal about +100 µs at 4K, under
  3%. The ad_sepulcher spawn live costs 1.85 / 3.4 ms with nothing on, far
  more than the `spin-sepulcher` timedemo: the live game has AD's
  decorations (see DESIGN.md).
- 2026-11-21, one round (preliminary): audio scenes show no frame-time
  cost (reverb, HRTF, occlusion, underwater filter all within ±2%), since
  mixing runs on its own thread; P2's counters are needed. **all-split4**
  (4 players, everything on) 1.59 → 4.24 ms at 540p and 2.47 → 8.29 ms at
  4K (120 fps; GPU 2.3 → 8.2 ms; p99 3.65 → 11.64 ms). Demo GPU per frame
  (attribution runs) 540p / 4K: spin-e1m1 143 / 833 µs, combat-e1m3
  157 / 914, spin-sepulcher 228 / 1167, spin-sepulcher-fte 946 / 2088.
- 2026-11-21, 447d83b, first 3-round quick run (vanilla demos), mean ms
  540p / 4K with spread: spin-e1m1 0.228 ±0.001 / 1.067 ±0.045,
  combat-e1m3 0.249 ±0.001 / 1.197 ±0.089, spin-sepulcher 0.421 ±0.012 /
  1.416 ±0.029, spin-sepulcher-fte 1.356 ±0.010 / 2.279 ±0.026. 540p is
  tight (under 3% spread); 4K combat spreads 7%, so 4K costs below that
  won't count. The quick tier took **9 minutes** against the 5-minute
  target (24 timed runs, 4 warm-ups, 8 attribution runs, ~15 s each, mostly
  startup and map load): candidates are 540p-only quick runs for CPU-only
  changes (as planned), shorter spin demos, and `-noattrib`.
- **2026-11-21, 7f66f0f: first full run (P4 baseline)**, 3 rounds, 71 min
  (target 30), no failures; report kept in `target/perf/baseline/`.
  Cost over off, 540p / 4K, significant unless marked:
  - SSAO: on +53 / +900 µs; **top (`r_ssao 3`) +140 / +5975 µs** (4K
    frame 1.43 → 7.41 ms, p99 11.75 ms).
  - **Bloom +89 / +1554 µs** (4K +109%).
  - Water: ray-traced reflections +212 / +1168 µs; planar +67 / +264;
    droplets +16 / +429; refraction and the rest within noise.
  - Teleporter windows +141 / +369; portal gun +98 / +232, recursion 6
    +158 / +394.
  - Real-time dlights +52 / +141, top +59 / +325; motion blur +13 / +113;
    enhanced particles +58 / +115; sky wind +10 / +22 (4K not sig.).
  - Gel +19 / +232; cable +56 / +274; washer +1 / +64; goggles about +100
    to +220 at 4K only.
  - Blood, impacts and debris: within noise on `combat-e1m3` (≤ +74 µs at
    4K). Suspicious for a rocket fight: check in P3 that these effects
    replay from the demo at all.
  - Audio: nothing on the main thread (P2 counters needed).
  - all-split4: +2693 / +5828 µs (4K 2.50 → 8.33 ms, GPU 2.3 → 8.2 ms).
  P5 order by 4K cost: SSAO top, bloom, ray-traced water, SSAO on,
  droplets, portal recursion, teleporter windows, real-time dlights top;
  at 540p (CPU): ray-traced water, portal recursion, teleporter windows,
  SSAO top, bloom.
