# Enhanced blood (`src/blood/`, Enhancements › Enhanced Blood Splatter)
`r_blood` (default 0, archived) enables cosmetic surface splatters and localized
MDL wounds independently of `r_debris` and scripted particle packs. Blood Settings
archives `r_blood_amount` (5 rays/hit), `_size` (1), `_lifetime` (120 s),
`_maxmarks` (512), `_wounds` (1), and `_gibs` (1). Classic blood messages and
DP blood point effects supply candidate hits, accepted only beside flesh
meshes (death sequences, gibs, heads); shootable buttons/triggers cannot bleed.
Confirmed live-enemy blood can use the nearest skin within 32 units when QC
box damage misses animated triangles; cover traces still block this fallback. Shots crossing dead
MDL triangles also bleed even when QC gives the corpse no damage handler.
The debris shooter's inferred firing line supplies spray direction. Local players' own bodies (`portal_only`) take no wounds; their spray leaves outwards.
Wounds appear at once; surface marks do not. Each spray ray launches a
droplet (`droplets.rs`, 220–480 u/s plus a slow downward gout) that falls
under 800 u/s² gravity, drawn as a camera-facing streak along its velocity.
Each frame's motion is swept with a world trace; the mark is made where it
lands (sky discards it). Droplets expire after 2.5 s, cap at 2048.
A 4×4 atlas contains 16 seeded irregular pools with satellite droplets; each
mark randomizes its tile, rotation and aspect ratio while keeping the dark red.
BSP polygons are clipped to the projection. Every
vertex of a receiving face must lie on the hit plane (within 0.5 units), so
adjacent walls/bevels cannot extrude a decal into flickering bands. Marks
on unrotated inline brush models follow their entity. Sky/liquid faces and
rotated brush entities receive no marks. Mesh patches store triangle indices
and barycentric coordinates, so they follow frame/group interpolation and
packed ragdoll vertices. Debris retains edict render keys across takeover.
Moving server gibs and cosmetic debris gibs paint contact surfaces at intervals;
a sleeping gib stops painting. Marks fade before expiry, recycle oldest first
at the cap, and clear on map changes/time rewind/disable; they are not saved.
Server flesh→gib/removal transitions near recent explosions and fresh gibs,
and explicit cosmetic dismemberments, launch 48 + 12×amount droplets at
300–720 u/s, stopped by world collision; plain explosions never bleed.
`blood_test [distance=64] [burst]` queues a diagnostic hit/burst; e.g.
`+r_blood 1 +map e1m1 "+delay 2 blood_test"` for a headless screenshot.


