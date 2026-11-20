use super::*;
use qformats::Bsp;
use qserver::Server;

#[derive(Default)]
pub(super) struct HullCache {
    size: usize,
    polygons: Vec<Vec<Vec3>>,
}

impl BBoxes {
    pub(crate) fn clear_map(&mut self) {
        self.pointfile.clear();
        self.hull = HullCache::default();
    }

    pub(crate) fn volumes(
        &mut self,
        server: &Server,
        bsp: Option<&Bsp>,
        eye: Vec3,
        r: &mut Renderer,
        out: &mut Geometry,
    ) {
        let mode = (server.core.cvars.value_of("r_showclip") as usize).min(2);
        let triggers = server.core.cvars.value_of("r_showtriggers") != 0.0;
        if mode == 0 && !triggers {
            return;
        }
        let Some(pic) = self
            .pic
            .or_else(|| r.register_pic_rgba_linear(1, 1, &[255; 4]).ok())
        else {
            return;
        };
        self.pic = Some(pic);
        let first = out.1.len() as u32;
        if let Some(bsp) = bsp.filter(|_| mode != 0) {
            if self.hull.size != mode {
                self.hull = HullCache {
                    size: mode,
                    polygons: super::hull::polygons(bsp, mode),
                };
            }
            let frustum = 1536.0f32.powi(2);
            for poly in &self.hull.polygons {
                let (mins, maxs) = poly
                    .iter()
                    .fold((poly[0], poly[0]), |(lo, hi), &p| (lo.min(p), hi.max(p)));
                let nearest = eye.max(mins).min(maxs);
                if (nearest - eye).length_sq() < frustum {
                    fill(out, poly, [70, 195, 255, 66]);
                }
            }
        }
        let triggers = if triggers {
            server.trigger_boxes()
        } else {
            Vec::new()
        };
        for &(mins, maxs, secret) in &triggers {
            let color = if secret {
                [216, 100, 250, 75]
            } else {
                [255, 166, 42, 64]
            };
            let v: [Vec3; 8] = std::array::from_fn(|i| corner(mins, maxs, i));
            for face in [
                [0, 1, 3, 2],
                [4, 6, 7, 5],
                [0, 4, 5, 1],
                [2, 3, 7, 6],
                [0, 2, 6, 4],
                [1, 5, 7, 3],
            ] {
                fill(out, &face.map(|i| v[i]), color);
            }
        }
        batch(out, pic, first, false);
        let first = out.1.len() as u32;
        for (mins, maxs, secret) in triggers {
            let color = if secret {
                [216, 100, 250, 225]
            } else {
                [255, 166, 42, 215]
            };
            wire_box(out, mins, maxs, color);
        }
        batch(out, pic, first, true);
    }
}

fn fill(out: &mut Geometry, poly: &[Vec3], color: [u8; 4]) {
    let first = out.0.len() as u32;
    out.0.extend(poly.iter().map(|p| FteVertex {
        pos: p.to_array(),
        uv: [0.5, 0.5],
        color,
    }));
    for i in 1..poly.len() as u32 - 1 {
        out.1.extend_from_slice(&[first, first + i, first + i + 1]);
    }
}

fn batch(out: &mut Geometry, pic: PicId, first_index: u32, lines: bool) {
    let num_indices = out.1.len() as u32 - first_index;
    if num_indices != 0 {
        out.2.push(FteBatch {
            pic,
            blend: FteBlend::Blend,
            lines,
            first_index,
            num_indices,
            soft: false,
            gel: false,
        });
    }
}
