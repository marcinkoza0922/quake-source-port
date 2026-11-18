# Part 2: This port's additions

What each of our additions costs on top of vanilla ultra, measured one at a
time, per quality tier, then all together. There's nothing to compare with
in other engines, so the baseline is our own vanilla ultra run. Shared
rules, resolutions, metrics and retooling steps are in
[README.md](README.md).

Appearance may change for a clear gain, but only behind a quality tier,
never by degrading the top tier silently (rule kept from HISTORY.md,
2026-11-14).

## Baseline

Part 1's settings ([VANILLA.md](VANILLA.md#settings)) in our engine, with
our default particle path (`r_fteparticles 1`, `r_particledesc classic`)
instead of classic particles, since nothing here is compared with ironwail.
Every addition below starts from this baseline and changes only its own
cvars.

**Pin every master toggle.** qprof today doesn't pin `r_enhancedfx`, which
gates blood, impacts and enhanced particles. The user's id1 `config.cfg`
has it on and ad's has none (default off). So the old `blood`, `impacts`
and `particles` scenarios probably ran with those effects off on
ad_sepulcher. R6 pins it, and a qprof test asserts that every cvar an
addition reads is pinned by the baseline or the scenario.

## Additions

Each row runs alone on top of the baseline at every tier, at 540p and 4K.
The reported number is the cost over the baseline: µs per frame at 540p
(CPU) and both µs and % of frame at 4K (GPU), plus p99.

| Addition | Enable | Tiers swept | Workload |
|---|---|---|---|
| Render scale | `r_renderscale` | 1.5, 2.0 (4K: 1.5, and 2.0 only if it fits in VRAM; record VRAM) | spin demos |
| Ambient occlusion | `r_ssao` | 1 / 2 / 3 | spin demos |
| Motion + speed blur | `r_motionblur 1`, `r_speedblur 1` | `r_motionblur_quality` 0 / 1 | combat demo |
| Enhanced water | `r_enhancedwater 1` + translucency, ripples, splashes, caustics | `r_water_quality` 0 / 1 / 2 | e2m2 demo + ad_sepulcher surface/underwater stills (HISTORY item 15) |
| Teleporter windows | `r_teleportwindows 1` | `r_portal_quality` 0 / 1 / 2 | `start` (episode teleporters), ad_sepulcher |
| Blood | `r_enhancedfx 1`, `r_blood 1` | `r_blood_quality` 0 / 1 / 2 | combat demo |
| Impacts | `r_enhancedfx 1`, `r_impact_fx` + bullets, nails, scorches | `r_particle_quality` 0 / 1 / 2 (caps the hit-effect tier; `r_impact_quality` is a no-op since 2026-11-16), plus `r_impact_limit` / `r_impact_lifetime` | combat demo |
| Enhanced particles | `r_enhancedfx 1`, `r_enhancedparticles 1` | `r_particle_quality` 0 / 1 / 2 | combat demo, ad_sepulcher (rain) |
| Soft particles | `r_particle_quality` (0 off, 1 low, 2 high; `r_softparticles` is a no-op since 2026-11-16) | 0 / 1 / 2 with `r_part_density` compensating quality 0's halved density, so only the soft level changes (matters because the baseline has 8× MSAA) | ad_sepulcher, combat demo |
| Ragdolls & debris | `r_debris 1` + ragdolls, gibs, shrapnel | `r_debris_quality` 0 / 1 / 2 | combat demo |
| Portal gun | customized game, `addon_portalgun 1` | `r_portal_quality` 0 / 1 / 2 × `addon_portal_depth` 0 / 2 / 6 × 2 or 4 portals | e1m1 fixed portals (`portal_set`), plus live placement |
| Gel sprayer | customized game, `addon_gel 1` | `addon_gel_quality` 0 / 1 | e1m1 spraying both gels; report p99 and max, the coat rebuild shows as spikes |
| Split-screen co-op | `coop 1`, `maxplayers N`, `cl_splitscreen N` | 2 and 4 players, vanilla ultra only | live, fixed step; per-view GPU table |
| Visible weapons (new 2026-11-18) | `cl_vwep 1` | off / on | the split-screen and portal scenes, where player bodies are on screen; expected tiny, checked once |

`r_particle_quality` is one preset for three knobs (soft particles,
impact tier, scripted particle density), so the Impacts, Enhanced
particles and Soft particles rows sweep the same cvar. Each row varies it
with only its own addition on and the density held constant, and the
report says which knob moved.

Blood, impacts, debris, water ripples and motion blur are all client-side,
so they should work on Part 1's demos. Using the same demos makes the
enhancement rows as repeatable as Part 1. Step A0 confirms this for each
row; any that don't replay fall back to live fixed-step runs.

Dropped from the old suite: the add-ons other than the portal gun and gels
(boomerang, sticky, hookshot, railgun, washer, cable, goggles, bullet time,
dodge), supersampling and its interaction pairs (HISTORY item 6),
`no-rain`, and colour, 8-bit emulation and colour-blind filters. Screen
shaders keep one regression check: `crt-guest-advanced-hd` at 4K against
the baseline, to catch a return of HISTORY item 22.

## All together

- **all-default**: every addition on at its default tier, portal gun and
  gels in use (two portals at default depth).
- **all-max**: every addition at its top tier, four portals at depth 6.
- **all-default-split4**: all-default with four split-screen players.

Report the excess too: all-together cost minus the sum of the individual
costs. A large excess means two additions interact (as SSAO and MSAA did
in HISTORY item 6) and the pair gets its own row.

The old `ultimate` / `ultimate-coop` (horde of monsters, everything on)
stays available with `qprof -only ultimate` as a stress test, but it isn't
part of the plan's numbers: with gameplay load mixed in, a slow result is
hard to attribute.

## Steps

- **A0. What replays from demos.** Record a short demo in a customized
  game with the portal gun and gels in use, and check playback shows them.
  Portal state travels as `//qsp_portal` stufftext, so it is probably in
  the demo; gel coats need checking. Record which enhancement rows render
  identically from a demo and live.
- **A1. Baseline.** qprof's baseline becomes vanilla ultra, every master
  toggle pinned, plus the pin test above.
- **A2. Tier sweeps.** A scenario can list a tier cvar and its values;
  qprof expands it and writes one table per addition (cost at each tier,
  both resolutions).
- **A3. Portal and gel scenes.** Fixed portal setups via `portal_set` (see
  the portal-gun skill) so the depth/count matrix is repeatable.
  `freecam x y z pitch yaw` (2026-11-17) can hold the camera on a fixed
  view while the game or a demo runs, which removes the player's movement
  from the measurement.
- **A4. Split-screen.** 2- and 4-player vanilla-ultra scenarios, sharing
  the co-op driving in `tools/qprof/src/ultimate.rs`.
- **A5. All-together scenarios** and the excess column in `report.md`.

## Items

Carried over from [HISTORY.md](HISTORY.md), to re-baseline here:

- **H7 (old item 7)**: destination views (portals, teleporter windows):
  remaining world drawing and clipping-plane fallbacks.
- **H5 (old item 5)**: split-screen GPU cost per view, now with correct
  per-view attribution.
- **H11 (old item 11)**: aim-assist target search per player in co-op.

Tools for these (README, Tools): spikes such as the gel coat rebuild
show best on samply's per-thread timeline, and the per-view CPU work in
split-screen (H5, H11) in a perf or callgrind profile of the 4-player
scenario. Additions that allocate per frame (HISTORY items 2, 19 and 20
removed some) get the same heaptrack census as V9.

New items go here once the first baseline exists, with a budget for each
addition proposed from it (for example, the default tier costs at most X%
of the 4K frame).

## Results

(date, commit, addition, tier, resolution, cost µs / % / p99, spread)

None yet.
