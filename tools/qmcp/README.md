# Quake tools MCP

A local Rust MCP server for this checkout. It speaks newline-delimited JSON-RPC
on stdio; stdout contains only protocol messages. No network service or API key.

```sh
cargo build -p qmcp
python3 tools/qmcp/register.py
python3 tools/qmcp/register.py --check
```

The installer backs up existing configs and registers **quake-tools** globally
with Codex, Claude Code and agy using their MCP CLIs. Restart existing harness
sessions to load it. Rebuild after source changes. Keep the checkout in place;
if you move it, remove the old registrations before rerunning the installer.
Registration points at `target/debug/qmcp`, with an absolute `--root CHECKOUT`,
so it works from other working directories. Python 3.11+ is needed for the
registration script; map lookup uses the existing Python `tools/qdata.py`.
`QUAKE_BASEDIR` inherited by the harness overrides the checkout's default data.

| Tool | Arguments |
|---|---|
| `list_artifacts` | `kind`: all/reports/screenshots; `offset`, `limit` |
| `read_report` | `path`; optional qbot `map` |
| `compare_qbot` | `before`, `after`: report or baseline paths |
| `compare_qbench` | `before`, `after`; `threshold_percent` defaults to 5 |
| `list_mods` | none |
| `list_maps` | `game`, `include_base` |
| `inspect_map` | `map`; `game`, `classname`, `keys`, `offset`, `limit` |
| `read_screenshot` | `path` |
| `capture_screenshot` | `map`; `game`, `width`, `height`, `camera` |

Examples of tool arguments:

```json
{"before":"tools/qbot/baseline-id1.json","after":"target/qbot/report.json"}
{"before":"target/qbench-before/report.json","after":"target/qbench/report.json"}
{"map":"e1m1","classname":"info_player_start"}
{"map":"e1m1","camera":[480,-352,88,0,90,0],"width":640,"height":360}
```

All tools except capture are read-only. JSON and PNG paths must resolve inside
`target/` or `tools/qbot/`, including absolute paths; symlink escapes are denied.
Individual reads cap at 8 MiB. Entity/artifact pages default to 50, maximum 200.
Entity pairs preserve duplicate keys; qdata filters use `KEY=value` (substring)
or `KEY==value` (exact). Map queries accept qdata mount lists such as
`+hipnotic,mymod`. No game data is written.

qbot comparison reports lost kills/secrets/events/exits, missing/added maps,
new failed goals/errors and developer log lines. It strips qbot timestamp/count
prefixes before comparing against baseline logs. It never updates baselines.
qbench comparison matches scene/profile/engine/resolution for Part 1 reports
(an object with `machine`, `demo_set` and `cells`; old flat-array reports by
game/map/engine/resolution) and reports FPS change, frame-time change, p99
frame-time change, raw runs, missing cases and failed measurements. A positive
frame-time change means slower. Missing cases are explicitly incomplete and
flag `has_regressions`. Part 1 reports also list which machine fields differ
(`machine_differs`) and both demo sets; check they match before attributing
performance differences.

Capture needs `cargo build --release` and a working Vulkan driver. It uses
current graphics settings, disables autosaves, enables god/notarget, optionally
sets a six-number camera, captures at simulated second 3 and quits at second 6.
Output is a fresh PNG plus log under `target/qmcp/`; the tool returns MCP image
content. A 45-second wall timeout kills and reaps the engine. Mission packs
hipnotic/rogue/quoth use their engine flags; capture accepts one game directory.
It exposes no arbitrary console commands. Benchmark/bot execution remains with
existing CLI tools; this server inspects their saved reports.

Verification:

```sh
cargo nextest run -p qmcp
cargo clippy -p qmcp --all-targets --message-format=short
```

The integration test launches the binary over stdio and covers initialization,
tool discovery, malformed requests, schema validation and artifact boundaries.
Pure tests cover baseline counters/log normalization and benchmark matching,
failed measurements and duplicates.
