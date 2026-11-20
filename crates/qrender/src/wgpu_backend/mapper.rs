//! WebGPU wire overlays use explicit edges so they work without
//! POLYGON_MODE_LINE, including in the browser.
use super::math::xform;
use super::models::Model;
use super::pipes::{DrawU, poly_index};
use super::scene::{Cmd, Lists, Pipe, Src};
use super::{NO_TEX, Renderer};
use crate::{FteBlend, FteVertex, MapperStats};

pub(super) fn stats(lists: &Lists, cmds: &[Cmd], soft: usize, leaves: u32) -> MapperStats {
    let mut stats = MapperStats {
        leaves,
        world_polys: lists.world_polys,
        draws: (cmds.len() + soft) as u32,
        ..Default::default()
    };
    for cmd in cmds {
        match cmd.src {
            Src::World => stats.triangles += cmd.count / 3,
            Src::Alias(_) if cmd.pipe != Pipe::AliasXray => stats.entity_polys += cmd.count / 3,
            Src::Poly if lists.draw_u[cmd.du as usize].ix[1] == 1 => {
                stats.entity_polys += cmd.count / 3
            }
            _ => {}
        }
    }
    stats.triangles += stats.entity_polys;
    stats
}

impl Renderer {
    pub fn mapper_stats(&self) -> MapperStats {
        self.mapper_stats
    }

    pub fn supports_wireframe(&self) -> bool {
        true
    }

    pub(super) fn wire_cmd(
        &self,
        mode: u32,
        lists: &mut Lists,
        draws: &[Cmd],
        viewmodel: bool,
    ) -> Option<Cmd> {
        let first = lists.poly_i.len() as u32;
        let color = if mode == 1 {
            [190, 240, 255, 230]
        } else {
            [255, 82, 31, 89]
        };
        for cmd in draws
            .iter()
            .filter(|c| c.viewmodel == viewmodel && c.pipe != Pipe::AliasXray)
        {
            let du = &lists.draw_u[cmd.du as usize];
            let range = cmd.first as usize..(cmd.first + cmd.count) as usize;
            let alias = match cmd.src {
                Src::Alias(mi) => match self.models.get(mi) {
                    Some(Model::Alias(alias)) => Some(alias),
                    _ => continue,
                },
                _ => None,
            };
            let indices = match cmd.src {
                Src::World => &lists.world_idx[range],
                Src::Alias(_) => &alias.unwrap().indices[range],
                Src::Poly if du.ix[1] == 1 => &lists.poly_i[range],
                _ => continue,
            };
            let mut vertices = Vec::with_capacity(indices.len());
            for &i in indices {
                let i = (i as i32 + cmd.base_vertex) as usize;
                let pos = match cmd.src {
                    Src::World => {
                        xform(&du.model, self.world.as_ref().unwrap().cpu.vertices[i].pos)
                    }
                    Src::Alias(_) => lists.alias_v[i].pos,
                    Src::Poly => lists.poly_v[i].pos,
                };
                vertices.push(FteVertex {
                    pos,
                    uv: [0.5, 0.5],
                    color,
                });
            }
            let base = lists.poly_v.len() as u32;
            for triangle in 0..(vertices.len() / 3) as u32 {
                let i = base + triangle * 3;
                lists
                    .poly_i
                    .extend_from_slice(&[i, i + 1, i + 1, i + 2, i + 2, i]);
            }
            lists.poly_v.extend(vertices);
        }
        let count = lists.poly_i.len() as u32 - first;
        if count == 0 {
            return None;
        }
        let du = lists.draw(DrawU {
            ix: [0, 0, 1, 0],
            ..Default::default()
        });
        let blend = if mode == 1 {
            FteBlend::Blend
        } else {
            FteBlend::Overlay
        };
        Some(Cmd {
            pipe: Pipe::Poly(poly_index(blend, true)),
            src: Src::Poly,
            mat: [NO_TEX; 3],
            first,
            count,
            base_vertex: 0,
            du,
            viewmodel,
        })
    }
}
