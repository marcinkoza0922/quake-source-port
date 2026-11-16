# User manual

This manual covers playing Quake with this port, with the emphasis on what
it adds over vanilla Quake and other source ports. Everything new is
optional: with default settings the game looks and plays like Quake.

## Getting started

1. Get your own copy of Quake. Point the port at the directory that
   contains `id1/` (and any mods next to it), with `QUAKE_BASEDIR` or
   `-basedir <dir>`.
2. Run `cargo run --release`, or start a mod straight away with
   `-game <dir> +map <map>`.
3. Config, saves and screenshots live in
   `~/.local/share/quake-source-port/<game>/`, one folder per mod.

The main menu has Single Player, Multiplayer, Options, **Help** (this
manual), Quit and **Map Packs**, with the Quake theme playing behind it
(Options → Audio → Main Menu Music). Every menu works with mouse, keyboard,
controller and touch; hover or select an option to see a tooltip explaining
it.

In Help, up/down picks a section; Page Up/Page Down, left/right, the
shoulder buttons or the mouse wheel over the text scroll it.

## Map Packs (mod browser)

Main menu → Map Packs lists every installed mod and map pack with its
description. Pick one to switch to it, then browse its maps and start any of
them directly. The browser tracks your best kills, secrets and time per map.
Each mod keeps its own config, so bindings and settings tuned for one mod do
not leak into another.

**Where map packs go.** The port looks for them in the Quake base directory
(the one holding `id1/`, set with `QUAKE_BASEDIR` or `-basedir`). Every
subfolder there that contains game data (a `.pak` or `.pk3` file, a
`progs.dat`, or a `maps/` folder) is listed, so install a map pack by
unpacking it into its own folder next to `id1/`, for example
`<basedir>/ad/` or `<basedir>/my_maps/maps/*.bsp`. Optionally add a
`modinfo.txt` with `title`, `base` and `description` lines to control how it
is listed. Packs are found when the game starts, so restart after adding
one. Saves, screenshots and config for each pack live in
`~/.local/share/quake-source-port/<folder>/`, not in the pack folder.

## Split-screen

Up to six players on one machine, in co-op or deathmatch.

- Multiplayer → New Game → **Local Players** sets how many people play.
- Each player gets their own view, HUD, sound and input device (keyboard
  and mouse for player 1, a controller each for the rest).
- The screen layout can be stacked, side by side, 2×2 or 3×2
  (`r_splitscreen_layout`).
- Each player picks a name and shirt/pants colours under Multiplayer →
  Setup.

## Controllers

The port is designed to be played on a gamepad.

- **Weapon wheel** (`+weaponwheel`): hold to slow time and pick a weapon
  with either stick or the mouse; release to switch.
- **Aim assist** (Options → Input → Controller): slowdown near enemies and
  rotation that follows them. It only ever applies to the stick, never to
  the mouse.
- **Gyro aiming** (Options → Input → Controller → Gyro): a toggle, plus
  separate sensitivity for the yaw, pitch and roll axes (roll is off by
  default and turns the view like yaw, negative values reverse it), a noise
  filter, and when gyro applies (always, off or only while `+gyroaction` is
  held). **Aim Style** switches the right stick between 2D aiming (it turns
  the view continuously) and **flick stick** (push the stick to snap the
  view toward that direction, then turn the stick to rotate it); the two
  combine well with gyro for fine aim.
- **Rumble**, stick sensitivity and inversion.
- **Button glyphs** match your pad (Xbox, PlayStation, Nintendo) and are
  detected automatically.
- Rebind everything in Options → Controls, including from the controller.
- Touchscreen controls with adjustable size, opacity and look speed.

## Saving

- **Autosaves** rotate separately from manual saves and quicksaves, so they
  never overwrite your own. Options → Gameplay → Autosave Settings chooses
  how many to keep and when to save: on level entry, on a timer, on finding
  a secret or all secrets, and on killing every enemy.
- **Save cards** in the Load/Save menus show the level, health, armor,
  weapons, keys and kills/secrets of each save.
- **On death** (Options → Gameplay → On Death), single-player can ask what
  to do in a paused menu, load the most recent save, let you pick a save,
  or restart the level.

## Gameplay and camera

Options → Gameplay:

- Field of view 60–130, view bob, gun kick, always run.
- **Camera**: first person, third person, or over-the-shoulder (right or
  left).
- **Viewmodel** position: centre, right, left or hidden.

## Assists

Options → Gameplay → Assists. They are engine-side, so they work with any
mod, and they switch off in deathmatch.

- Auto-aim with a lock indicator, cone and range settings.
- Damage taken and enemy health scaling, ammo pickup multiplier, infinite
  ammo.
- No fall damage, no drowning, no self damage.
- Health regeneration and extra air jumps.

## Accessibility

Options → Accessibility:

- Reduced motion and screen-flash intensity.
- Colour-blind filters.
- Configurable crosshair: shape, size, outline, and colour changes when on
  target or on a kill; optional hit markers.
- **Sound captions** and message settings (Messages & Captions), text
  scaling.
- Toggle (instead of hold) attack and run.
- A game-speed slider for slowing the whole game down.

## Enhancements

Options → Enhancements. All client-side and off by default, so they work
with any mod and online.

- **Debris**: tumbling gibs and shrapnel from shattered barrels, also with
  mod monsters; it can knock against doors and lifts.
- **Enhanced blood**: splatters on surfaces, wounds that follow bodies,
  airborne droplets that splat where they land.
- **Impact effects**: shotgun bullet decals, nails that embed in wood and
  enemies, explosion scorch marks.
- **Enhanced water**: translucency, ripples, splashes and caustics.
- **See-through teleporters** that show their destination with parallax.
- Ambient occlusion, per-object and speed motion blur, soft particles.
- **Screen shaders**: RetroArch `.slangp` presets (CRT and others).

Options → Graphics also has the usual render scale, anti-aliasing,
supersampling, texture filtering, 8-bit colour with dithering, fog, coloured
lighting and animation/movement interpolation.

## Add-ons

Add-ons are new weapons and equipment, run by the engine so they work with
any mod's progs. They are single-player and co-op only.

### Starting a game with add-ons

A game has add-ons only if it starts with them:

1. Single Player → tick the **Add-ons** checkbox, then New Game.
2. Choose which add-ons to bring and adjust their settings, then Start New
   Game.

Add-on games save as `.qsa` files instead of `.sav`, so other ports don't
try to load them. Save cards show an "ADD-ONS" badge. Loading a normal save
turns add-ons off. While playing an add-on game, the Add-ons page
(console: `menu_options addons`) switches individual add-ons on and off; each has a looping preview,
instructions, its bindings and its own settings.

### Weapons

Add-ons go into the regular weapon slots (choose which per add-on, or slot
9 and 0) and appear on the weapon wheel and the HUD. Fire and alt-fire are
your usual attack buttons.

| Add-on | Fire | Alt-fire | Ammo |
|---|---|---|---|
| **Portal gun** | Place the blue portal | Place the orange portal | None |
| **Railgun** | Piercing slug that hits everything in a line | — | 2 cells |
| **Hookshot** | Latch on and pull yourself to wood, metal, stone or enemies | — | None |
| **Stickybomb launcher** | Hold to charge, release to launch | Detonate | 1 rocket |
| **Boomerang** | Hold to mark enemies, release to throw; it stuns each in turn | Clear marks | None |
| **Gel sprayer** | Blue repulsion gel (bouncy) | Orange propulsion gel (fast) | None |
| **Powerwasher** | Long water jet that pushes enemies and washes gel off | Wide fan of jets | None |

Portals go on flat world surfaces and carry you, monsters and thrown objects
through with their momentum. With **See Through Portals** on you see the
other side live, including portals visible through portals; quality, number
of live views, distance and recursion depth are adjustable.

### Equipment

Equipment is switched on and off rather than held. Open the **equipment
wheel** (`+equipwheel`), or use `goggles cycle` to step through:

- **Night vision**
- **Thermal vision**

## Multiplayer

Besides LAN and internet NetQuake play (protocols 15, 666 and 999) the port
connects to **QuakeWorld** servers. Multiplayer → Join Game takes an
address or searches the LAN. FTE scripted particles are supported for mods
that use them.

## Command line and console

Common options: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, and
`+command args` to run console commands at start-up. The console (`~`)
accepts the usual Quake commands and cvars.
