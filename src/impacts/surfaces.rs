use super::*;

impl Impacts {
    pub(super) fn surface(&mut self, world: &dyn World, a: Vec3, b: Vec3, kind: u8, time: f64) {
        let tr = world.trace_line(a, b);
        if tr.frac <= 0.0 || tr.frac >= 1.0 {
            return;
        }
        let origin = if tr.entity == 0 {
            Vec3::ZERO
        } else {
            let Some(o) = world.entity_origin(tr.entity) else {
                return;
            };
            o
        };
        let p = tr.endpos - origin;
        let size = if kind == 0 { 1.5 } else { 32.0 };
        world.decal_polys(tr.entity, p, size * 1.5, tr.normal, false, &mut |poly| {
            if poly.iter().any(|v| (*v - p).dot(tr.normal).abs() > 0.5) {
                return;
            }
            let points = mesh::project_shaped(poly, p, tr.normal, size, [time as f32 * 17.0, 1.0]);
            if points.len() >= 3 {
                self.marks.push_back(Mark {
                    kind,
                    born: time,
                    entity: tr.entity,
                    normal: tr.normal,
                    points,
                });
            }
        });
    }
}

/// BSP has no material metadata. Use conventional texture names; unknown
/// materials deliberately behave as hard surfaces.
pub(super) fn wood(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["wood", "crate", "plank", "timber", "tree", "trunk"]
        .iter()
        .any(|s| n.contains(s))
        || n.starts_with("door03")
        || n.starts_with("door04")
}

pub(super) fn wooden(world: &BspWorld<'_>, tr: &qpart::Trace) -> bool {
    let (sub, p, n) = if tr.entity == 0 {
        (0, tr.endpos, tr.normal)
    } else {
        let Some(e) = world.ents.iter().find(|e| e.entity == tr.entity) else {
            return false;
        };
        let p = e.local(tr.endpos);
        let n = e.local(tr.endpos + tr.normal) - p;
        (e.submodel, p, n)
    };
    let Some(model) = world.bsp.models.get(sub) else {
        return false;
    };
    for f in
        &world.bsp.faces[model.first_face as usize..(model.first_face + model.num_faces) as usize]
    {
        let Some(pl) = world.bsp.planes.get(f.plane as usize) else {
            continue;
        };
        if (p.dot(Vec3::from(pl.normal)) - pl.dist).abs() > 0.15 {
            continue;
        }
        let mut poly = Vec::new();
        for i in f.first_edge..f.first_edge + f.num_edges {
            let se = world.bsp.surfedges[i as usize];
            let edge = world.bsp.edges[se.unsigned_abs() as usize];
            poly.push(Vec3::from(
                world.bsp.vertexes[edge[usize::from(se < 0)] as usize],
            ));
        }
        if poly.len() < 3
            || !(1..poly.len() - 1)
                .any(|i| mesh::ray(p + n, p - n, [poly[0], poly[i], poly[i + 1]]).is_some())
        {
            continue;
        }
        return world
            .bsp
            .texinfo
            .get(f.texinfo as usize)
            .and_then(|t| world.bsp.textures.get(t.miptex.max(0) as usize))
            .and_then(|t| t.as_ref())
            .is_some_and(|t| wood(&t.name));
    }
    false
}

pub(super) fn texture() -> Vec<u8> {
    // Bullet crater, diffuse scorch, and solid white for nail geometry.
    let mut data = vec![255; 96 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let dx = (x as f32 - 15.5) / 15.5;
            let dy = (y as f32 - 15.5) / 15.5;
            let radius = (dx * dx + dy * dy).sqrt();
            let noise = ((x * 73 + y * 139 + x * y * 17) % 31) as f32 / 155.0;
            let bullet = (y * 96 + x) * 4;
            let shade = if radius < 0.5 {
                18
            } else if dy < -dx {
                155
            } else {
                75
            };
            data[bullet..bullet + 3].fill(shade);
            data[bullet + 3] = ((0.85 - radius + noise).clamp(0.0, 0.15) / 0.15 * 255.0) as u8;
            let scorch = (y * 96 + x + 32) * 4;
            data[scorch + 3] = ((1.0 - radius + noise).clamp(0.0, 0.35) / 0.35 * 255.0) as u8;
        }
    }
    data
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn materials() {
        for name in ["WOOD1_5", "crate0", "+0planks", "door03_2"] {
            assert!(wood(name));
        }
        for name in ["metal1", "stone", "sky1", "*water", "unknown"] {
            assert!(!wood(name));
        }
    }
}
