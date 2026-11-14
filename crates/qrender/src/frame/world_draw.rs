use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_world_draw(
    w: &WorldGpu,
    buckets: &mut [Vec<u32>],
    dynw: &mut Dyn,
    surfs: &[u32],
    alt: bool,
    time: f64,
    alphas: [f32; 4],
    ent_alpha: f32,
    model: Mat4,
    lm_vis: &mut Vec<u32>,
) -> WorldDraw {
    for &si in surfs {
        if w.cpu.hidden_surfaces.contains(&si) { continue; }
        let s = &w.cpu.surfaces[si as usize];
        let t = (w.cpu.resolve_texture(s.texture, alt, time, si) as usize).min(buckets.len() - 1);
        buckets[t].push(si);
        if s.lm != NO_LM {
            lm_vis.push(s.lm);
        }
    }
    let mut cats: [Vec<vk::DrawIndexedIndirectCommand>; 4] = Default::default();
    for (t, b) in buckets.iter_mut().enumerate() {
        if b.is_empty() {
            continue;
        }
        let mut first = None;
        let mut count = 0u32;
        for &si in b.iter() {
            let s = &w.cpu.surfaces[si as usize];
            let idx =
                &w.cpu.indices[s.first_index as usize..(s.first_index + s.num_indices) as usize];
            let Some(o) = dynw.push(idx, 4) else { break };
            first.get_or_insert(o);
            count += s.num_indices;
        }
        b.clear();
        let Some(first) = first else { continue };
        let (kind, flags) = w.tex_info[t];
        let cat = match kind {
            TextureKind::Sky => 1,
            TextureKind::Liquid => {
                let ty = (flags & 0xf).clamp(1, 4) as usize;
                if alphas[ty - 1] * ent_alpha < 1.0 {
                    3
                } else {
                    2
                }
            }
            _ => 0,
        };
        cats[cat].push(vk::DrawIndexedIndirectCommand {
            index_count: count,
            instance_count: 1,
            first_index: (first / 4) as u32,
            vertex_offset: 0,
            first_instance: t as u32,
        });
    }
    let mut put = |c: &Vec<vk::DrawIndexedIndirectCommand>| match dynw.push(c, 16) {
        Some(ofs) if !c.is_empty() => Range {
            ofs,
            count: c.len() as u32,
        },
        _ => Range::default(),
    };
    WorldDraw {
        model,
        zfix: false,
        alpha: ent_alpha,
        opaque: put(&cats[0]),
        sky: put(&cats[1]),
        water: [put(&cats[2]), Range::default(), Range::default(), Range::default()],
        water_alpha: [put(&cats[3]), Range::default(), Range::default(), Range::default()],
        ibuf: None,
        cbuf: None,
    }
}

/// Per-frame setup of the GPU-culled world: leaf bits and zeroed draw
/// commands in dynbuf, the cull push constants, and the world's WorldDraw
/// (commands in dynbuf, indices in this frame's cull output buffer).
pub(super) fn gpu_world_draw(
    wg: &mut WorldGpu,
    dynw: &mut Dyn,
    vs: &ViewSetup,
    time: f64,
    alphas: [f32; 4],
    fi: usize,
) -> Option<(WorldDraw, CullFrame)> {
    let mut bits = std::mem::take(&mut wg.scratch_bits);
    wg.cpu.leaf_bits(vs.eye, &vs.frustum, &mut bits);
    let ntex = wg.tex_info.len().max(1) as u32;
    let c = &mut wg.cull;
    c.framecount = c.framecount.wrapping_add(1).max(1);
    let mut cmds = std::mem::take(&mut wg.scratch_cmds);
    cmds.clear();
    cmds.extend(c.cmd_tex.iter().zip(&c.cmd_first).map(|(&t, &first)| {
        let tex = match wg.cpu.anims.get(t as usize) {
            Some(a) if !a.2 && !a.0.is_empty() => wg.cpu.resolve_texture(t, false, time, 0),
            _ => t,
        };
        vk::DrawIndexedIndirectCommand {
            index_count: 0,
            instance_count: 1,
            first_index: first,
            vertex_offset: 0,
            first_instance: tex.min(ntex - 1),
        }
    }));
    // Staged as commands then dispatch args, copied to cmdbuf on the GPU.
    let disp_ofs = cmds.len() as u64 * 20;
    let stage_ofs = dynw.push(&cmds, 16)?;
    dynw.push(&[0u32, 1, 1, 0], 4)?;
    dynw.push(&bits, 4)?;
    let range = |r: &std::ops::Range<u32>| Range {
        ofs: r.start as u64 * 20,
        count: r.end - r.start,
    };
    let mut water = [Range::default(); 4];
    let mut water_alpha = [Range::default(); 4];
    for ty in 0..4 {
        let r = range(&c.cats[2 + ty]);
        if alphas[ty] < 1.0 {
            water_alpha[ty] = r;
        } else {
            water[ty] = r;
        }
    }
    let wd = WorldDraw {
        model: ident(),
        zfix: false,
        alpha: 1.0,
        opaque: range(&c.cats[0]),
        sky: range(&c.cats[1]),
        water,
        water_alpha,
        ibuf: Some(c.out[fi].buf),
        cbuf: Some(c.cmdbuf[fi].buf),
    };
    let push = CullPush {
        frustum: vs.frustum,
        eye: [vs.eye[0], vs.eye[1], vs.eye[2], 0.0],
        framecount: c.framecount,
        nmarks: c.nmarks,
        ncmds: cmds.len() as u32,
        ofs_vis: (disp_ofs / 4 + 4) as u32,
        ofs_lm: (OFS_VIS / 4) as u32,
        pad: 0,
        max_lm: MAX_VIS as u32,
    };
    let cf = CullFrame {
        push,
        groups: c.nmarks.div_ceil(64),
        stage_ofs,
        stage_len: disp_ofs + 16 + bits.len() as u64 * 4,
        disp_ofs,
        cmdbuf: c.cmdbuf[fi].buf,
    };
    wg.scratch_bits = bits;
    wg.scratch_cmds = cmds;
    Some((wd, cf))
}
