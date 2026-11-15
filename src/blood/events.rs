//! Match damage to flesh, and distinguish gibbing from an ordinary explosion.
use super::*;

type Poses<'a> = HashMap<u32, (&'a RenderEntity, Vec<Vec3>)>;

pub(super) enum PoseSet<'a, 'p> {
    #[cfg(test)]
    Ready(&'p Poses<'a>),
    Lazy {
        ents: &'a [RenderEntity],
        dynverts: &'a [u32],
        time: f64,
        cache: &'p mut Poses<'a>,
    },
}

impl<'a> PoseSet<'a, '_> {
    fn get(&self, key: u32) -> Option<(&RenderEntity, &[Vec3])> {
        match self {
            #[cfg(test)]
            Self::Ready(poses) => poses.get(&key).map(|&(e, ref v)| (e, v.as_slice())),
            Self::Lazy { cache, .. } => cache.get(&key).map(|&(e, ref v)| (e, v.as_slice())),
        }
    }

    fn for_each(
        &mut self,
        models: &HashMap<ModelId, Option<mesh::CachedMdl>>,
        mut filter: impl FnMut(&RenderEntity, &mesh::CachedMdl) -> bool,
        mut f: impl FnMut(u32, &RenderEntity, &mesh::CachedMdl, &[Vec3]),
    ) {
        match self {
            #[cfg(test)]
            Self::Ready(poses) => {
                for (&key, &(e, ref verts)) in *poses {
                    if let Some(Some(cm)) = models.get(&e.model)
                        && filter(e, cm)
                    {
                        f(key, e, cm, verts);
                    }
                }
            }
            Self::Lazy {
                ents,
                dynverts,
                time,
                cache,
            } => {
                for e in *ents {
                    if e.key == 0 || e.viewmodel {
                        continue;
                    }
                    let Some(Some(cm)) = models.get(&e.model) else {
                        continue;
                    };
                    if !filter(e, cm) {
                        continue;
                    }
                    let (_, verts) = cache
                        .entry(e.key)
                        .or_insert_with(|| (e, mesh::positions(&cm.mdl, e, dynverts, *time)));
                    f(e.key, e, cm, verts);
                }
            }
        }
    }
}

/// A blood-coloured QC particle alone is insufficient: shootable buttons
/// and triggers can emit SpawnBlood too. Match nearby flesh, allowing
/// collision-box damage to living enemies to stain the nearest skin.
fn contact(
    models: &HashMap<ModelId, Option<mesh::CachedMdl>>,
    p: Vec3,
    dir: Vec3,
    poses: &mut PoseSet<'_, '_>,
    world: &dyn World,
) -> Option<(u32, Vec3, Vec3)> {
    let mut nearest = None;
    poses.for_each(
        models,
        |e, cm| {
            if !cm.is_flesh && !cm.is_head {
                return false;
            }
            let r = cm.entity_radius(e) + 32.0;
            (Vec3::from(e.origin) - p).length_sq() <= r * r
        },
        |key, e, cm, verts| {
            let is_corpse = cm.is_corpse(e);
            for t in &cm.mdl.triangles {
                let tri = t.verts.map(|i| verts[i as usize]);
                let hit_contact = mesh::ray(p - dir * 24.0, p + dir * 24.0, tri)
                    .map(|(f, n)| (p + dir * ((f - 0.5) * 48.0), n))
                    .or_else(|| {
                        // QC hits boxes, not triangles. Only confirmed blood
                        // on living flesh gets a bounded nearest-skin fallback;
                        // wall impacts and cosmetic corpse rays remain exact.
                        if is_corpse {
                            return None;
                        }
                        let hit = mesh::closest_triangle(p, tri);
                        ((hit - p).length_sq() <= 32.0 * 32.0)
                            .then(|| (hit, (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize()))
                    });
                let Some((hit, n)) = hit_contact else {
                    continue;
                };
                let d = (hit - p).length_sq();
                let trace = world.trace_line(p, hit);
                if trace.frac < 1.0 && (trace.endpos - hit).length() > 1.0 {
                    continue;
                }
                if nearest.as_ref().is_none_or(|(best, _, _, _)| d < *best) {
                    nearest = Some((d, key, hit, if n.dot(dir) > 0.0 { -n } else { n }));
                }
            }
        },
    );
    nearest.map(|(_, key, p, n)| (key, p, n))
}

fn shot_contact(
    models: &HashMap<ModelId, Option<mesh::CachedMdl>>,
    a: Vec3,
    b: Vec3,
    poses: &mut PoseSet<'_, '_>,
    world: &dyn World,
) -> Option<(u32, Vec3, Vec3)> {
    let mut nearest = None;
    poses.for_each(
        models,
        |e, cm| {
            if e.portal_only || !(cm.is_flesh && cm.is_corpse(e) || cm.is_head) {
                return false;
            }
            let r = cm.entity_radius(e);
            mesh::point_segment_dist_sq(Vec3::from(e.origin), a, b) <= r * r
        },
        |key, _e, cm, verts| {
            for t in &cm.mdl.triangles {
                if let Some((f, n)) = mesh::ray(a, b, t.verts.map(|i| verts[i as usize]))
                    && nearest.as_ref().is_none_or(|(best, _, _, _)| f < *best)
                {
                    let p = a.lerp(b, f);
                    let tr = world.trace_line(a, p);
                    if tr.frac < 1.0 && (tr.endpos - p).length() > 1.0 {
                        continue;
                    }
                    let n = if n.dot(b - a) > 0.0 { -n } else { n };
                    nearest = Some((f, key, p, n));
                }
            }
        },
    );
    nearest.map(|(_, key, p, n)| (key, p, n))
}

impl Blood {
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn impacts(
        &mut self,
        set: Settings,
        cl: &qclient::ClientState,
        world: &dyn World,
        poses: &Poses<'_>,
        view: Vec3,
        debris: &crate::debris::Debris,
    ) {
        self.impacts_with(set, cl, world, PoseSet::Ready(poses), view, debris);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn impacts_with(
        &mut self,
        set: Settings,
        cl: &qclient::ClientState,
        world: &dyn World,
        mut poses: PoseSet<'_, '_>,
        view: Vec3,
        debris: &crate::debris::Debris,
    ) {
        let tier = set.tier();
        self.explosions.retain(|(_, t)| cl.time - *t < 0.6);
        self.explosions.extend(
            cl.shot_impacts
                .iter()
                .filter(|i| i.1)
                .map(|i| (Vec3::from(i.0), cl.time)),
        );
        let mut hits = Vec::new();
        for (p, burst) in std::mem::take(&mut self.debug_hits) {
            if burst {
                self.burst(p, set);
            } else {
                hits.push((p, (p - view).normalize()));
            }
        }
        for &p in &cl.blood_hits {
            let p = Vec3::from(p);
            let dir = (p - debris.shot_origin(view, p)).normalize();
            if let Some((key, hit, n)) = contact(&self.models, p, dir, &mut poses, world) {
                let (e, v) = poses.get(key).expect("matched entity pose must exist");
                if e.portal_only {
                    // a local player's own body wraps the camera: no wound
                    // meshes, and the spray leaves outwards, away from the eye
                    let out = (n * 2.0 - dir).normalize();
                    hits.push((hit + n * 4.0, out));
                    continue;
                }
                if tier.wounds {
                    self.wound(e, v, hit, n, cl.time, 7.0 * set.size);
                }
                hits.push((hit + n * 0.5, dir));
            }
        }
        // Non-damageable corpses still bleed, but a wall/trigger by itself
        // never becomes a blood source. Check the actual triangle segment.
        for &(p, boom) in &cl.shot_impacts {
            if boom {
                continue;
            }
            let b = Vec3::from(p);
            let a = debris.shot_origin(view, b);
            if let Some((key, p, n)) = shot_contact(&self.models, a, b, &mut poses, world) {
                let (e, v) = poses.get(key).expect("matched entity pose must exist");
                if tier.wounds {
                    self.wound(e, v, p, n, cl.time, 7.0 * set.size);
                }
                hits.push((p + n * 0.5, (b - a).normalize()));
            }
        }
        for (p, dir) in hits {
            for k in 0..tier.amount {
                let spread = Vec3::new(
                    self.random() - 0.5,
                    self.random() - 0.5,
                    self.random() - 0.8,
                );
                let d = if k == 0 {
                    dir
                } else {
                    (dir * 0.5 + spread).normalize()
                };
                let size = (5.0 + self.random() * 9.0) * set.size;
                let speed = 220.0 + self.random() * 260.0;
                self.launch(p, d, speed, size, tier.max_droplets);
            }
            // A heavier gout falls almost straight down from the wound.
            let drip = Vec3::new(dir.x * 0.3, dir.y * 0.3, -1.0).normalize();
            let speed = 40.0 + self.random() * 60.0;
            self.launch(p, drip, speed, 9.0 * set.size, tier.max_droplets);
        }
        self.gib_events(set, cl, debris);
    }

    fn gib_events(
        &mut self,
        set: Settings,
        cl: &qclient::ClientState,
        debris: &crate::debris::Debris,
    ) {
        let mut current = std::mem::take(&mut self.actors_scratch);
        current.clear();
        let mut gibs = Vec::new();
        let mut current_gibs = std::mem::take(&mut self.gibs_scratch);
        current_gibs.clear();
        for (key, e) in cl.active_entities() {
            let Some(name) = cl.model_name(e.model) else {
                continue;
            };
            let flags = cl.model_flags.get(e.model).copied().unwrap_or(0);
            if flags & (4 | 32) != 0
                || name
                    .rsplit('/')
                    .next()
                    .is_some_and(|s| s.starts_with("h_") || s.contains("head"))
            {
                current_gibs.insert(key as u32, e.model);
                if self.gibs.get(&(key as u32)) != Some(&e.model) {
                    gibs.push(Vec3::from(e.origin));
                }
                continue;
            }
            // The model id was cached before its server entity changed.
            if let Some(&id) = self.model_ids.get(&e.model)
                && self
                    .models
                    .get(&id)
                    .and_then(Option::as_ref)
                    .is_some_and(|c| c.is_flesh)
            {
                current.insert(key as u32, (id, Vec3::from(e.origin)));
            }
        }
        let mut bursts: Vec<Vec3> = debris.blood_bursts().collect();
        for (&key, &(id, p)) in &self.actors {
            let replaced = current.get(&key).is_none_or(|(m, _)| *m != id);
            if replaced
                && self
                    .explosions
                    .iter()
                    .any(|(b, _)| (*b - p).length() < 220.0)
                && gibs.iter().any(|g| (*g - p).length() < 120.0)
                && !bursts.iter().any(|b| (*b - p).length() < 48.0)
            {
                bursts.push(p + Vec3::new(0.0, 0.0, 12.0));
            }
        }
        self.actors_scratch = std::mem::replace(&mut self.actors, current);
        self.gibs_scratch = std::mem::replace(&mut self.gibs, current_gibs);
        for p in bursts {
            self.burst(p, set);
        }
    }

    /// Many independent rays, including shallow trajectories that carry
    /// droplets across a room. World traces keep the spray behind cover.
    pub(super) fn burst(&mut self, p: Vec3, set: Settings) {
        let tier = set.tier();
        let count = tier.burst_base + tier.amount * tier.burst_per_amount;
        for i in 0..count {
            let z = 1.0 - 2.0 * (i as f32 + self.random()) / count as f32;
            let a = i as f32 * 2.399963 + self.random() * 0.5;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let dir = Vec3::new(a.cos() * r, a.sin() * r, z);
            let speed = 300.0 + self.random() * 420.0;
            let size = (4.0 + self.random() * 15.0) * set.size;
            self.launch(p, dir, speed, size, tier.max_droplets);
        }
    }
}
