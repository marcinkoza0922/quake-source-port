# Performance optimisation plan

Findings from the `qprof` suite (2026-11-13, 1280×720 headless, RTX 3060 Ti,
single runs; noise about ±150 µs/frame). Function shares are of engine CPU
time during the measured window (`qprof -perf`). How to measure: see
`docs/DESIGN.md` (Tooling: qprof) and the perf-profiling skill.

Rules for every item:

- Output must not change. Take headless screenshots before and after and
  require `compare -metric AE` = 0 (visual-check skill). Items that change
  output on purpose (marked *visual*) need a side-by-side check instead.
- Measure before and after with the same qprof scenarios, at least
  `-runs 3`, and compare medians. Record results here with the date.
- One item per branch/PR.

## Baseline (to beat)

| scenario | map | fps | frame µs | p99 µs | GPU µs |
|---|---|--:|--:|--:|--:|
| vanilla | e1m1 | 798 | 1253 | 1750 | 1183 |
| vanilla | ad_sepulcher | 342 | 2928 | 3566 | 2861 |
| gel | e1m1 | 614 | 1629 | 18974 | 1196 |
| water | ad_sepulcher | 231 | 4327 | 11466 | 3138 |
| teleportwindows | ad_sepulcher | 242 | 4125 | 6579 | 3538 |
| all-enhancements | ad_sepulcher | 91 | 10936 | 18189 | 10870 |
| ultimate | ad_sepulcher | 79 | 12593 | 30894 | 12216 |
| ultimate-coop | ad_sepulcher | 44 | 22951 | 60488 | 16833 |

Headless frames include about 1.07 ms of image readback in `ui+present`;
confirm wins windowed (`qprof -window`, or qbench for parity).

## Phase 1: cheap, low-risk CPU wins

### 1. Look up settings by id, not by name
- Problem: `Cvars::find` lowercases the name into a new `String` and
  SipHashes it (`crates/qcore/src/cvar.rs:111`). The host calls `cv("…")`
  dozens of times per frame (`host/render.rs` render settings,
  `view_cvars`, `frame.rs`). About 10–15% of engine CPU on e1m1
  (`Cvars::find`, `hash_one<String>`, `view_cvars`).
- Plan: resolve the `CvarId`s once (a struct of ids registered at startup,
  like `qinput::InputCvars`) and read `value(id)`. Keep `find` for console
  use. As a stopgap, make `find` hash without allocating (case-insensitive
  hasher, or require lowercase names at registration).
- Expect: −100…150 µs/frame on e1m1, CPU-bound maps benefit most.
- Measure: `vanilla` on e1m1 and ad_sepulcher.

### 2. Stop allocating in `portal_surfaces`
- Problem: `World::portal_surfaces` (`crates/qrender/src/world/query.rs:138`)
  allocates `vec![false; surfaces.len()]` (the whole map) on every call,
  once per portal view, teleporter window and split-screen view per frame.
  Together with `build_world_draw` and `mark_leaves` it is ~45% of engine
  CPU with the portal gun and ~50% in 4-player co-op.
- Plan: keep a per-surface frame stamp (`Vec<u32>`) in `World` and bump a
  counter per call, as `cull.comp` does on the GPU; reuse the output `Vec`.
- Expect: large win in `portalgun`, `teleportwindows`, `ultimate-coop`.

### 3. Gel coat rebuild spikes
- Problem: each `REBUILD` interval `gel_fx::frame` rebuilds the whole coat
  (`src/addons/gel_fx.rs`): `gel_coat::group` compares every pair of discs
  (O(n²)) and value noise is evaluated per texel (`floorf` 13–17%). This
  is the 14–19 ms p99 in `gel`.
- Plan: (a) group with a spatial hash keyed by plane + cell so only nearby
  discs are compared; (b) rebuild only clusters whose discs changed; (c)
  precompute the noise into a small tiling texture, or evaluate it per
  vertex instead of per texel (*visual*).
- Expect: p99 under 3 ms on `gel`.

## Phase 2: the big ones

### 4. Particle system cost on AD (CPU and GPU)
- Problem: `qpart` (`System::frame`, `Geometry::particle`, `shade`,
  `Geometry::begin`) is 40–75% of engine CPU on every AD scenario,
  vanilla included, and the GPU `particles+vm` stage is ~1.64 ms of a
  2.86 ms frame on ad_sepulcher.
- First step (instrumented): profile JSON now includes live FTE particles,
  beams, decals and emitted vertices/indices; qprof reports their frame means
  and snapshots `r_partinfo` after measurement. `no-rain` disables sky rain
  for an A/B check. Sky rain is only a small share; identify the remaining
  effects before choosing a simulation/geometry optimisation.
- Candidate fixes, in order: skip simulating or building geometry for
  particles outside the frustum or PVS; cheaper `shade` (cache lighting per
  emitter or per cell instead of per particle, reusing the light-point
  cache); remove per-frame allocation in `Geometry::begin`; build quads in
  a vertex shader from one instance per particle (cuts CPU and upload);
  with large counts, simulate weather on the GPU.
- Expect: the largest single gain on AD maps (vanilla included).

### 5. Split-screen post-processing
- Problem: GPU `post` is 11.2 ms with 4 split views against 0.25 ms solo
  (about 44×). Post passes (SSAO, blur, screen shader) seem to run at
  full-screen size for each view.
- Plan: confirm with a per-view timestamp; then run each view's post at
  its viewport size, and run whole-screen passes (screen shader with
  `r_shader_scope 1`, goggles) once for the whole screen instead of per
  view.
- Expect: co-op from ~44 to well over 100 fps headless.

### 6. Supersampling combined with other enhancements
- Problem: all enhancements together cost far more than the sum of each
  one alone (+6.4 ms GPU on e1m1, +8 ms on AD; `translucent` reaches
  3–3.6 ms). SSAO alone adds 0.3 ms. Supersampling (`vid_fsaamode`) likely
  multiplies the cost of SSAO and the translucent/water passes.
- Plan: add pairwise scenarios to qprof (supersample+ssao,
  supersample+water); then run SSAO and similar effects at the base
  resolution and upscale (*visual*), or document the combination as
  expensive in the option tooltip.

### 7. Extra scene views (teleporter windows, portals)
- Problem: `teleportwindows` +1.3 ms CPU and +0.7 ms GPU on AD; each window
  re-walks the world on the CPU (`build_world_draw`).
- Plan: after item 2, cache each window's destination surface list (static
  destinations); cull windows by screen size and distance; consider the
  GPU cull path for portal views as for the main view.

## Phase 3: gameplay-scale and smaller items

### 8. Server spikes in large fights
- Problem: server p99 19–23 ms with the `sv_horde` load (mean 0.65 ms).
  `clip_to_links` and `Hull::recursive_check` lead the profile. Probable
  cause, not yet confirmed: all monsters think on the same server frame.
- Plan: confirm by counting QC thinks per server frame. If confirmed, find
  why thinks line up (spawn-time `nextthink`), and reduce trace cost
  (area-node tree sizes, cheaper box/hull rejection). Monster timing is
  gameplay: do not stagger thinks without checking qbot results.

### 9. Enhanced water and impacts on AD
- `water` +1.5 ms CPU and p99 11 ms; `impacts` +0.85 ms; `blood` +0.64 ms.
  Profile each alone with `-perf`; look for per-frame rebuilds of decal
  and ripple geometry that could be incremental.

### 10. Contents checks
- `check_water_transition` and `point_contents` take 4–8% on e1m1. Check
  for repeated checks of the same point per frame (client and server) and
  cache per entity per frame.

### 11. Aim-assist target search in co-op
- The input phase is 0.38 ms in 4-player co-op: `aim_targets` scans every
  edict per player. Only gather targets when a pad is used or autoaim is
  on (already partly gated), and share one candidate list between players.

### 12. Driver and synchronisation overhead
- The NVIDIA driver takes 13–38% and libc locking/memmove 10–37% of the
  process. Fewer draws and views (items 2, 4, 5, 7) reduce this; measure
  windowed, since the headless readback adds memmove and waits.

## Tooling follow-ups

- Run qprof with `-runs 3` for the before/after numbers of each item.
- Done: particle and vertex counters in the engine and qprof JSON (item 4).
- Add pairwise enhancement scenarios (item 6) and a per-view GPU timestamp
  for split-screen (item 5).
- Record a windowed baseline (`qprof -window`) once, for absolute numbers.

## Results log

(date, item, scenario, before → after, notes)

- 2026-11-13, item 1 (stopgap): `Cvars::find` now hashes the name
  case-insensitively with FNV-1a into an identity-hashed map, with no
  allocation or SipHash. `vanilla`/e1m1 `-perf`: lookup cost (`hash_one<String>`
  + `find` + SipHash) 39% → 14% of engine time. Frame time unchanged within
  noise headless (GPU/readback-bound). Screenshots pixel-identical. About 280
  lookups per frame remain (`deathmatch`, `addon_*`, `sv_gravity`, `fov`);
  resolving ids at startup is left for later, as the gain is now a few µs.
- 2026-11-13, item 2: `portal_surfaces` caches its surface list per eye
  leaf (16 entries, cleared with `set_transparent_liquids`) and dedupes with
  per-surface stamps instead of a map-sized `Vec<bool>`; portal views no
  longer reset the main view's `mark_leaves` cache. `ultimate-coop`/
  ad_sepulcher (3 runs): 44 → 51 fps, frame 22.5 → 19.5 ms, render CPU
  19.8 → 16.7 ms. `portalgun`/`teleportwindows` unchanged headless (GPU and
  readback bound). Screenshots identical except ≤ 0.008% pixels in co-op
  (live horde noise).
- 2026-11-13, item 3 (a, b, plus upload): the coat atlas is kept between
  rebuilds and each cluster's texels are cached by a hash of its square and
  splats, so a rebuild fills only changed clusters (600 splats: 27 → 0.7 ms
  after the first build). Texels whose summed field is below the lowest
  possible edge skip the noise; `noise` floors without libm; `group` uses a
  grid instead of comparing every pair. Most of the spike was the 16 MB atlas
  upload: `Renderer::update_pic_rows` now rewrites the image in place and
  uploads only the packed rows (the rest is cleared on the GPU). `gel`
  (3 runs): e1m1 p99 18.4 → 4.5 ms, fps 592 → 716; ad_sepulcher p99 14.3 →
  6.5 ms. Screenshots pixel-identical (apart from the console line printing
  the output path). Left: ~2 ms of `idle()` plus ~1 ms of submit wait per
  rebuild; removing them needs an asynchronous per-frame texture upload.
  (c), cheaper noise (*visual*), was not needed.

- 2026-11-13, item 4 investigation (1280×720 headless, RTX 3060 Ti,
  20 s measured after 4 s warm-up, 3 runs, 250 Hz simulation): added FTE
  counters and `no-rain` diagnostic; pinned classic FTE config and rain
  quantity 1. e1m1 vanilla/no-rain medians 771.6/771.2 fps, zero FTE particles.
  ad_sepulcher vanilla/no-rain: 321.0/314.4 fps, 3115/3181 µs frame,
  2924.50/2924.78 µs GPU, particles+vm 1682/1657 µs. Live particles
  33,622/31,766, generated vertices 134,960/127,728. Sky rain removes only
  5.5% of live particles and gives no frame-time win; it is not the main
  cause. Worldspawn maps `sky_seprain` to `fte_weather.tex_skyrainlit`.
  These are diagnostic measurements, not an optimisation or parity claim;
  no rendering algorithm changed. A separate 11 s spawn-point `r_partinfo`
  probe (inherited user graphics config, not the pinned combat benchmark)
  identifies `effectinfo.*DPP_ITSFCIRCLE` (~14,744 particles across its
  layers) and `effectinfo.*DPP_ITSLFLAME` (~10,492) as large populations.
  Follow-up below implements conservative behind-eye geometry rejection;
  broader visibility culling remains to investigate.
  Reports: `target/qprof/rain-investigation/`. Full tests: 444 passed,
  2 skipped; clippy clean. No screenshot comparison for this instrumentation
  change; the diagnostic intentionally removes sky rain.

- 2026-11-13, item 4 first optimisation: normal FTE billboards wholly
  behind the eye skip geometry generation using an oversized scale bound.
  The default Geometry sink remains unculled; the host enables rejection
  only when there are no gun portals and teleporter windows are disabled,
  because those views reuse the same geometry. Particle simulation,
  lighting/effects, RNG, beams and sparks retain their existing paths.
  ad_sepulcher, 1280×720 headless RTX 3060 Ti, pinned vanilla, 20 s measured
  after 4 s warm-up, three runs per build: median 282.6 → 396.0 fps (+40%),
  frame 3539 → 2525 µs, GPU 2938 → 2253 µs, particles+vm 1691 → 1003 µs.
  Live count unchanged at 33,622; vertices 134,960 → 77,743 (−42%).
  Nine fixed-time screenshots (5/10/15 s in all three runs) pixel-identical;
  inspected the 10 s image (pillar, rain and distant torch remain intact).
  Initial baseline overlapped compilation and was discarded; clean reports
  are `target/qprof/behind-before-clean` and `target/qprof/behind-after`.
  445 tests passed, 2 skipped; clippy clean. Windowed confirmation remains
  outstanding; these headless numbers include readback and are not a parity
  comparison. Next: conservative side-frustum rejection, then per-view
  geometry handling before extending culling to portal scenarios.

- 2026-11-13, item 4 follow-up: normal billboards also reject geometry
  wholly outside the top/bottom frustum planes, using the renderer's
  vertical half-FOV tangent and a conservative scale bound. Horizontal
  planes remain deferred until viewport/aspect calculation can be shared.
  Corrected the previous safeguard: split-screen guests also share the
  main camera's FTE geometry, so any guest now disables camera culling.
  Debug portal isolation likewise disables it because rendering overrides
  the host camera. An existing headless split-input test now verifies the
  guest/teleporter gating, and geometry tests cover both vertical planes,
  rotated quads, edge intersections, varied FOVs and unculling for sharing.
  ad_sepulcher, 1280×720 headless RTX 3060 Ti, pinned vanilla, 20 s measured
  after 4 s warm-up, 3 runs/build: median 390.4 → 401.5 fps (+2.8%, near
  run-to-run noise), frame 2562 → 2491 µs, GPU 2242 → 2198 µs,
  particles+vm 993 → 947 µs. Live particles unchanged at 33,622;
  vertices 77,743 → 73,406 (−5.6%). Nine combat screenshots pixel-identical,
  as are two pitched/rolled-camera shots and a four-player split-screen
  shot against the original unculling build. Inspected the tower/sky
  pitched view and the four torch-lit player views. Six headless visual
  runs with Vulkan validation enabled logged no validation errors. Reports: `target/qprof/vertical-before` and `vertical-after`.
  446 tests passed, 2 skipped; clippy clean. No windowed speedup claim.
