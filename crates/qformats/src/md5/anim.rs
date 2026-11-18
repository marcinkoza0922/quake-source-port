//! The `.md5anim` side: the animation's joint hierarchy matched to the
//! mesh's joints by name (vkQuake's MD5Anim_Load), and every frame's
//! absolute joint transforms.

use super::*;

/// Absolute joint transforms for every frame of `anim` (just the bind pose
/// without an animation).
pub(super) fn parse_anim(text: Option<&[u8]>, joints: &[Joint]) -> Result<Vec<Vec<Mat>>> {
    let bind: Vec<Mat> = joints.iter().map(|j| j.bind).collect();
    let Some(text) = text else {
        return Ok(vec![bind]);
    };
    // Joint transforms relative to their parent in the bind pose: the
    // fallback for mesh joints the animation does not drive.
    let rel_bind: Vec<Mat> = joints
        .iter()
        .map(|j| {
            if j.parent < 0 {
                j.bind
            } else {
                concat(&invert(&joints[j.parent as usize].bind), &j.bind)
            }
        })
        .collect();
    let mut t = Tokens::new(text);
    t.header()?;
    t.expect("numFrames")?;
    let num_frames = t.count(1 << 16)?;
    t.expect("numJoints")?;
    let anim_joints = t.count(4096)?;
    t.expect("frameRate")?;
    t.word()?;
    t.expect("numAnimatedComponents")?;
    let raw_count = t.count(1 << 20)?;
    if num_frames == 0 {
        return Err(err("animation has no frames"));
    }
    t.expect("hierarchy")?;
    t.expect("{")?;
    // (mesh joint, flags, offset, animation parent)
    let mut hier = Vec::with_capacity(anim_joints);
    for j in 0..anim_joints {
        let name = t.word()?;
        let parent = t.int()?;
        let flags = t.count(63)? as u32;
        let offset = t.count(raw_count)?;
        if parent < -1 || parent >= j as i64 || offset + flags.count_ones() as usize > raw_count {
            return Err(err(format!("bad hierarchy entry \"{name}\"")));
        }
        let mesh = joints.iter().position(|m| m.name == name);
        if mesh.is_some()
            && hier
                .iter()
                .any(|h: &(Option<usize>, u32, usize, i64)| h.0 == mesh)
        {
            return Err(err(format!("duplicate joint \"{name}\"")));
        }
        hier.push((mesh, flags, offset, parent));
    }
    // Mesh joint -> its animation joint, and each mesh joint's nearest
    // ancestor the animation drives (MD5_ResolveMappedMeshParent).
    let mut mesh_to_anim = vec![None; joints.len()];
    for (a, h) in hier.iter().enumerate() {
        if let Some(m) = h.0 {
            mesh_to_anim[m] = Some(a);
        }
    }
    let mut mapped = vec![None; joints.len()];
    for j in 0..joints.len() {
        let mut p = joints[j].parent;
        let mut steps = 0;
        while p >= 0 && mesh_to_anim[p as usize].is_none() {
            p = joints[p as usize].parent;
            steps += 1;
            if steps > joints.len() {
                return Err(err(format!(
                    "joint \"{}\" has bad parent chain",
                    joints[j].name
                )));
            }
        }
        mapped[j] = (p >= 0).then_some(p as usize);
    }
    // Pose parents: the animation's for driven joints (which must be the
    // mapped mesh parent), the mesh's for the rest.
    let mut pose_parent: Vec<Option<usize>> = joints
        .iter()
        .map(|j| (j.parent >= 0).then_some(j.parent as usize))
        .collect();
    for &(mesh, _, _, parent) in &hier {
        let Some(m) = mesh else { continue };
        let anim_parent = if parent < 0 {
            None
        } else {
            let Some(p) = hier[parent as usize].0 else {
                return Err(err(format!(
                    "joint \"{}\" has unmapped parent",
                    joints[m].name
                )));
            };
            Some(p)
        };
        if anim_parent != mapped[m] {
            return Err(err(format!(
                "joint \"{}\" has a different parent in the animation",
                joints[m].name
            )));
        }
        pose_parent[m] = anim_parent;
    }
    // Parents before children, whatever the file order.
    let order = parent_order(&pose_parent).ok_or_else(|| err("joint pose parents form a cycle"))?;
    t.expect("}")?;
    t.expect("bounds")?;
    t.expect("{")?;
    while t.peek() == Some("(") {
        t.vec3()?;
        t.vec3()?;
    }
    t.expect("}")?;
    t.expect("baseframe")?;
    t.expect("{")?;
    let mut base = Vec::with_capacity(anim_joints);
    for _ in 0..anim_joints {
        base.push((t.vec3()?, t.vec3()?));
    }
    t.expect("}")?;
    let mut relative = vec![rel_bind; num_frames];
    let mut raw = vec![0.0f32; raw_count];
    while t.peek() == Some("frame") {
        t.next();
        let f = t.count(num_frames - 1)?;
        t.expect("{")?;
        for r in raw.iter_mut() {
            *r = t.float()?;
        }
        t.expect("}")?;
        for (j, &(mesh, flags, offset, _)) in hier.iter().enumerate() {
            let Some(m) = mesh else { continue };
            let (p, q) = base[j];
            let mut c = [p[0], p[1], p[2], q[0], q[1], q[2]];
            let mut k = offset;
            for (bit, slot) in c.iter_mut().enumerate() {
                if flags & (1 << bit) != 0 {
                    *slot = raw[k];
                    k += 1;
                }
            }
            relative[f][m] = pos_quat([c[0], c[1], c[2]], quat([c[3], c[4], c[5]]));
        }
    }
    Ok(relative
        .into_iter()
        .map(|rel| {
            let mut abs: Vec<Mat> = rel.clone();
            for &j in &order {
                if let Some(p) = pose_parent[j] {
                    abs[j] = concat(&abs[p], &rel[j]);
                }
            }
            abs
        })
        .collect())
}

/// Joint indices ordered so every parent comes before its children, or
/// `None` on a cycle.
fn parent_order(parent: &[Option<usize>]) -> Option<Vec<usize>> {
    // 0 = unvisited, 1 = on the current chain, 2 = placed
    let mut state = vec![0u8; parent.len()];
    let mut order = Vec::with_capacity(parent.len());
    for start in 0..parent.len() {
        let mut chain = Vec::new();
        let mut j = Some(start);
        while let Some(k) = j {
            match state[k] {
                2 => break,
                1 => return None,
                _ => {}
            }
            state[k] = 1;
            chain.push(k);
            j = parent[k];
        }
        for &k in chain.iter().rev() {
            state[k] = 2;
            order.push(k);
        }
    }
    Some(order)
}
