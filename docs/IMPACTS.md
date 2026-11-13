# Optional weapon impacts

`src/impacts.rs` owns client-only marks and nails, independently of enhanced
blood, scripted particles and the debris master switch. Options → Enhancements → Impact Effects exposes
archived `r_impact_bullets`, `r_impact_nails` and `r_impact_scorches` (all default
0), plus `r_impact_lifetime` (120 seconds) and `r_impact_limit` (512). Marks
and nails share the cap, recycle oldest first and fade over their last five
seconds. Map changes, time rewind and disabling all effects clear state.

The client retains a separate typed-by-kind queue (`weapon_impacts`, 0 bullet,
1 nail, 2 explosion) until rendering completes. It is populated before FTE
particle replacement, so particle packs cannot swallow these effects. Existing
water/debris/blood impact consumers retain their independent queue. Wizard and
knight projectiles do not produce bullet holes or physical nails.

`surfaces.rs` clips marks against coplanar BSP polygons using the blood mesh
projection helper. Sky and liquids are excluded. Marks and embedded nails on
unrotated inline brush models follow their origins; rotated brushes are skipped.
Shotgun hits use small dark marks; explosions project larger scorches in six
axial directions within 96 units. A generated RGBA atlas with chipped bullet craters, diffuse scorches and white nail shading needs no
external assets. Geometry uses the existing depth-tested FTE alpha pass.

`nails.rs` tests flight segments against animated MDL triangles and packed
ragdoll poses. Mesh attachments store owner key/model, triangle, barycentric
coordinates and side, following animation and ragdoll takeover. Standard nail
models (`progs/spike.mdl`, `progs/s_spike.mdl`) are tracked between snapshots;
disappearance checks find flesh hits that emit blood instead of TE_SPIKE.
Crossing a cosmetic ragdoll can embed the cosmetic nail while the authoritative
projectile continues; its client model is hidden until it disappears. Effects
never change server collision, damage, projectile lifetime or pickups.

BSP has no material metadata. Wood is inferred from case-insensitive texture
names containing wood, crate, plank, timber, tree or trunk, plus conventional
wooden door03/door04 names. Unknown names use hard-surface behavior. This
heuristic cannot identify arbitrary custom wood textures. Impact flight direction
for position-only temp entities uses the existing debris shooter inference.
Loose nails use bounded 8 ms collision substeps, gravity, damped bounce and
sleep on floors. Sleeping loose nails retain a supporting brush entity and local
offset, follow its translation in every direction, and wake/fall if it is
removed or no longer supports them. Rotated supports keep nails awake.
An opaque shaded square shaft and tapered tip form their mesh.
No gameplay entity or physics interaction is created.

`impact_test bullet|nail|explosion` traces the crosshair into the world and
queues a real cosmetic impact; enable the matching option first. Example:
`cargo run --release -- -headless -autoquit 6 +r_impact_scorches 1 +map e1m1
"+delay 2 impact_test explosion" "+delay 3 screenshot target/shots/impacts"`.
Tests cover material classification, falling/bouncing/sleeping, deformed mesh
attachment, model replacement, moving-brush render offsets, and sleeping nails
riding a lift up/down/sideways and falling when it disappears.
