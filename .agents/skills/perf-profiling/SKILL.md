---
name: perf-profiling
description: Measure and improve frame-time performance of this Quake port against ironwail and vkQuake — qbench timedemo runs, quick headless timedemo A/B checks, GPU pass timestamps (QSP_GPUTIMES), CPU profiling with perf, and keeping optimisations pixel-identical. Use whenever the task is about fps, frame time, stutter, "faster", "slow on AD maps", CPU/GPU bound questions, benchmarking, profiling, optimising the renderer/client/server hot paths, or checking that a change didn't regress performance.
---

# Performance work

Target (PROPOSAL): within spitting distance of ironwail/vkQuake, ideally
faster. AD's large maps (ad_sepulcher, ad_tears) are the yardstick. Last
recorded numbers at 1280×720 on an RTX 3060 Ti: ad_sepulcher 3252 fps vs
ironwail 3354; ad_tears about 3050–3150 vs 2816 (DESIGN.md, M7). Frames are
**CPU-bound**: the GPU is busy about 180 µs of a 310 µs frame, and resolution
barely changes fps. Look at CPU work first.

## Pick the right measurement

| Question | Tool |
|---|---|
| Are we at parity with the references? | `qbench` (windowed, every engine, same settings) |
| Did my change make our engine faster or slower? | headless `timedemo` A/B on the same demo |
| Which GPU pass costs what? | `QSP_GPUTIMES=1` |
| Where does CPU time go? | `perf record` |

### qbench (parity; opens windows)

```sh
cargo build --release
target/release/qbench                                   # ad_sepulcher ad_tears, ours+ironwail+vkquake, 3 runs
target/release/qbench -game ad -runs 3 -secs 24 -width 1280 -height 720 -engines ours,ironwail ad_tears
```

It records the demo headless with our engine (god, 30°/s spin at the spawn
point) into `target/qbench/demos`, builds a sandbox base dir in
`target/qbench/base`, and runs each engine with
`-window +vid_vsync 0 <pinned settings> +timedemo qbench/<map>`: MSAA off,
8× anisotropy, render scale 1, each in that engine's cvar names. Then it
reads the fps line and kills the process group. Medians go to
`target/qbench/report.json`. `-rerecord` re-makes the demo. ironwail is
extracted from `<basedir>/ironwail-*-linux-x64.7z` on first use; vkQuake is
`<basedir>/vkquake.AppImage`. It needs the user's display, so tell them
before you run it.

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
targets, world, models, translucent, particles+vm, post, ui+present).

### CPU profile

```sh
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --release    # symbols without editing Cargo.toml
perf record --call-graph dwarf -o target/perf.data -- target/release/quake-source-port -headless ... "+timedemo ..."
perf report -i target/perf.data --no-children --percent-limit 1 --stdio | head -80
```

This needs `kernel.perf_event_paranoid <= 1`. If perf refuses, ask the user to
run `! sudo sysctl kernel.perf_event_paranoid=1`. Don't change system settings
yourself.

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
