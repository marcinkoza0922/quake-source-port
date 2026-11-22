---
name: perf-profiling
description: Measure and improve frame-time performance of this Quake port against ironwail and vkQuake — qbench timedemo runs, quick headless timedemo A/B checks, GPU pass timestamps (QSP_GPUTIMES), CPU profiling with perf, and keeping optimisations pixel-identical. Use whenever the task is about fps, frame time, stutter, "faster", "slow on AD maps", CPU/GPU bound questions, benchmarking, profiling, optimising the renderer/client/server hot paths, or checking that a change didn't regress performance.
---

# Performance work

Target (PROPOSAL): within spitting distance of ironwail/vkQuake, ideally
faster. Current parity numbers are in `docs/perf/VANILLA.md` (Results);
the 1280×720 MSAA-off numbers in DESIGN.md (M7) predate Part 1 and aren't
comparable. At 540p frames are CPU-bound; at 4K with 8× MSAA the GPU
matters. qbench measures both.

The plan lives in `docs/perf/`: `README.md` (shared rules, 540p/4K
resolutions, retooling steps R1–R8), `VANILLA.md` (Part 1: vanilla ultra
vs ironwail/vkQuake, visual parity), `ADDITIONS.md` (Part 2: our
enhancements, portal gun, gels, split-screen), `HISTORY.md` (the old
plan and its results log). qbench below is retooled (R1–R4); qprof is
not yet (R6).

## Pick the right measurement

| Question | Tool |
|---|---|
| Are we at parity with the references? | `qbench` (windowed, every engine, same settings) |
| What does a feature cost (lean plan, `docs/perf/PLAN.md`)? | `perfkit run [-quick\|-full] [-only scene]` (opens windows; `-list`, `-dry` don't) |
| GPU time of a timedemo | `+prof_timedemo /abs/file.json +timedemo D` (profile written at the demo's end) |
| Did my change make our engine faster or slower? | headless `timedemo` A/B on the same demo |
| Which GPU pass costs what? | `QSP_GPUTIMES=1` |
| Where does CPU time go? | `perf record` |

### qbench (parity; opens windows)

```sh
cargo build --release
target/release/perfkit record -verify          # demo set → target/perf/demos/latest (once per engine change)
target/release/qbench                          # full Part 1: both suites, 540p + 4K, ~1–1.5 h
target/release/qbench -only spin-e1m1 -res 540 -runs 3 -mhruns 0 -nocold   # quick check
target/release/perfkit stills                  # parity stills, all engines, ~6 min → target/perf/stills/latest/index.html
```

qbench plays `perfkit record`'s demos in every engine with the
"vanilla ultra" pins (`perfkit settings`), 960×540 in a window and
3840×2160 in gamescope, and writes `target/qbench/report.{md,json}`
(fps median/min/max, p99 from MangoHud runs, cold-start hitches). Our
engine runs with `XDG_DATA_HOME` in the sandbox, so it never touches the
user's configs or saves. Compare two reports with the `compare_qbench`
MCP tool. It opens windows (4K ones fill the screen) for the whole run,
so tell the user before running it. Details: DESIGN.md (Tooling: perfkit,
qbench) and `docs/perf/`.

### Headless timedemo (quick A/B, no window)

```sh
target/release/quake-source-port -headless -basedir target/qbench/base -game ad \
  -autoquit 40 +sv_autosave 0 +r_msaa 0 +r_ssao 0 +r_motionblur 0 "+timedemo qbench/ad_sepulcher" 2>&1 | grep fps
# → "1501 frames   2.1 seconds 729.4 fps"
```

The headless offscreen path is far slower than windowed (about 730 vs
3250 fps on the same demo), so **only compare headless with headless**. Run
qbench first if `target/qbench/base` doesn't exist yet. Pin the settings,
because headless runs exec the user's `config.cfg`. Run at least three times
per build and compare medians, since run-to-run noise is several percent.

### qprof (whole-feature CPU+GPU suite)

`target/release/qprof [-only a,b] [-group enhancement|addon|ultimate] [-perf] [-shots 2.5]`
runs vanilla, every enhancement, every add-on, and the ultimate/co-op horde
tests live and headless, and writes `target/qprof/latest/report.md` (costs
compared with vanilla, CPU/GPU phase and stage, hotspot ranking). `-perf` adds per-function
profiles of just the measured window. `-list` shows the scenarios. Details in
DESIGN.md (Tooling).

### GPU timestamps

`QSP_GPUTIMES=1` prints the average µs per stage on exit (cull, lightmap,
targets, world, models, translucent, particles+vm, post, ui+present). The
wgpu backend (`--features wgpu`) times whole render passes instead: scene,
ao-generate, ao-apply, translucent, soft-particles, velocity, post+ui.

### CPU profile

```sh
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --release    # symbols without editing Cargo.toml
perf record --call-graph dwarf -o target/perf.data -- target/release/quake-source-port -headless ... "+timedemo ..."
perf report -i target/perf.data --no-children --percent-limit 1 --stdio | head -80
```

This needs `kernel.perf_event_paranoid <= 1`. If perf refuses, ask the user to
run `! sudo sysctl kernel.perf_event_paranoid=1`. Don't change system settings
yourself.

### Other tools

Also installed: samply (per-thread timeline, for hitches and spikes),
heaptrack (allocations per frame), valgrind callgrind/cachegrind
(noise-free instruction counts for CPU-only code, not whole-engine
runs), hyperfine (load/startup wall time), `cargo asm` (codegen of a hot
function), turbostat/cpupower, bcc `offcputime` (root). Which to use for
what, and their limits on this machine: `docs/perf/README.md`, Tools.
heaptrack works as is (its cost is small: 773 vs 758 fps):

```sh
# --record-only: otherwise heaptrack opens its GUI on the user's screen and waits
heaptrack --record-only -o target/perf/ht target/release/quake-source-port -headless ... -autoquit 211 "+timedemo ..."
heaptrack_print target/perf/ht.zst -F ht.folded --flamegraph-cost-type allocations
c++filt < ht.folded > ht.dm.folded     # binutils demangles Rust v0 names
```

Headless time is simulated: each timedemo frame advances 1/72 s, so
`-autoquit` = frames / 72 + a second or two ends the run right after the
demo (15000 frames → 211). A larger value leaves thousands of idle
console frames in the profile.

## Safety

- Windowed runs that exit normally (`-autoquit`, `quit`, SIGTERM) save
  `config.cfg` with your `+cvar` overrides into the user's settings, which
  once silently changed their AD config (MSAA 8→0, anisotropy 16→8). Kill
  windowed runs with SIGKILL (`pkill -KILL -f quake-source-port`), as qbench
  does, or back up and restore `~/.local/share/quake-source-port/<game>/config.cfg`.
- Before comparing numbers or screenshots across runs, check that nobody's
  config changed in between.

## What has worked (and what hasn't)

These were ported from ironwail; check there first:

- **Worked**: a light-point cache keyed by the floored sample point, plus
  bilinear lightmap sampling (`lightcache_t`). A cached PVS leaf list per
  view leaf with frustum clip bits. GPU world culling (`cull_mark` →
  `shaders/cull.comp`) with indirect draws per texture. Contiguous per-leaf
  cull records. Branchless centre/extent frustum tests. A cheap
  light-cache hash and reused scratch buffers.
- **Didn't**: a per-frame BSP node walk (slower on ad_tears with 217k nodes),
  and 3 frames in flight (no gain while CPU-bound).
- Optimisations must not change output. Take the same headless screenshots
  before and after and require `compare -metric AE` = 0 (visual-check skill).
  The GPU-cull path was held to pixel-identical output on e1m1, e1m3, e2m2,
  start, ad_sepulcher, ad_tears and mg1 start.
- Record new numbers with the date, resolution and settings in the M7
  status in `docs/DESIGN.md`.
