//! Optional client-side surface-aware footsteps (`s_footsteps`, `s_footsteps_volume`).
//! Detects grounded movement and landings per local player (including split-screen),
//! classifies the floor texture via `hitfx::classify`, and plays CC0 recorded cues.

use std::sync::Arc;

use qclient::ClientState;
use qcore::math::Vec3;
use qnet::protocol::STAT_HEALTH;
use qpart::World;
use qsound::{FootstepSurface, Sfx, SoundEvent, SoundSystem, footsteps};

use crate::assets::Level;
use crate::fte::world::{BspWorld, BrushEnt, cached_world};
use crate::hitfx::Material;
use crate::splitscreen::Guest;

const MAX_LOCAL_PLAYERS: usize = 6;
const STEP_DISTANCE: f32 = 82.0;
const MIN_STEP_INTERVAL: f64 = 0.22;
const MIN_LAND_INTERVAL: f64 = 0.16;
const LAND_VZ_THRESHOLD: f32 = -190.0;
const FEET_OFFSET: f32 = 24.0;

#[derive(Default)]
struct PlayerStep {
    prev_origin: Option<Vec3>,
    prev_time: f64,
    dist: f32,
    was_grounded: bool,
    air_min_vz: f32,
    step_count: usize,
    last_step_t: f64,
}

struct CueBank {
    rate: u32,
    steps: [[Arc<Sfx>; footsteps::VARIANTS]; 7],
    lands: [Arc<Sfx>; 7],
}

impl CueBank {
    fn new(rate: u32) -> Self {
        let steps = FootstepSurface::ALL.map(|surf| {
            std::array::from_fn(|v| Arc::new(footsteps::render_step(surf, v, rate)))
        });
        let lands = FootstepSurface::ALL.map(|surf| Arc::new(footsteps::render_land(surf, rate)));
        Self { rate, steps, lands }
    }

    fn step(&self, surf: FootstepSurface, idx: usize) -> Arc<Sfx> {
        self.steps[surf.index()][idx % footsteps::VARIANTS].clone()
    }

    fn land(&self, surf: FootstepSurface) -> Arc<Sfx> {
        self.lands[surf.index()].clone()
    }
}

#[derive(Default)]
pub struct Footsteps {
    bank: Option<CueBank>,
    players: [PlayerStep; MAX_LOCAL_PLAYERS],
}

impl Footsteps {
    pub fn clear_map(&mut self) {
        self.players = Default::default();
    }

    fn ensure_bank(&mut self, rate: u32) -> &CueBank {
        if self.bank.as_ref().is_none_or(|b| b.rate != rate) {
            self.bank = Some(CueBank::new(rate));
        }
        self.bank.as_ref().unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        cl: &ClientState,
        guests: &[Guest],
        level: &Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        volume_scale: f32,
        now: f64,
        snd: &SoundSystem,
    ) {
        let Some(world) = cached_world(cl, level, brushes) else { return };
        let rate = snd.rate();
        self.ensure_bank(rate);

        self.step_player(0, cl, &world, volume_scale, now, true, snd);
        for (i, g) in guests.iter().take(MAX_LOCAL_PLAYERS - 1).enumerate() {
            self.step_player(i + 1, &g.cl, &world, volume_scale, now, false, snd);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn step_player(
        &mut self,
        slot: usize,
        cl: &ClientState,
        world: &BspWorld<'_>,
        volume_scale: f32,
        now: f64,
        is_primary: bool,
        snd: &SoundSystem,
    ) {
        let p = &mut self.players[slot];
        if cl.signon != 4 || cl.intermission != 0 || cl.stats[STAT_HEALTH] <= 0 {
            p.prev_origin = None;
            p.dist = 0.0;
            p.was_grounded = false;
            p.air_min_vz = 0.0;
            return;
        }
        let Some(ent) = cl.entities.get(cl.viewentity) else { return };
        let origin = Vec3::from_array(ent.origin);

        let Some(prev) = p.prev_origin else {
            p.prev_origin = Some(origin);
            p.prev_time = now;
            p.was_grounded = detect_floor(world, origin).is_some();
            return;
        };

        let dt = (now - p.prev_time).clamp(0.0, 0.25) as f32;
        p.prev_origin = Some(origin);
        p.prev_time = now;
        if dt <= 1e-4 {
            return;
        }

        let delta = origin - prev;
        let dh = (delta.x * delta.x + delta.y * delta.y).sqrt();
        // Skip teleports / respawn snaps.
        if dh > 120.0 || delta.z.abs() > 160.0 {
            p.dist = 0.0;
            p.air_min_vz = 0.0;
            p.was_grounded = detect_floor(world, origin).is_some();
            return;
        }

        let vz = delta.z / dt;
        let floor = detect_floor(world, origin);
        let grounded = floor.is_some();

        let bank = self.bank.as_ref().unwrap();
        let feet = [origin.x, origin.y, origin.z - FEET_OFFSET];
        let ent_id = cl.viewentity as i32;
        let atten = if is_primary { 0.0 } else { 1.2 };

        if !p.was_grounded && grounded {
            let surf = floor.unwrap();
            if p.air_min_vz <= LAND_VZ_THRESHOLD && (now - p.last_step_t) >= MIN_LAND_INTERVAL {
                let impact = ((-p.air_min_vz - 150.0) / 380.0).clamp(0.45, 1.0);
                let vol = (volume_scale * 0.65 * impact).clamp(0.0, 1.0);
                snd.event(SoundEvent::Start {
                    entity: ent_id,
                    channel: 0,
                    sfx: bank.land(surf),
                    origin: feet,
                    volume: vol,
                    attenuation: atten,
                });
                p.last_step_t = now;
                p.dist = 0.0;
            }
            p.air_min_vz = 0.0;
        } else if !grounded {
            p.air_min_vz = p.air_min_vz.min(vz);
        } else if let Some(surf) = floor {
            let speed = dh / dt;
            if speed > 42.0 {
                p.dist += dh;
                // Slightly tighten cadence when running at full speed.
                let cadence = if speed > 260.0 { STEP_DISTANCE * 0.94 } else { STEP_DISTANCE };
                if p.dist >= cadence && (now - p.last_step_t) >= MIN_STEP_INTERVAL {
                    let vol = (volume_scale * 0.48 * (speed / 280.0).clamp(0.55, 1.0)).clamp(0.0, 1.0);
                    let sfx = bank.step(surf, p.step_count);
                    snd.event(SoundEvent::Start {
                        entity: ent_id,
                        channel: 0,
                        sfx,
                        origin: feet,
                        volume: vol,
                        attenuation: atten,
                    });
                    p.step_count = p.step_count.wrapping_add(1);
                    p.dist = 0.0;
                    p.last_step_t = now;
                }
            }
        }
        p.was_grounded = grounded;
    }
}

/// Returns the floor material under `origin` if the player is grounded or wading.
fn detect_floor(world: &BspWorld<'_>, origin: Vec3) -> Option<FootstepSurface> {
    let feet = Vec3::new(origin.x, origin.y, origin.z - FEET_OFFSET + 2.0);
    // Wading in liquid counts as liquid footsteps (skip deep swimming when eye is also submerged).
    let feet_c = world.point_contents(feet);
    if feet_c == qpart::FTECONTENTS_WATER
        || feet_c == qpart::FTECONTENTS_SLIME
        || feet_c == qpart::FTECONTENTS_LAVA
    {
        let chest = Vec3::new(origin.x, origin.y, origin.z + 8.0);
        let chest_c = world.point_contents(chest);
        if chest_c == qpart::FTECONTENTS_EMPTY {
            return Some(match feet_c {
                qpart::FTECONTENTS_SLIME => FootstepSurface::Slime,
                qpart::FTECONTENTS_LAVA => FootstepSurface::Lava,
                _ => FootstepSurface::Water,
            });
        }
        // Fully submerged swimming: no boot footsteps.
        return None;
    }

    let mut best: Option<qpart::Trace> = None;
    for (dx, dy) in [(0.0, 0.0), (10.0, 10.0), (-10.0, 10.0), (10.0, -10.0), (-10.0, -10.0)] {
        let start = Vec3::new(origin.x + dx, origin.y + dy, origin.z - FEET_OFFSET + 8.0);
        let end = Vec3::new(origin.x + dx, origin.y + dy, origin.z - FEET_OFFSET - 5.0);
        let tr = world.trace_line(start, end);
        if tr.frac < 1.0 && tr.normal.z >= 0.7 {
            if dx == 0.0 && dy == 0.0 {
                best = Some(tr);
                break;
            }
            if best.is_none_or(|b| tr.frac < b.frac) {
                best = Some(tr);
            }
        }
    }
    let tr = best?;
    let mat = crate::impacts::surfaces::texture_at(world, &tr)
        .map_or(Material::Stone, |t| crate::hitfx::classify(&t.name));
    Some(match mat {
        Material::Metal => FootstepSurface::Metal,
        Material::Wood => FootstepSurface::Wood,
        Material::Dirt => FootstepSurface::Dirt,
        Material::Water => FootstepSurface::Water,
        Material::Slime => FootstepSurface::Slime,
        Material::Lava => FootstepSurface::Lava,
        Material::Stone | Material::None => FootstepSurface::Stone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_quake_metallic_and_dirt_floor_textures() {
        for name in [
            "sfloor4_2", "sfloor4_5", "sfloor1_1", "ecop1_1", "cop1_1", "wizmet1_2", "wmet4_8",
            "met5_1", "metflor1_2", "metflr1_1", "twall2_6", "tech04_1", "+0basebtn", "+0floorsw",
        ] {
            assert_eq!(crate::hitfx::classify(name), Material::Metal, "{name}");
        }
        for name in ["ground1_2", "ground1_6", "wgrnd1_6"] {
            assert_eq!(crate::hitfx::classify(name), Material::Dirt, "{name}");
        }
        for name in ["wizwood1_5", "woodflr1_4", "crate0_top", "door03_2"] {
            assert_eq!(crate::hitfx::classify(name), Material::Wood, "{name}");
        }
        for name in ["uwall1_2", "city4_7", "rock4_1", "azfloor1_1", "wbrick1_5"] {
            assert_eq!(crate::hitfx::classify(name), Material::Stone, "{name}");
        }
    }

    #[test]
    fn detects_metal_floor_at_e1m1_spawn() {
        let dir = std::env::var_os("QUAKE_BASEDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vkQuake directory with maps"));
        if !dir.join("id1").is_dir() {
            return;
        }
        let mut fs = qcore::fs::Fs::new(&dir);
        fs.add_game_dir("id1");
        let bytes = fs.open("maps/e1m1.bsp").expect("e1m1.bsp");
        let bsp = qformats::Bsp::parse(&bytes).expect("parse e1m1");
        let cm = qworld::CollisionModel::from_bsp(&bsp);
        let world = BspWorld { bsp: &bsp, cm: &cm, ents: &[] };
        // E1M1 info_player_start is at (480, -352, 88), standing on the sfloor4_2 riveted metal floor.
        let surf = detect_floor(&world, Vec3::new(480.0, -352.0, 88.0));
        assert_eq!(surf, Some(FootstepSurface::Metal));
    }
}
