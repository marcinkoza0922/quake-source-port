# Performance plan

Started 2026-11-17. Replaces the single `PERF_PLAN.md` (now
[HISTORY.md](HISTORY.md)), which had grown into a results log that mixed
two kinds of work with different rules. The plan is in two parts:

| | [Part 1: Vanilla](VANILLA.md) | [Part 2: Additions](ADDITIONS.md) |
|---|---|---|
| What | the base game at "vanilla ultra" settings, remaster md5 models included | each of this port's additions on top of vanilla ultra: alone, by quality tier, then all together |
| Compared with | ironwail and vkQuake | our own vanilla ultra baseline |
| Output rule | must look like the references (stills); optimisations stay pixel-identical (AE = 0) | appearance may change for a clear gain, behind quality tiers |
| Workload | timedemos recorded by our engine, played in all three engines | our engine only: demos where possible, otherwise fixed-step live runs |
| Tool | `qbench` (retooled) | `qprof` (retooled) |

Add-ons are out of scope, except the portal gun and the gel sprayer.
Screen shaders, 8-bit colour emulation and the colour-blind filter are
excluded too: they replace the look instead of adding work. The one known
cost there (slang CRT presets, HISTORY item 22) gets a single regression
check at 4K, not a tier sweep.

## Status (2026-11-17)

Branch `improve/perf-plan`. Details for each step are under Retooling
steps below; items are in [VANILLA.md](VANILLA.md#items) and
[ADDITIONS.md](ADDITIONS.md#items).

Done:

- R1–R3: the perfkit runner, the vanilla ultra settings table and the
  recorded demo set.
- R4: qbench runs Part 1 (both suites, 540p and 4K). The first full run's
  numbers are **void** (VANILLA.md, V4).
- R5: `perfkit stills`, with all nine parity views within the band.
- Engine fixes R5 found:
  - water alpha follows the map and `r_wateralpha` (default now 1);
  - no startup menu over a command-line demo;
  - external world textures (`textures/[<map>/]<name>`) load like the
    references' (V5, ad_tears' waterfall).
- V3: `perfkit partinfo`. The FTE sepulcher scene is like for like
  (+0.1% particles); id1's combat had no FTE particles, so V6 replaced
  it in the FTE suite with `combat-grendel-fte` (like for like, −0.1%).
- Demo set re-recorded (`56238f2-dirty`, all eight scenes verified in all
  three engines), with the combat script fixed: `god` and the first
  weapons used to be sent before the player spawned.
- Tooling fixes R5 found: the sandbox's `quake.rc` without `menu_main`,
  and a run's own `+cvar` replacing the base one.

Next, in order:

1. **Rerun qbench** (`target/release/qbench`, about 70 min) on the new
   demo set. Replace VANILLA.md's Results table and re-check V1/V2
   against the new numbers.
2. **V7**: why ours keeps more decals than vkQuake in the FTE combat.
3. **V1/V2** (if they hold after the rerun): classic-particle CPU cost on
   ad_sepulcher at 540p; the FTE cold-start hitch.
4. **R6**: qprof → Part 2 (ADDITIONS.md, steps A0–A5), including pinning
   `r_enhancedfx` in its baseline.
5. **R7**, if A0 shows portal and gel state replays from demos; then
   **R8**, the docs pass.

## Resolutions

Every measurement runs at two resolutions, each answering one question:

- **960×540** keeps the GPU idle, so frame time is CPU time: it shows CPU
  bottlenecks (world/entity setup, particles, draw submission, driver).
- **3840×2160** makes the GPU the limit: it shows which costs scale with
  pixels (MSAA, overdraw, post passes, shading).

The monitor is 2560×1440, so 4K runs inside `gamescope -w 3840 -h 2160
-W 2560 -H 1440` (installed, 3.16). 540p runs in a plain window: the R1
calibration showed gamescope costs 3–9% of frame time and not the same
share in every engine (see R1 below), so it is used only where it can't be
avoided, and engines are only ever compared within the same display mode.

## Test machine and noise control

RTX 3060 Ti (driver 620.70.06), i7-12700K (20 threads), Nobara/KDE. Every
report records GPU, driver, CPU, kernel, commit and the gamescope version.

- Median of 5 runs, with min–max spread reported. A change is a regression
  or a win only if the medians differ by more than 3% **and** by more than
  the larger run spread.
- CPU governor `performance` for the session (ask the user; don't change
  system settings ourselves). Locking GPU clocks (`nvidia-smi -lgc`) needs
  root and is optional; if used, record it in the report.
- Windowed runs are killed with SIGKILL so `config.cfg` is never written;
  the harness hashes every engine's config before and after and refuses to
  report if one changed. Always pass `+sv_autosave 0` to our engine.
- No other GPU load during a run (browser video, Blender, etc.).

## Metrics

- Mean fps and frame time from runs **without** MangoHud; **p50 / p99 /
  max frame time** for every engine from separate runs **with** a MangoHud
  frame log (works with ironwail's GL and the Vulkan engines alike), because
  a timedemo only prints an average. The split is because MangoHud's
  per-frame logging costs 0.5–8% (25% for ironwail inside gamescope at
  540p), unevenly between engines (R1).
- Ours additionally: host phases and GPU stage timestamps (`prof_start` /
  `prof_stop`, `QSP_GPUTIMES`), in a separate attribution run so the
  instrumentation never shares a run with the comparison numbers.
- Cold start: the first pass of each demo with every shader/pipeline cache
  removed (`__GL_SHADER_DISK_CACHE=0` for the NVIDIA driver cache, plus each
  engine's own pipeline cache file), reported as worst frame and count of
  frames over 33 ms. Warm medians hide compile hitches, and md5 models and
  new effects add pipelines.

## Retooling steps

Done in order; each is its own branch/PR. Shared code (settings table,
scene list, runner, baselines, report schema) goes in a small library crate
`tools/perfkit` used by both tools, so it isn't duplicated.

- **R1. Runner — done 2026-11-17.** `tools/perfkit` (DESIGN.md, Tooling):
  sandbox without user configs, config guard, windowed or gamescope runs,
  MangoHud frame logs cut to exactly the demo's frames, machine header.
  `perfkit calibrate` result (`target/perf/calibrate/report.md`; 120 s
  ad_sepulcher spin demo, ~7500 frames; 5 interleaved rounds; each engine
  at its own defaults, ours with the user's config, so **engines are not
  comparable with each other here**, only with themselves):

  | engine | res | window fps | gamescope cost | MangoHud cost (window / gamescope) |
  |---|---|--:|--:|--:|
  | ours | 960×540 | 1193 | +7.2% | +3.7% / +4.6% |
  | ours | 1920×1080 | 492 | +8.6% | +1.7% / +1.6% |
  | ironwail | 960×540 | 4285 | +5.5% | +8.2% / +25.1% |
  | ironwail | 1920×1080 | 2857 | +5.3% | +3.3% / +2.2% |
  | vkQuake | 960×540 | 617 | +6.7% (within noise) | +0.6% / −1.7% |
  | vkQuake | 1920×1080 | 617 | +3.3% (within noise) | +0.5% / +1.0% |

  Gamescope doesn't cap the frame rate, but it costs every engine a
  different share and adds periodic stalls (vkQuake p99 2.0 → 6.6 ms).
  Hence: 540p in a plain window, gamescope only for 4K, and fps never taken
  from MangoHud runs. Run-to-run spread is about 1–2% with the 120 s demo;
  the old 1500-frame qbench demo lasted 0.4 s in ironwail at 540p, so R3's
  demos must be at least this long. vkQuake runs at ~617 fps at both
  resolutions on this map: it is CPU-bound on the FTE rain and the 1703
  `misc_gtkmodel` decorations only it shows (DESIGN.md, M5). Every
  ironwail MangoHud run has one ~13.5 ms frame, right after the load.
- **R2. Settings table — done 2026-11-17.** `perfkit::settings`
  (profiles `ultra` and `ultra-fte`, table in
  [VANILLA.md](VANILLA.md#settings)). Tests check every pin against each
  engine's source; at run time a pin the binary answers with `Unknown
  command` fails the run. Ours now runs with `XDG_DATA_HOME` in the
  sandbox, and pins go through a generated cfg because ironwail truncates
  long command lines. Checked with one `-profile` calibration round per
  profile in all three engines (`target/perf/r2check-*`): every run
  completed, no pin rejected, vkQuake reports "8 AA Samples". qbench keeps
  its old pins (MSAA off, 8× aniso; its vkQuake `r_scale` was a no-op)
  until R4 moves it onto the perfkit runner, so its numbers stay
  comparable with the history until then.
- **R3. Demo set — done 2026-11-17.** `perfkit record [-only a,b]
  [-verify]` records the seven demos (VANILLA.md, Scenes) into
  `target/perf/demos/<commit>/` in about 4 minutes, headless and fully in
  the sandbox. `-verify` played each in all three engines (960×540, the
  scene's profile, one run, so the fps below only show the demos are
  sane, not a result); frame counts agree within one frame everywhere:

  | demo | frames | ours | ironwail | vkQuake |
  |---|--:|--:|--:|--:|
  | spin-e1m1 | 14999 | 5262 | 3809 | 885 |
  | spin-e2m2 | 14999 | 5257 | 3804 | 888 |
  | spin-mg1 | 15000 | 3429 | 2938 | 853 |
  | spin-sepulcher | 15001 | 3035 | 2859 | 780 |
  | spin-sepulcher-fte | 15001 | 798 (`.qsd`) | 2942 (`.dem`) | 382 (`.qsd`) |
  | spin-tears | 15061 | 3343 | 2509 | 798 |
  | combat-e1m3 | 7501 | 4805 | 3574 | 861 |

  Found on the way: the references keep only 256 characters of their
  command line (`CMDLINE_LENGTH`), which cut off the demo name for
  vkQuake, so the runner now puts everything but `vid_*` and `+exec`
  into the run's cfg, `timedemo` included, and a test keeps the longest
  command line under 240 characters. An engine that can't play a demo
  drops to its menu and never quits, so console errors ("couldn't
  open", `Host_Error`, `ERROR:`, illegible/bad server message) now fail
  the run at once instead of after the timeout. The recording server's
  `sv_fteparticles` (not the client's `r_fteparticles`) decides what AD
  shows; without FTE, AD sends classic particle messages every frame
  (`spin-sepulcher.dem` is 134 MB against 12.5 MB with FTE).
- **R4. qbench → Part 1 — done 2026-11-17.** qbench now runs on
  perfkit (DESIGN.md, Tooling): `main` and `fte` suites, 540p window and
  4K gamescope, cold-start + warm-up + interleaved fps and MangoHud
  rounds, `report.{json,md}` with machine header, percentiles and cold
  hitches; `compare_qbench` (qmcp) reads both report formats, keys Part 1
  cells by scene/profile/engine/resolution, and reports p99 change and
  machine differences. The r_partinfo particle-count check between ours
  and vkQuake (FTE suite) is `perfkit partinfo` (V3). The first full run took about
  70 minutes and covered 46 cells with no errors. We lead the fastest
  reference in every main-suite cell (+17% to +47%) except
  `spin-sepulcher` at 540p (−2%, within noise), and our p99 is the lowest
  everywhere. **Void** (R5 found menus drawn over the demos; rerun
  pending). Numbers and new items V1–V3:
  [VANILLA.md](VANILLA.md#results). In the FTE suite, "ours vs fastest
  ref" counts only references that played the same file as ours.
- **R5. Parity stills — done 2026-11-17.** `perfkit stills` (VANILLA.md,
  Visual parity): nine views on five maps, recorded once by our engine as
  demos with `screenshot` stufftexts patched in, played with `playdemo` in
  all three engines, compared with DSSIM against the band the references
  set, plus an HTML gallery. Every view is within the band. What it found:
  - Ours drew vanilla water opaque. It now takes the map's `wateralpha`
    key, otherwise `r_wateralpha` on water-vised maps, like the
    references. `r_wateralpha` now defaults to 1, and the Enhanced
    preset sets 0.5 with Translucent Water.
  - Ours drew its main menu over every `+timedemo`, and AD's `quake.rc`
    drew AD's menu over every AD timedemo in all three engines. Both are
    fixed, so R4's numbers are void until qbench is rerun (VANILLA.md, V4).
  - vkQuake 1.34.1's screenshots copy the wrong swapchain image unless
    vsync is on, and the references take the *first* `+vid_*` on their
    command line. A run's own `+cvar` now replaces the base one instead of
    following it.
- **R6. qprof → Part 2.** Baseline becomes vanilla ultra; scenarios
  regrouped into additions with tier sweeps; add-on scenarios other than
  the portal gun and gels removed; split-screen scenarios added; windowed
  with `-fixedstep` inside gamescope instead of headless (headless readback
  of a 4K frame would dominate). See [ADDITIONS.md](ADDITIONS.md).
- **R7. Demo-driven Part 2 (if possible).** If portal and gel state replay
  from demos (ADDITIONS.md, step A0), qprof plays demos instead of live
  scripts for those scenarios, which removes most run-to-run noise.
- **R8. Docs.** DESIGN.md Tooling, the perf-profiling skill (it still
  describes the old qbench defaults) and AGENTS.md if commands change.

## Baselines

`target/perf/baselines/<machine>/<part>.json`, written only by an explicit
`-save-baseline`; comparisons never overwrite them. Each part's document
keeps a short results table (date, commit, scene, resolution, numbers);
long write-ups go in the PR, not the plan.
