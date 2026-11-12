//! Automatic ragdoll rigs for alias models. MDLs are vertex-animated with
//! no skeleton, so a rig is derived from the pose the corpse lies in:
//! vertices are clustered into a handful of "bones" (k-means seeded by
//! farthest-point sampling, so limbs and heads get their own cluster),
//! clusters that share a triangle are linked, and each vertex is skinned to
//! its nearest cluster blended with the closest linked neighbour. This works
//! for any model, including mod monsters.
use qcore::math::Vec3;
use qformats::mdl::Mdl;

use super::math::anorm;

/// A cluster link: (a, b, rest length, kind).
#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub a: u8,
    pub b: u8,
    pub rest: f32,
    /// Hard links keep their length; soft ones only stop folding below half.
    pub hard: bool,
}

#[derive(Debug)]
pub struct Rig {
    /// Model-space rest positions and normals of every vertex.
    pub rest: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    /// Two bones per vertex and the first one's weight.
    pub bones: Vec<[u8; 2]>,
    pub weight: Vec<f32>,
    /// Cluster rest centroids (model space) and collision radii.
    pub centers: Vec<Vec3>,
    pub radius: Vec<f32>,
    pub links: Vec<Link>,
    /// Hard-linked neighbours per cluster (for shape matching).
    pub neighbors: Vec<Vec<u8>>,
}

const MAX_BONES: usize = 12;
/// Distance over which a vertex blends between two bones.
const BLEND: f32 = 5.0;

impl Rig {
    pub fn build(mdl: &Mdl, frame: usize) -> Option<Rig> {
        let pose = mdl.frames.get(frame)?.poses.first()?;
        let n = pose.verts.len();
        if n < 4 || mdl.triangles.is_empty() {
            return None;
        }
        let rest: Vec<Vec3> = pose.verts.iter().map(|v| Vec3::from(mdl.decode(*v))).collect();
        let normals = pose.verts.iter().map(|v| anorm(v.normal)).collect();
        let k = (n / 20).clamp(5, MAX_BONES).min(n);
        let (centers, assign) = kmeans(&rest, k);
        let k = centers.len();
        let tris: Vec<[usize; 3]> = mdl
            .triangles
            .iter()
            .filter_map(|t| {
                let v = t.verts.map(|i| i as usize);
                v.iter().all(|&i| i < n).then_some(v)
            })
            .collect();
        let mut adj = vec![vec![false; k]; k];
        for t in &tris {
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let (a, b) = (assign[t[i]], assign[t[j]]);
                if a != b {
                    adj[a][b] = true;
                    adj[b][a] = true;
                }
            }
        }
        connect_components(&centers, &mut adj);
        let mut links = Vec::new();
        for a in 0..k {
            for b in a + 1..k {
                if adj[a][b] {
                    links.push(Link { a: a as u8, b: b as u8, rest: (centers[a] - centers[b]).length(), hard: true });
                }
            }
        }
        // two-ring links stop limbs folding through the body
        for a in 0..k {
            for b in a + 1..k {
                if adj[a][b] {
                    continue;
                }
                if (0..k).any(|m| adj[a][m] && adj[m][b]) {
                    links.push(Link { a: a as u8, b: b as u8, rest: (centers[a] - centers[b]).length(), hard: false });
                }
            }
        }
        let neighbors: Vec<Vec<u8>> = (0..k).map(|a| (0..k).filter(|&b| adj[a][b]).map(|b| b as u8).collect()).collect();
        let mut bones = Vec::with_capacity(n);
        let mut weight = Vec::with_capacity(n);
        for (i, p) in rest.iter().enumerate() {
            let b0 = assign[i];
            let d0 = (*p - centers[b0]).length();
            let best = neighbors[b0]
                .iter()
                .map(|&b| (b as usize, (*p - centers[b as usize]).length()))
                .min_by(|x, y| x.1.total_cmp(&y.1));
            match best {
                Some((b1, d1)) => {
                    bones.push([b0 as u8, b1 as u8]);
                    weight.push((0.5 + (d1 - d0) / (2.0 * BLEND)).clamp(0.5, 1.0));
                }
                None => {
                    bones.push([b0 as u8, b0 as u8]);
                    weight.push(1.0);
                }
            }
        }
        let mut radius = vec![0.0f32; k];
        let mut count = vec![0u32; k];
        for (i, p) in rest.iter().enumerate() {
            radius[assign[i]] += (*p - centers[assign[i]]).length_sq();
            count[assign[i]] += 1;
        }
        for (r, c) in radius.iter_mut().zip(&count) {
            *r = ((*r / (*c).max(1) as f32).sqrt() * 0.6).clamp(2.0, 10.0);
        }
        Some(Rig { rest, normals, bones, weight, centers, radius, links, neighbors })
    }
}

/// Lloyd's k-means with farthest-point seeding; returns (centroids, assignment).
fn kmeans(pts: &[Vec3], k: usize) -> (Vec<Vec3>, Vec<usize>) {
    let mid = pts.iter().fold(Vec3::ZERO, |s, p| s + *p) / pts.len() as f32;
    let far = |from: Vec3| (0..pts.len()).max_by(|&a, &b| (pts[a] - from).length_sq().total_cmp(&(pts[b] - from).length_sq()));
    let mut centers = vec![pts[far(mid).unwrap_or(0)]];
    let mut dmin: Vec<f32> = pts.iter().map(|p| (*p - centers[0]).length_sq()).collect();
    while centers.len() < k {
        let Some(i) = (0..pts.len()).max_by(|&a, &b| dmin[a].total_cmp(&dmin[b])) else { break };
        if dmin[i] <= 1e-6 {
            break;
        }
        centers.push(pts[i]);
        for (d, p) in dmin.iter_mut().zip(pts) {
            *d = d.min((*p - pts[i]).length_sq());
        }
    }
    let mut assign = vec![0usize; pts.len()];
    for _ in 0..8 {
        for (a, p) in assign.iter_mut().zip(pts) {
            *a = nearest(&centers, *p);
        }
        let mut sum = vec![Vec3::ZERO; centers.len()];
        let mut cnt = vec![0u32; centers.len()];
        for (a, p) in assign.iter().zip(pts) {
            sum[*a] += *p;
            cnt[*a] += 1;
        }
        for (c, (s, n)) in centers.iter_mut().zip(sum.iter().zip(&cnt)) {
            if *n > 0 {
                *c = *s / *n as f32;
            }
        }
    }
    // drop empty clusters and renumber
    let mut cnt = vec![0u32; centers.len()];
    for (a, p) in assign.iter_mut().zip(pts) {
        *a = nearest(&centers, *p);
        cnt[*a] += 1;
    }
    let remap: Vec<Option<usize>> = {
        let mut next = 0;
        cnt.iter()
            .map(|&c| {
                (c > 0).then(|| {
                    next += 1;
                    next - 1
                })
            })
            .collect()
    };
    let centers: Vec<Vec3> = centers.iter().zip(&cnt).filter(|(_, c)| **c > 0).map(|(p, _)| *p).collect();
    for a in &mut assign {
        *a = remap[*a].unwrap_or(0);
    }
    (centers, assign)
}

fn nearest(centers: &[Vec3], p: Vec3) -> usize {
    let mut best = (f32::MAX, 0);
    for (i, c) in centers.iter().enumerate() {
        let d = (p - *c).length_sq();
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// Links the closest clusters of separate components (detached mesh parts).
fn connect_components(centers: &[Vec3], adj: &mut [Vec<bool>]) {
    let k = centers.len();
    loop {
        let mut comp = vec![usize::MAX; k];
        let mut stack = vec![0];
        comp[0] = 0;
        while let Some(a) = stack.pop() {
            for b in 0..k {
                if adj[a][b] && comp[b] == usize::MAX {
                    comp[b] = 0;
                    stack.push(b);
                }
            }
        }
        let mut best = (f32::MAX, 0, 0);
        for a in (0..k).filter(|&a| comp[a] == 0) {
            for b in (0..k).filter(|&b| comp[b] != 0) {
                let d = (centers[a] - centers[b]).length_sq();
                if d < best.0 {
                    best = (d, a, b);
                }
            }
        }
        if best.0 == f32::MAX {
            return;
        }
        adj[best.1][best.2] = true;
        adj[best.2][best.1] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmeans_separates_clumps() {
        let mut pts = Vec::new();
        for c in [Vec3::new(0.0, 0.0, 0.0), Vec3::new(50.0, 0.0, 0.0), Vec3::new(0.0, 50.0, 0.0)] {
            for i in 0..10 {
                pts.push(c + Vec3::splat(i as f32 * 0.1));
            }
        }
        let (centers, assign) = kmeans(&pts, 3);
        assert_eq!(centers.len(), 3);
        assert!(assign[..10].iter().all(|&a| a == assign[0]));
        assert_ne!(assign[0], assign[10]);
        assert_ne!(assign[10], assign[20]);
    }

    #[test]
    fn components_get_connected() {
        let centers = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0)];
        let mut adj = vec![vec![false; 3]; 3];
        connect_components(&centers, &mut adj);
        assert!(adj[1][2] && adj[0][1]);
    }
}
