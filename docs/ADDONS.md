# Add-ons design

### Add-ons (main menu → Add-ons; `qserver::addons`, `src/addons.rs`)
Optional engine-side toys, so they work with any progs. A game has add-ons
only if it was *started* with them: `Core::addons_on`, set by `map` from
`addons_enable` (the archived New Game checkbox) or by `load_game` from the
save, mirrored in read-only `addons_active` for the menu. It never changes
mid-game, so a game started without add-ons can't gain them. Which toys are on
is the per-toy cvar (`addon_portalgun`, default 1), editable live in an
add-on game (Main menu → Add-ons) and fixed (dimmed) otherwise. Toys only
run with no deathmatch and no remote client (`addons::allowed`/`all_local`).
- New Game: single-player menu has an "Add-ons" checkbox (left/right or
  click). Ticked, New Game opens `Page::NewGameAddons` (toy toggles, portal
  settings, Start New Game) with a warning about other ports.
- Saves (`addons/save.rs`): add-on games save as `.qsa` (never `.sav`) with a
  `QSPADDONS 1 <n>` header (toys, portals) before the v5 body, so other ports
  neither list nor parse them. `savegame_path` picks the extension, `find_save`
  loads either (newer wins), saving removes the same-named other-format file,
  autosave rotation carries each file's extension. Load/Save cards show an
  "ADD-ONS: …" badge (`SaveInfo::addons`). A plain `.sav` load turns add-ons off.
- Input: `+toy` cycles a player's slot through the enabled toys and back to
  the QC weapon; any weapon impulse (1–8, 10, 12, incl. the wheel) puts the
  toy away. `+attack2` is alt-fire. Both binds appear on Controls only
  while the game has add-ons, or with none running while the checkbox is
  ticked (`ADDON_BINDS`, synced on menu key/draw).
- While a toy is held, `ToySlot::buttons` keeps QC's attack bit at 0 and
  sends usercmd bit 2 (fire), bit 3 (alt) and the toy id in bits 4–7; QC
  only reads bits 0/1. The QC view model is replaced by the imported projector (player 1
  and each split-screen guest have their own `ToySlot`).
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
  shifted +16/+32/-16 along the exit's up; skipped if all are solid. Missiles explode on
  the wall before the check, so they don't cross.
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
with idle movement and firing recoil.

Reimport with `blender -b --factory-startup --python-exit-code 1 --python
tools/import_portal_gun.py -- /path/to/Meshy_export.zip`. This offline Blender
helper preserves the external source archive and needs no Quake game data.
The imported mesh and color atlas were supplied by the project owner; the large
FBX, original PNGs and extracted Blender files are not bundled. Generated game
assets are embedded, so installed binaries need neither Blender nor Downloads.
Each split-screen camera gets its own weapon, skin, and visibility mask;
death, invisibility and intermission hide it like the normal weapon.
Gameplay → Viewmodels controls all weapons: Center (default), Right, Left, Off
(`r_drawviewmodel` 1, 2, 3, 0 respectively, archived). Side placement translates
eight units along each camera's right axis without changing aim or mirroring
meshes. The portal gun follows the camera basis with negated alias pitch,
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
- Model: `assets/railgun/` (Meshy meshy-t2 1,365 tris, see its README),
  converted by `tools/import_toy_model.py` (generic copy of the portal
  importer: `--out --name --length --back`, glow mask = pixels with one
  dominant channel, drawn fullbright). Drawn with `portal_pose` + recoil.
- Menu: Add-ons → Railgun toggle (`addon_railgun`) and Railgun Settings
  (Damage slider, Trail Colour); `menu railgun` opens it.
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
- Presentation: chain links and a barbed head as FTE quads from the view
  model's muzzle (other players: their origin); cues synthesised in
  `qsound::hookshot` (launch clank + rattle, latch thunk, retract rattle,
  miss clink). Menu: Add-ons → Hookshot Settings (`Page::Hookshot`,
  `menu_options hookshot`).
- Model: `assets/hookshot/` from Meshy (nano-banana concept, meshy-t2
  1,342 tris + meshy-n1 texture, task `01a1d66e-1a99-7656-a692-de203abb1792`),
  converted by `blender -b --factory-startup --python
  tools/import_toy_model.py -- SRC.zip assets/hookshot --length 30` (generic
  copy of the portal importer: longest axis to +X, texture scaled by 0.5 for
  alias overbright, no mask); `hook_model.rs` palette-quantises it.
- Tests: `crates/qserver/tests/hookshot.rs` (e1m1, game data).
### Stickybomb launcher (`addon_sticky`, `addons/sticky.rs`)
TF2 homage. Fire (held) lobs a bomb every 0.6 s on the grenade arc
(forward 600 + up 200, `sv_gravity`), costing one rocket (the grenade
launcher's pool; `currentammo` follows when the QC weapon is the GL/RL).
Bombs are engine-side records, not edicts, so they never touch the edict
limit; `MAX_BOMBS` (64, all players) caps them and the shooter's oldest
goes off when it is reached. Each frame a flying bomb traces as a missile
(`MoveType::Missile`, owner ignored); on a hit it sticks: world (absolute
spot), brush entities and monsters (offset from the entity's origin, so it
rides doors, plats and monsters). Sky hits fizzle. When the host monster
dies (health ≤ 0 or `SOLID_NOT`) or is freed, the bomb falls and sticks to
whatever is below. Alt-fire (`+attack2`) edge detonates all the player's
bombs: a temporary edict calls QC `T_RadiusDamage(tmp, player, 120,
world)`, then TE_EXPLOSION and `weapons/r_exp3.wav` (only if precached).
A player leaving detonates theirs; the toy switched off removes all.
- Sync: `//qsp_sticky id x y z` when a bomb moves, `//qsp_sticky id` when
  it is gone, `//qsp_sticky clear` + full state on spawn →
  `ClientState::stickies`. Saves: `sticky owner x y z vx vy vz ent ox oy oz`
  header lines (ent −1 = flying; edict numbers survive save/load);
  `save::Header::extra` keeps any non-toy/portal header line verbatim.
- Look (`src/addons/sticky_model.rs`, `sticky_draw.rs`): the bomb is a
  40-triangle studded ball built in code (16×16 skin, dark red; skin 1
  lights its band in fullbright red) that blinks ~twice a second with a
  48-unit red dlight. The launcher is a Meshy import (concept nano-banana
  `01a1d669-d70f-7701-a63a-9822cdf2662d`, meshy-t2 + meshy-n1 task
  `01a1d66a-00dd-7228-897d-05a59fec46ac`, 18 credits) converted by
  `tools/import_toy_model.py` (generic copy of the portal importer, no
  energy mask: `blender -b --factory-startup --python
  tools/import_toy_model.py -- DIR_OR_ZIP --out assets/stickybomb`), skin
  quantized to the active palette's non-fullbright colours.
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
- Model: Meshy nano-banana concept → meshy-t2 Smart Topology (target 1000,
  990 tris) + meshy-n1 texture, task `01a1d66c-46a1-72b3-a95b-549075c70824`
  (18 credits). Imported with `blender -b --factory-startup --python-exit-code 1
  --python tools/import_toy_model.py -- model.fbx assets/boomerang boomerang
  --flat` (28 u long, thinnest axis Z, 256² atlas darkened ×0.75, converted to
  the mod palette at load, no fullbrights). The FBX stays external.
- Audition: `rustc --edition=2024 tools/boomerang_cues.rs -o target/boomerang-cues && target/boomerang-cues target/boomerang-audio` writes the four cues as 48 kHz WAVs.
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
  new equipment is one enum variant. Commands: `goggles` (toggle night
  vision), `goggles nv|thermal|off`, `equip <nv|thermal>` (toggle). Each
  switch plays a synthesised cue (`goggles::cue`, rising on / falling off,
  non-positional) and centerprints the new state.
- Rendering: the host passes `qrender::Goggles { mode, intensity, noise }`
  in `Settings`; the UBO carries it in `soft.zw`/`softf.z`, `PostPush` in
  `goggles`/`g_*`. Night vision (mode 1) is post only: luminance lifted
  with `1 - exp(-7·gain·l)`, an 8-tap ring glow around bright spots, green
  phosphor tint, grain, every third line darkened, faint flicker, oval
  vignette. Thermal (mode 2): scene shaders write a grey heat level instead
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
  `addon_goggles` (1), `addon_goggles_intensity` 0.25–3 (1; night-vision
  gain / heat contrast), `addon_goggles_noise` 0–2 (1; grain, scanlines).
- Headless check: `+addons_enable 1 +sv_autosave 0 +map e1m1 "+delay 2
  goggles thermal" "+delay 3 screenshot target/shots/thermal"`.
