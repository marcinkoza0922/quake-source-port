//! Reconstruct the solid/non-solid boundary of a compiled collision hull.
use qcore::math::Vec3;
use qformats::bsp::{Bsp, CONTENTS_CLIP, CONTENTS_SOLID, ClipNode};
use qworld::hull::HULL_MAXS;

type Plane = (Vec3, f32);
const EPS: f32 = 0.002;

fn bounds(mins: Vec3, maxs: Vec3) -> Vec<Plane> {
    let mut planes = Vec::with_capacity(6);
    for axis in 0..3 {
        let mut normal = Vec3::ZERO;
        normal[axis] = 1.0;
        planes.extend([(normal, maxs[axis]), (-normal, -mins[axis])]);
    }
    planes
}

fn clip(poly: &[Vec3], (normal, dist): Plane) -> Vec<Vec3> {
    let mut out = Vec::new();
    let Some(&last) = poly.last() else { return out };
    let mut previous = last;
    let mut prev_dist = normal.dot(previous) - dist;
    for &current in poly {
        let cur_dist = normal.dot(current) - dist;
        if (prev_dist < -EPS && cur_dist > EPS) || (prev_dist > EPS && cur_dist < -EPS) {
            out.push(previous.lerp(current, prev_dist / (prev_dist - cur_dist)));
        }
        if cur_dist <= EPS {
            out.push(current);
        }
        previous = current;
        prev_dist = cur_dist;
    }
    out
}

fn winding((normal, dist): Plane, center: Vec3, radius: f32, constraints: &[Plane]) -> Vec<Vec3> {
    let length = normal.length();
    if length < EPS || !length.is_finite() || !dist.is_finite() {
        return Vec::new();
    }
    let normal = normal / length;
    let p = center + normal * (dist / length - normal.dot(center));
    let axis = if normal.z.abs() < 0.9 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = normal.cross(axis).normalize() * radius;
    let v = normal.cross(u);
    let mut poly = vec![p + u + v, p - u + v, p - u - v, p + u - v];
    for &plane in constraints {
        poly = clip(&poly, plane);
        if poly.len() < 3 {
            break;
        }
    }
    poly
}

struct Hull<'a> {
    nodes: &'a [ClipNode],
    planes: Vec<Plane>,
    center: Vec3,
    radius: f32,
    polygons: Vec<Vec<Vec3>>,
    budget: usize,
}

impl Hull<'_> {
    fn node(&self, id: i32) -> Option<(Plane, [i32; 2])> {
        let node = self.nodes.get(id as usize)?;
        Some((*self.planes.get(node.plane as usize)?, node.children))
    }

    fn visit(&mut self, id: i32, constraints: &mut Vec<Plane>) {
        if self.budget == 0 || constraints.len() >= 128 {
            return;
        }
        let Some(((n, d), children)) = self.node(id) else {
            return;
        };
        self.budget -= 1;
        let poly = winding((n, d), self.center, self.radius, constraints);
        if poly.len() >= 3 {
            self.separate(poly, children, n, 0);
        }
        for (child, constraint) in children.into_iter().zip([(-n, -d), (n, d)]) {
            constraints.push(constraint);
            self.visit(child, constraints);
            constraints.pop();
        }
    }

    /// Clip the same candidate winding by both of the adjacent BSP trees.
    /// This discards internal partitions whose sides have equal solidity.
    fn separate(&mut self, poly: Vec<Vec3>, mut sides: [i32; 2], normal: Vec3, depth: usize) {
        if poly.len() < 3 || self.budget == 0 || depth >= 128 {
            return;
        }
        self.budget -= 1;
        let side = usize::from(sides[0] < 0);
        let Some(((n, d), children)) = self.node(sides[side]) else {
            if sides.iter().all(|&i| i < 0)
                && (sides[0] == CONTENTS_SOLID) != (sides[1] == CONTENTS_SOLID)
            {
                self.polygons.push(poly);
            }
            return;
        };
        let mut distances = poly.iter().map(|&p| n.dot(p) - d);
        let Some(first) = distances.next() else {
            return;
        };
        let (lo, hi) = distances.fold((first, first), |(lo, hi), d| (lo.min(d), hi.max(d)));
        if lo >= -EPS && hi <= EPS {
            let direction = normal.dot(n) * if side == 0 { 1.0 } else { -1.0 };
            sides[side] = children[usize::from(direction < 0.0)];
            self.separate(poly, sides, normal, depth + 1);
        } else if lo >= -EPS || hi <= EPS {
            sides[side] = children[usize::from(hi <= EPS)];
            self.separate(poly, sides, normal, depth + 1);
        } else {
            sides[side] = children[0];
            self.separate(clip(&poly, (-n, -d)), sides, normal, depth + 1);
            sides[side] = children[1];
            self.separate(clip(&poly, (n, d)), sides, normal, depth + 1);
        }
    }
}

pub(super) fn polygons(bsp: &Bsp, size: usize) -> Vec<Vec<Vec3>> {
    let Some(model) = bsp.models.first() else {
        return Vec::new();
    };
    let padding = HULL_MAXS[size] + Vec3::splat(16.0);
    let mins = Vec3::from(model.mins) - padding;
    let maxs = Vec3::from(model.maxs) + padding;
    let mut hull = Hull {
        nodes: &bsp.clipnodes,
        planes: bsp
            .planes
            .iter()
            .map(|p| (p.normal.into(), p.dist))
            .collect(),
        center: (mins + maxs) * 0.5,
        radius: (maxs - mins).length().max(1.0),
        polygons: Vec::new(),
        budget: 1_000_000,
    };
    hull.visit(model.headnode[size], &mut bounds(mins, maxs));
    if let Some(brushes) = bsp
        .brushlist
        .as_ref()
        .and_then(|m| m.iter().find(|b| b.model_num == 0))
    {
        for brush in brushes
            .brushes
            .iter()
            .filter(|b| i32::from(b.contents) == CONTENTS_CLIP)
        {
            let (mins, maxs) = (Vec3::from(brush.mins), Vec3::from(brush.maxs));
            let center = (mins + maxs) * 0.5;
            let radius = (maxs - mins).length().max(1.0);
            let mut planes = bounds(mins, maxs);
            planes.extend(brush.planes.iter().map(|p| (p.normal.into(), p.dist)));
            hull.polygons.extend(planes.iter().filter_map(|&p| {
                let poly = winding(p, center, radius, &planes);
                (poly.len() >= 3).then_some(poly)
            }));
        }
    }
    hull.polygons
}

#[cfg(test)]
mod tests {
    use super::*;
    use qformats::bsp::{CONTENTS_EMPTY, Model, Plane};

    #[test]
    fn hull_boundary_discards_partitions_between_empty_cells() {
        let bsp = Bsp {
            planes: vec![
                Plane {
                    normal: [1.0, 0.0, 0.0],
                    dist: 0.0,
                    kind: 0,
                },
                Plane {
                    normal: [0.0, 1.0, 0.0],
                    dist: 0.0,
                    kind: 1,
                },
            ],
            clipnodes: vec![
                ClipNode {
                    plane: 0,
                    children: [1, CONTENTS_EMPTY],
                },
                ClipNode {
                    plane: 1,
                    children: [CONTENTS_SOLID, CONTENTS_EMPTY],
                },
            ],
            models: vec![Model {
                mins: [-64.0; 3],
                maxs: [64.0; 3],
                origin: [0.0; 3],
                headnode: [0; 4],
                visleafs: 0,
                first_face: 0,
                num_faces: 0,
            }],
            ..Default::default()
        };
        let result = polygons(&bsp, 1);
        assert_eq!(result.len(), 2);
        assert!(result[0].iter().all(|p| p.x.abs() <= EPS && p.y >= -EPS));
        assert!(result[1].iter().all(|p| p.y.abs() <= EPS && p.x >= -EPS));
    }
}
