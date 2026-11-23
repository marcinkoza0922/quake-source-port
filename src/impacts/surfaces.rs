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
                let (center, radius) = mesh::bounds(points.iter().map(|pt| pt.pos));
                self.marks.push_back(Mark {
                    kind,
                    born: time,
                    entity: tr.entity,
                    normal: tr.normal,
                    center,
                    radius,
                    points,
                });
            }
        });
    }
}

/// BSP has no material metadata. Use conventional texture names; unknown
/// materials deliberately behave as hard surfaces.
pub(crate) fn wood(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["wood", "crate", "plank", "timber", "tree", "trunk"]
        .iter()
        .any(|s| n.contains(s))
        || n.starts_with("door03")
        || n.starts_with("door04")
}

pub(super) fn wooden(world: &BspWorld<'_>, tr: &qpart::Trace) -> bool {
    texture_at(world, tr).is_some_and(|t| wood(&t.name))
}

/// Check whether point `p` with hit normal `n` lies on face `f`, returning its miptex.
fn face_texture<'b>(world: &BspWorld<'b>, f: &qformats::bsp::Face, p: Vec3, n: Vec3) -> Option<&'b qformats::bsp::MipTex> {
    let pl = world.bsp.planes.get(f.plane as usize)?;
    let pn = Vec3::from(pl.normal);
    let fn_ = if f.side != 0 { -pn } else { pn };
    if fn_.dot(n) < 0.5 || (p.dot(pn) - pl.dist).abs() > 0.15 {
        return None;
    }
    let count = f.num_edges as usize;
    if count < 3 {
        return None;
    }
    // Stack buffer avoids a per-face Vec allocation for almost all Quake polygons.
    let mut stack = [Vec3::ZERO; 64];
    let mut heap = Vec::new();
    let poly: &[Vec3] = if count <= stack.len() {
        for (dst, i) in stack[..count].iter_mut().zip(f.first_edge..f.first_edge + f.num_edges) {
            let se = world.bsp.surfedges[i as usize];
            let edge = world.bsp.edges[se.unsigned_abs() as usize];
            *dst = Vec3::from(world.bsp.vertexes[edge[usize::from(se < 0)] as usize]);
        }
        &stack[..count]
    } else {
        heap.reserve(count);
        for i in f.first_edge..f.first_edge + f.num_edges {
            let se = world.bsp.surfedges[i as usize];
            let edge = world.bsp.edges[se.unsigned_abs() as usize];
            heap.push(Vec3::from(world.bsp.vertexes[edge[usize::from(se < 0)] as usize]));
        }
        &heap
    };
    if !(1..poly.len() - 1).any(|i| mesh::ray(p + n, p - n, [poly[0], poly[i], poly[i + 1]]).is_some()) {
        return None;
    }
    world
        .bsp
        .texinfo
        .get(f.texinfo as usize)
        .and_then(|t| world.bsp.textures.get(t.miptex.max(0) as usize))
        .and_then(|t| t.as_ref())
}

/// The miptex of the face `tr` hit, on the world or an inline brush entity.
/// Walks the model's BSP tree so lookups are O(depth) instead of scanning every face.
pub(crate) fn texture_at<'b>(world: &BspWorld<'b>, tr: &qpart::Trace) -> Option<&'b qformats::bsp::MipTex> {
    let (sub, p, n) = if tr.entity == 0 {
        (0, tr.endpos, tr.normal)
    } else {
        let e = world.ents.iter().find(|e| e.entity == tr.entity)?;
        let p = e.local(tr.endpos);
        let n = e.local(tr.endpos + tr.normal) - p;
        (e.submodel, p, n)
    };
    let model = world.bsp.models.get(sub)?;
    let head = model.headnode[0];
    if head >= 0 && (head as usize) < world.bsp.nodes.len() {
        const EPS: f32 = 0.15;
        let mut stack = [0i32; 64];
        let mut top = 0usize;
        stack[0] = head;
        top += 1;
        while top > 0 {
            top -= 1;
            let ni = stack[top];
            if ni < 0 {
                continue;
            }
            let Some(node) = world.bsp.nodes.get(ni as usize) else {
                continue;
            };
            let Some(pl) = world.bsp.planes.get(node.plane as usize) else {
                continue;
            };
            let dist = p.dot(Vec3::from(pl.normal)) - pl.dist;
            if dist.abs() <= EPS {
                let first = node.first_face as usize;
                let end = (first + node.num_faces as usize).min(world.bsp.faces.len());
                for f in &world.bsp.faces[first..end] {
                    if let Some(tex) = face_texture(world, f, p, n) {
                        return Some(tex);
                    }
                }
            }
            if dist >= -EPS && node.children[0] >= 0 && top < stack.len() {
                stack[top] = node.children[0];
                top += 1;
            }
            if dist <= EPS && node.children[1] >= 0 && top < stack.len() {
                stack[top] = node.children[1];
                top += 1;
            }
        }
        return None;
    }
    for f in
        &world.bsp.faces[model.first_face as usize..(model.first_face + model.num_faces) as usize]
    {
        if let Some(tex) = face_texture(world, f, p, n) {
            return Some(tex);
        }
    }
    None
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
