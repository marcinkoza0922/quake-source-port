use super::*;

/// Reused scratch buffers for [`build_world_draw`] to avoid per-entity/portal `Vec` allocations.
#[derive(Default)]
pub(crate) struct WorldDrawScratch {
    cats: [Vec<vk::DrawIndexedIndirectCommand>; 4],
    opaque_nofb: Vec<vk::DrawIndexedIndirectCommand>,
    used_buckets: Vec<usize>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_world_draw(
    w: &WorldGpu,
    buckets: &mut [Vec<u32>],
    scratch: &mut WorldDrawScratch,
    dynw: &mut Dyn,
    surfs: &[u32],
    alt: bool,
    time: f64,
    alphas: [f32; 4],
    ent_alpha: f32,
    model: Mat4,
    lm_vis: &mut LightmapRequests,
) -> WorldDraw {
    scratch.used_buckets.clear();
    let max_t = buckets.len() - 1;
    let has_hidden = !w.cpu.hidden_surfaces.is_empty();
    for &si in surfs {
        if has_hidden && w.cpu.hidden_surfaces.contains(&si) {
            continue;
        }
        let s = &w.draw_surfaces[si as usize];
        let t = (w.cpu.resolve_texture(s.texture, alt, time, si) as usize).min(max_t);
        if buckets[t].is_empty() {
            scratch.used_buckets.push(t);
        }
        buckets[t].push(si);
        if s.lm != NO_LM {
            lm_vis.push(s.lm);
        }
    }
    scratch.used_buckets.sort_unstable();
    let mut polys = 0;
    let mut triangles = 0;
    for c in &mut scratch.cats {
        c.clear();
    }
    scratch.opaque_nofb.clear();
    for &t in &scratch.used_buckets {
        let b = &mut buckets[t];
        let mut first = None;
        let mut count = 0u32;
        for &si in b.iter() {
            let s = &w.draw_surfaces[si as usize];
            let idx =
                &w.cpu.indices[s.first_index as usize..(s.first_index + s.num_indices) as usize];
            let Some(o) = dynw.push(idx, 4) else { break };
            first.get_or_insert(o);
            count += s.num_indices;
            polys += u32::from(s.num_indices > 0);
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
        triangles += count / 3;
        let cmd = vk::DrawIndexedIndirectCommand {
            index_count: count,
            instance_count: 1,
            first_index: (first / 4) as u32,
            vertex_offset: 0,
            first_instance: t as u32,
        };
        if cat == 0 && !w.tex_has_fb.get(t).copied().unwrap_or(false) {
            scratch.opaque_nofb.push(cmd);
        } else {
            scratch.cats[cat].push(cmd);
        }
    }
    let fb_count = scratch.cats[0].len() as u32;
    scratch.cats[0].extend_from_slice(&scratch.opaque_nofb);
    let mut put = |c: &Vec<vk::DrawIndexedIndirectCommand>| match dynw.push(c, 16) {
        Some(ofs) if !c.is_empty() => Range {
            ofs,
            count: c.len() as u32,
        },
        _ => Range::default(),
    };
    let opaque = put(&scratch.cats[0]);
    let opaque_fb = Range { ofs: opaque.ofs, count: fb_count.min(opaque.count) };
    WorldDraw {
        model,
        zfix: false,
        main_visible: true,
        alpha: ent_alpha,
        opaque,
        opaque_fb,
        sky: put(&scratch.cats[1]),
        water: [put(&scratch.cats[2]), Range::default(), Range::default(), Range::default()],
        water_alpha: [put(&scratch.cats[3]), Range::default(), Range::default(), Range::default()],
        ibuf: None,
        cbuf: None,
        bounds: [0.0; 4],
        polys,
        triangles,
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
    let leaf = wg.cpu.view_leaf(vs.eye);
    gpu_world_draw_ex(wg, dynw, leaf, vs.eye, &vs.frustum, [0.0, 0.0, 0.0, 1.0], time, alphas, fi)
}

/// Generalised GPU world cull for any view (main or destination portal) with an
/// explicit PVS leaf, eye position (backface test), frustum and optional clip plane.
#[allow(clippy::too_many_arguments)]
pub(super) fn gpu_world_draw_ex(
    wg: &mut WorldGpu,
    dynw: &mut Dyn,
    leaf: usize,
    eye: [f32; 3],
    frustum: &[[f32; 4]; 4],
    clip: [f32; 4],
    time: f64,
    alphas: [f32; 4],
    fi: usize,
) -> Option<(WorldDraw, CullFrame)> {
    let mut bits = std::mem::take(&mut wg.scratch_bits);
    wg.cpu.leaf_bits_for_leaf(leaf, frustum, &mut bits);
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
        main_visible: true,
        alpha: 1.0,
        opaque: range(&c.cats[0]),
        opaque_fb: range(&c.opaque_fb),
        sky: range(&c.cats[1]),
        water,
        water_alpha,
        ibuf: Some(c.out[fi].buf),
        cbuf: Some(c.cmdbuf[fi].buf),
        bounds: [0.0; 4],
        polys: 0,
        triangles: 0,
    };
    let push = CullPush {
        frustum: *frustum,
        clip,
        eye: [eye[0], eye[1], eye[2], 0.0],
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
