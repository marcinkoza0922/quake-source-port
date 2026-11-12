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

impl Debris {
    /// Starts simulating corpses that finished dying and new gibs.
    pub(super) fn take_over(&mut self, cl: &mut ClientState, lv: &Level, fs: &qcore::fs::Fs, time: f64) {
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
            let tr = self.tracks.entry(i).or_insert(Track { model, frame, since: time });
            if tr.model != model || tr.frame != frame {
                *tr = Track { model, frame, since: time };
            }
            let held = time - tr.since;
            let e = &cl.entities[i];
            if info.gib {
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
            let ready = (info.is_last(frame) && held >= HOLD_FINAL) || held >= HOLD_ANY;
            if !ready || i <= cl.maxclients || self.corpses.len() >= MAX_RAGDOLLS {
                continue;
            }
            let rig = self.rigs.entry((model, frame)).or_insert_with(|| Rig::build(&info.mdl, frame as usize).map(Rc::new));
            let Some(rig) = rig.clone() else { continue };
            let vel = msg_velocity(cl, e);
            let mut doll = Ragdoll::new(rig, Vec3::from(e.origin), e.angles, vel, &*cm);
            for &(p, t) in &self.booms {
                if time - t < BOOM_MEMORY {
                    doll.blast(p, 220.0, 450.0);
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
    pub(super) fn impacts(&mut self, cl: &ClientState, view: Vec3, time: f64) {
        let mut rng = self.rng.take().unwrap_or(Rng(0x9e37_79b9));
        for &(p, boom) in &cl.shot_impacts {
            let p = Vec3::from(p);
            if boom {
                self.booms.push((p, time));
                for c in &mut self.corpses {
                    if c.doll.gap(p) < DIRECT_HIT {
                        c.gib.get_or_insert((time + GIB_DELAY, p));
                    } else {
                        c.doll.blast(p, 220.0, 520.0);
                    }
                }
                for b in &mut self.bodies {
                    let (dir, d) = (b.pos - p).normalize_len();
                    if d < 200.0 {
                        b.push((dir + Vec3::new(0.0, 0.0, 0.5)) * ((1.0 - d / 200.0) * 500.0), &mut rng);
                    }
                }
                continue;
            }
            let dir = (p - view).normalize();
            for c in &mut self.corpses {
                if let Some((gap, i)) = c.doll.ray_gap(view, p)
                    && gap < 0.0
                {
                    let at = c.doll.pos[i];
                    c.doll.push(at, 20.0, dir * 260.0 + Vec3::new(0.0, 0.0, 40.0));
                }
            }
            for b in &mut self.bodies {
                if segment_dist(view, p, b.pos) < b.radius + 6.0 {
                    b.push(dir * 200.0 + Vec3::new(0.0, 0.0, 60.0), &mut rng);
                }
            }
        }
        self.rng = Some(rng);
        self.booms.retain(|b| time - b.1 < BOOM_MEMORY);
    }

    /// Gibs ragdolls hit directly by an explosion (unless the server gibbed
    /// them meanwhile, which releases the ragdoll first).
    pub(super) fn gib_pending(&mut self, cl: &mut ClientState, lv: &mut Level, fs: &qcore::fs::Fs, r: &mut Renderer, time: f64) {
        if !self.corpses.iter().any(|c| c.gib.is_some_and(|g| g.0 <= time)) {
            return;
        }
        let gib_ids: Vec<ModelId> = self
            .gib_models
            .get_or_insert_with(|| {
                ["progs/gib1.mdl", "progs/gib2.mdl", "progs/gib3.mdl"].iter().filter_map(|n| lv.named(fs, r, n)).map(|m| m.id).collect()
            })
            .clone();
        let mut rng = self.rng.take().unwrap_or(Rng(0x9e37_79b9));
        let mut k = 0;
        while k < self.corpses.len() {
            let Some((_, center)) = self.corpses[k].gib.filter(|g| g.0 <= time) else {
                k += 1;
                continue;
            };
            let c = self.corpses.swap_remove(k);
            let frame = cl.entities[c.ent].frame;
            self.gibbed.insert(c.ent, (c.model, frame));
            let mid = c.doll.pos.iter().fold(Vec3::ZERO, |s, p| s + *p) / c.doll.pos.len() as f32;
            cl.events.push(qclient::ClientEvent::TempSound { name: "player/udeath.wav", origin: mid.to_array() });
            for (n, p) in c.doll.pos.iter().enumerate() {
                cl.run_particle_effect(p.to_array(), [0.0; 3], BLOOD, 24);
                if gib_ids.is_empty() || n >= 6 {
                    continue;
                }
                let out = (*p - center).normalize() + Vec3::new(rng.signed(), rng.signed(), rng.signed()) * 0.5;
                let vel = out.normalize() * (200.0 + rng.unit() * 250.0) + Vec3::new(0.0, 0.0, 200.0 + rng.unit() * 150.0);
                let mut b = Body::new(gib_ids[n % gib_ids.len()], *p, vel, Quat::IDENTITY, 3.0);
                b.life = 10.0 + rng.unit() * 10.0;
                b.trail = true;
                b.random_spin(&mut rng, 10.0);
                self.bodies.push(b);
            }
        }
        self.rng = Some(rng);
    }
}
