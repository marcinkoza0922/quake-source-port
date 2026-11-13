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
