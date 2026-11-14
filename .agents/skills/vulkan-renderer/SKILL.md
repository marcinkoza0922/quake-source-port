---
name: vulkan-renderer
description: Work on qrender, the Vulkan 1.3 (ash) renderer of this Quake port — GLSL shaders compiled by build.rs, bindless descriptors, pipelines, per-frame buffers, world/alias/sprite/particle/post passes, render settings plumbing from host cvars, headless vs windowed paths, Vulkan validation, and the offscreen example. Use whenever touching crates/qrender or its shaders, adding a visual effect or post pass, fixing rendering bugs, flicker, z-fighting, validation errors (VUID), device loss, or MSAA/swapchain/resize issues, or porting a GL/vkQuake rendering feature.
---

# qrender

Vulkan 1.3 (dynamic rendering, descriptor indexing, multiDrawIndirect) via
`ash`, loaded at runtime. No SDL in this crate: the window surface comes in
through `SurfaceProvider`. The renderer doesn't know about cvars, QC or the
server. The host builds a `RenderFrame` (`types.rs`) every frame and copies
cvars into `qrender::Settings` in `src/host/render.rs`.

## Layout

| Path | What |
|---|---|
| `lib.rs` + `lib/*.rs` (`#[path]`) | `Renderer`: init, targets, assets, world loading, settings, descriptors, resources. `FRAMES = 2` in flight. |
| `frame.rs` + `frame/` | Per-frame recording: world draw, alias models, particles, post, UI; `frame/types.rs` |
| `pipelines.rs` + `pipelines/{build,desc}.rs` | Set layouts, push constants, every pipeline. Shaders embedded with `spv!("name.frag")`. |
| `gpu.rs` + `gpu/{alloc,upload}.rs` | Device, own block allocator (64 MB blocks, first fit + merge), uploads |
| `world.rs` + `world/` | BSP world buffers, GPU cull data, light point cache, liquid queries |
| `water.rs` | Optional liquid FX (ripples, splashes, caustics) |
| `convert.rs` | Palette and texture conversion, mip building, soft-emu LUT |
| `shaders/` | GLSL: `common.glsl` (set 0 layout, Frame UBO), world, alias, sky, water, particle, fte, poly, ui, post, ao/aoapply, vel*, `cull.comp`, `lightmap.comp` |

Set 0 (`common.glsl`): 0 Frame UBO, 1 bindless `sampler2D textures[]` (4096),
2 alias verts, 3 lightmap array, 4 materials (indexed by `firstInstance`),
5 palette (+ soft-emu LUT), 6/7 scene depth (single/MS), 8 scene colour MS,
9 dynamic verts (ragdolls, `alias.vert` flag 16). Compute (`cull.comp`,
`lightmap.comp`) has its own set layout and a 112-byte push constant.

## Shaders

`build.rs` compiles every `.vert`/`.frag`/`.comp` in `shaders/` with
`glslc --target-env=vulkan1.3 -O -Werror` (override the binary with `GLSLC`).
`#include "common.glsl"` works. **Warnings fail the build.** To add a shader,
drop the file in `shaders/` and load it with `spv!("x.frag")` in
`pipelines.rs`. A Set 0 change has to be made in `common.glsl`,
`pipelines.rs`'s layout and the descriptor writes in `lib/descriptors.rs`
together.

## Conventions and gotchas

- Rendering behaviour follows ironwail/vkQuake. Name the source when you port
  (`gl_zfix` depth bias for brush entities, `GL_CW` alias culling,
  postprocessed palette). The reference GLSL is in `ironwail/Quake/gl_shaders.c`
  and `vkQuake/Shaders/`.
- New effects default **off** behind a cvar, with any heavy targets created
  only while enabled (SSAO and motion blur do this). Wire the toggle with the
  add-setting skill.
- MSAA or render-scale changes rebuild pipelines and targets at the start of
  `render` (`Pipelines::build`). Vsync toggles recreate the swapchain, and the
  host calls `resize` on window resize. Anything sized to the screen must
  follow these rebuilds.
- Per-frame host-visible buffers are indexed by frame slot. Don't write a
  buffer the GPU may still be reading from the other slot.
- Headless (`Renderer::headless(w, h, &palette)`) and windowed paths differ.
  `read_pixels()` returns RGBA8, and the windowed capture swizzles BGRA.
  Test both if you touch targets, post or present.
- Scene order (`frame.rs`): opaque world + sky → opaque alias models →
  SSAO (when on) → world liquids → sorted translucent entities → particles
  and FTE effects (depth test, no depth write) → post → UI. Anything drawn
  before the SSAO split gets darkened by it, which is why water, particles
  and the view model come after.
- `r_drawentities 0` isolates the world while debugging.

## Debugging

```sh
QRENDER_VALIDATION=1 target/release/quake-source-port -headless -autoquit 4 +sv_autosave 0 \
  +map e1m1 "+delay 2 screenshot target/shots/v" 2>&1 | grep -iE 'VUID|validation error' | sort | uniq -c
```

- The Khronos layer is installed (`VkLayer_khronos_validation`). Without a
  debug messenger it prints to stdout. A clean run prints nothing (e1m1 is
  clean as of 2026-11-12). Add
  `VK_LOADER_DEBUG=layer` once to confirm that the layer loaded. Check AD and
  a map that uses your feature too, and the windowed path if the swapchain
  is involved (kill it with SIGKILL; see visual-check).
- `QRENDER_DEVICE=<index>` picks a GPU. `QSP_GPUTIMES=1` prints µs per pass
  on exit (perf-profiling skill).
- `cargo run --release -p qrender --example offscreen [map] [w h] [game]`
  renders a map without the host and writes `target/qrender-<map>.ppm`.
  Convert it before viewing: `magick target/qrender-start.ppm target/shots/start.png`.
- `cargo nextest run -p qrender` includes `tests/offscreen.rs`, which skips
  without game data or a Vulkan device.

## Done means

Clippy clean (`undocumented_unsafe_blocks` warns, so every `unsafe` ash call
block needs a `// SAFETY:` comment), no validation messages on the maps you
touched, screenshots inspected (and compared with ironwail/vkQuake when it's
a parity feature), and perf unchanged or measured (perf-profiling skill).
Update the qrender section of `docs/DESIGN.md`.
