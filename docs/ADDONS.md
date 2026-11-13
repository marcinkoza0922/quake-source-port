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
