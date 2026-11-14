//! Client presentation of the gel sprayer: splats merged into one
//! contiguous coat per surface (`gel_coat.rs`) clipped onto BSP faces like
//! blood marks, blobs flying (simulated here from the launch the
//! server sends), all drawn as gel-shaded FTE batches (`FteBatch::gel`:
//! fte.frag mode 9 turns the atlas alpha into a raised, glossy surface),
//! plus the synthesised squirts, splats, stretches, boings and zips.
use crate::blood::mesh;
use crate::fte::world::{BspWorld, brush_ents};
use qcore::math::{Vec3, angle_vectors};
use qpart::World;
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, Renderer, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

const TILE: usize = 128;
const ATLAS: usize = TILE * 2;
/// Atlas tile of the round blob in flight (0..=2 are splat shapes).
pub const DOME: usize = 3;
const BLOB_SIZE: f32 = 4.5;
/// Sound channel of the spray loop on the owner's entity.
const LOOP_CHANNEL: i32 = 7;

pub use super::gel_coat::COLORS;
/// Seconds between coat rebuilds while splats keep changing.
const REBUILD: f64 = 0.1;

struct Splat {
    id: u32,
    kind: u8,
    origin: Vec3,
    normal: Vec3,
    radius: f32,
}

struct Blob {
    kind: u8,
    pos: Vec3,
    vel: Vec3,
    born: f64,
}

#[derive(Default)]
pub struct GelFx {
    splats: Vec<Splat>,
    blobs: Vec<Blob>,
    /// Last blob time per owner (drives the spray loop).
    spraying: Vec<f64>,
    last_splat_sound: f64,
    time: f64,
    pic: Option<PicId>,
    /// The coat atlas and its polygons (world position, atlas uv, light),
    /// rebuilt when `dirty` (splats changed).
    coat_pic: Option<PicId>,
    coat: Vec<Vec<FteVertex>>,
    dirty: bool,
    built: f64,
    cm: Option<(String, qworld::CollisionModel)>,
    rate: u32,
    cues: Vec<Arc<Sfx>>,
}

fn nums(s: &str) -> Vec<f32> {
    s.split_whitespace().filter_map(|w| w.parse().ok()).filter(|v: &f32| v.is_finite()).collect()
}

impl GelFx {
    pub fn clear(&mut self) {
        self.splats.clear();
        self.blobs.clear();
        self.coat.clear();
        self.dirty = false;
        self.cm = None;
    }

    fn cue(&mut self, snd: &SoundSystem, k: usize) -> Arc<Sfx> {
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..qsound::gel::COUNT).map(|k| Arc::new(qsound::gel::render(k, snd.rate()))).collect();
        }
        self.cues[k].clone()
    }

    fn play(&mut self, snd: Option<&SoundSystem>, k: usize, origin: Vec3, volume: f32) {
        let Some(snd) = snd else { return };
        let sfx = self.cue(snd, k);
        snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin: origin.to_array(), volume, attenuation: 1.0 });
    }

    /// One `//qsp_gel` line (without the prefix).
    pub fn receive(&mut self, line: &str, time: f64, snd: Option<&SoundSystem>) {
        let (op, rest) = line.split_once(' ').unwrap_or((line, ""));
        let n = nums(rest);
        match (op, &n[..]) {
            ("c", _) => {
                self.splats.clear();
                self.blobs.clear();
                self.dirty = true;
            }
            ("b", &[o, k, x, y, z, vx, vy, vz]) => {
                let o = o as usize;
                if self.spraying.len() <= o {
                    self.spraying.resize(o + 1, -1.0);
                }
                if let Some(snd) = snd
                    && self.spraying[o] < time - 0.2
                {
                    let sfx = self.cue(snd, qsound::gel::SPRAY);
                    snd.event(SoundEvent::Start { entity: o as i32 + 1, channel: LOOP_CHANNEL, sfx, origin: [x, y, z], volume: 0.45, attenuation: 1.5 });
                }
                self.spraying[o] = time;
                self.blobs.push(Blob { kind: (k as u8).min(1), pos: Vec3::new(x, y, z), vel: Vec3::new(vx, vy, vz), born: time });
            }
            ("a", &[id, k, x, y, z, nx, ny, nz, r]) => {
                let origin = Vec3::new(x, y, z);
                self.splats.retain(|s| s.id != id as u32);
                let kind = (k as u8).min(1);
                // the blob that made it lands now
                if let Some(i) = self.blobs.iter().position(|b| b.kind == kind && (b.pos - origin).length() < 160.0) {
                    self.blobs.remove(i);
                    if time - self.last_splat_sound > 0.07 {
                        self.last_splat_sound = time;
                        self.play(snd, qsound::gel::SPLAT, origin, 0.55);
                    }
                }
                self.splats.push(Splat { id: id as u32, kind, origin, normal: Vec3::new(nx, ny, nz), radius: r.clamp(4.0, 128.0) });
                self.dirty = true;
            }
            ("d", &[id]) => {
                self.splats.retain(|s| s.id != id as u32);
                self.dirty = true;
            }
            ("s", &[k, x, y, z, speed]) => {
                let cue = if k == 0.0 {
                    qsound::gel::STRETCH
                } else {
                    qsound::gel::ZIP + usize::from(speed > 450.0) + usize::from(speed > 750.0)
                };
                self.play(snd, cue, Vec3::new(x, y, z), 0.8);
            }
            ("j", &[_, x, y, z, speed]) => {
                self.play(snd, qsound::gel::BOING, Vec3::new(x, y, z), (0.55 + speed / 2000.0).min(1.0));
            }
            _ => {}
        }
    }

    /// The sprayer view model for the local player.
    pub fn weapon(&self, owner: usize, time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<RenderEntity> {
        let id = super::toy_model::get("@gelsprayer", include_bytes!("../../assets/gel/model.mdl.zlib"), include_bytes!("../../assets/gel/texture.rgb.zlib"), true, lv, fs, r)?;
        let firing = self.spraying.get(owner).is_some_and(|&t| time - t < 0.15);
        let shake = if firing { (time as f32 * 60.0).sin() * 0.25 } else { 0.0 };
        let bob = (time as f32 * 2.5).sin() * 0.15;
        let (f, rt, u) = angle_vectors(view.angles.into());
        let origin = (Vec3::from(view.origin) + f * (24.0 + shake) + rt * 0.0 - u * (21.0 + bob)).to_array();
        // muzzle tipped up a little so the nozzle shows past the canisters
        let angles = [-view.angles[0] - 7.0, view.angles[1], view.angles[2]];
        Some(RenderEntity { model: id, origin, angles, viewmodel: true, ..Default::default() })
    }

    /// Flies blobs, stops spray loops, and adds splats and blobs to `geom`.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, cl: &qclient::ClientState, lv: &crate::assets::Level, r: &mut Renderer, view: &View, gravity: f32, snd: Option<&SoundSystem>, geom: &mut Geometry) {
        let dt = (cl.time - self.time).clamp(0.0, 0.1) as f32;
        self.time = cl.time;
        for (o, t) in self.spraying.iter_mut().enumerate() {
            if *t >= 0.0 && cl.time - *t > 0.12 {
                *t = -1.0;
                if let Some(snd) = snd {
                    snd.event(SoundEvent::Stop { entity: o as i32 + 1, channel: LOOP_CHANNEL });
                }
            }
        }
        if self.splats.is_empty() && self.blobs.is_empty() && self.coat.is_empty() && !self.dirty {
            return;
        }
        let Some(bsp) = lv.bsp.as_ref() else { return };
        if self.cm.as_ref().is_none_or(|(m, _)| *m != lv.mapname) {
            self.cm = Some((lv.mapname.clone(), qworld::CollisionModel::from_bsp(bsp)));
        }
        let Some((_, cm)) = self.cm.as_ref() else { return };
        let brushes = brush_ents(cl, cm);
        let world = BspWorld { bsp, cm, ents: &brushes };
        // blobs fly until they hit something (the server's splat follows)
        let now = cl.time;
        self.blobs.retain_mut(|b| {
            b.vel.z -= gravity * dt;
            let end = b.pos + b.vel * dt;
            let tr = world.trace_line(b.pos, end);
            b.pos = tr.endpos;
            tr.frac >= 1.0 && now - b.born < 3.0
        });
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let Ok(p) = r.register_pic_rgba_linear(ATLAS as u32, ATLAS as u32, &atlas()) else { return };
                self.pic = Some(p);
                p
            }
        };
        let first = geom.1.len() as u32;
        if self.dirty && (now - self.built >= REBUILD || now < self.built) {
            self.dirty = false;
            self.built = now;
            rebuild(&self.splats, &mut self.coat, &mut self.coat_pic, cl, &world, r);
        }
        if let Some(coat_pic) = self.coat_pic.filter(|_| !self.coat.is_empty()) {
            for poly in &self.coat {
                let base = geom.0.len() as u32;
                geom.0.extend_from_slice(poly);
                for i in 1..poly.len() as u32 - 1 {
                    geom.1.extend([base, base + i, base + i + 1]);
                }
            }
            let count = geom.1.len() as u32 - first;
            if count > 0 {
                geom.2.push(FteBatch { pic: coat_pic, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: count, soft: false, gel: true });
            }
        }
        let first = geom.1.len() as u32;
        let (_, vr, vu) = angle_vectors(view.angles.into());
        for b in &self.blobs {
            let c = COLORS[b.kind as usize].map(|v| (v * 255.0) as u8);
            let base = geom.0.len() as u32;
            for (x, y, uv) in [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
                let pos = b.pos + vr * x * BLOB_SIZE + vu * y * BLOB_SIZE;
                geom.0.push(FteVertex { pos: pos.to_array(), uv: tile_uv(uv, DOME), color: [c[0], c[1], c[2], 255] });
            }
            geom.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let count = geom.1.len() as u32 - first;
        if count > 0 {
            geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: count, soft: false, gel: true });
        }
    }
}

/// Rebuilds the coat atlas and its clipped polygons from the splats.
fn rebuild(splats: &[Splat], coat: &mut Vec<Vec<FteVertex>>, coat_pic: &mut Option<PicId>, cl: &qclient::ClientState, world: &BspWorld, r: &mut Renderer) {
    use super::gel_coat::{Disc, build, group};
    coat.clear();
    let discs: Vec<Disc> = splats.iter().map(|s| Disc { kind: s.kind, origin: s.origin, normal: s.normal, radius: s.radius }).collect();
    let groups = group(&discs);
    let (clusters, tex) = build(&discs, &groups);
    let pic = match *coat_pic {
        Some(p) => r.update_pic(p, &tex).ok().map(|_| p),
        None => r.register_pic_rgba_linear(super::gel_coat::ATLAS as u32, super::gel_coat::ATLAS as u32, &tex).ok(),
    };
    let Some(pic) = pic else { return };
    *coat_pic = Some(pic);
    let styles: Vec<i32> = (0..qnet::protocol::MAX_LIGHTSTYLES).map(|i| cl.lightstyle_value(i)).collect();
    for c in clusters.iter().flatten() {
        let light = |p: Vec3| {
            let l = r.light_point((p + c.normal * 4.0).to_array(), &styles);
            (((l[0] + l[1] + l[2]) / 3.0 / 170.0).clamp(0.4, 1.0) * 255.0) as u8
        };
        let mut polys = Vec::new();
        world.decal_polys(0, c.center, c.half * 1.5, c.normal, false, &mut |poly| {
            if poly.iter().any(|p| (*p - c.center).dot(c.normal).abs() > 0.5) {
                return;
            }
            let pts = mesh::project_shaped(poly, c.center, c.normal, c.half, [0.0, 1.0]);
            if pts.len() >= 3 {
                polys.push(pts);
            }
        });
        for pts in polys {
            let poly = pts
                .iter()
                .map(|p| {
                    let l = light(p.pos);
                    FteVertex { pos: (p.pos + c.normal * 0.3).to_array(), uv: c.atlas_uv(p.uv), color: [l, l, l, 245] }
                })
                .collect();
            coat.push(poly);
        }
    }
}

/// `uv` (0..1 across the splat) inside atlas tile `tile`, with a half-texel
/// guard so neighbouring tiles never bleed in.
pub fn tile_uv(uv: [f32; 2], tile: usize) -> [f32; 2] {
    let g = 1.0 / ATLAS as f32;
    let (tx, ty) = ((tile % 2) as f32 * 0.5, (tile / 2) as f32 * 0.5);
    [tx + g + uv[0].clamp(0.0, 1.0) * (0.5 - 2.0 * g), ty + g + uv[1].clamp(0.0, 1.0) * (0.5 - 2.0 * g)]
}

/// 2×2 atlas: white RGB, alpha = gel height. Tiles 0-2 are irregular
/// splats (a main pool, lobes and satellite drops from metaballs), tile 3 a
/// round dome for blobs in flight.
pub fn atlas() -> Vec<u8> {
    let mut out = vec![255u8; ATLAS * ATLAS * 4];
    let mut seed = 0x9e1u32;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / 16_777_216.0
    };
    for tile in 0..4 {
        let mut balls: Vec<(f32, f32, f32)> = vec![(0.0, 0.0, if tile == DOME { 0.62 } else { 0.36 })];
        if tile != DOME {
            for _ in 0..7 {
                let a = rnd() * std::f32::consts::TAU;
                let d = 0.2 + rnd() * 0.28;
                balls.push((a.cos() * d, a.sin() * d, 0.1 + rnd() * 0.16));
            }
            for _ in 0..5 {
                let a = rnd() * std::f32::consts::TAU;
                let d = 0.55 + rnd() * 0.2;
                balls.push((a.cos() * d, a.sin() * d, 0.04 + rnd() * 0.05));
            }
        }
        for j in 0..TILE {
            for i in 0..TILE {
                let (x, y) = ((i as f32 + 0.5) / TILE as f32 * 2.0 - 1.0, (j as f32 + 0.5) / TILE as f32 * 2.0 - 1.0);
                let field: f32 = balls.iter().map(|&(bx, by, r)| r * r / ((x - bx).powi(2) + (y - by).powi(2) + 1e-4)).sum();
                // a rounded top: steep rim, gentle dome
                // ripples so the surface is not a perfect dome
                let ripple = ((x * 5.3 + y * 2.1 + tile as f32).sin() * (y * 4.7 - x * 1.3).cos()) * 0.06;
                let h = ((field - 1.0) / 2.5).clamp(0.0, 1.0);
                let rim = ((field - 0.8) / 0.2).clamp(0.0, 1.0);
                let h = rim * rim * (3.0 - 2.0 * rim) * 0.2 + (1.0 - (1.0 - h).powi(3)) * 0.75 + if field > 1.0 { ripple } else { 0.0 };
                let h = h.clamp(0.0, 1.0);
                let fade = (1.0 - (x * x + y * y).sqrt()).clamp(0.0, 0.1) * 10.0;
                let px = ((tile / 2) * TILE + j) * ATLAS + (tile % 2) * TILE + i;
                out[px * 4 + 3] = (h * fade * 255.0) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_tiles_have_gel_with_a_clear_border() {
        let a = atlas();
        for tile in 0..4 {
            let at = |i: usize, j: usize| a[(((tile / 2) * TILE + j) * ATLAS + (tile % 2) * TILE + i) * 4 + 3];
            let covered = (0..TILE * TILE).filter(|k| at(k % TILE, k / TILE) > 0).count();
            assert!(covered > TILE * TILE / 6, "tile {tile}: {covered}");
            assert!((0..TILE).all(|k| at(k, 0) == 0 && at(0, k) == 0), "tile {tile} edge");
            assert!(at(TILE / 2, TILE / 2) > 200);
        }
    }

    #[test]
    fn splat_lines_add_and_remove() {
        let mut g = GelFx::default();
        g.receive("a 3 1 0 0 0 0 0 1 30", 0.0, None);
        g.receive("a 4 0 50 0 0 0 0 1 30", 0.0, None);
        assert_eq!(g.splats.len(), 2);
        g.receive("d 3", 0.0, None);
        assert_eq!(g.splats.len(), 1);
        g.receive("b 0 1 0 0 0 100 0 0", 0.0, None);
        assert_eq!(g.blobs.len(), 1);
        g.receive("c", 0.0, None);
        assert!(g.splats.is_empty() && g.blobs.is_empty());
    }
}
