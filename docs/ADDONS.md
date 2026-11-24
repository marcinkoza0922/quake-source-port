# Add-ons design

### Add-ons (Customize → Add-ons; `qserver::addons`, `src/addons.rs`)
Optional engine-side toys, so they work with any progs. Add-ons belong to a
*customized* game (DESIGN.md, "Customize"): `Core::custom`, set by `map`
from `custom_enable` (set by the launch pop-up's Play) or by
`load_game` from the save, mirrored in read-only `custom_active` for the
menu. It never changes mid-game, so a vanilla game can't gain add-ons. Which
toys are on is the per-toy cvar (`addon_portalgun`, default 0), editable live
in a customized game (Main menu → Customize → Add-ons) and fixed (dimmed)
otherwise. Toys only run with no deathmatch and no remote client
(`addons::allowed`/`all_local`).
- The launch pop-up's Customize Experience opens `Page::NewGameCustomize`;
  its Add-ons row opens `Page::NewGameAddons` (toy toggles, portal settings,
  Play) with a note about other ports. Back returns to the Customize page,
  and from there to the pop-up.
- Saves (`addons/save.rs`): customized games save as `.qsa` (never `.sav`)
  with a `QSPADDONS 1 <n>` header (toys, portals, add-on state, gameplay
  modifiers) before the v5 body, so other ports neither list nor parse them.
  `savegame_path` picks the extension, `find_save` loads either (newer wins),
  saving removes the same-named other-format file, autosave rotation carries
  each file's extension. Load/Save cards show a "Customized" tag
  (`SaveInfo::addons`). A plain `.sav` load makes the game vanilla.
- Browser: both add-on pickers use a narrow checklist and a detail panel with
  a looping action preview, scrollable instructions, bindings and a parameter
  submenu. Selection and enablement are independent. The running game's
  add-on lock still applies to the checklist.
- Input: each weapon has an archived `addon_<name>_slot` assignment (1–10,
  where 10 is slot 0). Repeated selection of a stock weapon slot cycles its
  QC weapon and enabled assigned add-ons. Slots 9 and 0 have default keys 9
  and 0; assignment changes the slot membership rather than its key binding.
  `weapon_slot <1..9|0>` uses local input state, preserving every mod-defined
  QC impulse value, including the console's `impulse 9` cheat.
  Enabled toys are owned immediately. Slots, next/previous cycling and exact
  selections share `wheel::Entry::usable`: stock weapons require ownership and
  ammo; sticky needs one rocket and railgun needs two cells. Ammo-free add-ons
  remain selectable. Unavailable slots preserve the current weapon; stock
  requests still reach QC when no add-on is held. Next/previous weapon (including
  mouse wheel) traverses usable weapons in slot order, stock first within each slot.
  An empty usable inventory consumes cycling without changing the held weapon.
  The radial weapon wheel adds one segment per enabled add-on in the same slot
  order, selecting each weapon directly. Add-ons use short labels and show their
  ammo and full names; the modern HUD includes them in a compact four-column grid.
  The former `+toy` action is removed. `+attack` and `+attack2` are shared
  fire/alt-fire actions: remapping them affects every add-on using them.
  Goggles instead expose the unique `goggles cycle` action. The browser
  offers keyboard/mouse and controller bindings and warns before replacing
  an occupied key. Bindings use the same persistent host configuration as
  Controls.
- While a toy is held, `WeaponSelection::buttons` keeps QC's attack bit at 0 and
  sends usercmd bit 2 (fire), bit 3 (alt) and the toy id in bits 4–7; QC
  only reads bits 0/1. The QC view model is replaced by the imported projector (player 1
  and each split-screen guest have their own `WeaponSelection`).
- Off costs one cvar lookup per frame; nothing else runs.
- Portal gun (`addons/portal.rs`, `portal_move.rs`): fire/alt-fire edges
  place slot 0/1 of the shooter's pair. Placement traces from the eye
  (`NoMonsters`, must hit world 0 with solid behind, so no sky/water
  surfaces or brush entities), then checks 9 rim/centre samples of a
  56×96 rectangle (on-plane, empty in front, nothing jutting out) and nudges
  8 units away from failing samples up to 8 times; overlap with any other
  portal is refused. Floor/ceiling portals point "up" along the shot; a
  floor shot within 16 u of a monster's feet centres under that monster.
- Crossing: while some pair is linked, physics snapshots movers' origin and
  velocity (walk/step/fly/toss/bounce) first; afterwards a mover touching a
  portal rectangle with the pre-frame velocity going in (or resting on a
  floor portal) is placed in front of the other portal, velocity rotated
  (`carry`: normal and right flip, up kept), at least 60 u/s out (360
  u/s up out of a floor portal, where a player with no move input also
  loses sideways speed, so two floor portals bob in place), view or
  yaw turned (`fixangle`); exit spots tried: lined up, centred, then
  shifted +16/+32/-16 along the exit's up; skipped if all are solid.
- Shooting, projectiles and gel through portals (`Core::linked_portals`, empty
  unless a portal pair is linked; level teleporters are unaffected): QC
  `traceline` (`host/trace.rs`), `physics_toss` (`Edicts::try_portal_move` for
  missiles, nails, grenades, enemy projectiles, shotgun pellets and toss
  items) and gel blobs (`addons/gel.rs`) intersect linked portal ovals via
  `portal_move::find_portal_hit` and continue from the exit with rotated
  position/velocity/angles (up to 4 hops per frame). Only traces/movers that
  actually enter a portal do extra work; otherwise the check is a few dot
  products (or skipped when `linked_portals` is empty).
- Lightning bolts (`addons/portal_bolt.rs`, `qclient::beams`): QC sends a
  straight beam to `trace_endpos` and traces that same line for damage, both
  wrong once the bolt went through a portal. `Core::portal_bolts` keeps this
  frame's portal-crossing traces; a QC beam (`Write*` or `te_lightning*`)
  matching one is followed in the same buffer by `//qsp_beam entity ux uy uz
  (ox oy oz)..` (the end unfolded into the shooter's space, centres of the
  portals entered), and the same shooter's damage traces to that end are
  moved onto the bolt's path. The client draws one leg per portal, following
  the viewer. With nothing linked these are empty-list checks.
- Monster fall damage (`addons/fall.rs`, gun on): landing >650 u/s calls
  QC `T_Damage` (world attacker) for 5 + (speed − 650)/10.
- Sync: `//qsp_portal owner slot [origin normal up]` stufftext, full state
  when a client spawns. `qclient` swallows these into `ClientState::portals`;
  other engines ignore `//` lines, so demos still play elsewhere.
  `portal_draw::gun_portals` turns them into `qrender::GunPortal`s (owner
  colour pair, linked exit). Saves write portals into the `.qsa` header
  (manual saves and autosaves).
- See-through (qrender, reusing teleporter windows): `portals::Portal`
  carries a full `rot` matrix and a `pvs` point; `portals::gun` builds the
  window for a linked portal (same rotation as `carry`, clip plane 0.5 in
  front of the exit wall). `prepare_portals` takes live gun portals first
  (facing, in frustum, nearest, `addon_portal_max` 1-4, within
  `addon_portal_dist`), then teleporter windows (4 top-level views), then
  nested gun portals breadth first up to `addon_portal_depth` levels
  (0-6, default 3): `Portal::through` composes the outer mapping with the
  inner one, so a nested view is just another window from the main eye;
  `MAX_PORTAL_VIEWS` (8) targets in all, created once a live portal was
  seen. `PortalDraw::parent` names the view it is seen in; `draw_portals`
  renders in reverse so children exist before their parent samples them.
  Each portal is an FTE quad in `fte.frag` mode 8: oval mask, rim in the
  owner colour, inside the target of the view (parent, portal) sampled at
  `gl_FragCoord` (flat fill past the depth limit or with
  `addon_portal_view 0`). Portal views also draw the classic and FTE
  particles (not soft) and every gun oval but their own exit; particle,
  FTE, world, alias, water and sky shaders discard behind `portal_clip`.
  The player's own body is a `portal_only` entity (views bit for its
  player), skipped in normal draws. `portal_set <0|1> [o n up]` (server
  command, single player) places the local player's portal exactly, no
  surface checks: `portal_set 0 480 80 48 0 1 0 0 0 1; portal_set 1 480 330
  48 0 -1 0 0 0 1; setpos 470 160 40 0 80 0` on e1m1 is a recursion tunnel.
  draws.
- Turning add-ons off (or a remote client joining) removes all portals.
- Tests: `crates/qserver/tests/portals.rs` (e1m1, needs game data).


### Portal gun presentation
`src/addons/portal_model.rs` loads the owner's supplied Meshy retro portal-gun
FBX as an embedded native alias model. `assets/portal-gun/` contains compressed
MDL geometry, a 512×512 RGB atlas and import statistics. The source is a
1,354-triangle meshy-t2 Smart Topology mesh. Its hard, faceted shape is kept as is:
the importer only decimates sources over 4,000 triangles, so this one is not
decimated. Triangles collapsed by
MDL position quantization are removed. The source color is baked onto a fresh UV
layout; non-energy metal is scaled by 0.6 (`METAL_GAIN`) so overbright model lighting matches the concept, then palette-converted; smooth lighting normals survive. The muzzle
points along +X and the full model spans 36 Quake units.
`portal_skin.rs` remaps the imported texture to the active mod's palette once
per level. Metal excludes fullbright indices; a source-color/UV mask supplies
eight owner/slot skins, retaining paired chamber/emblem colors and switching
muzzle energy with the last fired slot. The native renderer supplies lighting;
the FBX's PBR normal, metallic and roughness maps are not used.
The model is uploaded once per level and drawn as a depth-compressed view weapon,
with idle movement and firing recoil. The metal of the atlas is repainted
from the model's shape by `tools/paint_toy_texture.py --keep-mask` (since
2026-11-22, to match the owner's in-game mockup: plain warm grey-brown on
palette ramp 10 with a baked top light, one-texel light bevels on outer
edges and dark seams in inner ones, faint grit, no rust or camo blotches);
`--glow-ribs` gives the energy chambers bright bands between darker ribs.
The command is in `assets/portal-gun/import.json` (`cleanup`); rerun it
after a reimport (see DESIGN.md).

Reimport with `blender -b --factory-startup --python-exit-code 1 --python
tools/import_portal_gun.py -- /path/to/Meshy_export.zip`. This offline Blender
helper preserves the external source archive and needs no Quake game data.
The imported mesh and color atlas were supplied by the project owner; the large
FBX, original PNGs and extracted Blender files are not bundled. Generated game
assets are embedded, so installed binaries need neither Blender nor Downloads.
Each split-screen camera gets its own weapon, skin, and visibility mask;
death, invisibility and intermission hide it like the normal weapon.
Gameplay → Viewmodels controls all weapons: Center (default), Right, Left, None
(`r_drawviewmodel` 1, 2, 3, 0 respectively, archived). Side placement translates
eight units along each camera's right axis without changing aim or mirroring
meshes (except the two-handed cable, which stays centred). The portal gun follows the camera basis with negated alias pitch,
without the former fixed yaw/side offset, so looking up/down keeps its pose.

The authoritative server broadcasts transient reliable comment messages
`//qsp_portalfx owner slot kind sx sy sz ex ey ez`: rejected shot (0), placed
shot (1), or successful crossing (2). These are neither saved nor included in
late-join portal state. qclient validates the complete message, finite bounded
coordinates, and mode/kind before emitting `ClientEvent::PortalEffect`; malformed
messages are swallowed. Existing portal placement/collision timing is unchanged.

`portal_fx::Presentation` caps live effects at 64 and clears them on map changes
or time rewind. Cosmetic projectiles travel for 120 ms with a helical
particle wake; impact then triggers an expanding burst or contracting
failure fizz. The particles use exact RGB colors from `portal_draw::colour`
for all four owner pairs, independent of scripted particle settings. Crossing
bursts use the destination portal's slot color, with sound at both endpoints
when far apart. The weapon core follows that owner's last fired slot.

`qsound::portal` synthesizes four original mono PCM cues at the device rate:
FM snap and bass impulse (fire), rising resonant chime (placed), falling noisy
collapse (rejected/dissipated), and filtered air rush (crossing). Both slots have
distinct pitches. Eight cached cues feed normal positional SFX channels and
respect master SFX volume and all split-screen listeners. No Portal recordings
or extracted Valve assets are included. The add-on's existing local-only
eligibility remains in force.

Audition/export helper: `rustc --edition=2024 tools/portal_cues.rs -o target/portal-cues` then `target/portal-cues target/portal-audio` writes all eight
48 kHz mono WAVs from the same synthesis code used in-game. Generated previews
stay under ignored `target/`; the Rust sources are the editable assets.

### Railgun (`qserver::addons::railgun`, `src/addons/rail_fx.rs`)
Quake II/III homage. Holding fire shoots every 1.5 s (`REFIRE`) while the
player has 2 cells (`AMMO`; `currentammo` follows when it showed cells);
with fewer, a dry click every 0.5 s. The slug is a point trace from the eye
(`MoveType::Normal`, 8192 u): each damageable entity hit is recorded and the
trace resumes from the hit point ignoring it (up to 16), stopping at the
world or a non-damageable brush entity. Every hit gets QC
`T_Damage(target, player, player, addon_rail_damage)` (default 100), so
kills, gibs and mod rules apply. Refire timers live on `Level::rails`
(transient, not saved); `.qsa` saves only record `toy railgun`.
- Sync: `//qsp_rail owner kind sx sy sz ex ey ez nx ny nz [hx hy hz]…`
  (kind 1 fired, 0 dry; `n` wall normal or zero; hit points) →
  `qclient::RailEffect` → `ClientEvent::RailEffect`.
- Trail (FTE additive dots): dense core beam fading in 0.5 s, spiral (radius
  4 growing, one turn per 48 u) fading over 1 s, impact glow ring (2 s),
  16 gravity sparks (0.6 s), red bursts at pierced bodies. Colour:
  `addon_rail_color` 0 blue, 1 green, 2 red, 3 yellow, 4 purple, 5 white,
  rotated by owner so split-screen players differ.
- Sound (`qsound::rail`, synthesized): fire = crack, ringing zap and rising
  recharge whine ending in a ready tick at ~1.4 s; looping hum on the
  holder's entity channel 6 while held; dry click; impact hiss.
- Model: `assets/railgun/` (Meshy meshy-t2.5 3,527 tris, see its README),
  converted with `--keep-uv --glow-blue` so the Meshy UV layout is preserved
  and only the blue coil rings are marked in the glow mask. The Quake palette
  has no electric-blue fullbrights, so the masked texels go to
  `Renderer::load_alias_glow` as a true-colour RGBA glow layer while the body
  is darkened ×0.78 (`rail_model::BODY_GAIN`) before palette quantisation; drawn
  centred with `portal_pose` (1.5 u lower) + recoil.
- Menu: Add-ons → Railgun toggle (`addon_railgun`) and Railgun Settings
  (Damage slider, Trail Colour); `menu railgun` opens it.
- Split-screen: `splittoy <2..6> <0..6>` (debug) hands a guest a toy by
  `Toy::ALL` index, for headless screenshots; `toy <0..6>` does the same
  for player 1 (e.g. `"+delay 2 toy 5" "+delay 2.1 +attack"` sprays gel).
- Tests: `crates/qserver/tests/railgun.rs` (e1m1: ammo, refire, dry click).
### Hookshot (`qserver::addons::hookshot`, `hook_surface`; `src/addons/hook_fx.rs`)
Zelda homage. Fire sends a hook head out at 1600 u/s (trace `Normal`, so
monsters block it) up to `addon_hook_range` (default 1024). A hit latches
when its target is allowed: monsters (FL_MONSTER, alive) by
`addon_hook_enemies`; world and SOLID_BSP brushes by the material of the
face hit (`addon_hook_wood/metal/stone`, all default 1). Anything else, or
the range limit, retracts it (2200 u/s). Latched on a monster or mover, the
point follows the entity's origin.
- Materials: `hook_surface::Surfaces` holds every face (plane, polygon,
  class) per submodel, built from the BSP at spawn; `classify` maps texture
  names: wood (`wood`, `wizwood`, `crate`, `plank`, `door03/04`, ...),
  metal (`metal`, `met`, `wizmet`, `tech`, `comp`, `cop`, `slip`, `plat_`,
  ...), never (`sky*`, `*` liquids, `clip`, `trigger`), else stone
  (unknown counts as stone). Test: `texture_names_classify_like_id1_and_mods`.
- Pull: each frame sets the player's velocity to 750 u/s at the hook and
  clears FL_ONGROUND; normal player physics moves and collides, so the pull
  can't push anyone into geometry. It ends (keeping 45% of the speed plus a
  180 u/s hop) on arrival, a 0.2 s stall, a blocked line of sight, the
  hooked monster dying, or 3 s; fire/alt-fire or putting the toy away lets
  go without the hop.
- Sync: `//qsp_hook owner state x y z` every frame the hook is out
  (0 idle, 1 out, 2 latched, 3 back); `qclient::HookView`, `ClientEvent::Hook`
  on state changes. Hooks are transient: saves keep only `toy hookshot`.
- Presentation: launcher held centred (side placement comes from
  `r_drawviewmodel`); chain links and a barbed head as FTE quads from the view
  model's muzzle (other players: their origin); cues synthesised in
  `qsound::hookshot` (launch clank + rattle, latch thunk, retract rattle,
  miss clink). Menu: Add-ons → Hookshot Settings (`Page::Hookshot`,
  `menu_options hookshot`).
- Model: `assets/hookshot/` from Meshy (nano-banana concept, meshy-t2
  1,342 tris + meshy-n1 texture, task `01a1d66e-1a99-7656-a692-de203abb1792`),
  converted by `blender -b --factory-startup --python
  tools/import_toy_model.py -- SRC.zip assets/hookshot --length 30 --anchor
  centroid` (longest horizontal axis to +X, texture ×0.5 for alias
  overbright, no mask); `hook_model.rs` palette-quantises it. Atlas cleaned
  and worn by `tools/clean_toy_texture.py` (command in
  `assets/hookshot/import.json`).
- Tests: `crates/qserver/tests/hookshot.rs` (e1m1, game data).
### Stickybomb launcher (`addon_sticky`, `addons/sticky.rs`)
TF2 homage. Holding fire charges, releasing launches a bomb on the grenade
arc (forward 600 + up 200, `sv_gravity`); the forward speed grows to 1400
over `CHARGE` (2 s), when a still-held shot goes off by itself. 0.6 s
refire; putting the toy away, dying or running dry drops the charge. Each
bomb costs one rocket (the grenade
launcher's pool; `currentammo` follows when the QC weapon is the GL/RL).
Bombs are engine-side records, not edicts, so they never touch the edict
limit; `MAX_BOMBS` (64, all players) caps them and the shooter's oldest
goes off when it is reached. Each frame a flying bomb traces as a missile
(`MoveType::Missile`, owner ignored); on a hit it sticks: world (absolute
spot), brush entities and monsters (offset from the entity's origin, so it
rides doors, plats and monsters). Sky hits fizzle. When the host monster
dies (health ≤ 0 or `SOLID_NOT`) or is freed, the bomb falls and sticks to
whatever is below. Alt-fire (`+attack2`) edge detonates all the player's
bombs: a temporary edict calls QC `T_RadiusDamage(tmp, player, damage,
world)` (`addon_sticky_damage`, archived, default 120, clamped 25–300), then TE_EXPLOSION and `weapons/r_exp3.wav` (only if precached).
A player leaving detonates theirs; the toy switched off removes all.
- Sync: `//qsp_sticky id x y z` when a bomb moves, `//qsp_sticky id` when
  it is gone, `//qsp_sticky clear` + full state on spawn →
  `ClientState::stickies`. Saves: `sticky owner x y z vx vy vz ent ox oy oz`
  header lines (ent −1 = flying; edict numbers survive save/load);
  `save::Header::extra` keeps any non-toy/portal header line verbatim.
- Look (`src/addons/sticky_model.rs`, `sticky_draw.rs`): the bomb is a
  40-triangle studded ball built in code (16×16 skin, dark red; skin 1
  lights its band in fullbright red) that blinks ~twice a second with a
  48-unit red dlight. The launcher (2026-11-24) has no source asset;
  `python3 tools/build_stickybomb.py` (numpy/scipy, id1 palette) computes
  `assets/stickybomb/`: 3887 triangles, muzzle at +X, barrel axis at z = 0,
  512² atlas at 7.3 texels per unit. An original design made for the view
  from behind: an open carousel with six studded bombs between bronze star
  plates, a short barrel with hazard paint and a toothed crown, a skeleton
  frame, and on the left a detonator box whose sloped panel (gauge, lamp,
  toggles) faces the player, with an aerial and a cable. Each texel is
  painted from its model-space position (ray-cast occlusion on a voxel grid,
  worn edges, rivets, panel lines) in exact non-fullbright palette colours;
  `toy_model::parse` quantises them to the mod's palette at load.
  `sticky_draw::weapon` poses it centred like the portal gun, pushed out and
  tipped up (`PUSH`/`DROP`/`TILT`) so bombs and panel show at any pitch.
### Boomerang (`addons/boomerang.rs`, `src/addons/boomerang_fx.rs`)
Zelda-style homing boomerang (`addon_boomerang`, default 1). Holding fire
marks monsters: each server frame the monster nearest along the aim (centre
within its half-size + 4 + 2% of distance of the aim ray, in sight, ≤2048 u)
is appended to the queue (max 32); alt-fire clears it, putting the toy away
cancels it. Releasing fire throws a virtual projectile (no edict, so no QC
sees it) from the right hand, starting 30° off to the right; it steers
towards the current target (turn rate 4 + 6·age rad/s, so it curves and
never orbits), slides along walls, and stuns every live monster whose box
(+6) its path crosses, each once per throw. A queued target is done when hit;
dead/removed targets and ones not reached within 3 s are skipped. No marks:
it flies to the aim point (≤640 u) and back. After the queue (or 15 s) it
returns to the thrower's chest, flying through walls after 1.5 s of return
and caught at 40 u (forced after 8 s). One boomerang per player; fire does
nothing until it is caught.
- Stun (`addon_boomerang_stun`, default 4 s, 0.5–30): while stunned, a
  scheduled `nextthink` is pushed to the stun's end (so the AI resumes by
  itself; nextthink 0 is left alone) and horizontal speed is zeroed every
  frame (all speed for flyers/swimmers). Dying ends the stun. Speed:
  `addon_boomerang_speed` (default 700 u/s, 200–2000). Menu: Add-ons →
  Boomerang Settings (`menu_options boomerang`).
- Sync (`//qsp_boom`, reliable stufftext, parsed into
  `ClientEvent::Boomerang`): `m owner [ent headz]...` marks, `t owner x y z`
  throw, `f owner x y z` position every frame in flight, `c owner` caught,
  `s ent secs headz` stun. Not saved: a save mid-flight loads with the
  boomerang in hand, and stunned monsters keep their pushed `nextthink`
  (still frozen until then, without stars).
- Client: the held model is a view weapon (hidden while thrown), the flying
  one spins flat at 1080°/s. Markers are FTE billboards above the head: a
  pulsing red ring plus the queue number (procedural 3×5 digit atlas in
  `boomerang_model::atlas`); stunned monsters get five yellow stars circling
  over the head. `qsound::boomerang` synthesises throw whoosh, stun (wooden
  bonk + three warbling "tweety bird" chirp voices), catch clack and a swish
  repeated every 0.2 s in flight.
- Model: no source asset; `python3 tools/build_boomerang.py` (numpy/scipy, id1
  palette) computes `assets/boomerang/` byte for byte: 2508 triangles, 28 u
  long, thinnest axis Z, balance point at the origin, 256² atlas at 7 texels
  per unit. A bent-wood airfoil blade carries steel caps with a thin rim,
  bronze straps and boss at the elbow, and a leather-wrapped handle (+X arm).
  The skin is painted per texel in surface coordinates with exact
  non-fullbright palette colours (`toy_model::parse` quantises them to the
  mod's palette at load). The held copy (`view_model`, key `@boomerang_view`)
  is scaled ×0.6 and posed from the view axes (`cable_fx::angles_basis`), so
  it keeps its place at any pitch: handle down in the right hand, decorated
  top face towards the screen centre, blade reaching forward and left.
- Audition: `rustc --edition=2024 tools/boomerang_cues.rs -o target/boomerang-cues && target/boomerang-cues target/boomerang-audio` writes the four cues as 48 kHz WAVs.
### Gel sprayer (`addon_gel`, `addons/gel.rs`, `gel_phys.rs`; `src/addons/gel_fx.rs`)
Portal 2 homage. Holding fire sprays blue Repulsion gel, alt-fire orange
Propulsion gel: a blob every 45 ms at 950 u/s (±3.5% spread, plus half the
shooter's velocity) under `sv_gravity`. Blobs are engine-side (no edicts,
`MAX_BLOBS` 256) traced as missiles; on world they leave a splat (radius
26–40) on the face hit; on a monster or mover the gel drips onto the floor
below; sky fizzles. A new splat replaces same-colour splats on the same
plane within 0.45 r and other-colour ones within 0.8 r, so colours paint
over each other. `MAX_SPLATS` 1024, oldest out. No ammo.
- Repulsion (`gel_phys::after`, after physics with a pre-physics snapshot
  of walk/step/toss/bounce movers, taken only while blue gel exists): a
  landing faster than 170 u/s on blue floor goes back up at the landing
  speed (450–1500); a player jumping off blue gets 680 u/s (≈290 u);
  running into a blue wall faster than 180 u/s rebounds (95%) with lift.
  Walking onto it does not bounce. Grenades, gibs, items and monsters
  bounce too. A player's landing bounces in `gel_phys::land`, right after
  their own move and before QC `PlayerPostThink`, so PostThink never sees
  the landing: no fall damage or grunt on blue gel.
- Propulsion (`gel_phys::tune`, before `client_think`): a player on the
  ground on orange gets move input and `sv_maxspeed` ×2.6 and friction
  ×0.1; momentum carries on through the air, plain floor slows it.
- Footsteps: players on gel moving >60 u/s send step events (blue every
  ~0.42 s, orange ~0.3 s, scaled by speed).
- Sync (`//qsp_gel`, reliable stufftext → `ClientEvent::Gel`): `b owner kind
  pos vel` launch (the client flies the blob itself), `t kind pos vel` blob
  exiting a portal, `a id kind pos normal r`
  splat, `d id`, `c` clear + full state on spawn, `s kind pos speed`
  footstep, `j kind pos speed` bounce. Saves: `gel kind x y z nx ny nz r`
  header lines. The toy switched off removes all gel.
- Look: splats merge into one contiguous coat (`src/addons/gel_coat.rs`):
  splats on the same plane whose discs touch form a cluster; each cluster
  gets a square field in an atlas whose size, texel density, rebuild
  cadence and edge noise follow `addon_gel_quality` (`0` = low: 1024² atlas,
  4 u/texel capped at 256 texels, 0.2 s rebuild, precomputed 128×128
  2-octave tiling noise LUT; `1` = high, default: 2048² atlas, 2 u/texel
  capped at 512 texels, 0.1 s rebuild, 3-octave 3D world-space value noise;
  shelf packed). Inside each cluster, every splat adds `(1 − d²/r²)²` so
  overlaps sum instead of stacking puddles. The edge is where the sum
  crosses 0.3, roughened by noise; alpha is the height (steep rim, domed
  top, slight ripple), RGB the colour of whichever kind dominates (blended
  at a blue/orange border). `gel_fx::frame` recreates the GPU pic via
  `resize_pic` when `addon_gel_quality` switches and uploads dirty regions via
  `update_pic_rows` (staged per frame without `device_wait_idle`). Each
  cluster square is clipped to BSP faces like blood marks (`blood::mesh`,
  faces on the plane) and lit per vertex from the lightmap (`light_point`).
  The coat and flying blobs are `FteBatch { gel: true }`: `fte.frag` mode 9
  (`addon_gel_quality 1`, high) reads the atlas alpha as a height field,
  builds a tangent frame from screen derivatives, shifts the lookup for
  parallax, bends the normal by the height gradient and lights it as a thick
  liquid (two specular highlights, fresnel rim, darker thin edges, brighter
  thick middle); mode 10 (`addon_gel_quality 0`, low) uses a single height/RGB
  fetch, screen-space `dFdx`/`dFdy` slope normal perturbation, a single
  Blinn-Phong highlight and Fresnel rim without parallax or 4-tap neighbor
  gradient fetches. Texture RGB tints the vertex colour (light for the coat,
  white atlas for sprites). Parallax and gradient steps are in world units
  (uv-per-unit from derivatives), so any atlas scale shades alike. Atlas
  (`gel_fx::atlas`, 2×2 tiles of 128²): three metaball splats (unused by
  the coat) and a dome for blobs, droplets and the washer jet.
- Sound (`qsound::gel`, synthesised): looping wet spray while blobs fly
  (owner entity, channel 7), splat, blue footstep = rubber-band stretch
  squeak gliding up, bounce = spring boing, orange footstep = swept zip in
  three tiers by speed (>450, >750 u/s brighter and higher).
- Model (2026-11-16): `assets/gel/`, a rusty rifle-like cannon with twin
  octagonal-capped canisters (orange left, blue right) beside the barrel, box
  magazine and skeletal stock. From Meshy: nano-banana-pro concept
  `01a1e795-1889-7343-aee4-7e0b64b5de7c` (a clean 3/4 view redrawn from a
  six-view reference sheet), meshy-t2.5 3,548 → 3,484 tris + meshy-7 texture,
  task `01a1e795-e03c-7203-bd55-81c256f9a193`. Imported with
  `tools/import_toy_model.py -- model.glb --out assets/gel --length 40
  --gain 1.0 --keep-uv --flip --mirror` (the Meshy mesh faced -X and came
  out with the canisters swapped). Drawn level, 18.5 u ahead and 11.5 u
  below the eye, so the view looks down past the canisters along the barrel.
  Saturated blue/orange texels (the canisters) become a true-colour glow
  layer (`toy_model::gel_glow`, drawn unlit in exactly the painted colour),
  since the palette has no bright blue.
  Re-unwrapped by `tools/relayout_toy_uv.py` (Meshy's own layout varied
  a lot in texel density). Skin (2026-11-24): painted from scratch by
  `tools/paint_gel_texture.py` (command in `assets/gel/import.json`), which
  ignores Meshy's colours (pale blotches once lit, black cracks and stray
  colour on the canisters): steel barrel and drum, warm iron receiver, bronze
  canister caps and fittings, rubber hoses, walnut stock and grip, with panel
  lines, rivets, cap bolts, vents, gel currents, and a gel sight glass in each
  rear hub that faces the player. Metal is kept low in the ramps because view
  models are lit brightly, and the gel colours allow for contrast and bloom.
  Metal texels never pass the glow test and gel texels all do (asserted).
- Tests: `crates/qserver/tests/gel.rs` (e1m1: spray, save, wash, bounce,
  no fall damage on blue, orange speed); `gel_coat` unit tests (seams
  between splats are filled, walls and floors stay separate).
### Powerwasher (`addon_washer`, `addons/washer.rs`; `src/addons/water_fx.rs`)
Water cannon, meant to go with the gel sprayer. Fire: one straight,
lightning-gun style jet reaching 560 u (traced in 12 segments); alt-fire: a
fan of five straight 320 u jets at −14…14° yaw. No ammo. Per jet each frame: the first
monster/player/toss object hit is pushed along the jet (3200 u/s² stream,
1100 fan, × `addon_washer_push` 0.25–3, capped at 650 u/s; a grounded one
is lifted 110 u/s so step physics slides it; no damage); projectiles
(flymissile/bounce) within 30 u of the jet are turned towards its direction
(rockets re-aimed) and become the washer's; where it lands on a wall or
floor gel within 44 u (+0.6 r) is washed off.
- Sync: `//qsp_water owner mode x y z…` every frame while spraying (one
  end per jet), `owner 0` on stop → `ClientEvent::Water`. Not saved.
- Client: each jet is a solid water column, a camera-facing ribbon (16
  segments on high `addon_washer_quality 1` [default], 10 on low `0`) across
  the middle row of the dome tile; on high it uses the gel shader so it
  shades as a glossy cylinder, and on low a single translucent water batch
  (`gel: false`). Half width 1.6→4.5 u with travelling ripples and a flare
  at the tip, fading in at the nozzle and out at the end. It leaves the
  view model's muzzle along the aim and joins the server's line within
  48 u. Plus loose droplets around it (14 per jet on high, 6 on low) and
  splash droplets (capped at 120 on high, 48 on low) drawn in a non-gel
  translucent batch, a hiss loop (jet / fan, channel 7), splashes; blood
  surface marks within 44 u of each end are removed (`Blood::wash`).
- Model: `assets/washer/` (nano-banana-pro concept
  `01a1d71e-0b15-7650-b42c-64a73647d9fd`, meshy-t2.5 2,498 tris + meshy-7,
  task `01a1d721-4189-74a0-b4a3-702bf531235d`), imported with `--length 30
  --gain 0.6 --flip`, then re-unwrapped (`tools/relayout_toy_uv.py`; the import
  left one island per triangle on 5.6% of the atlas) and cleaned by
  `tools/clean_toy_texture.py --from` to a worn copper-brown tank and dark
  steel hose (commands in `assets/washer/import.json`).
- Menu: Add-ons → Gel Sprayer (Gel Quality `addon_gel_quality` low/high, default high), Powerwasher, Powerwasher Push, Jet Quality (`addon_washer_quality` low/high, default high) (`menu_options gel` / `menu_options washer`).
### Male-to-male cable (`addon_cable`, `addons/cable.rs`, `cable_rope.rs`; `src/addons/cable_fx.rs`, `cable_rope_fx.rs`, `cable_glow.rs`)
The player holds one plug in each hand. Alt-fire on a surface that takes
the plug, within `cable::REACH` (112 u) of the eye, plugs one end in;
otherwise it is a miss (cue only). Sockets are faces whose texture shows
electronics (`hook_surface::electronic`: `comp`, `tech`, `twall`, `tlight`,
`htek`, buttons `basebtn`/`butn`/`button`, switches `mtlsw`/`swtch`/
`floorsw`, slipgates `slip`, plus `screen`, `monitor`, `panel`, `console`,
...), or any metal with `addon_cable_metal 1` (default 0, for maps without
computer panels). World faces and brush entities (the socket then follows
the entity) both work.
- Plugged: fire shoots a lightning bolt from the free plug (infinite ammo):
  a 600 u `Normal` trace from the eye; every 0.1 s the first entity hit takes
  `addon_cable_damage` (default 30, the stock lightning gun's) through QC
  `T_Damage`, with the stock blood puff. Stock `weapons/lstart.wav` /
  `lhit.wav` play when the progs precached them.
- Tether (`cable_rope::Rope`): a rope from the socket to the player's hull
  centre that wraps round world corners. When the line from the last anchor
  to the player is blocked, a binary search between last frame's clear
  point and this frame's finds the corner and a pivot goes just outside it
  (nudged sideways towards the clear side and off the face). Each pivot
  stores its bend direction; it is removed when the rope straightens through
  it and the line from the previous anchor is clear. Only the world snags
  the rope (movers don't); at most 24 pivots. Length is measured along the
  rope; past `addon_cable_length` (default 512, 128–4096) the player loses
  the outward speed and gets a pull back of 10 u/s per unit over (≤ 500), so
  player physics does all the moving. Over by more than 160 u (teleporters,
  pushers) tears it out.
- Unplugged by alt-fire, putting the cable away (any other weapon), death,
  the socket's entity disappearing, add-ons going off or a level change.
- Sync: `//qsp_cable owner state fire sx sy sz nx ny nz ex ey ez length
  [px py pz]...` every frame while plugged (state 1, `fire` 1 while the bolt
  reaches `e`, the rope's pivots); state 0 once on unplugging, 2 for a
  miss. `qclient::CableView`, `ClientState::cables`, `ClientEvent::Cable`
  on plug/yank/miss/bolt start. Cables are transient: saves keep only `toy
  cable`.
- Toy id 8 needed a fourth id bit: usercmd buttons are `u16` in
  `ClientMove`/`UserCmd` (toy id bits 4–7, dodge bit 8). Only the low byte
  goes on the wire; add-ons never run with remote clients.
- Client: view model = two Meshy gloved fists (`assets/cable/`) low in
  the corners, each holding its plug pins up in a hammer grip (the fist is
  posed from forearm and grip directions with roll, `cable_fx::fist`);
  each plug's aim is a damped spring kicked by turning and acceleration
  (buzzing while the bolt fires) that the fist follows, the hands sway on
  a stiffer one. Plugging in drops the left hand out of view and cocks the
  right wrist so its plug, held by the tail, points ahead (0.25 s). Being
  two-handed, the cable ignores the Viewmodels left/right alignment. A world plug sits in the
  socket. Ropes are verlet chains (`RopeSim`, ~10 u links, ≤ 96 nodes)
  between the socket, the server's pivots and the owner's body, sharing
  the slack between spans, point-traced against the client BSP so they lie
  on floors; the owner's own rope ends at his plug. A 70 u loop hangs
  between the hands while unplugged, carried rigidly with them before each
  step (`RopeSim::carry`) so walking and turning only make it sway instead
  of whipping up past the eye. A rope not stepped for 0.25 s (put away,
  or the hand loop while plugged in) or whose anchor jumps further than its
  length (teleport) is laid afresh between its anchors. Ribbons are camera-facing dark rubber
  quads. The bolt is stock `progs/bolt2.mdl` segments from the plug's pins.
- Highlight (`cable_glow`): while the cable is held and not plugged,
  socket faces facing the eye within 320 u get a faint pulsing cyan
  outline, and the one under the crosshair within reach a brighter outline
  and fill (client BSP + the same texture rules, world and unrotated brush
  entities; the crosshair test stops at the world).
- Cues: synthesised `qsound::cable` (plug clunk + hum swell, yank pop +
  sparks, miss clunk).
- Menu: Add-ons → Male-to-Male Cable (`Page::Cable`, `menu_options cable`):
  Cable Length, Plug Into Metal, Bolt Damage. Default weapon slot 8 (the
  lightning gun's), wheel label "MM".
- Tests: `crates/qserver/tests/cable.rs` (e1m1: plug into a tech panel,
  bolt, tether holds a player walking away, alt-fire and weapon switch
  unplug, the floor refuses); `cable_rope` (wrap round a pillar and unwrap
  coming back), `cable_rope_fx` (slack rope lies on the floor, taut one
  stays straight), `hook_surface` electronics names, `qclient::cable`.
### Guns with magazines (`addons/guns.rs`, `qclient::guns`, `src/addons/mag_hud.rs`)
Shared by the akimbo pistols and the assault rifle. `Magazine` holds the
rounds and a running reload; `bullet` is a hitscan shot from the eye (through
linked portals) that hurts the first entity via QC `T_Damage` and sends the
stock blood particles or TE_GUNSHOT. `//qsp_gun` lines report shots, dry
clicks and reload start/cancel/finish (`ClientEvent::Gun`); `//qsp_mag` lines
(`MagSync`, on change) fill `ClientState::mags`, drawn by `mag_hud` above the
status bar with a bar while reloading.
- Reload action: `+reload` (qinput button, usercmd bit 9
  `BUTTON_TOY_RELOAD`, only while a toy is held) is shared by every add-on
  with `Addon::reload`; the add-on browser and Controls list it. R by default
  (`cl_reloadbind` merges it once into configs whose R is free).
### Akimbo pistols (`addons/akimbo.rs`; `src/addons/akimbo_fx.rs`, `akimbo_model.rs`)
A pistol in each hand: fire shoots the left one, alt-fire the right one
(`addon_akimbo_swap` exchanges them). Each has its own magazine
(`addon_akimbo_mag`, 12) and timers. A press shoots at once (at most every
0.11 s), a held trigger repeats every 0.22 s. Bullets are `guns::bullet`
with `addon_akimbo_damage` (14) inside a cone of `addon_akimbo_spread` (1.5)
degrees, denser in the middle; shots set `show_hostile` and a small
`punchangle`. The reserve is endless.
- Reload: `+reload` reloads every pistol that isn't full, the second one
  0.35 s later (`STAGGER`) than `addon_akimbo_reload` (1.6 s); a pistol not
  being reloaded can still shoot. Pulling the trigger of an empty one clicks
  and, with `addon_akimbo_autoreload` (1), reloads it. Putting the pistols
  away or dying interrupts reloads.
- Saves: `akimbo client left right` header lines (rounds loaded); a new level
  starts with full magazines.
- Model: `assets/akimbo/` from Meshy (concept nano-banana-pro
  `01a20ef1-53eb-706d-a574-5b6e1261ff60`, mesh meshy-t2.5 + meshy-n1
  `01a20ef1-c6a5-779d-a2af-235080de3ec6`, 24 credits), archived in
  `~/Assets/Meshy/akimbo`. `tools/prep_akimbo.py` (Blender) widens it, closes
  the slide's back and adds the octagonal muzzle brake, recoil weight and
  fins; `tools/paint_akimbo_texture.py` ignores Meshy's colours and paints a
  material per connected part (gunmetal slide, bronze frame, walnut grip with
  leather strips, hazard band). All commands are in `import.json`
  (`cleanup`). The right pistol is the source; `akimbo_model` mirrors it for
  the left hand and makes poses at load: slide back, magazine out, both
  (parts found by connectivity and bounds). The loose magazine is a box
  built in code.
- View: one pistol in each lower corner (`pose`, camera basis, so it stays
  put at any pitch; no left/right view model alignment), muzzles slightly
  inwards. Shots kick that pistol and its slide; an empty one keeps its slide
  back. A reload raises the pistol, turns the grip to the middle, drops the
  magazine, pushes a new one in and works the slide, timed by the event's
  `secs`. Flash and light follow the view model's muzzle; a tracer, smoke and
  a brass casing per shot (other players': from the server's muzzle point).
- Sound: `qsound::akimbo` (two shots, dry, magazine out and in, slide), on
  the owner's channels 6 and 7; recorded, see "Gun sounds" below. The slide
  and the muzzle brake move together.
- Menu: `Page::Akimbo` (`menu_options akimbo`). Default slot 2, label "AK".
- Tests: `crates/qserver/tests/akimbo.rs` (e1m1), `akimbo`, `akimbo_model`
  and `akimbo_fx` unit tests.
### Assault rifle (`addon_rifle`, `addons/rifle.rs`; `src/addons/rifle_*.rs`)
A full-auto carbine with a tube red dot sight. Fire shoots 750 rounds a minute
(`INTERVAL` 0.08 s; the remainder carries over frames, so the rate is the same
at any tic rate) from a magazine (`addon_rifle_mag`, default 30) of bullets of
`addon_rifle_damage` (10). Reload (2.2 s, `addon_rifle_reload`, +0.35 s to
chamber a round after running dry) takes what is missing from the nails when
it finishes (`currentammo` follows while it shows nails); it starts by itself
0.3 s after the last round (`addon_rifle_autoreload`). With no nails an empty
rifle clicks. Putting it away or dying cancels a reload; nothing is lost. The
cone (`CONE`, `BLOOM`, `RUN`, `AIR`) widens with sustained fire, speed and in
the air, and is about a twentieth when aimed. Aiming walks at 0.6 speed
(`rifle::tune`, on the move's parameters only, like orange gel). Each level
starts with a full magazine; `.qsa` saves keep `rifle <client> <rounds>`.
- Aiming (`rifle_ads::Ads`, one on `Host` and one per split-screen guest):
  turns hold or toggle (`addon_rifle_aim`) into a held `BUTTON_TOY_ALT`, the
  only thing the server sees (its own 0.2 s ramp, `AIM_TIME`, sets the cone).
  A menu, death, another weapon or a reload lowers the sight and drops the
  toggle; a held button raises it again after a reload. The eased amount
  drives the pose, the FOV (`Ads::fov`: magnification `addon_rifle_zoom`,
  default 1.3, on top of `zoom_fov`, the `+zoom` FOV; per view in
  split-screen) and `qinput::Input::look_scale` (mouse, sticks, gyro and touch
  slow by the same factor; keys, flick turns and aim assist do not).
- Recoil (`addon_rifle_recoil`) moves the real view angles, so bullets always
  go where the view and the dot point: each shot climbs 0.46 degrees and
  wanders, 55% of that when aimed; 70% of the climb settles back once firing
  stops, less what the player pulled down. No kick in demos.
- The sight: the models share one space (`tools/build_rifle.py`), where the
  tube's axis is exactly known (`rifle_model::LENS`, `APERTURE`, `TUBE`; a test
  checks no triangle crosses it). `rifle_pose` holds the rifle in the
  middle, right or left as Viewmodels says and blends from there to the tube
  centred on the view at 14 units (10.5 to 17 by FOV, so the sight keeps about
  the same size on screen and its rear never reaches the near plane). The dot
  (`rifle_dot`) is 2D, drawn after the scene where `viewangles` falls in the
  view, so it stays on the aim point whatever the rifle does, and only while
  that spot is seen through both ends of the tube (`Pose::through`). The
  crosshair is hidden meanwhile (it stays with Viewmodels at None).
  `addon_rifle_dot`: 0 red, 1 green, 2 yellow.
- Presentation (`rifle_fx`): muzzle flash (halved when aimed; the server also
  sets `EF_MUZZLEFLASH`), smoke, tracers at 9000 u/s, tumbling brass from the
  ejection port; the magazine is its own model that drops out and comes back
  during a reload (`rifle_pose::magazine`), with cues at `MAG_OUT`, `MAG_IN`
  and `BOLT` (only after running dry). Sounds:
  `qsound::rifle`, recorded ("Gun sounds" below; three shot takes in turn on
  channels 6–8) except the sight's synthesised rustle.
- Model: `assets/rifle/`. Meshy: nano-banana-pro concept
  `01a20ef1-534f-724b-848f-aae0c2ae9c26`, meshy-t2.5 (3,946 tris) + meshy-n1
  task `01a20ef1-cd19-75c4-b93f-4fe57e001aba`, asked for without any optic. The
  commands are in `assets/rifle/import.json`: `tools/import_toy_model.py
  --keep-uv` (Meshy's UVs are even; `relayout_toy_uv.py` would stretch them,
  it unwraps the MDL's per-axis packed grid), then `tools/build_rifle.py`,
  which scales it to 52 units, splits off the magazine, builds the sight and a
  crisp receiver rail, and paints both skins (material per connected part,
  ray-cast occlusion, worn edges, single-texel grain, exact palette colours).
- Menu: Add-ons → Assault Rifle (`Page::Rifle`, `menu_options rifle`). Default
  weapon slot 4, wheel label "AR"; always selectable.
- Tests: `crates/qserver/tests/rifle.rs` (e1m1: rate, reload, nails, dry
  click, cancel, save, 20 fps, aiming speed), `rifle` unit tests in qserver,
  `rifle_ads`, `rifle_pose`, `rifle_dot`, `rifle_model`, `qsound::rifle`.
### Gun sounds (`assets/gun-sounds/`, `tools/gun_sounds.py`)
The pistols' and the rifle's sounds are CC0 recordings (sources and licences
in `assets/gun-sounds/README.md`; raw files in `~/Assets/Sounds/guns/`). The
tool levels a shot's spike and body, gives it a smooth decay (the echo must
not read as a second shot), adds a low thump, dulls the top and soft-clips,
aiming at stock weapons as measured in id1 (full-scale peak, RMS 0.5–0.65
over the first 0.1 s); handling noises are cut, compressed and darkened.
Output is 22050 Hz 16 bit mono, loaded like any WAV (`load_sfx`).
### Equipment wheel and goggles (`src/addons/goggles.rs`, `equip_wheel.rs`, `src/host/goggles.rs`)
Equipment is switched on and off rather than held, so it does not use the
toy slot. `Equip` lists it in wheel order; items of one group exclude each
other (night vision and thermal share the goggle group). State is player 1's
`Equipment` on the host, cleared whenever `goggles_allowed()` fails (no
add-on game, deathmatch, remote client, or `addon_goggles` 0, which is
locked like the toy cvars). It is not saved: goggles start off after a load.
- Input: `+equipwheel` (qinput button, Controls → "Equipment Wheel
  (Add-ons)", shown with the other add-on binds) opens a radial wheel in the
  weapon wheel's style; it shares `wheel_aim` (either stick, or the mouse
  cursor), and releasing on a slot toggles it. Slots follow `Equip::ALL`, so
  new equipment is one enum variant. Commands: `goggles` / `goggles cycle` (normal → night vision →
  thermal → normal), `goggles nv|thermal|off`, `equip <nv|thermal>` (toggle). Each
  switch plays a synthesised cue (`goggles::cue`, rising on / falling off,
  non-positional) and centerprints the new state.
- Rendering: the host passes `qrender::Goggles { mode, intensity, noise, quality }`
  in `Settings`; the UBO carries it in `soft.zw`/`softf.z`, `PostPush` in
  `goggles`/`g_*`. Night vision (mode 1) is post only: luminance lifted
  with `1 - exp(-7·gain·l)`, a constant-offset ring glow around bright spots
  (`0` low skips taps, `1` medium default uses 4 diagonal taps, `2` high uses
  8 taps), green phosphor tint, grain, every third line darkened, faint
  flicker, oval vignette. Thermal (mode 2): scene shaders write a grey heat level instead
  of colour (`thermal()` in `common.glsl`): world `0.05 + 0.2·luma` plus 1.6×
  fullbright texels (torches, lights), lava 0.88+, other liquids colder,
  sky 0.02, particles 0.55+, sprites (explosions) 0.65+; alias models by
  `RenderEntity::heat` (texs.w bits 8–9): cold, warm 0.42 (gibs, heads,
  projectiles), body 0.6–0.95 (monsters, players), fire 0.85+ (flames).
  Heat comes from the model file name (`goggles::heat_of`, cached per level
  in `HeatTable`). Body-class models are drawn again after particles with
  `alias_xray` (depth GREATER, no write, additive, flag 1024 → flat 0.42),
  so they glow through walls. Post maps heat through a blue-grey → purple →
  red → orange → white palette, with gain, glow, grain and a soft vignette.
  Split-screen views share the post effect (player 1's goggles).
- Options (Add-ons → Goggles Settings, `menu_options goggles`):
  `addon_goggles` (1), `addon_goggles_quality` (`0` low / `1` medium default / `2` high),
  `addon_goggles_intensity` 0.25–3 (1; night-vision gain / heat contrast),
  `addon_goggles_noise` 0–2 (1; grain, scanlines).
- Headless check: `+custom_enable 1 +sv_autosave 0 +map e1m1 "+delay 2
  goggles thermal" "+delay 3 screenshot target/shots/thermal"`.

### Browser and gameplay preview tooling

`src/menu/addon_browser.rs` (add-on table in `addon_browser/list.rs`) owns selection, description scrolling and contextual
rows. The browser must list a separate, weapon-labelled slot setting for every
weapon supplied by the selected add-on, above the input remapping header. An
add-on may supply multiple weapons even though each current weapon add-on
supplies only one; equipment without weapons has no weapon slot settings.
The browser metadata stores a weapon list per add-on; row positions, binding
actions and slot edits use its length. Tall settings lists scroll with focus
to keep keyboard/controller selections visible within the settings panel.
Wheel scrolling targets the hovered settings box; Page Up/Down scroll the
focused box. Settings retain an independent offset and show a scrollbar.
Mapping labels use action names without internal shared/unique qualifiers.
Tab moves between the checklist, description, slot assignment, action
bindings and Configure Parameters. Arrow keys / D-pad select and adjust;
Enter / A toggles or captures a binding. New Game's Start button is reachable
by Tab / D-pad as well as mouse, Y or F10. Down from the last checklist entry
focuses Start; Up from Start returns to that entry. A activates focused Start.
Down at the last settings row stays in the bottom-right panel.
Bumpers cycle checklist, description, settings and Start without an on-screen
bumper prompt. The focused section has a two-unit gold outline, drawn with
`Canvas::round_frame`. The Start button includes the controller family's Y glyph;
the footer keeps contextual A/B prompts. Keyboard Start has no F10 label.
Wheel / PgUp / PgDn scroll the description.
Delete / Backspace clears keyboard bindings; X clears controller bindings.
`addon_bindings.rs` also protects the Controls menu: occupied keys keep their
old command until Yes (Y / controller A / click) confirms replacement;
Escape / B / Enter declines. The assigned key is drawn as a glyph, with Yes/No
prompts for the active device and controller family. Both Controls and the browser
use `draw_binding_columns`: only the active device column flashes during capture,
and the other device's existing glyphs remain visible. Capture accepts any device.
`host::tests::menu_controller` exercises platform button/stick events; optional
`QUAKE_MENU_SHOTS=target/shots` writes navigation, capture and conflict screenshots
(skips without game data/Vulkan).

`tools/capture_addon_previews.py` records the add-ons with a preview on e1m1 using the
headless renderer and installed game data (`--basedir` or normal discovery).
It needs a release executable (`--engine` to override) and Python Pillow.
The 48-frame, 12 fps, 320×92 silent loops in `assets/addon-previews/*.qav`
show linked portals, a boomerang stunning a soldier, planted bombs and remote
detonation, hookshot travel, railgun shots and all three goggle modes.
Source screenshots and logs stay in `target/addon-previews/`; config.cfg and
game files are never written by the capture tool. The bundled preview
assets are rendered footage, not game archives, maps, models or reference repos.

QAV1 has a little-endian header (magic, u16 width, height, frame count, fps),
then u32 compressed length + zlib RGB for each independent frame.
`src/draw/addon_video.rs` bounds decoding and the existing Pics cache uploads
each requested frame once. No video decoder dependency or background server
is needed; playback follows menu time and preserves the footage's aspect ratio.
Mods can override individual frames at `addons/previews/<id>/000.tga` through
`047.tga` using the normal filesystem priority. All bundled clips are tested
for complete decoding, dimensions and motion without needing installed game data.
