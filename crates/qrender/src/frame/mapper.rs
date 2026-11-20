use super::*;

fn ranges(w: &WorldDraw) -> impl Iterator<Item = Range> {
    [w.opaque, w.sky]
        .into_iter()
        .chain(w.water)
        .chain(w.water_alpha)
        .filter(|r| r.count > 0)
}

pub(super) fn stats(
    world: &[WorldDraw],
    alias: &[AliasDraw],
    sprites: &[SpriteDraw],
    particles: Range,
    fte: Option<&FteDraw>,
    leaves: u32,
) -> MapperStats {
    let mut stats = MapperStats {
        leaves,
        ..Default::default()
    };
    for draw in world {
        stats.world_polys += draw.polys;
        stats.triangles += draw.triangles;
        stats.draws += ranges(draw).count() as u32;
    }
    for draw in alias.iter().filter(|a| a.main_visible && !a.portal_only) {
        stats.entity_polys += draw.num_indices / 3;
        stats.draws += 1;
    }
    stats.entity_polys += sprites.len() as u32 * 2;
    stats.triangles += stats.entity_polys;
    stats.draws += sprites.len() as u32 + u32::from(particles.count > 0);
    stats.draws += fte.map_or(0, |(_, _, batches)| batches.len() as u32);
    stats
}

impl Renderer {
    /// Counts from the last primary scene with r_speeds or r_showtris enabled.
    pub fn mapper_stats(&self) -> MapperStats {
        self.mapper_stats
    }

    pub fn supports_wireframe(&self) -> bool {
        self.gpu.wireframe
    }
}

/// Draw the submitted BSP triangles as an overlay using their existing buffers.
/// vkQuake's r_showtris also selects a depth-tested or through-wall line pass.
pub(super) fn draw_world_wire(
    dev: &ash::Device,
    cmd: vk::CommandBuffer,
    p: &Pipelines,
    world: &WorldGpu,
    dynbuf: vk::Buffer,
    draws: &[WorldDraw],
    mode: usize,
) {
    // SAFETY: buffers, push layout and pipelines are live for this scene pass.
    unsafe {
        let stages = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, p.wire_world[mode]);
        dev.cmd_set_depth_bias(cmd, -5.0, 0.0, -0.15);
        dev.cmd_bind_vertex_buffers(cmd, 0, &[world.vbuf.buf], &[0]);
        for draw in draws {
            dev.cmd_bind_index_buffer(cmd, draw.ibuf.unwrap_or(dynbuf), 0, vk::IndexType::UINT32);
            let push = WorldPush {
                model: draw.model,
                params: [1.0, (mode + 1) as f32, 0.0, 0.0],
            };
            dev.cmd_push_constants(cmd, p.layout, stages, 0, bytes_of(&[push]));
            for range in ranges(draw) {
                dev.cmd_draw_indexed_indirect(
                    cmd,
                    draw.cbuf.unwrap_or(dynbuf),
                    range.ofs,
                    range.count,
                    20,
                );
            }
        }
        dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
    }
}
