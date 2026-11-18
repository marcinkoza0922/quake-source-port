//! The player model's weapon rig, independent of the model format. The gun
//! is a rigid piece of the player model (see [`super::classic`] and
//! [`super::enhanced`] for how each format gives it away). Fitting its rigid
//! transform in every pose against the `shotatt1` pose, where it points
//! straight along +x, gives the hand's transform for every frame, recoil and
//! the gun falling in death frames included. A weapon model placed at the
//! gun in `shotatt1` and moved by those transforms becomes a model with the
//! player's frames, so it animates and lerps exactly like the body.

use qcore::math::Vec3;

use crate::debris::math::{Mat3, Quat, add_outer, apply, extract_rotation};

/// The pose the weapon models are placed in.
pub const REFERENCE: &str = "shotatt1";

/// How a weapon model is fitted into the hand.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    /// The model whose first pose is used (a pickup or view model).
    pub source: String,
    pub scale: f32,
    /// Added after centring the model on the player's gun (player-model units).
    pub offset: [f32; 3],
    /// Keep only the pieces in front of the eye (view models carry parts
    /// behind the camera).
    pub front_only: bool,
}

pub struct RigPose {
    pub name: String,
    /// The gun's transform from the reference pose.
    r: Mat3,
    t: Vec3,
}

pub struct RigFrame {
    pub intervals: Vec<f32>,
    pub poses: Vec<RigPose>,
}

/// One frame of the player model for [`Rig::fit`]: group intervals and,
/// per pose, its name and the gun piece's vertex positions.
pub type GunFrame = (Vec<f32>, Vec<(String, Vec<Vec3>)>);

pub struct Rig {
    pub frames: Vec<RigFrame>,
    /// Per frame: an axe frame (`ax*`), a death frame (`death*`, `axdeth*`).
    axe_frame: Vec<bool>,
    death_frame: Vec<bool>,
    /// The gun's bounding-box centre in the reference pose.
    grip: Vec3,
    /// Largest fit error over the gun-frame poses (test diagnostics; in
    /// axe frames id's slung gun is also shrunk to about 0.83).
    #[cfg_attr(not(test), allow(dead_code))]
    pub residual: f32,
}

pub fn centroid(p: &[Vec3]) -> Vec3 {
    p.iter().fold(Vec3::ZERO, |s, &v| s + v) / p.len().max(1) as f32
}

pub fn bbox(p: impl IntoIterator<Item = Vec3>) -> (Vec3, Vec3) {
    p.into_iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

/// Best rigid transform with `b ≈ R a + t` (shape matching), warm-started
/// from `q`.
fn fit(a: &[Vec3], ca: Vec3, b: &[Vec3], q: Quat) -> (Quat, Vec3) {
    let cb = centroid(b);
    let mut m: Mat3 = [[0.0; 3]; 3];
    for (&pa, &pb) in a.iter().zip(b) {
        add_outer(&mut m, pb - cb, pa - ca, 1.0);
    }
    let q = extract_rotation(&m, q, 200);
    (q, cb - apply(&q.to_mat(), ca))
}

/// Vertex sets of the triangles' connected components, largest first.
pub fn components(num_verts: usize, tris: impl IntoIterator<Item = [usize; 3]>) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..num_verts).collect();
    fn root(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut used = vec![false; num_verts];
    for [a, b, c] in tris {
        let ra = root(&mut parent, a);
        for v in [b, c] {
            let rv = root(&mut parent, v);
            parent[rv] = ra;
        }
        used[a] = true;
        used[b] = true;
        used[c] = true;
    }
    let mut groups = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for v in (0..num_verts).filter(|&v| used[v]) {
        groups.entry(root(&mut parent, v)).or_default().push(v);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    out
}

impl Rig {
    /// The rig of `frames`; None without a [`REFERENCE`] pose.
    pub fn fit(frames: Vec<GunFrame>) -> Option<Rig> {
        let a = frames.iter().flat_map(|f| &f.1).find(|p| p.0.eq_ignore_ascii_case(REFERENCE))?.1.clone();
        let ca = centroid(&a);
        let (lo, hi) = bbox(a.iter().copied());
        let name = |f: &GunFrame| f.1.first().map_or(String::new(), |p| p.0.to_ascii_lowercase());
        let axe_frame: Vec<bool> = frames.iter().map(|f| name(f).starts_with("ax")).collect();
        let death_frame = frames.iter().map(|f| ["death", "axdeth"].iter().any(|d| name(f).starts_with(d))).collect();
        let mut q = Quat::IDENTITY;
        let mut residual = 0.0f32;
        let frames = frames
            .into_iter()
            .zip(&axe_frame)
            .map(|((intervals, poses), &axe)| RigFrame {
                intervals,
                poses: poses
                    .into_iter()
                    .map(|(name, b)| {
                        let (nq, t) = fit(&a, ca, &b, q);
                        q = nq;
                        let r = q.to_mat();
                        for (&pa, &pb) in a.iter().zip(&b).filter(|_| !axe) {
                            residual = residual.max((apply(&r, pa) + t - pb).length());
                        }
                        RigPose { name, r, t }
                    })
                    .collect(),
            })
            .collect();
        Some(Rig { frames, axe_frame, death_frame, grip: (lo + hi) * 0.5, residual })
    }

    pub fn axe_frame(&self, frame: u32) -> bool {
        self.axe_frame.get(frame as usize).copied().unwrap_or(false)
    }

    pub fn death_frame(&self, frame: u32) -> bool {
        self.death_frame.get(frame as usize).copied().unwrap_or(false)
    }

    /// A weapon's vertices (`pos`, with its triangles `tris`) centred on the
    /// gun in the reference pose per `spec`. Returns which vertices are kept
    /// and every vertex's placed position (dropped ones sit at the grip).
    pub fn place(&self, pos: &[Vec3], tris: impl IntoIterator<Item = [usize; 3]>, spec: &Spec) -> Option<(Vec<bool>, Vec<Vec3>)> {
        let mut keep = vec![false; pos.len()];
        for c in components(pos.len(), tris) {
            if !spec.front_only || c.iter().any(|&v| pos[v].x > 0.0) {
                c.iter().for_each(|&v| keep[v] = true);
            }
        }
        if !keep.contains(&true) {
            return None;
        }
        let (lo, hi) = bbox(pos.iter().zip(&keep).filter(|k| *k.1).map(|k| *k.0));
        let centre = (lo + hi) * 0.5;
        let offset = self.grip + Vec3::from_array(spec.offset);
        let placed = pos.iter().zip(&keep).map(|(&p, &k)| if k { (p - centre) * spec.scale + offset } else { self.grip }).collect();
        Some((keep, placed))
    }

    /// Placed positions and normals moved into every pose, frame by frame.
    pub fn posed(&self, placed: &[Vec3], normals: &[Vec3]) -> Vec<Vec<Vec<(Vec3, Vec3)>>> {
        self.frames
            .iter()
            .map(|f| {
                f.poses
                    .iter()
                    .map(|p| placed.iter().zip(normals).map(|(&v, &n)| (apply(&p.r, v) + p.t, apply(&p.r, n))).collect())
                    .collect()
            })
            .collect()
    }
}
