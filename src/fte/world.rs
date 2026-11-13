use super::*;

/// Particle world over the client's map: hull-0 traces, contents, and the
/// render faces for decal clipping.
pub(crate) struct BspWorld<'a> {
    pub(crate) bsp: &'a Bsp,
    pub(crate) cm: &'a CollisionModel,
    /// Inline brush entities (doors, lifts) for traces and decals.
    pub(crate) ents: &'a [BrushEnt],
}

/// A visible inline brush model entity (CL_PrepareTraceLineEntities).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BrushEnt {
    pub(crate) entity: i32,
    pub(crate) submodel: usize,
    pub(crate) origin: Vec3,
    /// forward, right, up when rotated.
    pub(crate) axis: Option<[Vec3; 3]>,
    pub(crate) mins: Vec3,
    pub(crate) maxs: Vec3,
}

impl BrushEnt {
    /// Entity-space point (the renderer's brush rotation, right negated).
    pub(crate) fn local(&self, p: Vec3) -> Vec3 {
        let d = p - self.origin;
        match self.axis {
            Some(a) => Vec3::new(d.dot(a[0]), -d.dot(a[1]), d.dot(a[2])),
            None => d,
        }
    }
    pub(crate) fn dir_to_world(&self, v: Vec3) -> Vec3 {
        match self.axis {
            Some(a) => a[0] * v.x - a[1] * v.y + a[2] * v.z,
            None => v,
        }
    }
}

/// Brush entities of the client's frame.
pub(crate) fn brush_ents(cl: &ClientState, cm: &CollisionModel) -> Vec<BrushEnt> {
    let mut out = Vec::new();
    for (i, e) in cl.active_entities() {
        let Some(sub) = cl.model_name(e.model).and_then(|n| n.strip_prefix('*')).and_then(|n| n.parse::<usize>().ok()) else {
            continue;
        };
        let Some(m) = cm.submodels.get(sub) else { continue };
        let origin = Vec3::from(e.origin);
        let rotated = e.angles != [0.0; 3];
        let (mins, maxs) = if rotated {
            let r = m.mins.length().max(m.maxs.length());
            (origin - Vec3::splat(r), origin + Vec3::splat(r))
        } else {
            (origin + m.mins, origin + m.maxs)
        };
        let axis = rotated.then(|| {
            let (f, r, u) = qcore::math::angle_vectors(Vec3::from(e.angles));
            [f, r, u]
        });
        out.push(BrushEnt { entity: i as i32, submodel: sub, origin, axis, mins, maxs });
    }
    out
}

pub(crate) fn fte_contents(c: i32) -> u32 {
    use qpart::*;
    match c {
        -1 => FTECONTENTS_EMPTY,
        -2 => FTECONTENTS_SOLID,
        -3 => FTECONTENTS_WATER,
        -4 => FTECONTENTS_SLIME,
        -5 => FTECONTENTS_LAVA,
        -6 => FTECONTENTS_SKY,
        // anything else (currents, clip) is assumed water (cont_qtof)
        _ => FTECONTENTS_WATER,
    }
}
