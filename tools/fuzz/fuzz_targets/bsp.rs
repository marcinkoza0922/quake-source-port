//! Hostile map: parse, then the collision and visibility walks a server does.
#![no_main]
use libfuzzer_sys::fuzz_target;
use qworld::{CollisionModel, Vec3};

fuzz_target!(|data: &[u8]| {
    let Ok(b) = qformats::Bsp::parse(data) else { return };
    let _ = qformats::parse_entities(&b.entities);
    for l in 0..b.leafs.len().min(8) {
        let _ = b.leaf_pvs(l);
    }
    if let Some(g) = &b.lightgrid {
        for p in [[0.0; 3], [64.0, -64.0, 32.0], [f32::NAN, 1e30, -1e30]] {
            let _ = g.sample(p, &[256; 64]);
        }
    }
    if let Some(n) = &b.face_normals {
        for f in 0..b.faces.len().min(64) {
            for e in 0..8 {
                let _ = n.vertex_normal(f, e);
            }
        }
    }
    let m = CollisionModel::from_bsp(&b);
    let (lo, hi) = (Vec3::splat(-512.0), Vec3::splat(512.0));
    for s in 0..b.models.len().min(4) {
        for h in 0..3 {
            let hull = m.hull(s, h);
            let _ = hull.trace(lo, hi);
            let _ = hull.trace(Vec3::new(0.0, 0.0, 64.0), Vec3::new(0.0, 0.0, -512.0));
            let _ = hull.point_contents(hull.first, Vec3::ZERO);
        }
    }
    let _ = m.point_contents(Vec3::ZERO);
    let mut out = Vec::new();
    m.touched_leafs(lo, hi, &mut out, 64);
    let _ = m.box_in_pvs(lo, hi, &b.leaf_pvs(1));
});
