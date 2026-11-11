# quake-source-port

A Quake source port written from scratch in Rust, with a Vulkan renderer
(`ash`) and a custom, fit-to-purpose ECS engine. SDL3 handles windowing,
input, gamepads and audio.

Goals:

- Compatible with everything ironwail / vkQuake run: id1, hipnotic, rogue,
  Arcane Dimensions, and the Machine Games episodes (MG1 / MG3).
- Performance on par with or better than ironwail / vkQuake.
- Controller support with standard aim assist, plus a better mod menu.
- An automated test bot that plays each level, finds every secret and kills
  every enemy.

**Status:** early and in progress. id1 is playable, with a modern HUD,
animation and movement interpolation, menus you can use with the mouse, and
graphics options. See [`docs/DESIGN.md`](docs/DESIGN.md) for the
architecture and milestone plan.

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
| `tools/` | `qinspect` (data dumper), `qbot` (level-traversal test bot), `qbench` (benchmarks) |

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
