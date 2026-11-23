# Optimisation opportunities (2026-11-22)

Input for P5 in [PLAN.md](PLAN.md). Built from the P4 baseline
([BASELINE-2026-11-21.md](BASELINE-2026-11-21.md), its Hotspots by pass)
and a read of the code at `967783a`. For each item: what it costs, the
cause, what to change, and how to check the change. **Verified** means the
cause was confirmed in the code. **Suspected** means it still needs a
finer measurement. Gains are estimates until a perfkit run confirms them.

Rules from the plan still apply. Vanilla paths stay pixel-identical
(AE = 0). Changes to enhancements need a visual check, on wgpu/web too
(it must look right there, not match Vulkan pixel for pixel).

## Summary

| # | Item | Measured cost | Kind | Expected gain |
|---|---|---|---|---|
| 1 | SSAO top tier | +5975 µs at 4K | shader + targets | ~4–5 ms at 4K |
| 2 | Bloom glow attachment | +1554 µs at 4K | targets + pipelines | ~0.7–1 ms at 4K |
| 3 | Destination views drawn without culling | ~185 µs per view, 1.4 ms in all-split4 | CPU/GPU culling | ~0.1 ms per view |
| 4 | Ray-traced water: always on, no culling | +212 µs at 540p, +1168 at 4K | gating + BLAS + trace | most of it on dry views; ~half on wet ones |
| 5 | FTE particles (AD maps) | ~0.9 ms CPU, 0.5–1.3 ms GPU | CPU geometry + upload | ~0.4 ms CPU |
| 6 | Render-pass splits under 8× MSAA | not measured alone | pass structure | needs timing |
| 7 | Water droplets | +429 µs at 4K | post shader | ~0.3 ms at 4K |
| 8 | Per-frame cvar lookups and allocations (V9/H1) | small, all scenes | CPU | a few µs per frame, every frame |
| 9 | all-split4 `view+sound` 5 → 309 µs | 0.3 ms CPU | unknown | needs a profile |
| 10 | Classic particle traffic (`spin-sepulcher.dem`, 133 MB) | 120 µs client parse | CPU | needs a profile |

Suggested order: start with the small, safe changes (4a, 4b, 3c, 5a, 8),
which are mostly one-function edits. Then do the big ones by 4K cost: 1,
2, 3, 4, 5. Item 1a is a product decision, not an optimisation (see
Decisions).

---

## 1. SSAO top tier (`r_ssao 3`): `ao-generate` 291 → 4872 µs at 4K

**Cause (verified).**
- `ssao_quality` 2 renders AO at full scene resolution
  (`crates/qrender/src/lib/targets.rs:166`) with 12 taps instead of 8
  (`ao.frag`).
- Each tap calls `view_pos` → `fetch_depth` + `linear_depth`. With the 4
  neighbour fetches for the normal, that is 17 depth reads and 17
  reciprocal depth conversions per pixel.
- The tap radius is capped at a quarter of the screen height, which is 540
  px at 4K. Depth has no mip chain, so far taps land in random cache lines.
  This is why the cost grows 17× for 6× the work.
- **The Enhanced preset sets `r_ssao 3`** (`src/host/presets.rs:63`), so
  every Enhanced user pays the full ~6 ms at 4K.

**Changes.**
- a. *(Decision)* Enhanced uses `r_ssao 2`. This is a one-line change.
- b. **Linear-depth mip chain (SAO).** Build a mip chain once per frame:
  `aosource.frag` already writes sample 0 of depth, so have it write
  *linear* depth (R32F), then add 4–5 downsample levels. Each tap then
  reads mip `clamp(floor(log2(h)) - 3, 0, 4)`. This is the standard fix
  for scattered taps, and it brings the taps back into cache.
- c. Store linear depth in the AO source so the shader stops calling
  `linear_depth` per tap. This works together with b.
- d. Quality 2 at half resolution with 12 taps, using the 4×4 bilateral
  upsample that `aoapply.frag` already has. Look at it next to the current
  full-resolution output before choosing.

**Check.** perfkit `ssao` scene, both tiers, at 4K. A visual comparison of
on and top at the spawn of `spin-sepulcher`. Also check
`wgpu_backend` for the same shader.

## 2. Bloom: `world` 673 → 1407 µs, `post` 175 → 728 µs at 4K

**Cause (verified).**
- With `r_bloom 1`, every scene-pass pipeline writes a second colour
  attachment: the glow (`pipelines/desc.rs:116`). With 8× MSAA it is an 8×
  MSAA RGBA8 image at scene size (`lib/targets.rs:239-246`), which is
  resolved at the end of the pass.
- On every pass split (AO, soft particles, viewmodel: item 6) it is also
  stored and loaded again (`frame.rs:1415-1419`).
- Vulkan needs the same sample count on all colour attachments, so the
  glow can't simply be made single-sampled.
- `r_bloom_bright` defaults to 0, so the first downsample reads only the
  glow: 13 taps at full resolution per half-resolution pixel. That is
  reasonable.

**Changes (pick one).**
- a. **Glow as a separate, narrow pass.** Draw only the draws that can
  glow (fullbright textures, glow-capable alias skins, emissive particles)
  into a single-sampled half-resolution target, depth-tested against a
  half-resolution depth. The AO source already makes one when SSAO is on.
  Everything else stays on single-attachment pipelines. In Quake maps
  glowing draws are a small part of the scene.
- b. **Glow factor in the scene alpha.** Write a per-pixel glow fraction
  into the scene colour's alpha and get the glow back as `scene.rgb ×
  fraction` in the first bloom pass. This removes the attachment entirely.
  The catch: scene alpha is the AO sky mask (`ao.frag` reads `.a`), so the
  encoding must keep "0 = sky". Also, the glow colour becomes the lit
  colour rather than the fullbright colour.
- Either way, the wgpu backend has its own glow
  (`wgpu_backend/bloom.rs:180-181`) and should change in the same way.

**Check.** perfkit `bloom` scene at 4K (the world pass should return to
about 700 µs). A visual comparison on a map with lots of fullbrights
(ad_sepulcher torches, e1m1 lights).

## 3. Destination views drawn without culling (H7)

About 185 µs of world per view, and about 3× a split-screen player's main
world pass (187 against 58 µs at 540p). The cost is the same at 540p and
4K, so it isn't fill. In all-split4 that is four destinations, about 1.4 ms
of GPU and the scene's biggest single item.

**Cause (verified).**
- a. A destination view draws `portal_surfaces(portal.pvs)`
  (`frame/portals.rs:176`). That is every surface in the destination leaf's
  PVS, built on the CPU and cached per leaf, with no frustum test, facing
  test or aperture test. The main view instead goes through `cull.comp`
  (leaf frustum + backface + surface frustum).
- b. `draw_portals` then draws every brush entity of the main list
  (`frame/portals.rs:307`, `brushes.iter().skip(1)`).
- c. Whenever any destination view exists, the **main** view also stops
  frustum-culling brush entities (`frame.rs:341`, `frame.rs:347`:
  `portal_draws.is_empty() && …`). With the portal gun out or a teleporter
  window in view, the main view draws every brush entity in the map.

**Changes.**
- a. Run `cull.comp` once per destination. Use the destination's frustum,
  narrowed to the aperture scissor that `portal_scissor::aperture` already
  computes, plus the portal clip plane. Each run needs its own slice of the
  indirect-command and output-index buffers. Simpler fallback: CPU
  frustum-cull the cached PVS list per view (this loses the per-leaf cache
  but still draws far less).
- b. Cull brush entities per destination: sphere/box against the
  destination frustum, mapped with `from_destination` as the alias models
  already are (`frame/portals.rs:354`).
- c. In the main view, keep frustum culling for brush entities and give
  destinations their own culled list. This is pixel-identical and the
  smallest of the three changes.

**Check.** perfkit `teleporter-windows`, `portalgun` (on and top) and
`all-split4`. Screenshots through a teleporter window and a recursive
portal pair must be unchanged.

## 4. Ray-traced water reflections: +212 µs at 540p, +1168 µs at 4K

**Cause (verified).**
- a. `rt_on = refl_mode == 2 && (view or splits)` (`frame.rs:45`). It
  doesn't depend on whether any water is in view. On a dry view, the
  renderer still:
  - poses every alias model;
  - refits or rebuilds the alias BLAS and rebuilds the TLAS
    (`frame/rt.rs:120-175`);
  - adds the eye's whole PVS to the lightmap updates (`frame.rs:276-282`);
  - stops frustum-culling brush entities (`frame.rs:341`, `frame.rs:347`).
- b. The alias BLAS has one geometry without the `OPAQUE` flag
  (`rt.rs:104`). So every alias candidate hit runs `rt_opaque_at` in the
  shader (`rt.glsl:73`), although only "holey" skins (`texs.w & 1`) need
  it.
- c. All alias models in the first view's list are posed, however far
  they are from any water.
- d. One ray query per water fragment at full resolution: `translucent`
  goes from 94 to 971 µs at 4K.

**Changes.**
- a. **Gate on visible water.** Enable RT only when the frame draws a
  liquid surface (any `wd.water`/`wd.water_alpha` range in the main or a
  split view's world draw, or last frame's count from the cull pass). Dry
  views then cost nothing. Keep the TLAS from the last wet frame so
  reflections don't pop for a frame when water comes into view, or build it
  on the first wet frame and accept one frame of latency.
- b. Split the alias BLAS into two geometries: opaque skins first with
  `GeometryFlagsKHR::OPAQUE`, holey skins second. `prepare_rt` already
  orders the draws, so sort by `texs.w & 1`. This cuts any-hit work for
  almost every model.
- c. Pose only models that a reflection can reach: inside the view
  frustum mirrored about the visible water plane, or within a radius of the
  visible water's bounds. Most maps have one water plane in view at a time.
- d. *(Visual change)* At `r_water_quality 1`, trace at half resolution
  with a depth-aware upsample. Ripples already blur the reflection.
  Optionally skip the ray where the Fresnel weight is below a threshold and
  use the fallback colour.
- Minor: `prepare_rt` allocates `out`, `sig` and `inst` every frame. Keep
  them on the frame slot.

**Check.** perfkit `water-rt` (on and top), plus a dry-view live scene
with `r_water_reflections 2` (for example the e1m1 spawn), which should
cost about zero after a. Reflection screenshots over e2m1's pool.

## 5. FTE particles: ~0.9 ms CPU, 0.5–1.3 ms GPU on AD maps (H4, V1)

This is the largest CPU cost measured, and it is also the 1.88 ms of a live
ad_sepulcher frame with nothing on. ~34k particles give 57k–102k vertices
per frame.

**Cause (CPU part verified, GPU part suspected).**
- Every particle is expanded to a quad on the CPU (4 vertices + 6
  indices). `src/fte.rs` reuses its buffers between frames (good). But
  `frame.rs:709-711` copies all vertices, indices and batches again with
  `concat()` when spray or gel batches exist. `frame.rs:724` copies them
  once more with `to_vec()` when any portal-gun portal exists. Then
  `dynw.push` copies them into the mapped buffer (`frame.rs:764`). With the
  gel sprayer and portal gun out, that is three full copies of ~100k
  vertices per frame.
- Every batch binds its pipeline again, even when it is the same as the
  previous batch (`frame.rs:1756-1764`).
- Under 8× MSAA with soft particles on, `depth_samples = samples`: the
  fragment shader reads 8 depth samples per particle fragment.

**Changes.**
- a. **No extra copies.** Append the spray, gel and gun-portal quads to
  the host vectors before they reach the renderer (they are built from the
  same `FteVertex` type), or write them directly after `fr.fte_vertices` in
  the dynamic buffer. This is pixel-identical.
- b. **Vertex pulling.** Upload one record per particle (position,
  colour, size, rotation, UV rect: about 32 bytes) and build the quad in
  `fte.vert` from `gl_VertexIndex`, with no index buffer. That is 4× less
  CPU writing and upload. Beams, lines, decals and model-shaped effects keep
  the current path. This must be pixel-identical: compare
  `spin-sepulcher-fte` stills.
- c. Merge adjacent batches with the same pipeline, texture and mode, and
  skip binding a pipeline that is already bound.
- d. Soft particles: read one depth sample (or the sample-0 snapshot the
  AO source makes) instead of all 8. This changes soft edges slightly under
  MSAA, so do a visual check.
- e. Then time the FTE batches alone (a timestamp pair around them) and
  run `perf` on `sys.frame`, to split the 0.9 ms between simulation and
  geometry.

**Check.** perfkit `spin-sepulcher-fte`, `goggles-*` (live ad_sepulcher
base) and `all-split4`, plus `perfkit partinfo` for like-for-like particle
counts.

## 6. Render-pass splits under 8× MSAA

**Cause (verified, cost not measured).** The scene pass ends and restarts
up to three times (`frame.rs:1471`, `1643`, `1743`, `1770`):
- after the opaque draws when SSAO is on;
- before soft FTE particles (depth becomes read-only);
- before the viewmodel (depth writable again).

Each restart stores and loads every MSAA attachment: colour, D32 depth,
and the bloom glow when on. At 4K with 8× MSAA that is ~265 MB per
attachment per split before compression.

**Changes.**
- Soft particles read a depth copy (the AO source's sample-0 snapshot
  when SSAO is on, otherwise a one-off copy at the same split point)
  instead of the live attachment. Depth then stays writable, and the
  viewmodel split goes away.
- Removing the glow attachment (2) makes each remaining split cheaper.

**Check.** First measure: add GPU timestamps around a split, or A/B with
`r_ssao 1` with and without soft particles on `spin-sepulcher-fte` at 4K.

## 7. Water droplets: `post` 229 → 571 µs at 4K

**Cause (verified).** `ws_droplets` (`water_screen.glsl:64`) runs for
every pixel of the post pass during the 2 s window: one or two bead layers
with hashed cells, one or two runner layers with sway and trail hashing,
and on high quality 4 extra blurred scene taps where drops are. Its result
is low frequency (cells are 1/11 and 1/26 of the screen), but it is
computed at full resolution.

**Changes.**
- a. Evaluate the droplet field (offset.xy, mask, spec/rim) into a
  quarter-resolution RGBA16F target once per frame, and sample it
  bilinearly in `post.frag`. Rims and highlights may soften, so do a visual
  check. At 4K that is 1/16 of the work.
- b. `post.frag` is an uber-shader: goggles, motion blur, droplets,
  refraction and palette all live in one pipeline. Specialisation
  constants for "droplets off" or "refraction off" may lower register use
  for the common case. Check occupancy with Nsight before and after.
- Note: the droplets' "off" run had a 9% spread at 4K (1.521 ±0.138 ms).
  Re-measure before and after.

## 8. Per-frame cvar lookups and allocations (V9, H1)

**Cause (verified).**
- The `Settings` build in `src/host/render.rs` (from line 442, `let v =
  |n| self.cv(n)`) does about 109 cvar name lookups every frame. The rest
  of `src/host/*.rs` adds more (`frame.rs` 19, `render.rs` 33 direct
  `cv(…)` calls), and `audio.rs` does `by_name`/`value_of` per frame.
- The V9 census counted ~114 allocations per steady-state frame (status
  bar text, `view_cvars`'s `format!` names, `user_dir` path joins,
  `Lang::from_code`, …).
- Also seen while reading the code:
  - `frame.rs:355` builds a new `surfs` `Vec` per brush entity per frame;
  - `frame/bloom.rs:56` builds `vps` per view;
  - `prepare_rt` allocates three `Vec`s per frame.

**Changes.**
- Rebuild `Settings` only when a cvar has changed. Use a global cvar
  change counter (add one if there isn't one) and keep the last `Settings`
  otherwise. This removes nearly all the lookups at once, which beats
  switching each lookup to a `CvarId`. Per-frame inputs (time, view,
  goggles) stay outside the cached part.
- Work through the V9 census in its order (status bar, `user_dir`,
  language code, cshift names), then re-run heaptrack.
- This is pixel-identical by construction. Check with an A/B of the 540p
  vanilla demos (540p spread is under 3%, so a few µs only show in
  callgrind).

## 9. all-split4: `view+sound` 5 → 309 µs (H5, H11)

**Cause unknown.** The audio probes don't explain it. Reverb and occlusion
run for the main view only (`src/audio.rs:112-141`) and are throttled by
time and distance. More likely it is per-player work that grows with four
views: `view_cvars`, blood, impacts and debris per view, or aim-assist
target search (H11). Profile with `perf record` on a 4-player live run.
After that, the P2 audio counters settle whether the mixer thread matters.

## 10. Classic particle traffic: `spin-sepulcher.dem` is 133 MB

The client phase costs 120 µs per frame on this demo (11 µs on e1m1). The
baseline calls it a recorder problem. It is more likely real traffic:
recorded with `sv_fteparticles 0`, AD sends classic particle messages
every frame, as noted in VANILLA.md V1. The FTE version of the same map
and length is 12 MB.

First get a message-type histogram of the demo. If `svc_particle`
dominates, this is V1's CPU path: profile parsing and spawning of classic
particles and optimise those. If it doesn't, fix the recorder. Either way,
until then the 540p costs of bloom and SSAO read low against this baseline.

---

## Measurement fixes that change the list

These come before optimising, so items aren't chosen on wrong numbers.

- **Blood, impacts and debris** show ~0 cost in a rocket fight. They
  probably don't replay from a demo (P3). Move them to a live fight;
  ragdoll physics is CPU work that could be a hidden item 11.
- **Off-run directories overwrite each other** (`e1m1-off`,
  `ad_sepulcher-off`, …). Name them after the scene so per-pass analysis
  of the add-ons and goggles uses the right `profile.json`.
- **P2 audio counters**: until they exist, audio costs are unknown, not
  zero.
- **Run time** (71 min against 30): share one engine start across the
  timedemos of the same game, use 120 s spins, and run the quick tier
  without attribution runs.

## Decisions for the user

- Enhanced preset: keep `r_ssao 3` (item 1a), or drop to 2 until 1b/1c
  land.
- Bloom (item 2): a separate glow pass (2a: more code, keeps the
  fullbright colour) or the glow fraction in alpha (2b: less code, changes
  the AO mask encoding and the glow colour).
- Half-resolution ray-traced reflections at quality 1 (item 4d), and a
  quarter-resolution droplet field (item 7a): both change pixels in
  enhancement paths.
