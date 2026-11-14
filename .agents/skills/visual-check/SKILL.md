---
name: visual-check
description: See what the engine actually renders by running it headless and reading the PNG screenshots, optionally side by side with ironwail or vkQuake. Use after any change that could alter what is on screen (renderer, shaders, lighting, fog, sky, water, particles, ragdolls/debris, HUD, status bars, menus, console, captions, crosshair, view model), when asked to "check how it looks", "take a screenshot", "compare with vkQuake/ironwail", or when verifying that a map or mod loads and renders. Tests passing is not evidence that a visual change is right; look at it.
---

# Visual check (headless screenshots)

Headless runs use an offscreen Vulkan target. They need no display, never
write `config.cfg`, and simulate time in fixed `1/host_maxfps` steps, so they
finish faster than real time. Console output goes to stdout.

## Basic recipe

```sh
cargo build --release
target/release/quake-source-port -headless -autoquit 6 +sv_autosave 0 \
  +map e1m1 "+delay 3 screenshot target/shots/e1m1"
```

Then open `target/shots/e1m1.png` with the Read tool and actually look at it.

- **Always pass `+sv_autosave 0`.** Headless runs still autosave on level
  entry into `~/.local/share/quake-source-port/<game>/autosaveN.sav`, which
  pushes the user's own autosaves out of the rotation. (`cargo run --release --`
  works too, but calling the binary skips cargo's checks when nothing changed.)
- Each `+cmd args` is one console command, run in order at startup. Quote
  commands that contain spaces. Set cvars before `+map`.
- `delay <s> <cmd>` and `-autoquit <s>` count simulated seconds. Allow about
  2–3 s after the map loads for fades, lerping and particles, and make
  `-autoquit` later than the last delay.
- `screenshot <path>` adds `.png` when there is no extension, creates the
  parent directory and logs `Wrote <path>`. Keep shots under `target/shots/`
  (gitignored). Use a new name per run, or delete the old file first, so you
  never look at a stale image.
- The default size is 1280×720. Use `-width W -height H` to change it.
- Headless runs still **exec the user's `config.cfg`**, so their graphics
  settings apply (MSAA, SSAO, motion blur, 8-bit colour, brightness...). When
  a shot has to be reproducible or compared, pin the cvars that matter on the
  command line, e.g. `+r_msaa 0 +r_ssao 0 +r_motionblur 0 +r_softemu 0`.

## Framing the shot

```sh
target/release/quake-source-port -headless -autoquit 8 +sv_autosave 0 +map e1m1 \
  "+delay 1 god" "+delay 1 notarget" \
  "+delay 2 setpos 480 -352 88 0 90 0" "+delay 3 screenshot target/shots/a" \
  "+delay 4 setpos 752 2000 -60 20 180 0" "+delay 5 screenshot target/shots/b"
```

- `setpos x y z [pitch yaw roll]` works in single player once the player has
  spawned, so put it inside a delay. You can take several shots in one run.
- Get coordinates from the map's entities
  (`python3 tools/qdata.py ents <map> -c info_player_start`; see the game-data
  skill), from monsters and items deeper in the level, or from a qbot `-v`
  log.
- `god` and `notarget` stop monsters from changing the scene. Use `give`,
  `impulse 9` and `impulse <n>` to pick weapons, and
  `r_drawviewmodel 0` / `crosshair 0` to clean up the frame.
- Mods: `-game ad +map ad_sepulcher`. Mission packs: `-hipnotic` / `-rogue` /
  `-quoth`, which can be combined with `-game`.
- Menus: `"+delay 2 menu_options graphics"`. Pages: gameplay, controls,
  input, mouse, controller, autosave, hud, layout, messages, accessibility,
  graphics, audio (`menu_options` in `src/host/commands.rs`). Sub-pages that
  have no name there (Water, Debris, Assists) need an alias added to that
  match first. Also `menu_main`, `menu_load`, `menu_save`, `menu_autosaves`,
  `menu_mods`, `menu_multiplayer` and `toggleconsole`.
- `r_drawentities 0` hides everything except the view model, which helps
  isolate world rendering.

## Reading the result

- Check the console output too, e.g. by piping it through
  `grep -iE 'error|warn|missing|couldn|unknown'`. A clean image can hide
  Host_Error spam or a missing texture fallback.
- For before/after comparisons, take the same shot on both builds (same
  setpos, same delay) and diff them. ImageMagick and PIL are installed:
  `compare -metric AE a.png b.png diff.png` prints the number of changed
  pixels, and diff.png shows where. Optimisations that should not change
  output should give 0. Earlier GPU-culling work was held to pixel-identical
  output on e1m1, e1m3, e2m2, start, ad_sepulcher, ad_tears and mg1 start.
- Describe what you see in specific terms (e.g. "fog is denser than ironwail
  at distance", "grate texels glow orange"). Don't just say "looks fine".

## Windowed runs (only when you need the real swapchain)

Use them for present, vsync, swapchain capture, or input. A windowed run that
exits normally (`-autoquit`, `quit`, SIGTERM) **saves `config.cfg` with your
`+cvar` overrides into the user's settings**. Kill it with
`pkill -KILL -f quake-source-port`, or back up
`~/.local/share/quake-source-port/<game>/config.cfg` first and restore it after.

## Comparing with the reference engines

ironwail and vkQuake are windowed only. Extract ironwail with
`7z x "$QUAKE_BASEDIR"/ironwail-*-linux-x64.7z -otarget/refengines` (qbench
does this on first use). vkQuake is `$QUAKE_BASEDIR/vkquake.AppImage`.

Never point them at the real data dir: they write configs and screenshots
into it. Build a sandbox whose directories are real and whose files are
symlinks. Then remove the linked configs and saves, so that anything the
engine writes becomes a new local file instead of overwriting a file through
a link:

```sh
data="$(realpath "${QUAKE_BASEDIR:-vkQuake directory with maps}")"
mkdir -p target/refbase && cp -rs "$data/id1" target/refbase/      # add the mod dir the same way
find target/refbase -type l \( -name '*.cfg' -o -name '*.sav' -o -name '*.png' \) -delete
```

Run with `-basedir target/refbase` and the same settings, using each
engine's own cvar names. qbench's per-engine lists in
`tools/qbench/src/main.rs` (search for `vid_fsaa`) map MSAA, anisotropy and
render scale. Then use the same `setpos` (both engines support it) and
`screenshot`, and look for the newest image in the sandbox game dir. Known,
explained differences: vkQuake looks about 20% brighter on id1 because it
picks up id1's `contrast 1.4` / `gamma 0.9`. AD detects the engine and turns
on different particle paths. See the M5 notes in `docs/DESIGN.md`.
