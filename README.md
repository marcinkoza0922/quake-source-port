# quake-source-port

A Quake source port written from scratch in Rust, with a Vulkan renderer
(`ash`) and a custom, fit-to-purpose ECS engine. SDL3 handles windowing,
input, gamepads and audio.

## What sets it apart

Beyond a faithful, fast Quake, this port adds a few things of its own:

- **Split-screen, up to six local players.** Co-op or deathmatch on one
  machine, with one view per player, per-player input, HUD and sound, and
  the layout of your choice (stacked, side by side, 2×2, 3×2).
- **Designed for controllers.** Gamepad aim assist (slowdown and
  magnetism, never applied to the mouse), a slow-motion weapon wheel,
  rumble, button glyphs that match your pad (Xbox, PlayStation, Nintendo),
  full menu navigation and in-game rebinding.
- **Save usability.** Autosaves rotate separately from manual saves and
  quicksaves, with triggers on level entry, a timer, secrets and clearing
  a level. Save cards show the level, health, armor, weapons, keys and
  kills/secrets. A single-player death opens a paused recovery menu.
- **Optional visual enhancements** (Options → Enhancements), all
  client-side and off by default:
  - Ragdoll corpses, tumbling gibs, and debris from shattered barrels
    that works with mod monsters too.
  - Enhanced blood: splatters on surfaces, wounds that follow bodies,
    airborne droplets that splat where they land.
  - Shotgun and explosion decals, and nails that embed in wood and
    enemies.
  - Water ripples, splashes and caustics.
  - See-through teleporters that show their destination with parallax.
  - Ambient occlusion, per-object motion blur, and RetroArch `.slangp`
    screen shaders.
- **Gameplay assists.** Damage, enemy health and ammo scaling, no fall or
  drown damage, infinite ammo, health regeneration and air jumps. They
  are engine-side, so they work with any mod's progs, and they switch off
  in deathmatch.
- **Accessibility.** Reduced motion, flash intensity, colour-blind
  filters, configurable crosshairs, sound captions, text scaling, toggled
  attack/run and a game-speed slider.
- **A real mod menu.** It browses every installed mod with its
  description, lists each mod's maps, tracks your best kills, secrets and
  time per map, and keeps a separate config per mod.
- **FTE scripted particles** and the QuakeWorld protocol, alongside
  NetQuake protocols 15, 666 (FitzQuake) and 999 (RMQ).

## Compatibility and performance

- Aims to run everything ironwail and vkQuake do: id1, hipnotic, rogue,
  Arcane Dimensions, and the Machine Games episodes (MG1 / MG3). A
  smoke test loads all 611 playable maps across 32 installed mods
  without errors.
- Vulkan renderer using indirect draws, bindless textures and GPU
  lightstyle updates, benchmarked against ironwail and vkQuake with the
  `qbench` timedemo tool.

**Status:** early and in progress. Performance is still catching up to
ironwail on the largest Arcane Dimensions maps. See
[`docs/DESIGN.md`](docs/DESIGN.md) for the architecture and milestone
plan.

## Building

You need a stable Rust toolchain (edition 2024), a Vulkan driver and SDL3.

```sh
cargo build --release
```

## Running

You need your own Quake game data, which this repository does not include.
Point the port at a directory that contains `id1/` (plus any mod
directories):

```sh
QUAKE_BASEDIR=/path/to/quake cargo run --release
# or
cargo run --release -- -basedir /path/to/quake -game ad +map ad_start
```

Options: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` / `-quoth`,
`-width <w>`, `-height <h>`, `-fullscreen`, `-window`, `-headless`,
`-autoquit <seconds>`, and `+command args` console commands.

Config, saves and screenshots go in
`$XDG_DATA_HOME/quake-source-port/<game>/` (default
`~/.local/share/quake-source-port/<game>/`).

Headless screenshot (no display needed):

```sh
cargo run --release -- -headless -autoquit 6 +map e1m1 "+delay 3 screenshot target/shots/x"
```

## Layout

| Path | Contents |
|---|---|
| `src/` | Host binary (wires the crates together, plus menus, HUD and console) |
| `crates/qcore` | Math, cvars/commands, filesystem (PAK/PK3) |
| `crates/qformats` | BSP29/BSP2, MDL, SPR, WAD, LMP, image and WAV parsing |
| `crates/qecs` | Custom ECS |
| `crates/qvm` | QuakeC VM |
| `crates/qworld` | Collision and physics |
| `crates/qserver`, `qclient`, `qnet` | Server, client and protocols |
| `crates/qrender` | Vulkan renderer |
| `crates/qsound`, `qinput`, `qplatform`, `qui` | Audio, input and aim assist, SDL3 platform layer, UI |
| `tools/` | `qinspect` (data dumper), `qbot` (level-breaking test bot, for internal testing), `qbench` (benchmarks) |

## Development

```sh
cargo nextest run
cargo clippy --message-format=short
```

Tests that need game data skip themselves when `QUAKE_BASEDIR` is not set.

## License

GPL-2.0-or-later. See [`LICENSE`](LICENSE). Parts of the logic are ported
from [ironwail](https://github.com/andrei-drexler/ironwail) and
[vkQuake](https://github.com/Novum/vkQuake), which are GPL-2.0-or-later
themselves and descend from id Software's GPL release of the Quake source.
Quake is a trademark of id Software. This project is not affiliated with
id Software, Bethesda or Machine Games.
