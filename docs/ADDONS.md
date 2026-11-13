# Add-ons design

### Add-ons (main menu → Add-ons; `qserver::addons`, `src/addons.rs`)
Optional engine-side toys, so they work with any progs. `addons_enable`
(default 0) is the master switch; each toy has its own cvar
(`addon_portalgun`, default 1) and options page, dimmed while the master is
off. Toys are only allowed with no deathmatch and no remote client
(`addons::allowed`/`all_local`); otherwise every slot empties.
- Input: `+toy` cycles a player's slot through the enabled toys and back to
  the QC weapon; any weapon impulse (1–8, 10, 12, incl. the wheel) puts the
  toy away. `+attack2` is alt-fire. Both binds appear on Controls only
  while `addons_enable` is on (`ADDON_BINDS`, synced on menu key/draw).
- While a toy is held, `ToySlot::buttons` keeps QC's attack bit at 0 and
  sends usercmd bit 2 (fire), bit 3 (alt) and the toy id in bits 4–7; QC
  only reads bits 0/1. The QC view model is replaced by the procedural projector (player 1
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
  colour pair, linked exit). Saves write portals to `<save>.qsp` beside the
  untouched `.sav` (manual saves only, not autosaves).
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
`src/addons/portal_model.rs` builds an original native MDL: a beveled iron
receiver, brown armor plates, tarnished copper induction collars, squat emitter
jaws, recessed core and dorsal mode indicator. `portal_skin.rs` paints a
128×64 palette-indexed atlas with panel seams, slotted fasteners, scratches,
oxidation, cooling slots and grip ribs. Faces have projected UVs; only the core
tile changes between the eight owner/slot skins. Metal excludes fullbright
palette indices. No game assets are copied. It is uploaded once per level and
drawn as a depth-compressed view weapon, with idle movement and firing recoil.
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
respect master SFX volume and all split-screen listeners. No Portal recordings,
models, textures or other external assets are included. The add-on's existing
local-only eligibility remains in force.

Audition/export helper: `rustc --edition=2024 tools/portal_cues.rs -o target/portal-cues` then `target/portal-cues target/portal-audio` writes all eight
48 kHz mono WAVs from the same synthesis code used in-game. Generated previews
stay under ignored `target/`; the Rust sources are the editable assets.
