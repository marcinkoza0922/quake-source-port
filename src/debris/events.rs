//! Debris events: taking over corpses and gibs, breaking boxes into shards,
//! pushes from shots and explosions, and client-side gibbing.
use super::*;

fn msg_velocity(cl: &ClientState, e: &qclient::ClientEntity) -> Vec3 {
    let dt = cl.mtime[0] - cl.mtime[1];
    let d = Vec3::from(e.msg_origins[0]) - Vec3::from(e.msg_origins[1]);
    if !(0.001..0.5).contains(&dt) || [d.x, d.y, d.z].iter().any(|c| c.abs() > 100.0) {
        return Vec3::ZERO;
    }
    d / dt as f32
}

fn is_explosive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["expl", "exbox", "barrel"].iter().any(|k| n.contains(k))
}

fn inside(p: Vec3, lo: Vec3, hi: Vec3, pad: f32) -> bool {
    (0..3).all(|k| p[k] >= lo[k] - pad && p[k] <= hi[k] + pad)
}

fn segment_dist(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let (dir, len) = (b - a).normalize_len();
    let t = (p - a).dot(dir).clamp(0.0, len);
    (a + dir * t - p).length()
}

/// Killing-hit strength (in shotgun pellets) at or above which the mixed
/// ragdoll start always goes limp at death.
const MIXED_HARD: f32 = 6.0;
/// A point-blank explosion counts as this many pellets.
const BOOM_IMPACT: f32 = 12.0;

/// Mixed start: chance that a kill of strength `impact` plays the death
/// animation; it grows the further the hit falls below `MIXED_HARD`.
pub(super) fn animation_chance(impact: f32) -> f32 {
    (1.0 - impact / MIXED_HARD).clamp(0.0, 1.0)
}

impl Debris {
    /// Strength of the hit that just killed the monster at `origin`: the
    /// pellets of the latest volley passing near it plus nearby explosions.
    fn kill_impact(&self, origin: Vec3, time: f64) -> f32 {
        let body = origin + Vec3::new(0.0, 0.0, 8.0);
        let near = |s: &&(Vec3, Vec3, f64)| time - s.2 < SHOT_MEMORY && segment_dist(s.0, s.1, body) < 40.0;
        let last = self.shots.iter().filter(near).map(|s| s.2).max_by(f64::total_cmp);
        let pellets = last.map_or(0, |t| self.shots.iter().filter(near).filter(|s| s.2 == t).count()) as f32;
        let booms: f32 = self
            .booms
            .iter()
            .filter(|b| time - b.1 < BOOM_MEMORY)
            .map(|b| (1.0 - (b.0 - body).length() / 220.0).max(0.0) * BOOM_IMPACT)
            .sum();
        pellets + booms
    }

    /// Starts simulating corpses that finished dying and new gibs.
    pub(super) fn take_over(&mut self, cl: &mut ClientState, lv: &Level, fs: &qcore::fs::Fs, time: f64, set: &Settings) {
        let Some(cm) = self.cm.clone() else { return };
        let mut gibs = self.bodies.iter().filter(|b| b.ent.is_some()).count();
        for k in 0..cl.visedicts.len() {
            let i = cl.visedicts[k];
            if cl.debris_owned.get(i).copied().unwrap_or(true) {
                continue;
            }
            let (model, frame) = (cl.entities[i].model, cl.entities[i].frame);
            let Some(m) = lv.models.get(model).copied().flatten() else { continue };
            if m.kind != Kind::Alias {
                continue;
            }
            let Some(info) = self.info(cl, fs, model) else { continue };
            if !info.gib && !info.is_death(frame) {
                continue;
            }
            let tr = self.tracks.entry(i).or_insert(Track { model, frame, since: time, limp: None });
            if tr.model != model || tr.frame != frame {
                *tr = Track { model, frame, since: time, limp: tr.limp.filter(|_| tr.model == model) };
            }
            let (held, limp) = (time - tr.since, tr.limp);
            let e = &cl.entities[i];
            if info.gib {
                if !set.gibs {
                    continue;
                }
                // wait for a second update so the throw velocity is known
                if gibs >= MAX_GIBS || (e.msg_origins[0] == e.msg_origins[1] && held < 0.3) {
                    continue;
                }
                let vel = msg_velocity(cl, e);
                let rot = mat_to_quat(&entity_rotation(e.angles));
                let mut b = Body::new(m.id, Vec3::from(e.origin), vel, rot, 3.0);
                b.frame = frame.max(0) as u32;
                b.skin = e.skin.max(0) as u32;
                b.trail = true;
                b.ent = Some((i, model));
                if vel.length() > 50.0 {
                    b.random_spin(self.rng(), 9.0);
                }
                self.bodies.push(b);
                gibs += 1;
                cl.debris_owned[i] = true;
                continue;
            }
            let at_death = match set.start {
                Start::Animation => false,
                Start::AtDeath => true,
                Start::Mixed => match limp {
                    Some(l) => l,
                    None => {
                        let l = self.rng().unit() >= animation_chance(self.kill_impact(Vec3::from(e.origin), time));
                        self.tracks.get_mut(&i).unwrap().limp = Some(l);
                        l
                    }
                },
            };
            let e = &cl.entities[i];
            let ready = at_death || (info.is_last(frame) && held >= HOLD_FINAL) || held >= HOLD_ANY;
            if !set.ragdolls || !ready || i <= cl.maxclients || self.corpses.len() >= set.max_ragdolls {
                continue;
            }
            let rig = self.rigs.entry((model, frame)).or_insert_with(|| Rig::build(&info.mdl, frame as usize).map(Rc::new));
            let Some(rig) = rig.clone() else { continue };
            let vel = msg_velocity(cl, e);
            let mut doll = Ragdoll::new(rig, Vec3::from(e.origin), e.angles, vel, &*cm);
            for &(p, t) in &self.booms {
                if time - t < BOOM_MEMORY {
                    doll.blast(p, 220.0, 450.0 * set.force);
                }
            }
            // the killing hit: the latest volley that strikes the body
            let hits = |&&(from, p, t): &&(Vec3, Vec3, f64)| time - t < SHOT_MEMORY && doll.ray_gap(from, p).is_some_and(|(g, _)| g < 12.0);
            if let Some(last) = self.shots.iter().filter(hits).map(|s| s.2).max_by(f64::total_cmp) {
                let volley: Vec<_> = self.shots.iter().filter(hits).filter(|s| s.2 == last).copied().collect();
                for (from, p, _) in volley {
                    doll.shot(from, p, set.force);
                    doll.fling(from, p, set.force, set.kill_lift);
                }
            }
            let colormap = (e.colormap > 0 && (e.colormap as usize) <= cl.scores.len()).then(|| {
                let c = cl.scores[e.colormap as usize - 1].colors;
                [((c >> 4) & 15) as u8, (c & 15) as u8]
            });
            let alpha = if e.alpha == qnet::protocol::ENTALPHA_DEFAULT { 1.0 } else { (e.alpha as f32 - 1.0) / 254.0 };
            self.corpses.push(Corpse { ent: i, model, id: m.id, skin: e.skin.max(0) as u32, colormap, alpha, doll, gib: None });
            cl.debris_owned[i] = true;
        }
        let (ents, mtime) = (&cl.entities, cl.mtime[0]);
        self.tracks.retain(|&i, t| ents.get(i).is_some_and(|e| e.model == t.model && e.msgtime == mtime));
    }

    fn brush_box(&mut self, cl: &ClientState, fs: &qcore::fs::Fs, model: usize) -> Option<(Vec3, Vec3)> {
        *self.brush_bounds.entry(model).or_insert_with(|| {
            let name = cl.model_name(model).filter(|n| n.ends_with(".bsp") && is_explosive(n))?;
            let b = qformats::Bsp::parse(&fs.open(name)?).ok()?;
            let m = b.models.first()?;
            Some((Vec3::from(m.mins), Vec3::from(m.maxs)))
        })
    }

    /// Exploding boxes break into metal shards: one that turns into another
    /// model (id1 becomes the explosion sprite) has exploded; one that just
    /// vanishes only counts next to an explosion, since leaving the PVS
    /// also removes it.
    pub(super) fn brush_breaks(&mut self, cl: &ClientState, lv: &Level, fs: &qcore::fs::Fs, r: &mut Renderer) {
        let mut cur = HashMap::with_capacity(self.brushes.len());
        for &i in &cl.visedicts {
            let e = &cl.entities[i];
            if lv.models.get(e.model).copied().flatten().is_none_or(|m| m.kind != Kind::Brush) {
                continue;
            }
            if let Some((lo, hi)) = self.brush_box(cl, fs, e.model) {
                let o = Vec3::from(e.origin);
                cur.insert(i, (e.model, o + lo, o + hi));
            }
        }
        let prev = std::mem::replace(&mut self.brushes, cur);
        for (ent, (model, lo, hi)) in prev {
            if self.brushes.get(&ent).is_some_and(|b| b.0 == model) {
                continue;
            }
            let e = &cl.entities[ent];
            let changed = e.msgtime == cl.mtime[0] && e.model != 0 && e.model != model;
            let boom = self
                .booms
                .iter()
                .map(|b| b.0)
                .chain(cl.shot_impacts.iter().filter(|i| i.1).map(|i| Vec3::from(i.0)))
                .find(|&b| inside(b, lo, hi, 48.0));
            if changed || boom.is_some() {
                let below = (lo + hi) * 0.5 - Vec3::new(0.0, 0.0, (hi.z - lo.z) * 0.4);
                self.shatter(r, lo, hi, boom.unwrap_or(below));
            }
        }
    }

    fn shatter(&mut self, r: &mut Renderer, lo: Vec3, hi: Vec3, center: Vec3) {
        let id = match self.shard {
            Some(id) => id,
            None => {
                let Ok(id) = r.load_alias(&fragment::shard_model()) else { return };
                self.shard = Some(id);
                id
            }
        };
        let mut rng = self.rng.take().unwrap_or(Rng(0x9e37_79b9));
        for _ in 0..14 {
            let p = Vec3::new(
                lo.x + (hi.x - lo.x) * rng.unit(),
                lo.y + (hi.y - lo.y) * rng.unit(),
                lo.z + (hi.z - lo.z) * rng.unit(),
            );
            let out = (p - center).normalize() + Vec3::new(rng.signed(), rng.signed(), rng.signed()) * 0.4;
            let vel = out.normalize() * (250.0 + rng.unit() * 250.0) + Vec3::new(0.0, 0.0, 120.0 + rng.unit() * 200.0);
            let rot = Quat::axis_angle(Vec3::new(rng.signed(), rng.signed(), rng.signed()).normalize(), rng.unit() * 6.0);
            let mut b = Body::new(id, p, vel, rot, 1.2);
            b.frame = rng.next() % fragment::SHAPES;
            b.life = 8.0 + rng.unit() * 4.0;
            b.random_spin(&mut rng, 18.0);
            self.bodies.push(b);
        }
        self.rng = Some(rng);
        // drop the oldest shards over the cap
        let shards = self.bodies.iter().filter(|b| b.ent.is_none()).count();
        if shards > MAX_SHARDS {
            let mut lives: Vec<f32> = self.bodies.iter().filter(|b| b.ent.is_none()).map(|b| b.life).collect();
            lives.sort_by(f32::total_cmp);
            let cut = lives[shards - MAX_SHARDS];
            self.bodies.retain(|b| b.ent.is_some() || b.life >= cut);
        }
    }

    /// Bullets and nails nudge what they pass through; explosions fling
    /// everything nearby and gib ragdolls they land on.
    pub(super) fn impacts(&mut self, cl: &ClientState, view: Vec3, time: f64, set: &Settings) {
        let force = set.force;
        let mut rng = self.rng.take().unwrap_or(Rng(0x9e37_79b9));
        for &(p, boom) in &cl.shot_impacts {
            let p = Vec3::from(p);
            if boom {
                self.booms.push((p, time));
                for c in &mut self.corpses {
                    if set.gibs && c.doll.gap(p) < DIRECT_HIT {
                        c.gib.get_or_insert((time + GIB_DELAY, p));
                    } else {
                        c.doll.blast(p, 220.0, 520.0 * force);
                    }
                }
                for b in &mut self.bodies {
                    let (dir, d) = (b.pos - p).normalize_len();
                    if d < 200.0 {
                        b.push((dir + Vec3::new(0.0, 0.0, 0.5)) * ((1.0 - d / 200.0) * 500.0 * force), &mut rng);
                    }
                }
                continue;
            }
            let from = self.shot_origin(view, p);
            let dir = (p - from).normalize();
            self.shots.push((from, p, time));
            for c in &mut self.corpses {
                if c.doll.ray_gap(from, p).is_some_and(|(gap, _)| gap < 0.0) {
                    c.doll.shot(from, p, force);
                }
            }
            for b in &mut self.bodies {
                if segment_dist(from, p, b.pos) < b.radius + 6.0 {
                    b.push((dir * 200.0 + Vec3::new(0.0, 0.0, 60.0)) * force, &mut rng);
                }
            }
        }
        // hits that drew blood: on a living body, which may die this frame
        for &p in &cl.blood_hits {
            let p = Vec3::from(p);
            let from = self.shot_origin(view, p);
            self.shots.push((from, p, time));
        }
        self.rng = Some(rng);
        self.booms.retain(|b| time - b.1 < BOOM_MEMORY);
        self.shots.retain(|s| time - s.2 < SHOT_MEMORY);
    }
}
