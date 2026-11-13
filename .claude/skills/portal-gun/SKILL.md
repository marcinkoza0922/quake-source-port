---
name: portal-gun
description: Work on the Add-ons portal gun of this Quake port — placement and surface checks, crossing/teleporting movers, the //qsp_portal client sync and `.qsa` save header (`addons/save.rs`), see-through portal views and their recursion (portals seen in portals), particles in portal views, the portal options (addon_portal_*), and reproducing portal scenes headlessly with the portal_set command. Use whenever the task mentions the portal gun, portals placed by the player, "+toy", Add-ons, see-through or recursive portals, a portal that won't place, a player or object that crosses wrong, portal rendering glitches, or when you need a screenshot of portals.
---

# Portal gun (Add-ons)

The big picture lives in `docs/DESIGN.md` → "Add-ons". Read that section
first; this skill is the practical map and the recipes.

## Where things are

| Concern | File |
|---|---|
| Master switch, toy ids, cvars (`addon_portal_*`), `.qsa` save header (`addons/save.rs`) text | `crates/qserver/src/addons.rs` |
| Placement (trace, 9-sample rectangle check, nudging), `Portals` state, stufftext outbox | `crates/qserver/src/addons/portal.rs` |
| Crossing: pre-physics snapshot, `carry` (rotation), exit placement | `crates/qserver/src/addons/portal_move.rs` |
| `portal_set` debug command | `crates/qserver/src/commands.rs` |
| Toy slot / `+toy` / usercmd bits | `src/addons.rs`, `src/host/frame.rs`, `src/host/split_input.rs` |
| Client state → `qrender::GunPortal` (colours, linked exit) | `src/addons/portal_draw.rs` |
| Window maths: `gun`, `through` (nesting), `to_source`, `clip` | `crates/qrender/src/portals.rs` |
| View selection + recursion (`prepare_portals`), view rendering (`draw_portals`), oval quad | `crates/qrender/src/frame/portals.rs` |
| Per-view FTE batches, nested slots, UBOs per portal view | `crates/qrender/src/frame.rs` (search `slot_in`, `portal_draws`) |
| Oval shader (mode 8) and clip discards | `crates/qrender/shaders/fte.frag`, `particle.frag`, `world.frag`, `alias.frag`, `water.frag`, `sky.frag` |
| Menu rows and tooltips | `src/menu/pages.rs`, `src/menu/page_info.rs` (add new options with the `add-setting` skill) |
| Tests | `crates/qserver/tests/portals.rs` (e1m1, game data), `qrender::portals::tests` |

## Key invariants

- The rotation in `qrender::portals::gun` must match `portal_move::carry`
  (normal and right flip, up kept). Change one, change both, and keep
  `gun_window_matches_teleport_rotation` passing.
- A nested view is `outer.through(&inner)`: one window from the *main* eye
  with the composed mapping, so every view samples its children in screen
  space at `gl_FragCoord`. Keep views at full scene size and the same
  projection, or the nesting breaks.
- `PortalDraw` order is breadth first (top-level, then children);
  `draw_portals` renders in reverse so children exist before parents sample
  them. Only the first 4 views write `portal_plane/min/max` in the main UBO
  (teleporter sheets in `water.frag`); nested ones never need them.
- A portal view skips the oval of its own exit (it sits on the clip plane).
- Total views are capped by `qrender::MAX_PORTAL_VIEWS` (render targets,
  allocated once a live portal or a teleporter window exists). Each view is
  a full scene draw: mind the cost when raising limits.
- Anything drawn in portal views must discard behind `fr.portal_clip`
  (zero in the main view), or the room behind the exit wall leaks through.

## Settings

`addons_enable` (New Game checkbox; `map` copies it into the game's fixed `addons_on`), `addon_portalgun`, `addon_portal_view`
(see-through), `addon_portal_max` (top-level live views, 1–4),
`addon_portal_dist`, `addon_portal_depth` (recursion levels, 0–6, default 3).
Flow: cvar → `src/host/render.rs` → `qrender::Settings::gun_portal_*`.

## Reproducing a scene headlessly

Aiming the gun from the console is fiddly (it needs a flat, solid, dry
world surface). Use `portal_set` instead — it places the local player's
portal exactly, with no surface checks (single player only):

```
portal_set <0|1> ox oy oz  nx ny nz  ux uy uz   # place (normal, up)
portal_set <0|1>                                # remove
```

Give each console command its own `+delay` (a `;` inside `+delay` only
delays the first one). Facing pair in the e1m1 start corridor, a recursion
tunnel:

```sh
cargo build --release
target/release/quake-source-port -headless -autoquit 6 +sv_autosave 0 \
  +addons_enable 1 +addon_portal_depth 4 +map e1m1 \
  "+delay 2 portal_set 0 480 80 48 0 1 0 0 0 1" \
  "+delay 2 portal_set 1 480 330 48 0 -1 0 0 0 1" \
  "+delay 2 setpos 470 160 40 0 80 0" \
  "+delay 4 screenshot target/shots/portal"
```

Then Read the PNG. Particles through a portal: put the exit facing south
(`portal_set 0 480 40 48 0 -1 0 0 0 1`), `give 7`, `give r 50`,
`impulse 7`, turn to yaw -90, `+attack`/`-attack`, turn back to yaw 80 and
screenshot ~0.2 s later; the explosion shows only inside the oval.

To test the real gun path instead, hold the toy (`+toy` then `-toy`) and
use `+attack`/`+attack2` with `setpos … pitch yaw roll`; the
`place_pair` helper in `crates/qserver/tests/portals.rs` has known-good
e1m1 shots (corridor floor at `480 200 24` pitch 89, far wall pitch -8).

Always pass `+sv_autosave 0`, and never save a config from a windowed run
with portal cvars on the command line.

## Verify

- `cargo nextest run` (portal tests skip without game data).
- `cargo clippy --message-format=short`.
- Screenshots per above for anything visual; compare `addon_portal_depth 0`
  against a deeper setting when touching recursion.
- Update the "Add-ons" section of `docs/DESIGN.md`.
