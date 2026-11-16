//! CPU lightmap building (R_BuildLightMap + R_AddDynamicLights, a port of
//! `lightmap.comp`) into an RGBA8 atlas array storing light / 2.

use super::textures::write_rows;
use crate::DLight;
use crate::world::{ATLAS, LmSurf, World};

pub(super) struct Lightmaps {
    pub(super) tex: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    /// Style values the atlas was last built with.
    styles: [f32; 256],
    /// Lightmap surfaces using each style.
    style_surfs: Vec<Vec<u32>>,
    /// Surfaces touched by dynamic lights last frame (rebuilt to clear them).
    dlit: Vec<u32>,
    stamp: Vec<u32>,
    frame: u32,
    scratch: Vec<u8>,
}

fn atlas_texture(dev: &wgpu::Device, size: u32, layers: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = dev.create_texture(&wgpu::TextureDescriptor {
        label: Some("lightmap"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: layers.max(1) },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    (tex, view)
}

/// Texels of one surface into `out` (RGBA8, light / 2).
fn build_surface(s: &LmSurf, samples: &[u32], styles: &[f32; 256], dlights: &[DLight], out: &mut Vec<u8>) {
    let (w, h) = (s.size & 0xffff, s.size >> 16);
    let nst = (w * h) as usize;
    out.clear();
    for y in 0..h {
        for x in 0..w {
            let mut l = [0.0f32; 3];
            if s.sample_ofs == u32::MAX {
                l = [s.tmins[3]; 3];
            } else {
                let mut i = s.sample_ofs as usize + (y * w + x) as usize;
                for k in 0..4 {
                    let st = (s.styles >> (k * 8)) & 0xff;
                    if st == 255 {
                        break;
                    }
                    let v = samples.get(i).copied().unwrap_or(0);
                    let sc = styles[st as usize];
                    for (c, lc) in l.iter_mut().enumerate() {
                        *lc += ((v >> (c * 8)) & 0xff) as f32 * sc;
                    }
                    i += nst;
                }
                l = l.map(|c| c / (256.0 * 128.0));
            }
            for d in dlights {
                let n = [s.plane[0], s.plane[1], s.plane[2]];
                let dist = d.origin[0] * n[0] + d.origin[1] * n[1] + d.origin[2] * n[2] - s.plane[3];
                let rad = d.radius - dist.abs();
                if rad < d.minlight {
                    continue;
                }
                let minlight = rad - d.minlight;
                let imp: [f32; 3] = std::array::from_fn(|k| d.origin[k] - n[k] * dist);
                let ls = imp[0] * s.vecs0[0] + imp[1] * s.vecs0[1] + imp[2] * s.vecs0[2] + s.vecs0[3] - s.tmins[0];
                let lt = imp[0] * s.vecs1[0] + imp[1] * s.vecs1[1] + imp[2] * s.vecs1[2] + s.vecs1[3] - s.tmins[1];
                let sd = (ls - (x * 16) as f32).abs();
                let td = (lt - (y * 16) as f32).abs();
                let dd = if sd > td { sd + td * 0.5 } else { td + sd * 0.5 };
                if dd < minlight {
                    for (lc, dc) in l.iter_mut().zip(d.color) {
                        *lc += (rad - dd) / 128.0 * dc;
                    }
                }
            }
            for c in l {
                out.push((c.min(2.0) * 127.5 + 0.5) as u8);
            }
            out.push(255);
        }
    }
}

impl Lightmaps {
    pub(super) fn new(dev: &wgpu::Device, queue: &wgpu::Queue, layers: u32) -> Self {
        let (tex, view) = atlas_texture(dev, 1, layers);
        write_rows(queue, &tex, 0, [0, 0, 0], 1, 1, &[128, 128, 128, 255]);
        Self {
            tex,
            view,
            styles: [-1.0; 256],
            style_surfs: Vec::new(),
            dlit: Vec::new(),
            stamp: Vec::new(),
            frame: 0,
            scratch: Vec::new(),
        }
    }

    /// Allocates the atlas for a level; texels are built on the first update.
    pub(super) fn for_world(dev: &wgpu::Device, w: &World) -> Self {
        let (tex, view) = atlas_texture(dev, ATLAS, w.atlas_layers);
        let mut style_surfs = vec![Vec::new(); 256];
        for (i, s) in w.lm_surfs.iter().enumerate() {
            for k in 0..4 {
                let st = (s.styles >> (k * 8)) & 0xff;
                if st == 255 {
                    break;
                }
                style_surfs[st as usize].push(i as u32);
            }
        }
        Self {
            tex,
            view,
            styles: [f32::NAN; 256],
            style_surfs,
            dlit: Vec::new(),
            stamp: vec![0; w.lm_surfs.len()],
            frame: 0,
            scratch: Vec::new(),
        }
    }

    /// Rebuilds surfaces whose styles changed or that dynamic lights touch
    /// (or touched last frame). The first call builds every surface.
    pub(super) fn update(&mut self, queue: &wgpu::Queue, w: &World, styles: &[f32; 256], dlights: &[DLight]) {
        if self.stamp.len() != w.lm_surfs.len() {
            return;
        }
        self.frame = self.frame.wrapping_add(1).max(1);
        let mut todo: Vec<u32> = Vec::new();
        let first = self.styles[0].is_nan();
        let mut mark = |i: u32, stamp: &mut Vec<u32>, frame: u32| {
            if stamp[i as usize] != frame {
                stamp[i as usize] = frame;
                todo.push(i);
            }
        };
        if first {
            for i in 0..w.lm_surfs.len() as u32 {
                mark(i, &mut self.stamp, self.frame);
            }
        } else {
            for (st, surfs) in self.style_surfs.iter().enumerate() {
                if styles[st] != self.styles[st] {
                    for &i in surfs {
                        mark(i, &mut self.stamp, self.frame);
                    }
                }
            }
        }
        self.styles = *styles;
        for i in std::mem::take(&mut self.dlit) {
            mark(i, &mut self.stamp, self.frame);
        }
        let mut lit = Vec::new();
        if !dlights.is_empty() {
            for s in &w.surfaces {
                if s.lm == u32::MAX {
                    continue;
                }
                let hit = dlights.iter().any(|d| {
                    let dist = crate::world::dot(d.origin, s.normal) - s.dist;
                    dist.abs() < d.radius
                        && (0..3).all(|k| d.origin[k] + d.radius >= s.mins[k] && d.origin[k] - d.radius <= s.maxs[k])
                });
                if hit {
                    lit.push(s.lm);
                    mark(s.lm, &mut self.stamp, self.frame);
                }
            }
        }
        let lit_set: std::collections::HashSet<u32> = lit.iter().copied().collect();
        for i in todo {
            let Some(s) = w.lm_surfs.get(i as usize) else { continue };
            let dl: &[DLight] = if lit_set.contains(&i) { dlights } else { &[] };
            build_surface(s, &w.samples, styles, dl, &mut self.scratch);
            let (sw, sh) = (s.size & 0xffff, s.size >> 16);
            let (ax, ay) = (s.atlas & 0xffff, s.atlas >> 16);
            write_rows(queue, &self.tex, 0, [ax, ay, s.tmins[2] as u32], sw, sh, &self.scratch);
        }
        self.dlit = lit;
    }
}
