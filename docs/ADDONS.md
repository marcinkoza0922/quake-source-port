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
  only reads bits 0/1. The view model is hidden for that player (player 1
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

