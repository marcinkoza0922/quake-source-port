---
name: write-tests
description: Write and run tests in this Quake port the way the codebase does — pure unit tests, game-data tests that skip without QUAKE_BASEDIR, a headless qserver harness driving real progs, client loopback, offscreen Vulkan render tests, menu tests, synthetic qworld geometry, the qwfake QuakeWorld server, and qbot data tests — plus running them with cargo nextest. Use whenever adding or fixing behaviour that should be pinned by a test, when a test fails or is flaky, when asked to "add tests", "cover this", "reproduce in a test", or when deciding how to prove a gameplay/physics/protocol/parser change works.
---

# Tests in this repo

```sh
cargo nextest run                          # everything; quiet profile (.config/nextest.toml)
cargo nextest run -p qserver assists       # one crate, name filter
cargo nextest run -p quake-source-port menu
cargo nextest run --profile verbose -p qworld some_test   # only when debugging a failure
cargo nextest run -p qformats -- --nocapture             # see eprintln output
```

AGENTS.md: never run verbose output except when debugging a specific failure.
Keep output short (`| tail`).

**Skipped is not passed.** Game-data tests `return` early (with an
`eprintln!("skip...")`) when the data is missing, and nextest reports them as
passed. When the point of the run is to exercise real data, confirm it is
there first (`ls "${QUAKE_BASEDIR:-vkQuake directory with maps}/id1"`). This
matters most in git worktrees, which don't have the gitignored data symlink
(worktree-tasks skill).

## Choose the layer

| What you're pinning | Pattern | Example |
|---|---|---|
| Pure logic (math, parsing a buffer, menu state) | `#[cfg(test)] mod tests` or a sibling `tests.rs` | `src/menu/tests.rs`, `crates/qinput/src/tests.rs`, `crates/qworld/src/pmove/tests.rs` |
| Parser robustness | malformed input must return `Err`, never panic | `crates/qformats/tests/robust.rs` |
| Real asset formats | parse real paks | `crates/qformats/tests/data.rs` |
| Collision and physics without data | synthetic boxes + mock edicts | `crates/qworld/tests/unit.rs`, `tests/common.rs` |
| Gameplay with real QuakeC | headless `Server` on e1m1 | `crates/qserver/tests/assists.rs`, `smoke.rs` |
| Protocol end to end | server bytes → `ClientState` parser | `crates/qclient/tests/loopback.rs` |
| Rendering | `Renderer::headless` + `read_pixels` (skips without a Vulkan device) | `crates/qrender/tests/offscreen.rs` |
| QuakeWorld client | `qwfake::FakeServer` on a local UDP socket | `src/qw/tests.rs` |
| Level traversal | qbot on a real map | `cargo nextest run -p qbot` |

## Game-data helper (copy this; each test crate has its own)

```rust
fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    d.join("id1").is_dir().then_some(d)
}
```

Use `CARGO_MANIFEST_DIR` (crates run tests from their own dir), and adjust
the `../..` for the root crate. If a test needs a specific mod or map, skip
when *that* is missing too. Never commit game data, and don't hardcode
counts that change when the user installs another mod.

## Headless server harness

```rust
fn start(cvars: &[&str]) -> Option<(Server, usize)> {
    let mut fs = Fs::new(&basedir()?);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    for c in cvars { sv.command(c); }            // cvars/commands before spawning
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    Some((sv, c))
}

fn run(sv: &mut Server, c: usize, frames: usize, mv: ClientMove) {
    for _ in 0..frames {
        sv.set_move(c, mv);
        sv.frame(1.0 / 72.0);
        let _ = sv.take_reliable(c);             // drain, or buffers grow
        let _ = sv.take_datagrams(c);
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
}
```

- Read edict fields with `l.vm.ent_f(1, sv.core.sv.fields.health)` (entity 1
  is the first client). Drive input with `ClientMove { buttons: 1 /*attack*/,
  impulse: 9, angles: [pitch, yaw, roll], .. }`.
- The server is deterministic, so assert exact values where you can, and
  give tolerances only for genuinely continuous quantities.
- Let the level settle about 30 frames before measuring. Spawn-time thinks,
  item drops and the "Autosaved" path all run first.

## Expectations

- New parsers get robustness cases (truncated and garbage input) as well as
  happy-path tests.
- Ported quirks get a test that names the reference behaviour, so nobody
  "fixes" it later.
- Visual changes also need eyes on a screenshot (visual-check skill). A
  "non-black pixels" test catches crashes, not wrong output.
- Server, physics and QC changes: run qbot and check the baselines (qbot
  skill). It's the broadest regression test the repo has.
- Clippy must be clean too (`cargo clippy --message-format=short`); test
  code is linted.
