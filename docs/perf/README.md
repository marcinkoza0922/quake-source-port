# Performance plan

> **Superseded on 2026-11-21 by [PLAN.md](PLAN.md)** (a leaner plan: ours
> only until after optimising, one scene per feature, one everything-on
> split-screen scene). The tooling, rules and open items below still apply
> where PLAN.md points to them.

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

## Status (2026-11-18)

Details for each step are under Retooling steps below; items are in
[VANILLA.md](VANILLA.md#items) and [ADDITIONS.md](ADDITIONS.md#items).
The profiling tools are under [Tools](#tools).

Since the 2026-11-17 status (merged as #112, then #113–#115), the qbench
rerun is still pending; the only new measurement is V9's first allocation
census (~114 heap allocations per frame on `spin-e1m1`). What changed
under the plan:

- **Engine changes that touch the workload** (3dca905, 554c3bb,
  79284be):
  - Sky wind (`r_skywind`, default 1): two extra sky samples per pixel
    on skyboxes with a `<sky>wind.cfg`. None of the suite's maps has one,
    but the sky shader changed for every map, so it falls under the
    pixel-identical rule (V10).
  - Per-frame host work grew: `view_cvars` reads 7 more cvars
    (`v_idlescale`, `v_i*`) and builds 4 `format!` names per frame for
    `gl_cshiftpercent_*` (H1, V9).
  - Cbuf runs once per frame and a frame that spawns a server runs a
    server frame. This changes only *recording* (a script's first
    `wait` now costs a frame), not playback. The 56238f2 demo set stays
    valid; re-record only when the scenes change.
  - MD5 parents (MG3's `ogre_rocket`), the render entity list split out
    of `view.rs`, `cl_vwep` (default 0) and `freecam`: no effect on Part
    1's demos. `freecam x y z [pitch yaw]` works in demos, so a timedemo
    can now hold a fixed view (useful for V1/V2 and Part 2's A3).
  - Headless timedemo is uncapped; the ~90 fps the compat sweep saw was a
    GPU that needed a reset (COMPAT_AUDIT, item 8).
- **Found while updating the plan:** `r_softparticles` is a compat cvar
  that does nothing, and `r_particle_quality 1` turns soft particles on,
  so the `ultra-fte` pins ran ours *with* soft particles against vkQuake
  1.34.1, which has none. Fixed (V8): `r_particle_quality 0` +
  `r_part_density 2`, particle counts still within 0.1% of vkQuake's.
- **New tools installed** (2026-11-18): samply, cargo-show-asm (cargo);
  heaptrack, valgrind + kcachegrind, hyperfine (dnf). See [Tools](#tools)
  and R9.

Done before 2026-11-18:

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

1. ~~**V8**~~: done 2026-11-18 (VANILLA.md).
2. **Session setup** (ask the user; we don't change system settings):
   `kernel.perf_event_paranoid=1` (samply refuses at 2, today's value)
   and the `performance` governor (today `powersave`).
3. **Rerun qbench** (`target/release/qbench`, about 70 min) on current
   master and the 56238f2 demo set. Replace VANILLA.md's Results table
   and re-check V1/V2 against the new numbers.
4. **R9**: profiling baselines for every main-suite scene at 540p: a
   perf/samply CPU profile and a heaptrack allocation census, kept next
   to the qbench report so later changes are compared with them.
5. **V9** (allocations and cvar lookups per frame), then **V1/V2** if they
   hold after the rerun, then **V7** (decals in the FTE combat). V10 rides
   along with the first pixel check.
6. **R6**: qprof → Part 2 (ADDITIONS.md, steps A0–A5), including pinning
   `r_enhancedfx` in its baseline.
7. **R7**, if A0 shows portal and gel state replays from demos; then
   **R8**, the docs pass (now including the Tools section and R9).

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

## Tools

What is installed on the test machine (2026-11-18) and the question each
one answers. Fps comparisons come only from qbench and the headless A/B;
everything below is for attribution, never for the numbers in a results
table.

| Tool | Answers | How we use it | Limits here |
|---|---|---|---|
| qbench / perfkit | parity with the references, p99, cold start | Part 1 numbers | windows for the whole run |
| headless `timedemo` | did my change make ours faster? | quick A/B, ≥3 runs per build, medians | several times slower than windowed (`spin-e1m1`, 540p, `ultra`: 758 fps headless on 2026-11-18 against 5277 windowed in qbench); compare only with itself |
| `QSP_GPUTIMES=1` | which GPU stage costs what | µs per stage on exit | ours only |
| `perf` 7.1 (`record --call-graph dwarf`, `stat`) | where CPU time goes; IPC, cache and branch misses | text reports the agent can read; `qprof -perf` | user space only at `perf_event_paranoid` 2 |
| **samply** 0.13 | the same samples on a per-thread timeline: which frame stalled and what each thread did then | V2's cold-start hitch, H8 server spikes, H12 waits; `samply record --save-only -o prof.json.gz`, then `samply load` (local server, Firefox Profiler UI); `samply import perf.data` reuses a perf recording | refuses at `perf_event_paranoid` 2 (needs 1) |
| cargo-flamegraph | one SVG per profile, for PRs | `flamegraph --perfdata target/perf.data` | for differential graphs add `inferno` (`cargo install inferno`: `inferno-diff-folded`) |
| **heaptrack** 1.5 (+ GUI) | heap allocations per frame, where, and how many are temporary | V9's allocation census: `heaptrack --record-only` (otherwise it opens its GUI and waits), `heaptrack_print -F` + `c++filt` for text | sees the NVIDIA driver's allocations too; count only steady-state frames |
| **valgrind**: callgrind + kcachegrind, cachegrind | exact instruction counts: CPU A/B without run-to-run noise | CPU-only paths (server frames, PVS/culling, particle simulation, light cache) in a harness that doesn't touch Vulkan; `--toggle-collect` on the function under test | ~50× slower; the NVIDIA driver under valgrind is impractical, so not for whole-engine runs |
| **hyperfine** 1.20 | wall time of whole runs with warm-up, outliers and JSON export | map load and startup time (not measured so far); A/B of two binaries on the same headless command | measures the process, not frames; not for fps |
| **cargo-show-asm** (`cargo asm`) | what a hot function compiled to | after perf names a function: bounds checks, missed vectorisation, inlining (frustum tests, light cache, particle update) | – |
| `turbostat`, `cpupower` | CPU frequency, C-states, governor during a run | check that a slow run wasn't a clock drop; record the governor in reports | turbostat needs root (ask the user) |
| bcc-tools (`offcputime`, `profile`) | off-CPU time: where the main thread waits (fences, present, locks) | H12 | root (ask the user) |
| Vulkan validation layers 1.4.341 | best-practices and synchronisation warnings, including NVIDIA-specific ones | one-off audit for H12 (`VK_LAYER_KHRONOS_validation` with `VK_VALIDATION_VALIDATE_BEST_PRACTICES_NVIDIA`) | slows every frame; never with numbers |
| MangoHud, gamescope | frame-time logs; 4K on a 1440p monitor | inside qbench | see R1 for their costs |

Not installed, would help: a GPU frame debugger (RenderDoc, or NVIDIA
Nsight Graphics/Systems, which also show GPU/CPU overlap) for the 4K
items, and `hotspot` (dnf) as a perf GUI. Tracy would need
instrumentation in the engine; `prof_start`/`prof_stop` and
`QSP_GPUTIMES` cover that for now.

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
  describes the old qbench defaults, and none of the tools new on
  2026-11-18) and AGENTS.md if commands change.
- **R9. Attribution runs (new 2026-11-18).** One command per kind of
  profile, on perfkit's sandbox and demos, so profiles of different
  commits are comparable and nobody hand-types pins (the ad-hoc runs on
  2026-11-18 needed 21 `+cvar`s on the command line):
  - `perfkit profile [-only a,b] [-tool perf|samply|heaptrack] [-res
    540|2160]`: ours only, headless, `ultra` pins from the settings table;
    writes `target/perf/profiles/<commit>/<scene>.<tool>.*` plus a text
    summary (perf: top self/children functions; heaptrack: allocations
    per steady-state frame and the top call sites). Headless is fine
    here: the question is the shape of the profile, not the fps.
  - `-windowed` for perf/samply (the headless readback adds memmove and
    waits that windowed runs don't have, HISTORY item 12), SIGKILLed like
    qbench.
  - A callgrind harness for the CPU-only paths (server frames on the
    combat script, `mark_leaves`/PVS, particle simulation, light cache)
    with `--toggle-collect`, so CPU micro-optimisations get a noise-free
    instruction-count A/B instead of a ±3% fps band. Start with H8/H10
    (server, contents checks), which have no GPU part at all.
  - `perfkit loadtime` with hyperfine: startup and map load per scene map
    (median of 10, warm cache), the one cost no suite measures yet.
  - qprof's `-perf` moves onto the same code in R6.

## Baselines

`target/perf/baselines/<machine>/<part>.json`, written only by an explicit
`-save-baseline`; comparisons never overwrite them. Each part's document
keeps a short results table (date, commit, scene, resolution, numbers);
long write-ups go in the PR, not the plan.
