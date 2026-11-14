# AGENTS.md
- Language: Rust.
- Specs: `PROPOSAL.md` (what), `docs/DESIGN.md` (how; keep updated).
- File length: strive to keep Rust files under 500 lines, and keep
  `docs/DESIGN.md` under 500 lines. Split a file only where there is a
  logical seam (e.g. a cohesive group of types, a subsystem, or a distinct
  `impl` concern); do not split arbitrarily just to hit the number.
  A single function over 500 lines that cannot be split logically is left
  as it is.
- Tests: `cargo nextest run` (quiet profile in `.config/nextest.toml`). Never
  run verbose test output unless debugging a specific failure.
- Lint/build: `cargo clippy --message-format=short`; `.cargo/config.toml` is
  set to quiet. Truncate long output.
- If requirements are unclear, ask before assuming.
- You may create any helper tools/functionality needed to build, test, or
  debug. Put them in `tools/` and document them in DESIGN.md.
- Game data: `QUAKE_BASEDIR` (default `./vkQuake directory with maps`); tests
  needing it must skip when absent. Never commit game data or reference repos.
- License GPL-2.0-or-later; porting logic from `ironwail/`/`vkQuake/` is allowed.
- Visual check without a display: `cargo run --release -- -headless -autoquit 6
  +map e1m1 "+delay 3 screenshot target/shots/x"`, then view the PNG.
  Headless runs never write config.cfg.

## Project skills
- Shared skills live in `.agents/skills/<name>/SKILL.md`. Read the relevant
  skill before its workflow; agents without skill discovery can open it directly.
- `.claude/skills` links to that directory for compatibility; edit the shared copy.

## Asset tools
- Meshy: available via MCP. Consider using it to generate 3D models for the project.
- Blender: available via MCP. 3D modeling, scenes, rendering.
- ArtCraft (pure Rust, installed locally). Use only when the project needs
  that asset type:
  - Photocraft: raster editing, layers/masks, PSD
  - VectorCraft: vector illustration
  - SoundCraft: DAW, multitrack/MIDI/mixing, CLI-scriptable
  - MaterialCraft: procedural node-graph materials, exports maps
  - TextureCraft: 3D PBR texture painting, mesh map baking
  - EffectCraft: motion graphics/VFX compositing, render queue
  - FilmCraft: video editing, color, sound
- Invocation notes (fill in as discovered):
