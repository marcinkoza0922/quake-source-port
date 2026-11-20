//! IQM skeletal pose evaluation, 3×4 affine matrix math, bounds, and tests.

use super::Iqm;
use crate::Vec3;

const IDENT34: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

impl Iqm {
    pub fn num_bones(&self) -> usize {
        self.joints.len().max(1)
    }

    pub fn num_frames_effective(&self) -> usize {
        if self.poses.is_empty() || self.num_frames == 0 || self.joints.is_empty() {
            1
        } else {
            self.num_frames as usize
        }
    }

    /// Computes per-frame 3x4 row-major skinning matrices (`M_frame * inv_bind`).
    /// Returned length is `num_frames_effective() * num_bones()`.
    pub fn compute_frame_matrices(&self) -> Vec<[f32; 12]> {
        let nbones = self.num_bones();
        let nframes = self.num_frames_effective();
        if self.joints.is_empty() || self.poses.is_empty() || self.num_frames == 0 {
            return vec![IDENT34; nframes * nbones];
        }
        let local_bind: Vec<[f32; 12]> = self
            .joints
            .iter()
            .map(|j| trs_matrix(j.translate, j.rotate, j.scale))
            .collect();
        let parents: Vec<i32> = self.joints.iter().map(|j| j.parent).collect();
        let world_bind = resolve_hierarchy(&local_bind, &parents);
        let inv_bind: Vec<[f32; 12]> = world_bind.iter().map(|m| invert34(*m)).collect();

        let pose_count = self.poses.len().min(nbones);
        let pose_parents: Vec<i32> = self.poses[..pose_count].iter().map(|p| p.parent).collect();
        let mut out = Vec::with_capacity(nframes * nbones);
        let mut cursor = 0usize;
        for _ in 0..nframes {
            let mut local_pose = vec![IDENT34; pose_count];
            for (pi, pose) in self.poses.iter().enumerate() {
                let mut ch = pose.channel_offset;
                for (bit, c) in ch.iter_mut().enumerate() {
                    if (pose.mask & (1 << bit)) != 0 {
                        let raw = self.frames.get(cursor).copied().unwrap_or(0) as f32;
                        cursor += 1;
                        *c += raw * pose.channel_scale[bit];
                    }
                }
                if pi < pose_count {
                    local_pose[pi] = trs_matrix(
                        [ch[0], ch[1], ch[2]],
                        [ch[3], ch[4], ch[5], ch[6]],
                        [ch[7], ch[8], ch[9]],
                    );
                }
            }
            let world_pose = resolve_hierarchy(&local_pose, &pose_parents);
            for j in 0..nbones {
                if j < pose_count {
                    out.push(mul34(world_pose[j], inv_bind[j]));
                } else {
                    out.push(IDENT34);
                }
            }
        }
        out
    }

    /// Overall axis-aligned bounds and bounding radius.
    pub fn compute_bounds(&self) -> (Vec3, Vec3, f32) {
        if !self.bounds.is_empty() {
            let mut lo = [f32::MAX; 3];
            let mut hi = [f32::MIN; 3];
            let mut rad = 0.0f32;
            for b in &self.bounds {
                for i in 0..3 {
                    lo[i] = lo[i].min(b.bbmin[i]);
                    hi[i] = hi[i].max(b.bbmax[i]);
                }
                rad = rad.max(b.radius);
            }
            if lo[0].is_finite() && hi[0].is_finite() {
                let corner_r = (lo[0].abs().max(hi[0].abs()).powi(2)
                    + lo[1].abs().max(hi[1].abs()).powi(2)
                    + lo[2].abs().max(hi[2].abs()).powi(2))
                .sqrt();
                return (lo, hi, rad.max(corner_r));
            }
        }
        if self.positions.is_empty() {
            return ([-16.0; 3], [16.0; 3], 28.0);
        }
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        let mut rad2 = 0.0f32;
        for &p in &self.positions {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
            rad2 = rad2.max(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
        }
        (lo, hi, rad2.sqrt())
    }
}

/// Row-major 3x4 matrix for `T * R(quat) * S(scale)`.
fn trs_matrix(t: Vec3, q: [f32; 4], s: Vec3) -> [f32; 12] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let [x, y, z, w] = if len > 1e-8 { q.map(|c| c / len) } else { [0.0, 0.0, 0.0, 1.0] };
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);
    [
        (1.0 - 2.0 * (yy + zz)) * s[0],
        (2.0 * (xy - wz)) * s[1],
        (2.0 * (xz + wy)) * s[2],
        t[0],
        (2.0 * (xy + wz)) * s[0],
        (1.0 - 2.0 * (xx + zz)) * s[1],
        (2.0 * (yz - wx)) * s[2],
        t[1],
        (2.0 * (xz - wy)) * s[0],
        (2.0 * (yz + wx)) * s[1],
        (1.0 - 2.0 * (xx + yy)) * s[2],
        t[2],
    ]
}

fn mul34(a: [f32; 12], b: [f32; 12]) -> [f32; 12] {
    let mut out = [0.0f32; 12];
    for r in 0..3 {
        let ra = r * 4;
        for c in 0..3 {
            out[ra + c] = a[ra] * b[c] + a[ra + 1] * b[4 + c] + a[ra + 2] * b[8 + c];
        }
        out[ra + 3] = a[ra] * b[3] + a[ra + 1] * b[7] + a[ra + 2] * b[11] + a[ra + 3];
    }
    out
}

fn invert34(m: [f32; 12]) -> [f32; 12] {
    let (a00, a01, a02) = (m[0], m[1], m[2]);
    let (a10, a11, a12) = (m[4], m[5], m[6]);
    let (a20, a21, a22) = (m[8], m[9], m[10]);
    let c00 = a11 * a22 - a12 * a21;
    let c01 = a12 * a20 - a10 * a22;
    let c02 = a10 * a21 - a11 * a20;
    let det = a00 * c00 + a01 * c01 + a02 * c02;
    if det.abs() < 1e-12 || !det.is_finite() {
        return IDENT34;
    }
    let inv = 1.0 / det;
    let i00 = c00 * inv;
    let i01 = (a02 * a21 - a01 * a22) * inv;
    let i02 = (a01 * a12 - a02 * a11) * inv;
    let i10 = c01 * inv;
    let i11 = (a00 * a22 - a02 * a20) * inv;
    let i12 = (a02 * a10 - a00 * a12) * inv;
    let i20 = c02 * inv;
    let i21 = (a01 * a20 - a00 * a21) * inv;
    let i22 = (a00 * a11 - a01 * a10) * inv;
    let (tx, ty, tz) = (m[3], m[7], m[11]);
    [
        i00, i01, i02, -(i00 * tx + i01 * ty + i02 * tz),
        i10, i11, i12, -(i10 * tx + i11 * ty + i12 * tz),
        i20, i21, i22, -(i20 * tx + i21 * ty + i22 * tz),
    ]
}

fn resolve_hierarchy(local: &[[f32; 12]], parents: &[i32]) -> Vec<[f32; 12]> {
    let n = local.len();
    let mut world = vec![IDENT34; n];
    let mut state = vec![0u8; n]; // 0 = unvisited, 1 = visiting, 2 = done
    fn visit(i: usize, local: &[[f32; 12]], parents: &[i32], world: &mut [[f32; 12]], state: &mut [u8]) {
        if state[i] != 0 {
            return;
        }
        state[i] = 1;
        let p = parents[i];
        world[i] = if p >= 0 && (p as usize) < local.len() && (p as usize) != i {
            let pi = p as usize;
            visit(pi, local, parents, world, state);
            mul34(world[pi], local[i])
        } else {
            local[i]
        };
        state[i] = 2;
    }
    for i in 0..n {
        visit(i, local, parents, &mut world, &mut state);
    }
    world
}

#[cfg(test)]
mod tests {
    use super::super::*;

    /// Build a minimal valid 2-bone animated IQM v2 binary for testing.
    fn synth_iqm() -> Vec<u8> {
        let text = b"\0body\0progs/body.tga\0root\0child\0walk\0";
        let hdr_len = 124usize;
        let ofs_text = hdr_len;
        let ofs_meshes = (ofs_text + text.len() + 3) & !3;
        let ofs_va = ofs_meshes + 24;
        let ofs_pos = ofs_va + 5 * 20;
        let ofs_uv = ofs_pos + 3 * 12;
        let ofs_nrm = ofs_uv + 3 * 8;
        let ofs_bi = ofs_nrm + 3 * 12;
        let ofs_bw = ofs_bi + 3 * 4;
        let ofs_tris = (ofs_bw + 3 * 4 + 3) & !3;
        let ofs_joints = ofs_tris + 12;
        let ofs_poses = ofs_joints + 2 * 48;
        let ofs_anims = ofs_poses + 2 * 88;
        let ofs_frames = ofs_anims + 20;
        let filesize = ofs_frames + 2 * 2;

        let mut d = Vec::with_capacity(filesize);
        d.extend_from_slice(IQM_MAGIC);
        let header_u32s: [u32; 27] = [
            IQM_VERSION, filesize as u32, 0,
            text.len() as u32, ofs_text as u32,
            1, ofs_meshes as u32,
            5, 3, ofs_va as u32,
            1, ofs_tris as u32, 0,
            2, ofs_joints as u32,
            2, ofs_poses as u32,
            1, ofs_anims as u32,
            2, 1, ofs_frames as u32, 0,
            0, 0, 0, 0,
        ];
        for v in header_u32s { d.extend_from_slice(&v.to_le_bytes()); }
        d.extend_from_slice(text);
        while d.len() < ofs_meshes { d.push(0); }
        for v in [1u32, 6, 0, 3, 0, 1] { d.extend_from_slice(&v.to_le_bytes()); }
        for (kind, fmt, sz, ofs) in [
            (IQM_POSITION, IQM_FLOAT, 3u32, ofs_pos as u32),
            (IQM_TEXCOORD, IQM_FLOAT, 2, ofs_uv as u32),
            (IQM_NORMAL, IQM_FLOAT, 3, ofs_nrm as u32),
            (IQM_BLENDINDEXES, IQM_UBYTE, 4, ofs_bi as u32),
            (IQM_BLENDWEIGHTS, IQM_UBYTE, 4, ofs_bw as u32),
        ] {
            for v in [kind, 0, fmt, sz, ofs] { d.extend_from_slice(&v.to_le_bytes()); }
        }
        for f in [0.0f32, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 10.0, 0.0] { d.extend_from_slice(&f.to_le_bytes()); }
        for f in [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0] { d.extend_from_slice(&f.to_le_bytes()); }
        for f in [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0] { d.extend_from_slice(&f.to_le_bytes()); }
        d.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
        d.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0]);
        while d.len() < ofs_tris { d.push(0); }
        for v in [0u32, 1, 2] { d.extend_from_slice(&v.to_le_bytes()); }
        d.extend_from_slice(&21u32.to_le_bytes());
        d.extend_from_slice(&(-1i32).to_le_bytes());
        for f in [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] { d.extend_from_slice(&f.to_le_bytes()); }
        d.extend_from_slice(&26u32.to_le_bytes());
        d.extend_from_slice(&0i32.to_le_bytes());
        for f in [5.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] { d.extend_from_slice(&f.to_le_bytes()); }
        d.extend_from_slice(&(-1i32).to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        for f in [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] { d.extend_from_slice(&f.to_le_bytes()); }
        for _ in 0..10 { d.extend_from_slice(&0.0f32.to_le_bytes()); }
        d.extend_from_slice(&0i32.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        for f in [5.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] { d.extend_from_slice(&f.to_le_bytes()); }
        for f in [2.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] { d.extend_from_slice(&f.to_le_bytes()); }
        d.extend_from_slice(&32u32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&2u32.to_le_bytes());
        d.extend_from_slice(&10.0f32.to_le_bytes());
        d.extend_from_slice(&IQM_LOOP.to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&10u16.to_le_bytes());
        d
    }

    #[test]
    fn parse_and_skin_animated_iqm() {
        let data = synth_iqm();
        let iqm = Iqm::parse(&data).unwrap();
        assert_eq!(iqm.meshes.len(), 1);
        assert_eq!(iqm.meshes[0].name, "body");
        assert_eq!(iqm.meshes[0].material, "progs/body.tga");
        assert_eq!(iqm.joints.len(), 2);
        assert_eq!(iqm.anims.len(), 1);
        assert_eq!(iqm.anims[0].name, "walk");
        let mats = iqm.compute_frame_matrices();
        assert_eq!(mats.len(), 4);
        assert!((mats[1][3]).abs() < 1e-4);
        assert!((mats[3][3] - 20.0).abs() < 1e-4, "mats[3]={:?}", mats[3]);
    }
}
