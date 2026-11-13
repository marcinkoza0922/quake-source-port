use super::*;

impl Impacts {
    pub(super) fn nail(
        &mut self,
        world: &BspWorld<'_>,
        poses: &Poses<'_>,
        a: Vec3,
        b: Vec3,
        time: f64,
        hard: bool,
    ) -> bool {
        let tr = world.trace_line(a, b);
        let dir = (b - a).normalize();
        let mut nearest = None;
        for (&key, (e, verts)) in poses {
            let Some(m) = self.models.get(&e.model) else {
                continue;
            };
            for t in &m.triangles {
                let tri = t.verts.map(|i| i as usize);
                let v = tri.map(|i| verts[i]);
                if let Some((f, n)) = mesh::ray(a, b, v)
                    && f < tr.frac
                    && nearest
                        .as_ref()
                        .is_none_or(|(best, _, _, _, _, _)| f < *best)
                {
                    let p = a.lerp(b, f);
                    let bary = barycentric(p, v);
                    nearest = Some((
                        f,
                        key,
                        e.model,
                        tri,
                        bary,
                        if n.dot(dir) > 0.0 { -1.0 } else { 1.0 },
                    ));
                }
            }
        }
        if let Some((f, key, model, tri, bary, side)) = nearest {
            self.nails.push_back(Nail {
                born: time,
                pos: a.lerp(b, f),
                dir,
                vel: Vec3::ZERO,
                anchor: Some(Anchor::Mesh {
                    key,
                    model,
                    tri,
                    bary,
                    side,
                }),
                sleeping: true,
                support: None,
            });
            return true;
        } else if hard && tr.frac > 0.0 && tr.frac < 1.0 {
            // Sky and liquid faces return no decal polygons.
            let origin = if tr.entity == 0 {
                Some(Vec3::ZERO)
            } else {
                world.entity_origin(tr.entity)
            };
            let Some(origin) = origin else { return false };
            let mut receives = false;
            world.decal_polys(
                tr.entity,
                tr.endpos - origin,
                1.0,
                tr.normal,
                false,
                &mut |poly| {
                    if poly
                        .iter()
                        .all(|p| (*p - (tr.endpos - origin)).dot(tr.normal).abs() < 0.5)
                    {
                        receives = true;
                    }
                },
            );
            if !receives {
                return false;
            }
            let stuck = surfaces::wooden(world, &tr);
            let reflected = dir - tr.normal * (2.0 * dir.dot(tr.normal));
            self.nails.push_back(Nail {
                born: time,
                pos: tr.endpos + tr.normal * 0.4 - origin * if stuck { 1.0 } else { 0.0 },
                dir,
                vel: if stuck {
                    Vec3::ZERO
                } else {
                    reflected * 100.0 + tr.normal * 25.0
                },
                anchor: stuck.then_some(Anchor::Brush(tr.entity)),
                sleeping: stuck,
                support: None,
            });
        }
        false
    }

    pub(super) fn step_nails(
        &mut self,
        world: &dyn World,
        poses: &Poses<'_>,
        dt: f32,
        gravity: f32,
    ) {
        self.nails.retain_mut(|n| {
            match &n.anchor {
                Some(Anchor::Mesh {
                    key,
                    model,
                    tri,
                    bary,
                    side,
                }) => {
                    let Some((e, verts)) = poses.get(key) else {
                        return true;
                    };
                    if e.model != *model || tri.iter().any(|&i| i >= verts.len()) {
                        return false;
                    }
                    let v = tri.map(|i| verts[i]);
                    n.pos = v[0] * bary.x + v[1] * bary.y + v[2] * bary.z;
                    // Tip penetrates the skin; shaft follows the deformed triangle.
                    n.dir = -(v[1] - v[0]).cross(v[2] - v[0]).normalize() * *side;
                    return true;
                }
                Some(Anchor::Brush(entity)) => {
                    return *entity == 0 || world.entity_origin(*entity).is_some();
                }
                None => {}
            }
            if n.sleeping {
                if let Some((entity, offset)) = n.support {
                    if let Some(origin) = world.entity_origin(entity) {
                        n.pos = origin + offset;
                    } else {
                        n.sleeping = false;
                    }
                }
                // Recheck support after carrying: a removed door/lift must
                // release the nail instead of leaving it suspended in air.
                let down = world.trace_line(n.pos, n.pos - Vec3::new(0.0, 0.0, 1.0));
                if n.sleeping
                    && down.frac < 1.0
                    && down.normal.z > 0.7
                    && n.support.is_none_or(|(entity, _)| down.entity == entity)
                {
                    return true;
                }
                n.sleeping = false;
                n.support = None;
            }
            if dt <= 0.0 {
                return true;
            }
            let steps = (dt / 0.008).ceil().max(1.0) as usize;
            let h = dt / steps as f32;
            for _ in 0..steps {
                n.vel.z -= gravity * h;
                let tr = world.trace_line(n.pos, n.pos + n.vel * h);
                n.pos = tr.endpos;
                if tr.frac < 1.0 {
                    n.pos += tr.normal * 0.4;
                    let vn = n.vel.dot(tr.normal);
                    if vn < 0.0 {
                        n.vel = (n.vel - tr.normal * vn) * 0.65 - tr.normal * vn * 0.3;
                    }
                    if tr.normal.z > 0.7 && n.vel.length() < 25.0 {
                        // Keep sleeping nails in the supporting mover's local
                        // space, just like embedded nails and surface marks.
                        n.support = (tr.entity != 0)
                            .then(|| world.entity_origin(tr.entity))
                            .flatten()
                            .map(|origin| (tr.entity, n.pos - origin));
                        n.sleeping = tr.entity == 0 || n.support.is_some();
                        n.vel = Vec3::ZERO;
                        let flat = Vec3::new(n.dir.x, n.dir.y, 0.0);
                        n.dir = if flat.length_sq() > 0.001 {
                            flat.normalize()
                        } else {
                            Vec3::new(1.0, 0.0, 0.0)
                        };
                        break;
                    }
                } else {
                    // Cosmetic tumble, deterministic and independent of gameplay RNG.
                    n.dir = (n.dir + Vec3::new(n.dir.z, 0.3, -n.dir.x) * h * 9.0).normalize();
                }
            }
            true
        });
    }
}
fn barycentric(p: Vec3, v: [Vec3; 3]) -> Vec3 {
    let a = v[1] - v[0];
    let b = v[2] - v[0];
    let c = p - v[0];
    let d = a.dot(a) * b.dot(b) - a.dot(b) * a.dot(b);
    let y = (b.dot(b) * c.dot(a) - a.dot(b) * c.dot(b)) / d;
    let z = (a.dot(a) * c.dot(b) - a.dot(b) * c.dot(a)) / d;
    Vec3::new(1.0 - y - z, y, z)
}
