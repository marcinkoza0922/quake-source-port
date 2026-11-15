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

## Priority diversion: enhancements and add-ons (*visual*)

Added 2026-11-14. Visual and logic parity with reference ports for vanilla
Quake and third-party map packs remains strict (no compromises). For this
port's own enhancements and add-ons, appearance-altering optimisations are
acceptable when they bring meaningful CPU/GPU gains; changes that noticeably
degrade look should expose quality settings (e.g. `Low` / `Medium` / `High`)
rather than forcing a single degraded mode. These items take priority over
the remaining parity-bound phases below.

### 13. SSAO resolution and quality tiers (*visual*)
- Problem: `SSAO` in `src/menu/pages.rs` (`off` 0, `low` 0.5, `medium` 1.0,
  `high` 2.0) currently scales only occlusion intensity (`pc.size.z` in
  `crates/qrender/shaders/ao.frag`). All three non-off levels allocate
  `t.ao` at full `scene_w × scene_h` (`crates/qrender/src/lib/targets.rs`),
  run `N = 12` depth taps per pixel in `ao.frag`, and run a 4×4 (16-tap)
  bilateral blur in `aoapply.frag`. Even after the sample-0 snapshot, AO
  costs ~1.86–2.28 ms at 200% scale (1.5 ms across 4 views in `ultimate-coop`).
- Plan:
  - Add real quality tiers (decoupling quality from intensity, or mapping
    presets cleanly): half-resolution `t.ao` with depth-aware upsample in
    `aoapply.frag` (4× fewer `ao.frag` pixels), capped at output resolution
    when `r_renderscale > 1`.
  - Scale `ao.frag` sample count `N` (e.g. 4–6 / 8–10 / 12) and `aoapply.frag`
    blur taps by quality tier.
- Measure: `ssao`, `supersample-renderscale200-ssao`, `all-enhancements`,
  `ultimate-coop`.
- **Status (Done)**:
  - Reworked `r_ssao` to `0` = off, `1` = low, `2` = medium, `3` = high (menu `Ambient Occlusion`: `off` / `low` / `medium` / `high`), separated `r_ssao_strength` (default `2`, matching the prior `high` intensity), added `ssao_quality` to `qrender::Settings`, and updated `qprof`'s `SSAO` preset to `r_ssao 3`.
  - Capped `t.ao` and MSAA `t.ao_source` at output resolution when `r_renderscale > 1`, halved them on `low` and `medium`, scaled `ao.frag` taps (`6` with 2×2 Bayer on `low`, `8` on `medium`, `12` on `high`), and updated `aoapply.frag` to read full-res scene depth (sample 0 under MSAA) with sky early-out, split-view clamping, and `2×2` (`low`) vs `4×4` (`medium`/`high`) bilateral upsampling.
  - Verified with `QRENDER_VALIDATION=1 cargo nextest run -p qrender` (including quality cycling across 1x/4x MSAA, 100%/200% scale, and split-screen), `QRENDER_VALIDATION=1` headless screenshots (`e1m1`, `ad_sepulcher`, 200% + 8x MSAA, 4-player split-screen), and `qprof -runs 3` medians (`target/qprof/before-ssao` → `target/qprof/after-ssao`, 1280×720):
    - `renderscale200-ssao`: `ad_sepulcher` GPU **3191 → 2631 µs** (-560 µs, FPS 307 → 368; AO total **1127 → 480 µs**, `ao-generate` **946 → 296 µs**); `e1m1` GPU **2558 → 1766 µs** (-792 µs, FPS 380 → 546).
    - `supersample-renderscale200-ssao`: `ad_sepulcher` GPU **5202 → 4114 µs** (-1088 µs, FPS 190 → 237; AO total **1835 → 607 µs**, `ao-generate` **1475 → 233 µs**); `e1m1` GPU **4275 → 2754 µs** (-1521 µs, FPS 230 → 354).
    - `all-enhancements`: `ad_sepulcher` GPU **7906 → 6645 µs** (-1261 µs, FPS 126 → 149; AO total **1827 → 556 µs**); `e1m1` GPU **5662 → 4206 µs** (-1456 µs, FPS 174 → 233).
    - `ultimate-coop` (`ad_sepulcher`, 4 views): GPU **11000 → 10278 µs** (-722 µs, FPS 89 → 95; AO total **1449 → 722 µs**).
    - Tier scaling on `ad_sepulcher` (499-frame spawn `prof_stop` AO total / `ao-generate`): 100% scale `low` **112.7 / 24.7 µs**, `medium` **131.4 / 31.5 µs**, `high` **291.0 / 184.8 µs**; 200% + 8x sample shading `low` **245.1 / 24.8 µs**, `medium` **302.3 / 31.8 µs**, `high` **527.2 / 186.7 µs**.

### 14. Add-on portals & teleporter window target quality (*visual*)
- Problem: `destinations` is the largest GPU stage in `ultimate-coop`
  (~4.3 ms GPU, ~3.35 ms in `world`, ~0.66 ms in `particles`). Each portal
  view (`crates/qrender/src/frame/portals.rs`, up to `MAX_PORTAL_VIEWS = 8`)
  renders into full `scene_w × scene_h` targets (`t.portals` in `targets.rs`)
  with full scene MSAA (`t.color_ms`), full water ripples/caustics, and all
  particles/FTE batches.
- Plan:
  - Render portal views at reduced scale (e.g. 0.5× or 0.75× output res)
    and single-sampled (no MSAA scratch/resolve), with quality tiers
    controlling resolution scale and `addon_portal_depth` / `addon_portal_max`
    / `addon_portal_dist` budgets.
  - Provide an option/tier to skip expensive extras inside destination views
    (especially nested depth ≥ 1): `water_grad` ripples/caustics, classic/FTE
    particles, or distant small alias models.
- Measure: `portalgun`, `teleportwindows`, `ultimate`, `ultimate-coop`.
- **Status (Done)**:
  - Added `r_portal_quality` (`0` = low, `1` = medium [default], `2` = high; Enhancements → `"Portal Quality"` and `qrender::Settings::portal_quality`).
  - Rendered destination views (`t.portals` in `crates/qrender/src/lib/targets.rs`) single-sampled (`samples = 1`, dedicated `portal_depth` and single-sampled `portal` scene pipelines in `crates/qrender/src/pipelines.rs`, no MSAA scratch/resolve or sample shading) at output-capped `min(scene, output)` scale (`0.5×` `low`, `0.75×` `medium`, `1.0×` `high`) with `sampler_linear` when scaled and `FrameUbo::portal_uv` screen-space UV scaling/clamping in `water.frag` / `fte.frag`.
  - Applied per-tier destination budgets (`low`: `max <= 2, depth <= 1, dist <= 2048`; `medium`: `max <= 4, depth <= 3, dist <= 4096`; `high`: uncapped), skipped `water_grad` ripples/caustics (`pu.water = 0`) and non-oval particles (`low`: all destination views; `medium`: nested `depth >= 1`), and culled alias draws per portal against the portal frustum/clip plane and tiny/distant bounds (preserving main-view `main_visible`). Pinned `r_portal_quality 1` in `qprof` `VANILLA` and `2` in `ADDON_ON`.
  - Verified with `QRENDER_VALIDATION=1 cargo test -p qrender --test offscreen`, `QRENDER_VALIDATION=1 cargo nextest run`, `cargo clippy --all-targets --message-format=short`, headless screenshots (`e1m1` recursive portals `0/1/2` + 8× MSAA/200%, `start` teleporter at 8× MSAA/200%, `ad_sepulcher` 4-player split-screen), and `qprof -runs 3` medians (`target/qprof/item14-before-{ad,e1m1}` → `target/qprof/item14-after-{ad,e1m1}`, 1280×720):
    - `teleportwindows` (`ad_sepulcher`, `r_portal_quality 1`): FPS **270 → 283**, frame **3701 → 3534 µs** (−167 µs), GPU **3623 → 3474 µs** (−149 µs); `destinations` **588 → 524 µs** (`models` **61 → 13 µs**, `resolve` **19 → 2 µs**).
    - `ultimate` (`ad_sepulcher`, 8× SSAA + 200% scale, `r_portal_quality 2`): FPS **108 → 116**, frame **9217 → 8639 µs** (−578 µs), GPU **8919 → 8387 µs** (−532 µs); `destinations` **677 → 582 µs** (`models` **73 → 13 µs**, `resolve` **36 → 2 µs**, `setup` **10 → 2 µs**).
    - `ultimate-coop` (`ad_sepulcher`, 4 views, `r_portal_quality 2`): FPS **94 → 97**, frame **10620 → 10324 µs** (−296 µs), GPU **10313 → 10032 µs** (−281 µs); 4-view `destinations` **1172 → 964 µs** (`models` **245 → 86 µs**, `resolve` **53 → 10 µs**).
    - `portalgun` (`e1m1`, `r_portal_quality 2`): FPS **746 → 752**, frame **1341 → 1329 µs** (−12 µs), GPU **1266 → 1259 µs** (−7 µs); CPU `render` **290 → 254 µs**.
    - Tier scaling (`prof_stop`, 1280×720, 8× SSAA + 200% scale): `ad_sepulcher` (`r_teleportwindows 1` + enhanced water, 500 frames) `destinations` **142.8 µs** (`low`, GPU **6045.7 µs**) / **1858.0 µs** (`medium`, **7760.1 µs**) / **1856.1 µs** (`high`, **7767.3 µs**); `e1m1` (`addon_portal_depth 4`, 750 frames) `destinations` **70.7 µs** (`low`, GPU **2950.9 µs**) / **107.6 µs** (`medium`, **2987.8 µs**) / **127.6 µs** (`high`, **3008.1 µs**).

### 15. Enhanced water caustics, ripples & splashes (*visual*)
- Problem: in `crates/qrender/shaders/common.glsl`, `water_grad` loops over
  up to `MAX_RIPPLES = 32` active ripples per fragment in both `water.frag`
  and `world.frag` (to distort caustics), and `caustic()` evaluates two 3×3
  Worley cellular layers (18 cell checks per submerged world pixel). At
  `r_renderscale 2` + 8× sample shading, `water` adds +631 µs GPU on
  `ad_sepulcher` (+443 µs in `world`).
- Plan:
  - Quality tiers for caustics/ripples: Low/Medium use a precomputed tiling
    caustic texture (two scrolling texture taps instead of dual Worley) and
    skip `water_grad` distortion on `world.frag` caustics; High retains dual
    Worley.
  - Add distance fade cutoffs in `water.frag`/`world.frag` to skip ripple and
    caustic loops past sub-pixel range, and cap active ripples (`8`/`16`/`32`)
    plus `WaterFx` splash `MAX_DROPS` (`2048`) / `MAX_PUFFS` (`512`) in
    `crates/qrender/src/water.rs` by quality.
- Measure: `water`, `supersample-renderscale200-water`, `all-enhancements`.

### 16. Per-object motion blur quality (*visual*)
- Problem: in `crates/qrender/shaders/post.frag`, whenever any moving entity
  populates `vel_draws`, every static background pixel (`best < 0.25`) runs
  an 8-tap 24 px neighborhood search in `vel_tex`, and blurred pixels take
  `N = 12` scene taps; `frame.rs` also re-renders moving models into `t.vel`.
- Plan:
  - Add quality tiers: Low skips the 8-tap dilation search on static pixels
    and reduces blur taps (e.g. `N = 6`); High keeps the dilated smear (or
    replaces the 8-tap per-pixel search with a coarse tile-max velocity
    pass).
  - Skip `vel_draws` submission for tiny/distant entities with sub-pixel
    screen motion.
- Measure: `motionblur`, `all-enhancements`.

### 17. Soft particles MSAA depth lookup (*visual*)
- Problem: in `crates/qrender/shaders/fte.frag`, under MSAA (`ns > 1`), every
  soft-particle fragment loops `0..ns` (up to 8 `texelFetch`es on
  `scene_depth_ms`), and any soft batch splits dynamic render passes in
  `frame.rs`.
- Plan: add a fast/low-cost mode reading sample 0 only under MSAA (or
  reusing `t.ao_source` when available) instead of all `ns` samples.
- Measure: `supersample`, `all-enhancements` on `ad_sepulcher`.

### 18. Gel sprayer coat build, upload & shading (*visual*)
- Problem: even after item 3, `gel` has 4.5–6.5 ms p99 spikes from
  `Renderer::update_pic_rows` calling `self.idle()` (~3 ms stall every
  `REBUILD = 0.1 s` while spraying), `gel_coat.rs` evaluating 3 octaves of
  3D value noise across `2048×2048` (`TEXEL = 2.0`), and `fte.frag` mode 9
  performing parallax + 4 neighbor height taps + dual Phong + Fresnel per
  fragment.
- Plan:
  - Replace the synchronous `self.idle()` in `update_pic_rows`
    (`crates/qrender/src/lib/assets.rs`) with per-frame staging/ring upload.
  - Add quality tiers for `gel_coat.rs` / `gel_fx.rs` / `fte.frag` mode 9:
    coarser coat texel size (`TEXEL = 4.0`, `1024×1024` atlas) and longer
    `REBUILD` cadence (`0.2 s`), tiling noise LUT or smooth edge without
    3D noise, and a fast gel shader path without parallax + 4-tap gradient
    fetches.
- Measure: `gel`, `all-addons`, `ultimate`.

### 19. Powerwasher jet geometry & shading (*visual*)
- Problem: `src/addons/water_fx.rs` builds 40 ribbon segments + 40 loose
  drops per jet (200 segments + 200 drops in 5-jet fan mode, plus up to 400
  splash drops), all shaded with `gel: true` (`fte.frag` mode 9).
- Plan: reduce `SEGMENTS`/`DROPS` (e.g. 40 → 12–16) and switch washer jets
  and tiny spray droplets off full 5-tap parallax `gel: true` shading onto a
  cheaper translucent water batch (or gate full `gel` shading behind High).
- Measure: `washer`, `all-addons`, `ultimate`.

### 20. Ragdolls & debris simulation quality (*visual*)
- Problem: `src/debris/` runs 60 Hz Verlet (`ITERS = 6`, `MAX_BONES = 12`)
  with 2 BSP hull traces per bone per step (plus `Scene` mover traces),
  shape-matching polar decomposition, and CPU linear-blend skinning + MDL
  repacking (`pack()`) every awake frame for up to 48–64 ragdolls.
- Plan: add quality tiers (`Low` / `Medium` / `High`) scaling `MAX_BONES`
  (e.g. 6–8 vs 12), `ITERS` (3 vs 6), sim rate (30 Hz vs 60 Hz), downward
  clearance trace skipping, `r_debris_movers`, lower corpse/gib/shard caps
  with fade-out lifetimes, and frustum-culled CPU skinning (`pack()`).
- Measure: `debris`, `ultimate`, `ultimate-coop`.

### 21. Blood & impact decal budgets (*visual*)
- Problem: while item 9 removed idle CPU overhead, max settings still allow
  up to 2048 blood marks (`r_blood_amount 12`), 2048 impact marks
  (`r_impact_fx 4`), and 600 s lifetimes, driving heavy overdraw and mesh
  wound work in sustained horde fights.
- Plan: expose quality presets (`Low` / `Medium` / `High`) scaling mark
  caps, lifetimes, droplet/spark counts, and `r_blood_wounds` /
  `r_blood_gibs` mesh painting, plus frustum culling on decal emission.
- Measure: `blood`, `impacts`, `ultimate`, `ultimate-coop`.

### 22. Slang screen-shader input scale (*visual*)
- Problem: `Slang::prepare_shader` (`crates/qrender/src/slang.rs`) allocates
  `Slang::src` at `scene_w × scene_h`, so `r_renderscale 2` quadruples the
  input processed by multi-pass `.slangp` CRT presets.
- Plan: feed `slang` at output resolution (or 1× scale) when
  `r_renderscale > 1` (at least for `r_shader_scope 0`), or provide a toggle
  for preset resolution scaling.
- Measure: `screenshader` + `renderscale200`, `all-enhancements`.

### 23. Goggles post-process glow (*visual*)
- Problem: `goggles()` in `crates/qrender/shaders/post.frag` executes an
  8-tap ring glow per output pixel for both night vision and thermal modes
  (plus `alias_xray` re-draws in thermal).
- Plan: reduce to 4 taps (or disable glow on a fast/low-fx mode) if post
  shows up in `goggles-nv` / `goggles-thermal` / `ultimate-coop`.
- Measure: `goggles-nv`, `goggles-thermal`, `ultimate-coop`.

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
  cache); reuse geometry storage (implemented below); build quads in
  a vertex shader from one instance per particle (cuts CPU and upload);
  with large counts, simulate weather on the GPU.
- Expect: the largest single gain on AD maps (vanilla included).

### 5. Split-screen post-processing
- Original suspicion: GPU `post` was 11.2 ms with four split views against
  0.25 ms solo. This was a timestamp attribution error: only the first
  scene wrote stage boundaries, leaving later scenes in `post`.
- Instrumented: each scene now writes all stage boundaries, including its
  post pass, and shared UI/screen shader/present is counted once. qprof
  reports both aggregate stages and each view's contribution (results below).
- Next: use the corrected scene/destination breakdown to choose a target.
  The original prediction of >100 fps from fixing post is unsupported;
  no post-processing optimisation has been made or justified by that bucket.

### 6. Supersampling combined with other enhancements
- Problem: all enhancements together cost far more than the sum of each
  one alone (+6.4 ms GPU on e1m1, +8 ms on AD; `translucent` reaches
  3–3.6 ms). SSAO alone adds 0.3 ms. Supersampling (`vid_fsaamode`) likely
  multiplies the cost of SSAO and the translucent/water passes.
- Investigation: qprof now has `supersample-ssao` and
  `supersample-water` pairs and reports GPU excess above additive cost
  (see results below). `vid_fsaamode` enables sample shading, whereas
  `r_renderscale 2` quadruples scene pixels; all-enhancements enables both.
  SSAO already uses a single-sampled target with sample shading disabled
  (including its apply pass). Separate prepare/generate/to-apply/apply/resume
  timestamps and a controlled 200%-scale SSAO matrix now identify AO generation
  as the dominant cost with MSAA, even without sample shading (results below).
  A full-resolution RG32F sample-0 depth/alpha snapshot now removes repeated
  multisample reads while retaining exact output (results below); extraction
  and its barriers are included in `ao-prepare`. The original timings identified
  the expensive interval, not the hardware cause.
  `gl_lodbias auto` also selects a −1.5 mip bias with 8x sample shading, so
  controlled sample-shading deltas include that sampler change. Running AO
  at output resolution when render scale exceeds 1 would be a *visual* change.

### 7. Extra scene views (teleporter windows, portals)
- Problem: `teleportwindows` +1.3 ms CPU and +0.7 ms GPU on AD; each window
  re-walks the world on the CPU (`build_world_draw`).
- Implemented: item 2 caches destination surface lists. Destination world
  index/indirect ranges are now reused within a frame when several windows,
  recursive portals or split players share that exact list. CPU lightmap
  requests now use a reusable dense bitset to avoid the accumulated
  sort/dedup while preserving the exact ascending union (results below).
  A compact immutable draw-metadata table now avoids reading the full CPU
  surface records during destination/brush construction; it lowers render
  CPU time without a demonstrated frame-rate gain in the full windowed
  co-op cycle (results below).
- Destination subintervals identify world drawing as about 78% of
  destination GPU time in the full fixed-step windowed co-op cycle.
  Conservative aperture scissors now reduce rasterization while preserving
  cached geometry, full-size projection, recursion and teleporter UV warp
  (results below). The fixed recursive-portal benchmark improves clearly;
  total co-op frame-time changes are small. Remaining candidates: measure
  clipping-plane fallback frequency and remaining world geometry/shading
  before choosing tighter aperture bounds or destination GPU surface culling.
  Keep GPU time, CPU construction/cache reuse and fence wait separate.

## Phase 3: gameplay-scale and smaller items

### 8. Server spikes in large fights
- Problem: server p99 19–23 ms with the `sv_horde` load (mean 0.65 ms).
  `clip_to_links` and `Hull::recursive_check` lead the profile. Probable
  cause, not yet confirmed: all monsters think on the same server frame.
- Plan: confirm by counting QC thinks per server frame. If confirmed, find
  why thinks line up (spawn-time `nextthink`), and reduce trace cost
  (area-node tree sizes, cheaper box/hull rejection). Monster timing is
  gameplay: do not stagger thinks without checking qbot results.

### 9. Enhanced water and impacts on AD — done (2026-11-14)
- `impacts` (+1.01 ms CPU) and `blood` (+0.93 ms CPU) done (overhead over
  `vanilla` dropped to +1 µs and +13 µs).
- `water` (+190 µs frame / +169 µs render CPU / +189 µs GPU) done (overhead
  over `vanilla` dropped to +14 µs frame / +46 µs render CPU / +13 µs GPU;
  see results log).

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
- Done: standalone and combined sample-shading/200%-scale SSAO/water
  diagnostic scenarios (item 6), with additive GPU costs, interaction excess
  and controlled setting differences in `report.md`.
- Done: separate SSAO generation/application/transition intervals and the
  200%-scale SSAO matrix at 1 sample, 8x MSAA and 8x sample shading (item 6).
- Done: per-view GPU stages for split-screen and a separate destination
  rendering interval (items 5/7); qprof reports per-view contributions without
  double-counting aggregate GPU totals. Earlier reports measured only the
  first view and charged later views to `post`.
- Done: destination GPU subintervals per scene (item 7): setup, world,
  models, particles and resolve/barriers, plus the enclosing interval's
  residual. Counts include recursive destination draws; contributions use
  all profiled GPU frames as the denominator and do not inflate totals.
- Done: fixed-step windowed co-op baseline (`qprof -window -fixedstep 250`),
  with the simulation schedule independent of frame rate (results below).
  Use this mode for co-op A/B work; keep wall-time play checks separate.

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

- 2026-11-13, item 4 horizontal follow-up: normal billboards also reject
  geometry wholly outside the left/right frustum planes. Scene target sizing,
  viewport rounding, reserved status-bar rows and 4:3 FOV adaptation now share
  `qrender::projection` helpers between rendering and host geometry culling.
  Existing guest/portal/teleporter/debug-camera safeguards remain in force.
  ad_sepulcher, 1280×720 headless RTX 3060 Ti, pinned vanilla, 20 s measured
  after 4 s warm-up, 3 runs/build: median 393.8 → 421.5 fps (+7.0%), frame
  2540 → 2372 µs, GPU 2224 → 2011 µs, particles+vm 926 → 693 µs.
  Live particles unchanged at 33,611; vertices 73,455 → 53,626 (−27.0%).
  Nine fixed-time combat screenshots and four viewport checks (4:3 with
  classic status bar, wide pitched/rolled camera, portrait, four-player
  split-screen; render scales 0.25/0.73/1/2) have `compare -metric AE = 0`.
  Inspected the tower/torch wide view, classic bar and four player views.
  All eight viewport runs with Vulkan validation logged no validation errors.
  450 tests passed, 2 skipped; clippy clean. Reports:
  `target/qprof/horizontal-before` and `horizontal-after`.
  Windowed confirmation (same map/resolution/GPU and pinned vanilla, 20 s
  measured after 4 s warm-up, 3 runs/build, `host_maxfps 10000`, vsync off):
  median 452.4 → 492.2 fps (+8.8%), frame 2210 → 2032 µs, GPU 914 → 725 µs,
  particles+vm 728 → 538 µs. Live combat advances by wall time here, so particle
  counts vary slightly across builds; headless provides the deterministic
  output comparison. Reports: `target/qprof/horizontal-window-before` and
  `horizontal-window-after`. These are our engine's A/B results, not reference
  engine parity measurements.
  Next: per-view geometry handling before enabling culling for shared views.

- 2026-11-13, item 4 geometry storage: qpart and qrender now share
  `qcore::vertex::ParticleVertex`, removing the host's vertex conversion copy.
  After upload, the host returns vertex/index/renderer-batch vectors (including
  appended add-on geometry) for reuse; raw qpart batches are reused and stably
  sorted in place instead of cloned. Empty-effect frames hand storage through
  for add-ons; frames without a view preserve the cache. A regression test
  covers changing counts/blends, empty frames, unchanged indices/draw order
  and retention of allocations.
  ad_sepulcher, 1280×720 RTX 3060 Ti, pinned vanilla, 20 s measured after
  4 s warm-up, 3 runs/build: headless median 437.9 → 451.2 fps (+3.0%),
  frame 2283 → 2216 µs, render CPU 2092 → 2006 µs; teleporter windows
  260.6 → 267.0 fps (+2.5%), frame 3838 → 3746 µs, render CPU
  3359 → 2726 µs. GPU times stayed roughly level; the smaller headless
  frame-time changes are near run-to-run noise. All six runs have identical
  particle/geometry counters and all 18 fixed-time combat screenshot pairs
  have `compare -metric AE = 0`.
  Windowed vanilla confirmation (same map/resolution/GPU and pinned settings,
  20 s after 4 s warm-up, 3 runs/build, vsync off, `host_maxfps 10000`):
  median 505.3 → 536.6 fps (+6.2%), frame 1979 → 1863 µs, render CPU
  1866 → 1754 µs, GPU 727 → 733 µs. Combat advances by wall time, so
  populations vary slightly; deterministic headless runs establish output
  equivalence. Reports: `target/qprof/reuse-before`, `reuse-after`,
  `reuse-window-before`, `reuse-window-after`. These are engine A/B results,
  not reference-engine parity measurements. Two additional id1 gel screenshot
  pairs also have AE = 0 when both builds use the same screenshot output path
  (different paths otherwise change the printed console notification). Inspected
  the torch-lit AD corridor and the gel weapon/HUD scene. AD vanilla and
  teleporter-window runs and id1 gel runs with Vulkan validation logged no
  errors. 451 tests passed, 2 skipped; clippy clean. Config-file hashes stayed
  unchanged after all windowed runs. Next: per-view geometry handling before
  enabling culling for shared views.

- 2026-11-13, item 6 diagnostics: added `supersample-ssao` and
  `supersample-water` to qprof without altering standalone scenarios or
  all-enhancements. The Markdown report compares each pair with additive
  GPU cost (`supersample + effect − vanilla`) and shows the largest stage
  excess; it omits incomplete pairs and missing stage timestamps. A regression
  test covers baseline subtraction, incomplete pairs and missing timestamps.
  RTX 3060 Ti, 1280×720 headless, pinned qprof settings, 20 s after 4 s
  warm-up, 250 Hz simulation, 3 runs/scenario/map (36 runs total). Values
  below come from each scenario's run with median fps, matching qprof:

  | scenario | e1m1 frame / GPU µs | ad_sepulcher frame / GPU µs |
  |---|--:|--:|
  | vanilla | 1354 / 1234 | 2213 / 2007 |
  | supersample | 1619 / 1497 | 2487 / 2377 |
  | ssao | 1663 / 1543 | 2386 / 2276 |
  | water | 1352 / 1237 | 2481 / 2209 |
  | supersample-ssao | 2208 / 2086 | 2996 / 2884 |
  | supersample-water | 1627 / 1505 | 2817 / 2701 |

  SSAO interaction excess is +280/+238 µs (e1m1/AD), mostly in
  `translucent` (+278/+249 µs). That timestamp bucket includes SSAO before
  liquid draws, so it does not identify water as the cause. Water interaction
  excess is +5/+123 µs, with the AD difference mostly in `world` (+143 µs);
  this is small relative to the original multi-millisecond all-enhancements
  excess and partly near measurement noise. These pairs alone do not explain
  that original result. Code inspection confirms SSAO generation already
  uses a single-sampled image and both generation/apply disable sample shading.
  Next: separate 8x MSAA, sample shading and 200% render scale (four times
  the scene pixels) with controlled scenarios before choosing a visual change.
  No renderer or simulation changes, and no speedup or windowed parity claim.
  Particle/geometry counters match across all six AD scenarios; e1m1 has
  no FTE particles. Inspected the AD torch-lit corridor with both pairs and
  e1m1's metal corridor with SSAO, weapon and HUD intact. All run logs are
  free of unknown commands/cvars and Host_Error. No Vulkan code changed.
  No before/after image comparison was needed
  because the engine binary is unchanged. Reports: `target/qprof/pairwise-id1`
  and `pairwise-ad-clean`; the initial AD attempt overlapping checks was
  discarded. 452 tests passed, 2 skipped; clippy clean.


- 2026-11-14, item 6 controlled MSAA/scale diagnostics: added
  `msaa8-renderscale200` and `supersample-renderscale200`, plus controlled
  setting deltas and additive interaction costs in qprof's report. The
  baseline now explicitly pins `gl_lodbias auto`; both measured user configs
  already had that value. `vid_fsaamode 1` therefore includes the automatic
  −1.5 mip bias at 8x, not just a change in shader invocation count.
  RTX 3060 Ti, 1280×720 headless, 20 s measured after 4 s warm-up,
  250 Hz simulation, three runs per scenario/map (36 total, spanning
  November 13–14). Each value is from the run selected by median fps:

  | scenario | e1m1 frame / GPU µs | ad_sepulcher frame / GPU µs |
  |---|--:|--:|
  | vanilla | 1333 / 1238 | 2160 / 2008 |
  | msaa8 | 1381 / 1289 | 2232 / 2068 |
  | supersample | 1601 / 1506 | 2486 / 2391 |
  | renderscale200 | 1413 / 1315 | 2299 / 2113 |
  | msaa8-renderscale200 | 1587 / 1487 | 2416 / 2325 |
  | supersample-renderscale200 | 2400 / 2303 | 3561 / 3464 |

  Normal-scale MSAA costs +51/+60 µs GPU (e1m1/AD); at 200% scale,
  +172/+212 µs. Sample-shading mode adds +217/+323 µs at normal scale,
  growing to +816/+1139 µs at 200%. World rendering accounts for
  +192/+240 µs and +736/+862 µs of those mode deltas, respectively;
  particles+viewmodel adds +23/+74 µs and +82/+242 µs. The combined
  sample-shading/200%-scale cost exceeds its standalone additive GPU
  prediction by +720/+968 µs. This confirms amplification by scene pixel
  count, but does not explain the entire original all-enhancements cost.
  Next: measure SSAO and enhanced water with the combined sample-shading
  and 200%-scale setting, alongside a fresh all-enhancements control,
  before choosing an output-preserving optimisation.
  All 36 runs have matching per-map particle/geometry counters. Inspected
  e1m1's metal corridor and the AD pillar/torch corridor at MSAA and combined
  sample shading/200% scale, with weapons and HUD intact. Logs contain no
  unknown commands/cvars or Host_Error. No engine code changed, so there is
  no before/after screenshot comparison, Vulkan change or speedup claim.
  Report: `target/qprof/aa-scale/`. 453 tests passed, 2 skipped; clippy clean.


- 2026-11-14, item 6 combined SSAO/water diagnostics: added
  `supersample-renderscale200-ssao` and
  `supersample-renderscale200-water`, with additive interaction and controlled
  effect deltas in qprof. Standalone and all-enhancements settings are
  unchanged. RTX 3060 Ti, 1280×720 headless, 20 s measured after 4 s warm-up,
  250 Hz simulation, three runs per scenario/map (42 total). Values are from
  each scenario's run selected by median fps:

  | scenario | e1m1 frame / GPU µs | ad_sepulcher frame / GPU µs |
  |---|--:|--:|
  | vanilla | 1322 / 1231 | 2193 / 2036 |
  | ssao | 1629 / 1536 | 2371 / 2278 |
  | water | 1322 / 1232 | 2460 / 2199 |
  | supersample-renderscale200 | 2391 / 2296 | 3451 / 3376 |
  | supersample-renderscale200-ssao | 6227 / 6130 | 6410 / 6335 |
  | supersample-renderscale200-water | 2417 / 2322 | 4082 / 4007 |
  | all-enhancements | 8043 / 7935 | 11195 / 11116 |

  Adding SSAO to combined sample shading/200% scale costs +3834/+2958 µs
  GPU (e1m1/AD), versus standalone +305/+243 µs. Excess above additive cost
  is +3529/+2716 µs, concentrated in `translucent` (+3531/+2691 µs).
  This bucket contains AO generation, depth/colour barriers, AO application
  and liquid draws; these measurements cannot distinguish those costs.
  Water costs +27/+631 µs at combined settings, with interaction excess
  +26/+467 µs (mostly `world` +24/+443 µs); the e1m1 effect is near noise.
  The same all-enhancements bucket is 3837/2996 µs, close to the SSAO probe's
  3838/2970 µs, identifying SSAO at combined settings as a substantial part
  of the expensive combined case. Other costs remain: all-enhancements
  generates 135,014 AD FTE vertices versus 53,626 in the diagnostic probes,
  because teleporter windows disable shared-geometry camera culling.
  Next: separate GPU timestamps for AO generation, application and barriers,
  then compare 200%-scale SSAO with and without MSAA/sample shading to
  identify the output-preserving optimisation. Retain current full-resolution
  AO output until that evidence exists.
  All 36 diagnostic runs have identical per-map particle/geometry counters;
  the six all-enhancements controls are internally consistent and intentionally
  use a different effect configuration. Inspected e1m1's SSAO corridor and
  CRT/blur/impact scene, and AD's SSAO pillar/torch corridor and combined-water
  view, with weapons and HUD intact. Logs contain no unknown commands/cvars
  or Host_Error. No engine or Vulkan code changed, so no before/after image
  comparison or validation run was needed. These are setting-cost diagnostics,
  not an optimisation speedup or windowed parity claim.
  Report: `target/qprof/combined-effects/`. 454 tests passed, 2 skipped;
  clippy clean.

- 2026-11-14, item 6 SSAO stage investigation: GPU timestamps now separate
  `ao-prepare`, `ao-generate`, `ao-to-apply`, `ao-apply` and `ao-resume`
  from `translucent`. Generation/application draws are distinguished from
  target setup, layout transitions and rendering boundaries; these intervals
  include GPU stalls, not isolated barrier instruction costs. All queries
  are also written with AO off or no view. The existing split-screen limit
  remains: these stages measure view 0, later views are charged to `post`.
  Added `renderscale200-ssao` and `msaa8-renderscale200-ssao` and an AO stage
  table and controlled MSAA/sample-shading deltas in qprof.
  RTX 3060 Ti, 1280×720 headless, pinned settings, 20 s after 4 s warm-up,
  250 Hz simulation, three runs per scenario/map: 24 before, 48 after.
  Values below come from the instrumented run selected by median fps:

  | SSAO settings | e1m1 frame / GPU µs | e1m1 generate / apply µs | AD frame / GPU µs | AD generate / apply µs |
  |---|--:|--:|--:|--:|
  | 100%, 1 sample | 1597 / 1507 | 215 / 36 | 2335 / 2249 | 184 / 36 |
  | 200%, 1 sample | 2700 / 2611 | 1099 / 138 | 3341 / 3254 | 958 / 138 |
  | 200%, 8x MSAA | 5288 / 5200 | 3438 / 140 | 5373 / 5281 | 2715 / 141 |
  | 200%, 8x sample shading | 6165 / 6075 | 3481 / 144 | 6621 / 6527 | 2784 / 148 |

  Generation accounts for about 90–91% of the combined AO intervals on AD and
  e1m1. At 200%, switching from 1 sample to 8x MSAA raises generation by
  +2339/+1757 µs (e1m1/AD); adding sample shading then changes it by only
  +43/+69 µs. Application stays around 138–148 µs; combined prepare,
  to-apply and resume are 210/147 µs with sample shading. This identifies
  the generation interval under MSAA as the main target, even without
  sample shading. Both AO pipelines already disable sample shading; the
  generation shader repeatedly fetches sample 0 of multisampled depth and
  reads sample-0 scene alpha. The timings do not establish the hardware
  reason for its cost. Next experiment: preserve exact sample-0 depth/alpha
  in a full-resolution single-sample snapshot, include extraction overhead,
  and require pixel identity before considering it an optimisation. No
  downsampling or shader algorithm change was made here.
  Matching before/after median frame changes range from −0.1% to +1.9%
  (combined SSAO e1m1 6104→6165 µs, AD 6499→6621 µs); these are
  instrumentation checks, not a speedup or windowed parity claim.
  All 24 fixed-time before/after combat screenshot pairs have AE = 0 and
  matching particle/geometry counters. Inspected e1m1's shaded metal corner,
  impacts and weapon/HUD, and AD's shaded pillar, torch and axe/HUD.
  Tests cover profiling pixel identity and complete queries with AO on,
  off and no view, plus report deltas and incomplete-stage omission.
  Six additional Vulkan-validation runs (AO off, normal SSAO, and combined
  8x sample-shading/200%-scale SSAO on both maps) logged no validation errors;
  benchmark logs contain no unknown commands/cvars or Host_Error.
  455 tests passed, 2 skipped; clippy clean. Reports:
  `target/qprof/ao-stages-before/`, `ao-stages-after/` and
  `ao-stages-validation/`.

- 2026-11-14, item 6 sample-0 snapshot: MSAA SSAO now extracts exact
  sample-0 depth/alpha into a full-resolution RG32F image before generation.
  The AO algorithm, sample positions, sky/fog mask and blur are unchanged;
  single-sample SSAO reads the original attachments. The snapshot is allocated
  on the first MSAA SSAO frame, reused across frames/views and recreated with
  scene targets after resolution/scale/sample changes. Its draw, target setup
  and barriers are included in `ao-prepare`, so the comparison includes
  extraction overhead.
  RTX 3060 Ti, 1280×720 headless, pinned settings, 20 s after 4 s warm-up,
  250 Hz simulation, three runs per scenario/map (24 before, 24 after).
  Values below come from each build's run selected by median fps:

  | SSAO at 200% scale | e1m1 frame µs before → after | AD frame µs before → after | e1m1 GPU µs before → after | AD GPU µs before → after |
  |---|--:|--:|--:|--:|
  | 8x MSAA | 5375 → 3882 | 5526 → 4239 | 5278 → 3779 | 5425 → 4155 |
  | 8x sample shading | 6267 → 4672 | 6750 → 5464 | 6171 → 4581 | 6650 → 5377 |

  MSAA frame time falls 27.8%/23.3% (e1m1/AD); sample-shading frame time
  falls 25.5%/19.1%. MSAA AO generation falls 3455→1788 / 2744→1490 µs,
  while prepare rises 24→170 / 27→156 µs. Summed AO intervals fall
  3813→2277 / 3074→1864 µs, including extraction. Sample-shading generation
  falls 3491→1796 / 2790→1501 µs, with prepare 107→256 / 51→213 µs.
  One-sample controls (100% and 200% scale) vary −2.4%…+4.1% in frame time,
  with generation nearly unchanged; small overall deltas remain noisy.
  All 24 fixed-time combat screenshot pairs have ImageMagick AE = 0 and
  identical frame/particle/geometry counters. Six additional before/after
  pairs cover fractional scale, 2x/4x/8x MSAA, SSAO off, and two/four split
  views: AE = 0, with no Vulkan validation errors in all 12 runs. Inspected
  e1m1's shaded metal corner/impact/weapon/HUD, AD's pillar/torch/axe/HUD,
  and four-player views. Benchmark logs contain no unknown commands/cvars
  or Host_Error. The offscreen regression now exercises split views followed
  by resize and sample-count changes, requiring the original pixels after
  target recreation. 455 tests passed, 2 skipped; clippy clean.
  Reports: `target/qprof/snapshot-before/`, `snapshot-after/`; extra captures
  and validation logs: `target/perf-snapshot/`.
  Windowed confirmation (same GPU/resolution, pinned combined 8x sample
  shading/200%-scale SSAO, 20 s after 4 s warm-up, three runs/build/map,
  vsync off, `host_maxfps 10000`): e1m1 median 189.5→276.5 fps,
  5278→3616 µs frame (−31.5%), 5126→3477 µs GPU; ad_sepulcher
  175.7→228.3 fps, 5691→4380 µs frame (−23.0%), 5580→4261 µs GPU.
  Wall-time combat populations vary in windowed runs; deterministic headless
  pairs establish pixel equivalence. Reports: `snapshot-window-before/`
  and `snapshot-window-after/` under `target/qprof/`. Windowed logs are
  clean and all user config-file hashes stayed unchanged. These are engine
  A/B results, not reference-engine parity measurements. The all-enhancements
  follow-up is recorded below; split-view GPU timestamp attribution remains
  outstanding.

- 2026-11-14, item 6 all-enhancements confirmation: compared the saved
  pre-snapshot executable with the current build (3359a61), RTX 3060 Ti,
  1280×720 headless, pinned qprof all-enhancements, 20 s measured after 4 s
  warm-up, 250 Hz simulation, three runs/build/map. Values are from the
  median-fps run, matching qprof's selection:

  | map | fps before → after | frame µs | GPU µs | p99 µs |
  |---|--:|--:|--:|--:|
  | e1m1 | 126.6 → 151.2 | 7897 → 6613 (−16.3%) | 7805 → 6508 | 9895 → 8750 |
  | ad_sepulcher | 88.1 → 95.5 | 11348 → 10473 (−7.7%) | 11269 → 10384 | 22481 → 19118 |

  SSAO generation falls 3509→1827 / 2779→1507 µs (e1m1/AD), with
  prepare increasing 43→270 / 26→231 µs, including snapshot extraction.
  All six fixed-time combat screenshot pairs have AE = 0; frame and FTE
  particle/geometry counters match in every pair. Inspected e1m1's impact
  sparks/shaded corner/weapon and AD's pillar/torch/axe, with CRT-filtered
  HUDs. Benchmark logs have no unknown commands/cvars, Host_Error or
  validation errors (validation was not enabled for these timing runs).
  Raw profiles, logs and shots: `target/qprof/combined-snapshot-before/`
  and `combined-snapshot-after/`; comparison: `target/perf-combined/`.
  Each profile contains 5000 measured frames; an additional replacement
  probe is excluded from the three-run median. No rendering code changed
  here; windowed all-enhancements confirmation remains outstanding.
  Verification: 455 tests passed, 2 skipped; clippy clean.

  Next: finer GPU attribution for destination views and split-screen
  (items 5/7). After the snapshot, AD's largest scene buckets are
  `particles+vm` 2051 µs, `targets` 1952 µs and `world` 1682 µs;
  `targets` includes `draw_portals`, so it does not isolate attachment setup.
  Split-screen currently times only the first view's scene and charges later
  views to `post`; separate view/destination work before choosing a fix.
  The 2119 µs `ui+present` bucket includes readback and the whole-screen
  shader; it cannot be treated as pure readback or compared with windowed.

- 2026-11-14, items 5/7 GPU attribution: every scene view now writes a full
  set of stage boundaries; aggregate stages sum all views, while shared
  UI/screen shader/present is counted once. `destinations` isolates
  `draw_portals` (all portal/teleporter destinations, including recursion),
  separately from target transitions. Scene begin/clear now falls into
  `world`, so old `targets`/`world` values are not directly comparable.
  `gpu_views_us` and qprof's GPU scene views table report each view's
  contribution with the same per-profiled-frame denominator as the aggregate,
  plus its active-frame count. They do not inflate the aggregate GPU total.
  Query pools reserve four views and grow safely after draining both frame
  slots; result storage is reused, and only written queries are read.

  RTX 3060 Ti, 1280×720 headless, pinned qprof settings, 8 s measured after
  4 s warm-up, 250 Hz simulation, three runs/build/scenario on ad_sepulcher
  (2000 measured frames each; shorter diagnostic workload than the 20 s
  optimisation baselines). Values come from the median-fps run:

  | scenario | frame µs before → after | GPU µs before → after |
  |---|--:|--:|
  | vanilla | 2020 → 1954 | 1916 → 1870 |
  | teleportwindows | 3476 → 3354 | 3379 → 3275 |
  | ultimate-coop | 16714 → 16727 | 11721 → 11649 |

  This is instrumentation, with no speedup claim: co-op frame time changes
  +0.08%; the smaller control changes (−3.3%/−3.5%) are near run noise.
  Co-op's old `post` bucket was 7086 µs in this workload; actual post is
  423 µs across four views (105/104/108/107 µs). Scene totals are
  2445/2304/2326/2286 µs, with shared UI/present 2288 µs. The largest scene
  stages are world 2422 µs, destinations 2237 µs, AO total 1503 µs and
  particles/viewmodel 1316 µs. Teleporter-only destinations cost 495 µs;
  their main-view particles/viewmodel cost 1482 µs.

  All nine combat screenshot pairs are pixel-identical (AE = 0), with
  matching frame and particle/geometry counters. All per-view contributions
  sum to the aggregate within JSON rounding. Two additional pixel-identical
  pairs cover recursive e1m1 portals and four AD split views at 8x MSAA,
  200% scale and high SSAO; all four runs with Vulkan validation are clean.
  Inspected the recursive orange ovals/player model, four torch-lit AD
  views/weapons/HUDs, and the co-op thermal/impact scene. Tests cover changing
  counts (1/2/4/5 views), query-pool growth, resetting statistics, unchanged
  pixels, and parsing/reporting old and new profiles without double-counting.
  457 tests passed, 2 skipped; six qprof tests also passed after extending
  parser coverage; clippy clean. Reports: `target/qprof/views-before/` and
  `views-after/`; comparisons/validation: `target/perf-views/`.

  Next: CPU-profile repeated scene building in `ultimate-coop` before
  choosing another optimisation (items 4/7). Render CPU excluding fence is
  14747 µs and fence wait only 696 µs: co-op remains CPU-bound despite its
  substantial GPU costs. Revisit particle geometry shared by multiple views
  and repeated world/destination draw building using that profile. The
  original post-only >100 fps prediction is withdrawn. No rendering or
  gameplay algorithm changed; windowed confirmation remains outstanding.

- 2026-11-14, item 7 destination world draw reuse: a full 20 s co-op
  `qprof -perf` run (4 s warm-up, 250 Hz headless simulation, RTX 3060 Ti,
  1280×720, release with line-table symbols) found `build_world_draw` at
  24.34% of sampled engine cycles on the main cores and u32 quicksort at
  18.31%. The render path sorts the accumulated lightmap requests; this
  profile has no call stacks, so the sort symbol alone does not identify
  all callers. The main-core event has about 216k process samples; the
  small-core event has only 229 and is not used for these percentages.
  Profile: `target/qprof/coop-cpu-investigation/`.

  `PortalWorldCache` now reuses immutable world index/indirect ranges when
  destination views share the same cached PVS surface-list allocation.
  The cache belongs to one render call and is shared by all scene views,
  including recursive portals and split-screen players. Identity uses
  `Arc::ptr_eq`, retaining the list for the frame; different allocations
  build independently. Time, texture animation, liquid alpha and hidden
  surfaces are common within a render call and rebuild next frame. View
  UBOs, clipping, entities and particles remain separate. Cache hits also
  avoid appending duplicate lightmap requests; the final sorted/deduped
  union is unchanged. No persistent buffers or rendering algorithm changed.

  Headless A/B, same GPU/resolution, pinned `ultimate-coop` on ad_sepulcher,
  20 s measured after 4 s warm-up, 250 Hz, three runs/build, both builds
  with line-table symbols. Medians select the run by fps as qprof does:

  | metric | before | after | change |
  |---|--:|--:|--:|
  | fps | 41.9 | 58.4 | +39.4% |
  | frame µs | 23875 | 17114 | −28.3% |
  | render CPU excluding fence µs | 21934 | 13414 | −38.8% |
  | p99 frame µs | 65348 | 40966 | −37.3% |
  | GPU µs | 16159 | 16463 | +1.9% |

  Short controls (same map/settings, 8 s after 4 s warm-up, three runs/build):
  vanilla 2038→2027 µs, teleportwindows 3470→3429 µs (−0.5%/−1.2%, within
  noise). All six control screenshot pairs have AE = 0. The nine full-cycle
  horde pairs differ by 61–8986 pixels (up to 0.98%); repeated baseline runs
  also differ (188/203/9407 pixels at 5/10/15 s for runs 0 and 1). Inspected
  the difference image and four-player portal/blood/impact scene; changes
  occupy effects regions and have the same scale as baseline variation.
  Frame and FTE counters match in every pair. Exact-output checks use fixed
  quiet scenes: recursive e1m1 portals and four AD split views with teleporter
  windows, high SSAO, 8x MSAA and 200% scale both have AE = 0. All four
  validation runs are clean; inspected the recursive ovals/player model.
  Unit coverage checks range sharing and lightmap requests across cache hits,
  distinct lists and new frames. The offscreen test checks two players
  viewing the same teleporter against a single-view reference, then changes
  time and layout and requires the original pixels again. 458 tests passed,
  2 skipped; clippy clean. No server/physics change.
  Reports: `target/qprof/destination-before/`, `destination-after/`,
  `destination-controls-before/`, `destination-controls-after/`;
  comparisons/validation: `target/perf-destination-cache/`.

  Windowed check (same GPU/resolution, pinned ultimate-coop, 20 s after
  4 s warm-up, three runs/build, vsync off, `host_maxfps 10000`): before
  75.8/75.0/64.8 fps, after 79.5/73.7/73.1 fps. Median frame 13338→13571 µs
  (+1.7%), render CPU excluding fence 10376→10341 µs, GPU 8797→10756 µs.
  This does not confirm a windowed speedup. Wall-time combat differs with
  frame rate, and the run spread is much larger than the median change;
  the unchanged configuration and clean logs do not establish identical
  live workloads. All config-file hashes stayed unchanged. Reports:
  `target/qprof/destination-window-before/` and `destination-window-after/`.
  These results are engine A/B measurements, not reference-engine parity.

  Next: make the windowed co-op workload repeatable (fixed scene/cameras or
  recorded movement) and profile the optimised full cycle before choosing
  another fix. The headless CPU saving is clear, but does not by itself
  establish a live-play fps improvement. Revisit the remaining lightmap
  request sorting and world/destination work with those measurements.


- 2026-11-14, items 5/7 measurement follow-up: the engine and qprof accept
  `-fixedstep Hz` (1–1000), explicitly selecting a fixed simulation step
  with uncapped rendering, including the swapchain path. Normal windowed
  runs retain wall-time simulation; headless defaults are unchanged.
  `-realtime` and `-dedicated` reject the engine flag. qprof reports now
  distinguish wall-time simulation from fixed steps; previous windowed
  headers incorrectly described the frame cap as a simulation frequency.

  Fixed-step windowed baseline, pinned `ultimate-coop` on ad_sepulcher,
  RTX 3060 Ti, 1280×720, 20 simulated seconds after 4 s warm-up, 250 Hz,
  vsync off, `host_maxfps 10000`, three runs: 59.8/59.3/58.8 fps (1.7%
  full spread), exactly 5000 measured frames in every run. Median frame
  16853 µs, render CPU excluding fence 14016 µs, GPU 15638 µs, p99 42809 µs.
  Config hashes are unchanged. Report: `target/qprof/coop-fixedstep-window/`.
  These numbers establish a baseline, not an optimisation or reference-engine
  parity result; the workload differs from previous wall-time combat.

  Fixed steps control commands, camera movement and simulation ticks, but
  do not remove all effects nondeterminism. Comparing run 0 to runs 1/2 at
  5/10/15 measured seconds changes 0–9623 pixels (at most 1.05%); the
  earlier headless horde captures also varied. Inspected the four green
  night-vision views, weapons, bullet-time bars and HUDs. FTE counters
  match (zero in this scenario); this does not count add-on effects.

  Quiet output checks compare the implicit headless 250 Hz path with
  explicit `-fixedstep 250`: recursive e1m1 portals and four AD split views
  with teleporter windows, high SSAO, 8x MSAA and 200% scale both have
  `compare -metric AE = 0`. All four Vulkan-validation runs are clean.
  Inspected the nested orange portals/player and four torch-lit AD views.
  Tests cover flag validation/conflicts, forwarding an independent simulation
  frequency, preserving defaults and reporting wall-time/fixed clocks.
  458 tests passed, 2 skipped; clippy clean. No server or rendering algorithm
  changed. Checks: `target/perf-fixedstep/`.

  Isolated fresh full-cycle CPU capture (same headless 250 Hz workload,
  release with line-table symbols, 999 Hz `cycles:u`, measured-window filter):
  main-core engine self-time is `build_world_draw` 13.10%, u32 quicksort
  9.04%, `mark_leaves` 5.73%, `gpu_world_draw` 5.22%. About 69k main-core
  samples, none lost; the 103 small-core samples are not used. These are
  diagnostic shares, not before/after frame-time results. Profile JSON:
  `target/qprof/coop-optimized-fixedstep/`; data and reports:
  `target/perf-fixedstep/optimized.perf.data`, `optimized-self.txt`.
  DWARF call chains were captured but stop at the sort in this panic-abort
  release, so they do not establish its caller. Before optimising that sort,
  rebuild the profiling binary with unwind tables (e.g.
  `RUSTFLAGS="-C force-unwind-tables=yes"`) and verify caller attribution;
  measure the lightmap-request count and duplication as well. Destination
  construction remains the largest engine symbol. The repeatable windowed
  baseline is ready for the next isolated A/B change; no new renderer
  optimisation is claimed here.


- 2026-11-14, item 7 CPU lightmap union: a second diagnostic capture
  confirmed 100,370 requested / 75,165 unique lightmaps per frame on
  average during the 5000-frame `ultimate-coop` cycle (25.1% repeated
  requests). Peak requests 351,641, of which 172,741 are unique; peak unique
  count across the cycle 173,490. The map has 194,369 lightmaps. These are
  counts, not a timing comparison; the temporary diagnostic logging was
  removed before benchmarking. Data: `target/perf-lightmaps/counts.json`.

  Unwind tables alone still did not recover the caller. A short separate
  capture built with `RUSTFLAGS="-C force-unwind-tables=yes -C force-frame-pointers=yes"`
  and `perf record --call-graph fp` recovered quicksort → `Renderer::render`
  → host render/frame. `addr2line -i` resolves the call to `frame.rs:819`,
  the lightmap-request sort. Caller evidence: `target/perf-lightmaps/fp-sort-stacks.txt`
  and `sort-callsite.txt`; the diagnostic build is not used for A/B timings.

  `frame::lightmaps::LightmapRequests` replaces the per-frame vector's
  sort/dedup with a bitset bounded by the world's dense lightmap table
  (24,304 bytes for ad_sepulcher). Brushes and destination views set bits;
  scanning set bits in ascending order yields the exact former sorted
  union. The scan covers only the requested word span. Bitset/output
  allocations persist in the renderer, with contents reset each frame and
  on map-size changes. The complete CPU list still goes into the bump buffer;
  it is not limited by the separate GPU cull list. Tests compare against the
  old algorithm with 350k repeated requests, cover word boundaries, empty
  frames, changing map sizes, allocation reuse and unions above `MAX_VIS`.


  Fresh standard-release A/B (no diagnostic flags/logging), pinned
  `ultimate-coop` on ad_sepulcher, RTX 3060 Ti, 1280×720, full 20 s cycle
  after 4 s warm-up, fixed 250 Hz simulation, three runs/build/mode:

  | metric | headless before | headless after | windowed before | windowed after |
  |---|--:|--:|--:|--:|
  | fps | 55.2 | 56.7 | 55.9 | 58.1 |
  | frame µs | 18115 | 17638 | 17884 | 17205 |
  | render CPU excluding fence µs | 14546 | 13562 | 15043 | 13913 |
  | GPU µs | 17187 | 16879 | 16161 | 15885 |
  | p99 frame µs | 43972 | 42774 | 45534 | 43736 |

  Render CPU falls 6.8% headless / 7.5% windowed; frame time falls
  2.6% / 3.8%. Headless fps runs before 48.4/55.8/55.2, after
  55.4/56.7/57.4: its small total-frame improvement is within the broad
  run spread. Windowed is tighter: before 55.9/55.8/55.9, after
  58.2/58.1/58.1 (+3.9% median fps). Windowed uses vsync off and
  `host_maxfps 10000`; config hashes stay unchanged. Every run measures
  5000 frames. These are engine A/B results, not reference-engine parity.
  Reports: `target/qprof/lightmaps-{headless,window}-{before,after}/`.


  Vanilla AD control (headless, same pinned baseline, 8 s after 4 s warm-up,
  three runs/build): 488.4→488.4 fps, frame 2047.38→2047.69 µs (+0.02%),
  render CPU 1770.51→1779.08 µs (+0.5%, within noise). All three combat
  screenshots have AE = 0, with unchanged counters. Reports:
  `target/qprof/lightmaps-controls-{before,after}/`.

  Quiet recursive e1m1 portals and four AD split views (teleporter windows,
  high SSAO, 8x MSAA, 200% scale) both have `compare -metric AE = 0`;
  all four Vulkan-validation runs are clean. Inspected the nested orange
  portals/player, four torch-lit AD views and full four-player blood/portal
  combat scene. The 18 live co-op screenshot pairs differ by 61–9776 pixels
  (up to 1.1%); repeated baseline pairs also differ by 0–10010 pixels.
  Particle/geometry counters match in every pair. These effects variations
  do not replace the exact quiet-scene and vanilla checks.

  Both benchmark builds' startup logs contain unsupported `addon_hitstop_*`
  entries from the saved config. They are ignored before measurement on
  both sides. Quiet validation and vanilla controls use the same temporary
  config copy with only those unsupported entries removed; original configs
  are untouched and hashes match across all four co-op benchmark phases.
  Checks and comparisons: `target/perf-lightmaps/`.
  461 tests passed, 2 skipped; clippy clean. No server/physics change.
  Next: reprofile destination draw construction with the lightmap sort gone
  before selecting another world/portal CPU change.

  Integration with current master: qprof now pins the newly registered
  `addon_hitstop` off in its baseline, inherited by add-on scenarios, so
  the saved settings that previously printed unknown commands cannot change
  the historical workload. The measurements above predate that integration.


- 2026-11-14, item 7 compact CPU draw metadata: fresh full-cycle profiles
  after the lightmap union and lightmap parity fixes still put
  `build_world_draw` first at 16.03/16.13/16.19% of main-core engine
  cycles. `mark_leaves` is 6.09/6.17/6.15%, `gpu_world_draw`
  5.69/5.61/5.54%. These are measured-window self-time shares from
  three headless 20 s captures after 4 s warm-up, fixed 250 Hz,
  1280×720 on RTX 3060 Ti, release with line-table symbols. Each main-core
  event has 121k–125k process samples, with none lost; the sparse small-core
  events are not used for these shares. Annotated samples concentrate
  around the surface metadata reads in both grouping and index-copy loops;
  this suggests testing a smaller working set, rather than proving a
  hardware-cache cause. These diagnostic captures are not A/B timings.
  Profiles: `target/qprof/destination-reprofile/`; annotation and compact
  self reports: `target/perf-destination-profile/`.

  `WorldGpu::draw_surfaces` stores only first index, index count, texture
  and lightmap in a 16-byte `DrawSurface`, instead of walking the full
  116-byte `Surface` during CPU draw construction. The table is built after
  world/external-brush loading and freed with the level; its source fields
  are immutable afterward. Extra CPU memory is 16 bytes per surface.
  Texture animation (including alternate/random frames), hidden-surface
  filtering, index order, buffer-capacity handling, categories, and lightmap
  union all retain their existing paths. No new persistent GPU resources.

  Fresh windowed A/B, pinned `ultimate-coop` on ad_sepulcher, same
  GPU/resolution, full 20 simulated seconds after 4 s warm-up, fixed 250 Hz,
  vsync off and `host_maxfps 10000`, three runs/build, both release builds
  with line-table symbols. Median selects the run by fps as qprof does:

  | metric | before | after | change |
  |---|--:|--:|--:|
  | fps | 62.7 | 62.9 | +0.3% |
  | frame µs | 15938 | 15892 | −0.3% |
  | render CPU excluding fence µs | 12687 | 11905 | −6.2% |
  | fence wait µs | 1738 | 2511 | +44.4% |
  | GPU µs | 14884 | 14943 | +0.4% |
  | p99 frame µs | 38551 | 37526 | −2.7% |

  Before fps 63.1/62.7/62.6, after 63.1/62.9/62.7; every run measures
  exactly 5000 frames and config hashes stay unchanged. Render CPU ranges
  do not overlap (12614–12760 → 11769–11928 µs), but the saving mostly
  becomes fence wait. The total-frame change is within run variation:
  **no demonstrated fps gain** in this workload and no reference-engine
  parity claim. Reports: `target/qprof/compact-window-{before,after}/`.

  Vanilla AD control (headless, pinned baseline, 8 s after 4 s warm-up,
  three runs/build): 501.0→501.9 fps, frame 1995.98→1992.40 µs (−0.2%,
  within noise), render CPU 1725→1708 µs. All three fixed-time combat
  screenshot pairs have `compare -metric AE = 0`; FTE counters match.
  Reports: `target/qprof/compact-controls-{before,after}/`.

  Quiet recursive e1m1 portals and four AD split views (teleporter windows,
  high SSAO, 8x MSAA and 200% scale) both have AE = 0; all four
  Vulkan-validation runs are clean. Inspected the nested orange portals
  and player model, the four torch-lit AD views with weapons/HUDs, and
  the full co-op combat/difference images. Nine live co-op pairs differ
  by 0–9235 pixels (up to 1.0%); baseline run 0 vs 1 differs by
  123/98/9408 pixels at 5/10/15 measured seconds. Differences occupy effects
  regions, and FTE counters match in all pairs. These live variations do
  not replace the exact quiet-scene checks. Checks, config hashes and
  comparisons: `target/perf-destination-profile/`.
  469 tests passed, 2 skipped; clippy clean. No server/physics change.

  Next: profile the remaining scene-building cost after this working-set
  reduction, especially repeated PVS leaf marking across split views;
  keep CPU time and GPU fence wait separate when choosing another target.


- 2026-11-14, item 7 all-visible leaf reuse: a fresh full-cycle CPU
  capture after compact draw metadata put `mark_leaves` first at 6.83%
  of main-core engine cycles, ahead of `build_world_draw` 6.76% and
  `gpu_world_draw` 6.33% (116k process samples, none lost). With transparent
  liquids, every camera requests the same complete candidate list, but
  changing raw view leaves rebuilt it. All-visible queries now share
  candidate cache key 0; normal PVS queries keep their actual leaf key.
  Solid/missing-VIS leaves and absent VIS data use the same all-visible
  key. Liquid-transparency changes still invalidate candidate and portal
  surface caches. Every view independently rebuilds its frustum-clipped
  leaf bits; the portal surface cache remains keyed by destination leaf.
  No extra cache storage or GPU resources are added.

  Fresh pinned `ultimate-coop` windowed A/B on ad_sepulcher, RTX 3060 Ti,
  1280×720, 20 simulated seconds after 4 s warm-up, fixed 250 Hz,
  vsync off, `host_maxfps 10000`, three runs/build, release with line-table
  symbols on both sides. Medians select the run by fps as qprof does:

  | metric | before | after | change |
  |---|--:|--:|--:|
  | fps | 59.2 | 59.9 | +1.2% |
  | frame µs | 16888 | 16681 | −1.2% |
  | render CPU excluding fence µs | 12876 | 12168 | −5.5% |
  | fence wait µs | 2385 | 2905 | +21.8% |
  | GPU µs | 15766 | 15906 | +0.9% |
  | p99 frame µs | 41389 | 40700 | −1.7% |

  Before fps 60.0/59.2/58.9, after 60.0/59.9/59.4. Every run measures
  exactly 5000 frames; config hashes are unchanged. Render CPU ranges
  do not overlap (12648–13047 → 12168–12464 µs). Total-frame changes
  are within the run spread; no demonstrated fps gain or reference-engine
  parity claim. This fresh baseline is slower than the preceding session's,
  so these results must not be compared across sessions as another A/B.
  Reports: `target/qprof/vis-window-{before,after}/`.

  A separate follow-up headless full-cycle CPU capture no longer lists
  `mark_leaves` above the 0.1% main-core engine self-time threshold.
  `build_world_draw`/`gpu_world_draw` lead at 7.59%/7.11% (113k process
  samples, none lost). Both diagnostic captures use the same fixed-step
  20 s/4 s workload and measured-window filter; these shares are not a
  before/after frame-time comparison. Profiles:
  `target/qprof/vis-reprofile/`, `vis-reprofile-after/`; normalized reports:
  `target/perf-vis-reuse/{before,after}-self.txt`.

  Vanilla AD control (headless, pinned baseline, 8 s after 4 s warm-up,
  three runs/build) is 479.7→472.7 fps, frame 2084.78→2115.36 µs (+1.5%),
  render CPU 1805→1849 µs (+2.5%). These small changes sit within broad
  run variation (fps ranges 477.9–486.5 / 467.6–479.3); they do not
  establish a control speedup or regression. All three fixed-time combat
  screenshot pairs have AE = 0, and frame/FTE counters match in every
  control and co-op pair. Reports: `target/qprof/vis-controls-{before,after}/`.

  Quiet recursive e1m1 portals and four AD split views with transparent
  water both have AE = 0, as does a split-view run switching to opaque
  water during play (8x MSAA, 200% scale, high SSAO). The split checks
  explicitly enable enhanced water and translucency so `r_wateralpha`
  actually selects the tested modes. All six final Vulkan-validation runs
  are clean. Inspected the nested orange portals/player, the torch-lit
  split views with weapons/HUDs in both modes, and the full co-op combat
  and difference images. Nine live co-op pairs differ by 0–10022 pixels
  (up to 1.1%); baseline run 0 vs 1 differs by 178/61/9955 pixels at
  5/10/15 measured seconds. Differences occupy effects regions; these
  live variations do not replace the exact quiet-scene checks.
  Three new unit tests cover shared all-visible keys, restoring normal
  PVS after transparency toggles, missing-VIS/solid-leaf fallbacks,
  per-camera frustum results and portal surface deduplication.
  472 tests passed, 2 skipped; clippy clean. No server/physics change.
  Checks, config hashes and comparisons: `target/perf-vis-reuse/`.

  Next: investigate destination rendering, the largest aggregate GPU stage
  at 4771 µs in the median windowed run, before selecting another change.
  Retain the full-cycle cameras/workload and separate GPU time, render CPU
  and fence wait; smaller CPU costs alone have not established an fps gain.

- 2026-11-14, item 7 destination GPU breakdown (instrumentation): timestamps
  now separate setup, world drawing, models, particles and resolve/barriers
  inside each destination view. Queries are indexed by scene and destination,
  independently of reverse recursive draw order; only written prefixes are
  collected after the frame fence. `gpu_destinations_us` and qprof's new table
  sum these subintervals per scene, normalized by all profiled GPU frames.
  The count named `frames` in this diagnostic array counts destination draws,
  recursion included. These values are not added to aggregate GPU totals.
  The table also reports the enclosing stage minus its subintervals as gaps.
  Intervals include GPU stalls and boundary/timestamp overhead; setup includes
  clears and resolve includes rendering end, MSAA resolve and barriers.

  Fresh pinned `ultimate-coop` windowed A/B on ad_sepulcher, RTX 3060 Ti,
  1280×720, 20 simulated seconds after 4 s warm-up, fixed 250 Hz,
  vsync off, `host_maxfps 10000`, three runs/build, release with line-table
  symbols on both sides. Medians select the run by fps as qprof does:

  | metric | before | instrumented |
  |---|--:|--:|
  | fps | 60.9 | 60.4 |
  | frame µs | 16428 | 16556 |
  | render CPU excluding fence µs | 11957 | 11647 |
  | fence wait µs | 2903 | 3343 |
  | GPU µs | 15783 | 15935 |
  | destinations GPU µs | 4758 | 4745 |
  | p99 frame µs | 40995 | 39928 |

  Before fps 61.7/60.7/60.9, instrumented 60.2/60.4/60.7; all six runs
  measure exactly 5000 frames with identical FTE counters. This change adds
  diagnostics, with frame time +0.8% and GPU time +1.0% in these medians;
  **no optimisation or speedup claim**. CPU/fence differences must not be
  interpreted as a CPU improvement. Config hashes are unchanged. Build/warm-up
  and interrupted batches are excluded. Reports:
  `target/qprof/destination-stages-window-{before,after}/`.

  Instrumented median destination contribution across all four scene views:

  | subinterval | µs per GPU frame | share |
  |---|--:|--:|
  | setup | 26 | 0.5% |
  | world | 3727 | 78.5% |
  | models | 247 | 5.2% |
  | particles | 662 | 13.9% |
  | resolve/barriers | 83 | 1.8% |

  All three runs put world drawing at 77.8–78.5% of destination GPU time.
  Scene destination draw counts are 2514/2136/2557/2215 across 4998 profiled
  GPU frames; their subintervals reconcile with the enclosing stage within
  report rounding (aggregate residual 0.02–0.05 µs/frame). World submission
  currently uses the destination PVS list without the main view's GPU
  frustum/facing rejection. With transparent liquids that list is all-visible.
  Next: test conservative destination visibility rejection, comparing saved
  GPU work against extra CPU construction and lost draw-range cache sharing.
  Aperture/scissor bounds are another candidate, but must retain full-size
  projection, recursive sampling, teleporter UV warp and filtering margins;
  neither optimisation is implemented here.

  Vanilla AD control (headless, pinned baseline, 8 s after 4 s warm-up,
  three runs/build): 487.0→480.0 fps, frame 2053→2083 µs (+1.5%), render
  CPU 1705→1801 µs, GPU 1943→1935 µs. FPS ranges 477.6–493.2 / 478.6–481.8
  overlap. All runs measure 2001 frames, FTE counters match and all three
  combat screenshot pairs have `compare -metric AE = 0`. Reports:
  `target/qprof/destination-stages-controls-{before,after}/`.

  Quiet recursive e1m1 portals, four AD split views with transparent water,
  and the split scene switching to opaque water also have AE = 0 (8x MSAA,
  200% scale, high SSAO). All six Vulkan-validation runs are clean. Inspected
  nested orange portals/player, four torch-lit views with weapons/HUDs, full
  co-op night vision and the live combat difference image. Nine live co-op
  pairs differ by 66–10109 pixels (up to 1.1%), concentrated in effects;
  baseline run 0 vs 1 differs by 186/208/7819 pixels at 5/10/15 measured
  seconds. These live differences do not replace the exact controlled checks.
  Tests cover reverse-order query intervals, changing/absent scene counts,
  slot bounds, old/new profile parsing and avoiding double-counted GPU work.
  472 tests passed, 2 skipped; clippy clean. No shader or server/physics change.
  Checks, config hashes and comparisons: `target/perf-destination-stages/`.

- 2026-11-14, item 7 aperture scissors: destination rasterization is bounded
  by the source aperture projected with the scene's existing view matrix.
  Bounds expand by the sheet shader's 0.25-unit acceptance margin, the
  teleporter warp amplitude (0.0015 of the full texture dimensions, including
  split views), and two pixels for sampling/rounding. Near-plane crossings or
  invalid projections use the full view; off-screen bounds clamp to the scene
  region. Nested portals already supply source-space bounds. Target sizes,
  projection, complete scene clears, shared geometry ranges, lightmap unions,
  draw order and shader sampling remain unchanged. No additional GPU resources
  or settings are introduced. This is specific to this port's aperture
  sampling, rather than a reference-engine surface-culling port.

  Fixed recursive e1m1 portal benchmark, RTX 3060 Ti, 1280×720 headless,
  pinned qprof vanilla plus 8x MSAA, 200% render scale, high SSAO and portal
  depth 4, fixed 250 Hz, 6 simulated seconds measured after 2 s warm-up,
  three runs/build. Facing corridor portals are placed at
  `480 80 48 / normal 0 1 0 / up 0 0 1` and
  `480 330 48 / normal 0 -1 0 / up 0 0 1`, with camera
  `setpos 470 160 40 0 80 0`, god/notarget, no combat or rotation.
  Medians select the run by fps:

  | metric | before | after | change |
  |---|--:|--:|--:|
  | fps | 194.5 | 254.8 | +31.0% |
  | frame µs | 5142 | 3925 | −23.7% |
  | GPU µs | 5066 | 3847 | −24.1% |
  | destinations GPU µs | 1795 | 551 | −69.3% |
  | destination world GPU µs | 1302 | 162 | −87.6% |

  Before fps 194.8/194.3/194.5, after 254.9/254.8/254.4; every run measures
  1501 frames. All three fixed-time screenshot pairs have AE = 0. These are
  our engine's headless A/B results, including readback, not windowed or
  reference-engine parity numbers. Reports and commands:
  `target/perf-destination-scissor/portal-{bench,comparison}*`.

  Fresh full-cycle `ultimate-coop` windowed A/B on ad_sepulcher, same GPU
  and output resolution, all enhancements/add-ons, 20 simulated seconds after
  4 s warm-up, fixed 250 Hz, vsync off, `host_maxfps 10000`, three runs/build,
  release with line-table symbols on both sides. Windowed runs use isolated
  copies of the user's configs; source config hashes remain unchanged.

  | metric | before | after | change |
  |---|--:|--:|--:|
  | fps | 61.5 | 62.7 | +2.0% |
  | frame µs | 16251 | 15940 | −1.9% |
  | render CPU excluding fence µs | 11201 | 11531 | +2.9% |
  | fence wait µs | 3518 | 2826 | −19.7% |
  | GPU µs | 15710 | 15273 | −2.8% |
  | destinations GPU µs | 4784 | 4299 | −10.1% |
  | destination world GPU µs | 3734 | 3350 | −10.3% |
  | p99 frame µs | 37779 | 39815 | +5.4% |

  Before fps 62.6/61.5/61.2, after 62.9/62.7/62.5. FPS ranges overlap;
  the overall co-op change is small. Destination GPU ranges do not overlap
  (4711–4784 → 4280–4319 µs), establishing reduced destination GPU cost in
  this workload. There is no CPU saving or p99 improvement claim: render CPU
  rises in the selected medians, and p99 ranges overlap (37779–39909 /
  39285–39815 µs). All six runs measure exactly 5000 frames with identical
  FTE counters and destination counts (2514/2136/2557/2215 draws across
  4998 profiled GPU frames). Reports:
  `target/qprof/destination-scissor-window-{before,after}/`.

  Vanilla AD headless control (8 s after 4 s warm-up, fixed 250 Hz,
  three runs/build): 483.3→481.8 fps, frame 2069→2075 µs (+0.3%), GPU
  1940→1939 µs; fps ranges 483.1–489.8 / 477.2–483.7 overlap. All runs
  measure 2001 frames, FTE counters match and all three captures have AE = 0.
  Reports: `target/qprof/destination-scissor-controls-{before,after}/`.

  Nine quiet/edge scene pairs have `compare -metric AE = 0`: recursive
  e1m1 portals; AD transparent split views and transparency toggling;
  isolated AD teleporters at 4/128/2048 units (including sample shading and
  warped sampling); nearby gun portals, a wide pitched/rolled view at 73%
  render scale and a portrait view at 25% scale with no MSAA. All 18
  Vulkan-validation runs are clean. Inspected the nested orange portal/player,
  warped teleporter view, wide portal scene, benchmark capture and co-op
  difference image. Nine live co-op pairs differ by 66–9109 pixels (under 1%),
  concentrated in effects; baseline run 0 vs 1 differs by 159/0/9256 pixels
  at 5/10/15 measured seconds. Live variation does not replace exact captures.
  Four new tests cover texture-sized split-view warp margins, clipping/invalid
  fallbacks, clamped empty scissors and projected samples in transformed
  nested apertures across varied viewports and camera angles.
  476 tests passed, 2 skipped; clippy clean. No shader or server/physics change.
  Checks and comparisons: `target/perf-destination-scissor/`.

  Next: measure how often aperture bounds fall back at the near plane and
  how much remaining world cost comes from geometry outside the destination
  frustum. Use those findings to choose tighter aperture clipping or GPU
  surface rejection; avoid losing draw-range sharing for a small GPU saving.

- 2026-11-14, item 4 qpart CPU simulation & geometry batching:
  - `crates/qpart/src/util.rs`: replaced runtime `f32::sin`/`f32::cos` in
    `psin`/`pcos` with precomputed 128-entry `PSIN`/`PCOS` tables matching
    `libm::sinf`/`cosf` at quantized table angles.
  - `crates/qpart/src/system{,.rs,/defs.rs,/update.rs,/types_pass.rs}`:
    moved non-beam particles from index-chased linked lists in `self.parts`
    into per-type contiguous `Vec<Particle>` (`TypeRt.vec`, oldest-to-newest)
    while keeping `RenderType::Beam` particles in the arena for stable
    `BeamSeg::p` handles; replaced per-frame `updates`/`run_list.clone()`/`keep`
    allocations with reusable `run_scratch` + in-place `run_list`; added
    order-preserving slice compaction (`reap_vec`) and a hoisted fast path in
    `update_type_particles`; split `update_types`/`draw_beam` into
    `crates/qpart/src/system/types_pass.rs`.
  - `crates/qpart/src/geom.rs`, `crates/qpart/src/geom/shapes.rs`: added
    `Sink::particles_rev`, hoisting view vectors, type look/blend state,
    texture lookup, and `push_batch` per particle type instead of dispatching
    per particle.

  A/B benchmarks on RTX 3060 Ti, 1280×720, `qprof -runs 3` (20 s measured
  after 4 s warm-up, sim step 1/250 s), exact matching FTE counters on
  `ad_sepulcher` (`vanilla`: live 33611, decals 210, vertices 53626, indices
  80124; `all-enhancements`: live 33611, decals 210, vertices 135014,
  indices 202207):

  Windowed `ad_sepulcher` (`-window -fixedstep 250 -maxfps 10000`, uncapped):

  | scenario | metric | before | after | change |
  |---|---|--:|--:|--:|
  | vanilla | fps | 450 | 816 | +81.3% |
  | vanilla | frame µs | 2223 | 1225 | −44.9% |
  | vanilla | render cpu (excl. fence) µs | 2081 | 1084 | −47.9% |
  | vanilla | p99 frame µs | 3784 | 2384 | −37.0% |
  | vanilla | GPU µs | 887 | 893 | +0.7% |
  | all-enhancements | fps | 122 | 122 | +0.0% (CPU→GPU bound) |
  | all-enhancements | frame µs | 8201 | 8182 | −0.2% |
  | all-enhancements | render cpu (excl. fence) µs | 6861 | 5139 | −25.1% |
  | all-enhancements | fence µs | 1132 | 2828 | +149.8% |
  | all-enhancements | GPU µs | 8132 | 8121 | −0.1% |

  Headless (`-headless`, offscreen readback in `ui+present`, `1280×720`):
  - `ad_sepulcher` `vanilla`: `render cpu` 1887→1023 µs (−45.8%), fps 479→490,
    fence 107→916 µs, GPU 1954→1952 µs.
  - `ad_sepulcher` `all-enhancements`: `render cpu` 5677→4616 µs (−18.7%),
    fps 109→108, fence 3359→4443 µs, GPU 9070→9102 µs.
  - `e1m1` (0 particles): `vanilla` 755→756 fps (`render cpu` 234→232 µs);
    `all-enhancements` 140→139 fps (`render cpu` 1661→1718 µs).

  Verification: `vanilla@ad_sepulcher` (9/9) and `vanilla@e1m1` (9/9)
  screenshots have `compare -metric AE = 0`; `all-enhancements` has `AE = 0`
  on 15/18 pairs (remaining 3 `#0_05`/`#2_05` pairs match baseline 5s variance).
  Vulkan validation clean (`-validation -headless` on `ad_sepulcher`); user
  `config.cfg` SHA-256 hashes unchanged; `cargo nextest run`, `cargo nextest
  run -p qpart`, and `cargo clippy --message-format=short` pass cleanly.
  Reports: `target/qprof/qpart-cpu-{before,after}/` and
  `target/qprof/qpart-cpu-win-{before,after}/`.

  Next (remaining item 4 work): per-view particle geometry culling / sharing
  across teleporter/portal/split views, and GPU-side quad expansion or
  weather/emitter optimizations.
- 2026-11-14, item 9 (impacts & blood CPU + shared `CollisionModel` /
  `brush_ents`): eliminated per-frame filesystem opens (`resolve_loose` /
  `read_dir`) for non-`.mdl` entities (`*1..*35`, sprites), per-frame
  `to_ascii_lowercase()` death-pose scans, eager `mesh::positions` builds
  over every entity, unculled triangle sweeps, per-frame `HashMap`
  allocations, and duplicate per-subsystem `CollisionModel` / `brush_ents`
  builds across `fte`, `blood`, `hitfx`, and `impacts`:
  - `src/blood/mesh.rs`: added zero-alloc `is_death_name` / `is_mdl_path`,
    `point_segment_dist_sq`, and `CachedMdl` (precomputing `is_flesh`,
    `has_death_pose`, `is_head`, `is_gib_or_head`, `death_frames`, and
    conservative entity bounding `radius`).
  - `src/impacts.rs`, `src/impacts/nails.rs`: cached `Option<CachedMdl>` per
    `ModelId` (storing `None` for brush/sprite models), reused `flights_scratch`,
    and switched nail triangle collision and `mesh::positions` to lazy
    per-ray bounding-sphere evaluation plus active `Anchor::Mesh` keys only.
  - `src/blood.rs`, `src/blood/events.rs`: cached `Option<CachedMdl>` per
    `ModelId`, replaced eager pose builds with `PoseSet::Lazy` + bounding-sphere
    rejection in `contact`/`shot_contact`, skipped `mesh::positions` on
    non-wounded gibs, computed poses only for active owned wounds in `draw`,
    and reused `trails_scratch`/`actors_scratch`/`gibs_scratch`.
  - `src/assets.rs`, `src/fte/world.rs`, `src/fte.rs`, `src/hitfx.rs`,
    `src/host/render.rs`: added `Level::collision_model()` (`OnceLock`) and
    `cached_world` so `fte`, `blood`, `hitfx`, and `impacts` share one lazy
    `CollisionModel` per level and one `brush_ents` list per frame.

  A/B benchmarks on RTX 3060 Ti, `1280×720` headless, `qprof -runs 3` (20 s
  measured after 4 s warm-up, sim step `1/250 s`) on `ad_sepulcher`, exact
  matching FTE counters (live 33611, decals 210, vertices 53626 / 135014,
  indices 80124 / 202207):

  | scenario | metric | before | after | change |
  |---|---|--:|--:|--:|
  | vanilla | frame / render cpu µs | 2037 / 1031 | 2040 / 1025 | +3 / −6 µs |
  | impacts | fps | 462 (CPU) | 491 (GPU) | +6.3% |
  | impacts | frame / render cpu µs | 2163 / 2037 | 2037 / 1026 | −126 / −1011 µs (+1006→+1 µs vs vanilla) |
  | blood | fps | 473 (CPU) | 490 (GPU) | +3.6% |
  | blood | frame / render cpu µs | 2115 / 1964 | 2040 / 1038 | −75 / −926 µs (+933→+13 µs vs vanilla) |
  | water | frame / render cpu µs | 2225 / 1199 | 2225 / 1169 | +0 / −30 µs |
  | all-enhancements | render cpu µs | 4782 | 4210 | −572 µs (−12.0%, GPU-bound 9056 µs) |

  Verification: `compare -metric AE = 0` across all 12 pairs (`vanilla`,
  `impacts`, `blood`, `water` at `05`, `10`, `15` s); Vulkan validation clean
  (`-headless -validation -autoquit 4 -game ad +map ad_sepulcher +r_blood 1
  +r_impact_bullets 1 +r_impact_nails 1 +r_impact_scorches 1 +r_impact_fx 4`);
  full `cargo nextest run` (with game data) and `cargo clippy --message-format=short`
  pass cleanly. Reports: `target/qprof/item9-{before,after}/`.

- **2026-11-14 — Item 9 (`water`): skip `novis` on water-vised maps & `WaterFx` fast paths**:
  Root cause on `ad_sepulcher` (`58233` leaves): `frame.rs` previously called
  `wg.cpu.set_transparent_liquids(alphas.iter().any(|&a| a < 1.0))` unconditionally,
  forcing `novis` (`vis_leaf = 0`, all 58,233 leaves + whole-map GPU cull/lightmap/world/models)
  even though `ad_sepulcher` already has water-vised PVS across its liquid leaves.
  Additionally, `WaterFx::update` called `world.liquid_surface` and tracked hash maps
  for stationary entities and scanned entities even when ripples/splashes were disabled.
  Changes on `improve/perf-water`:
  - `crates/qrender/src/world{,.rs,/build.rs,/cull.rs}`: added `water_vised`
    (`World::check_water_vis`, porting ironwail's `Mod_CheckWaterVis` with zero-alloc
    RLE vis scan per liquid content type) and `World::needs_transparent_novis(alphas)`
    so `novis` is only enabled when an actually translucent liquid (`alpha < 1.0`) is
    un-vised on the map.
  - `crates/qrender/src/{frame.rs,water.rs}`: gated `set_transparent_liquids` with
    `needs_transparent_novis(alphas)`, fast-reset `WaterFx` when both ripples and
    splashes are disabled, skipped `liquid_surface` / extra map lookups for stationary
    entities, and avoided duplicate `liquid_surface` queries for moving entities.

  A/B benchmarks on RTX 3060 Ti, `1280×720` headless, `qprof -runs 3 -shots 5` (20 s
  measured after 4 s warm-up, sim step `1/250 s`) on `ad_sepulcher` and `e1m1`:

  | map | scenario | metric | before | after | change |
  |---|---|---|--:|--:|--:|
  | `ad_sepulcher` | `vanilla` | fps / frame µs | 491 / 2035 | 491 / 2036 | +0 / +1 µs |
  | `ad_sepulcher` | `water` | fps | 450 | 488 | +38 (+8.4%) |
  | `ad_sepulcher` | `water` | frame / p99 µs | 2225 / 3692 | 2049 / 3119 | −176 / −573 µs (+190→+14 µs vs vanilla) |
  | `ad_sepulcher` | `water` | render cpu µs | 1190 | 1063 | −127 µs (+169→+46 µs vs vanilla) |
  | `ad_sepulcher` | `water` | GPU total µs | 2138 | 1962 | −176 µs (+189→+13 µs vs vanilla) |
  | `ad_sepulcher` | `water` | GPU `cull` / `lightmap` / `world` / `models` (median run) | 68.12 / 122.73 / 95.03 / 45.46 | 25.55 / 27.96 / 70.62 / 30.96 | −42.57 / −94.77 / −24.41 / −14.50 µs |
  | `e1m1` | `water` | fps / frame / GPU µs | 768 / 1302 / 1206 | 772 / 1294 / 1198 | +4 / −8 / −8 µs |

  Verification: `compare -metric AE = 0` across all 12 `qprof` pairs (`vanilla` and
  `water` at `05`, `10`, `15` s on `ad_sepulcher` and `e1m1`) and all 4 quiet water-feature
  screenshots (`e1m1`, `e1m3`, `ad_sepulcher`, `ad_tears`); Vulkan validation clean
  (`-headless -validation -autoquit 4` on `ad_sepulcher` and `e1m1` with `water` cvars);
  full `cargo nextest run` (with game data) and `cargo clippy --message-format=short` pass
  cleanly. Reports: `target/qprof/water-{before,after}/`.

  Next candidate items: remaining Item 4 (GPU billboard expansion / per-view particle
  culling across multiple views), Item 8 (`ultimate-coop` server p99 spikes), Item 10
  (`check_water_transition` / `point_contents`), Item 11 (`aim_targets` in co-op).
