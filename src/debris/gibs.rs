//! Client-side gibbing of ragdolls hit directly by an explosion. Pieces
//! come from the game's own gib models (any precached model with the gib
//! flag, preferring ones named after the monster) plus the monster's head
//! when one exists, falling back to the stock id1 gibs.
use super::*;

/// Heads whose names do not follow `h_<model>`.
const HEADS: [(&str, &str); 6] = [
    ("soldier", "h_guard"),
    ("enforcer", "h_mega"),
    ("hknight", "h_hellkn"),
    ("shalrath", "h_shal"),
    ("shambler", "h_shams"),
    ("oldone", "h_shams"),
];
const STOCK: [&str; 3] = ["progs/gib1.mdl", "progs/gib2.mdl", "progs/gib3.mdl"];
/// Gib pieces per ragdoll (plus the head).
const PIECES: usize = 6;

fn stem(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).trim_end_matches(".mdl").to_ascii_lowercase()
}

impl Debris {
    /// Every gib model the game precached: (stem, model).
    fn gib_pool(&mut self, cl: &ClientState, lv: &mut Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Rc<[(String, ModelId)]> {
        if let Some(p) = &self.gib_models {
            return p.clone();
        }
        let mut pool = Vec::new();
        for idx in 1..cl.model_precache.len() {
            let Some(name) = cl.model_name(idx).filter(|n| n.ends_with(".mdl")) else { continue };
            let s = stem(name);
            if s.starts_with("h_") || s.contains("head") {
                continue;
            }
            let Some(m) = lv.models.get(idx).copied().flatten() else { continue };
            if self.info(cl, fs, idx).is_some_and(|i| i.gib) {
                pool.push((s, m.id));
            }
        }
        if pool.is_empty() {
            pool = STOCK.iter().filter_map(|n| Some((stem(n), lv.named(fs, r, n)?.id))).collect();
        }
        let pool: Rc<[(String, ModelId)]> = pool.into();
        self.gib_models = Some(pool.clone());
        pool
    }

    fn head_for(&mut self, cl: &ClientState, lv: &mut Level, fs: &qcore::fs::Fs, r: &mut Renderer, model: usize) -> Option<ModelId> {
        if let Some(h) = self.heads.get(&model) {
            return *h;
        }
        let head = cl.model_name(model).and_then(|name| {
            let s = stem(name);
            let h = HEADS.iter().find(|x| x.0 == s).map_or(format!("h_{s}"), |x| x.1.to_string());
            let dir = name.rfind('/').map_or("", |k| &name[..=k]);
            lv.named(fs, r, &format!("{dir}{h}.mdl")).filter(|m| m.kind == Kind::Alias).map(|m| m.id)
        });
        self.heads.insert(model, head);
        head
    }

    /// Gibs ragdolls hit directly by an explosion (unless the server gibbed
    /// them meanwhile, which releases the ragdoll first).
    pub(super) fn gib_pending(&mut self, cl: &mut ClientState, lv: &mut Level, fs: &qcore::fs::Fs, r: &mut Renderer, time: f64) {
        if !self.corpses.iter().any(|c| c.gib.is_some_and(|g| g.0 <= time)) {
            return;
        }
        let pool = self.gib_pool(cl, lv, fs, r);
        let mut k = 0;
        while k < self.corpses.len() {
            let Some((_, center)) = self.corpses[k].gib.filter(|g| g.0 <= time) else {
                k += 1;
                continue;
            };
            let c = self.corpses.swap_remove(k);
            let head = self.head_for(cl, lv, fs, r, c.model);
            let s = cl.model_name(c.model).map(stem).unwrap_or_default();
            let own: Vec<ModelId> = pool.iter().filter(|g| !s.is_empty() && g.0.contains(&s)).map(|g| g.1).collect();
            let ids: Vec<ModelId> = if own.is_empty() { pool.iter().map(|g| g.1).collect() } else { own };
            let frame = cl.entities[c.ent].frame;
            self.gibbed.insert(c.ent, (c.model, frame));
            let mid = c.doll.pos.iter().fold(Vec3::ZERO, |s, p| s + *p) / c.doll.pos.len() as f32;
            self.blood_bursts.push(mid);
            cl.events.push(qclient::ClientEvent::TempSound { name: "player/udeath.wav", origin: mid.to_array() });
            let top = (0..c.doll.pos.len()).max_by(|&a, &b| c.doll.pos[a].z.total_cmp(&c.doll.pos[b].z));
            let mut rng = self.rng.take().unwrap_or(Rng(0x9e37_79b9));
            let mut pieces = 0;
            for (n, p) in c.doll.pos.iter().enumerate() {
                cl.run_particle_effect(p.to_array(), [0.0; 3], BLOOD, 24);
                let model = if Some(n) == top && head.is_some() {
                    head
                } else if pieces < PIECES && !ids.is_empty() {
                    pieces += 1;
                    Some(ids[n % ids.len()])
                } else {
                    None
                };
                let Some(model) = model else { continue };
                let out = (*p - center).normalize() + Vec3::new(rng.signed(), rng.signed(), rng.signed()) * 0.5;
                let vel = out.normalize() * (200.0 + rng.unit() * 250.0) + Vec3::new(0.0, 0.0, 200.0 + rng.unit() * 150.0);
                let mut b = Body::new(model, *p, vel, Quat::IDENTITY, 3.0);
                b.life = 10.0 + rng.unit() * 10.0;
                b.trail = true;
                b.random_spin(&mut rng, 10.0);
                self.bodies.push(b);
            }
            self.rng = Some(rng);
        }
    }
}
