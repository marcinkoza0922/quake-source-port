//! Grenade arc assist: the predicted flight of whatever the held weapon
//! throws (grenades and other gravity projectiles; see `arc_learn`), drawn
//! as a solid or dashed line until its fuse runs out, with a target ring
//! where it comes to rest, sticks, hits a monster or explodes.
use qcore::math::{Vec3, angle_vectors};
use qnet::protocol::{STAT_HEALTH, STAT_WEAPON};
use super::arc_learn::Profile;
use crate::gfx::Renderer;
use qrender::{FteBatch, FteBlend, FteVertex, PicId, View};

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

const STEP: f32 = 1.0 / 72.0;
const WIDTH: f32 = 0.6;
const DASH: f32 = 10.0;
/// Landing ring radius (units).
const RING: f32 = 14.0;

/// Simulates a throw under the engine's MOVETYPE_BOUNCE/TOSS physics:
/// launch from `profile` in the view basis, gravity, 1.5 overbounce (TOSS
/// slides) and rest on floors.
/// `trace` returns (fraction, endpos, normal, explodes): a hit on a
/// damageable entity (monster, player) ends the arc there.
/// Also returns the normal of the last surface touched (for the ring).
pub(crate) fn simulate(mut trace: impl FnMut(Vec3, Vec3) -> (f32, Vec3, Vec3, bool), start: Vec3, angles: Vec3, pvel: Vec3, gravity: f32, profile: &Profile) -> (Vec<Vec3>, Option<Vec3>) {
    let mut last = None;
    let pts = run(&mut trace, start, angles, pvel * profile.inherit, gravity * profile.gravity, profile, &mut last);
    (pts, last)
}

/// First-contact offsets of the extreme random throws (every sign corner
/// of `profile.spread`) from the unjittered throw's first contact. Bounces
/// after that diverge too much to cover, so only the first impact counts.
pub(crate) fn spread_offsets(mut trace: impl FnMut(Vec3, Vec3) -> (f32, Vec3, Vec3, bool), start: Vec3, angles: Vec3, pvel: Vec3, gravity: f32, profile: &Profile) -> Vec<Vec3> {
    let s = profile.spread;
    if s.length_sq() < 1e-6 { return Vec::new(); }
    let first = |trace: &mut _, vel: Vec3| {
        let p = Profile { vel, stick: true, ..*profile };
        let mut last = None;
        let pts = run(trace, start, angles, pvel * p.inherit, gravity * p.gravity, &p, &mut last);
        last.and(pts.last().copied())
    };
    let Some(centre) = first(&mut trace, profile.vel) else { return Vec::new() };
    let signs = |v: f32| if v > 0.0 { vec![-v, v] } else { vec![0.0] };
    let mut out = Vec::new();
    for x in signs(s.x) {
        for y in signs(s.y) {
            for z in signs(s.z) {
                if let Some(p) = first(&mut trace, profile.vel + Vec3::new(x, y, z)) {
                    out.push(p - centre);
                }
            }
        }
    }
    out
}

fn run(trace: &mut impl FnMut(Vec3, Vec3) -> (f32, Vec3, Vec3, bool), start: Vec3, angles: Vec3, push: Vec3, gravity: f32, p: &Profile, last: &mut Option<Vec3>) -> Vec<Vec3> {
    let (f, r, u) = angle_vectors(angles);
    let mut vel = f * p.vel.x + r * p.vel.y + u * p.vel.z + push;
    let mut pos = start + f * p.off.x + r * p.off.y + u * p.off.z + Vec3::new(0.0, 0.0, p.lift);
    let backoff = p.backoff;
    let mut pts = vec![pos];
    let mut t = 0.0;
    while t < p.fuse {
        vel.z -= gravity * STEP;
        let mut left = STEP;
        for _ in 0..4 {
            let end = pos + vel * left;
            let (frac, hit, n, boom) = trace(pos, end);
            pos = hit;
            if frac >= 1.0 { break; }
            pts.push(pos);
            *last = Some(n);
            if boom || p.stick { return pts; }
            left *= 1.0 - frac;
            vel -= n * vel.dot(n) * backoff;
            if n.z > 0.7 && (vel.z < 60.0 || backoff == 1.0) {
                return pts;
            }
            if frac == 0.0 && left == STEP { return pts; }
        }
        pts.push(pos);
        t += STEP;
    }
    pts
}

#[derive(Default)]
pub(crate) struct GrenadeArc {
    pic: Option<PicId>,
    ring: Option<PicId>,
}

pub(crate) struct Style {
    pub dashed: bool,
    pub color: [u8; 4],
}

impl GrenadeArc {
    /// `spread`: landing offsets of the extreme throws ([`spread_offsets`]),
    /// stretching the ring into an ellipse that covers them.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(&mut self, pts: &[Vec3], normal: Option<Vec3>, spread: &[Vec3], style: &Style, view: &View, r: &mut Renderer, geom: &mut Geometry) {
        if pts.len() < 2 { return; }
        let (from, at) = (pts[0], *pts.last().unwrap());
        self.draw_ring(at, at - from, normal, spread, style.color, view, r, geom);
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let Ok(p) = r.register_pic_rgba_linear(4, 4, &[255; 64]) else { return };
                *self.pic.insert(p)
            }
        };
        let eye = Vec3::from(view.origin);
        let first = geom.1.len() as u32;
        let mut dist = 0.0f32;
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = (b - a).length();
            if len < 0.01 { continue; }
            let dir = (b - a) / len;
            let side = dir.cross(eye - a);
            if side.length_sq() < 1e-6 { dist += len; continue; }
            let side = side.normalize();
            // dashed: split the segment at dash boundaries along the arc
            let mut s = 0.0;
            while s < len {
                let (on, next) = if style.dashed {
                    let phase = (dist + s) / DASH;
                    (phase as i64 % 2 == 0, ((phase.floor() + 1.0) * DASH - dist).min(len))
                } else {
                    (true, len)
                };
                if on && next > s {
                    let base = geom.0.len() as u32;
                    let (p0, p1) = (a + dir * s, a + dir * next);
                    for (p, o, uv) in [(p0, -1.0, [0.0, 0.0]), (p1, -1.0, [1.0, 0.0]), (p1, 1.0, [1.0, 1.0]), (p0, 1.0, [0.0, 1.0])] {
                        g_push(geom, p + side * (o * WIDTH), uv, style.color);
                    }
                    geom.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                }
                s = next.max(s + 0.01);
            }
            dist += len;
        }
        geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: geom.1.len() as u32 - first, soft: false, gel: false });
    }
}

impl GrenadeArc {
    /// Target ring at the end of the arc, lying on the last surface touched
    /// (or facing the camera when the fuse runs out in mid-air), stretched
    /// along the throw (`dir`) and across it to cover `spread`.
    #[allow(clippy::too_many_arguments)]
    fn draw_ring(&mut self, at: Vec3, dir: Vec3, normal: Option<Vec3>, spread: &[Vec3], color: [u8; 4], view: &View, r: &mut Renderer, geom: &mut Geometry) {
        let pic = match self.ring {
            Some(p) => p,
            None => {
                let Ok(p) = r.register_pic_rgba_linear(32, 32, &ring_pic()) else { return };
                *self.ring.insert(p)
            }
        };
        let eye = Vec3::from(view.origin);
        let n = normal.filter(|n| n.length_sq() > 0.5).unwrap_or_else(|| (eye - at).normalize());
        let (a, b) = ellipse_axes(n, dir, spread, RING.max((eye - at).length() * 0.035));
        let c = at + n * 0.5;
        let first = geom.1.len() as u32;
        let base = geom.0.len() as u32;
        for (x, y, uv) in [(-1.0, -1.0, [0.0, 0.0]), (1.0, -1.0, [1.0, 0.0]), (1.0, 1.0, [1.0, 1.0]), (-1.0, 1.0, [0.0, 1.0])] {
            g_push(geom, c + a * x + b * y, uv, [color[0], color[1], color[2], color[3].saturating_add(64)]);
        }
        geom.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: 6, soft: false, gel: false });
    }
}

/// Half-axes of the landing ring on the plane with normal `n`: `base`
/// (grows with distance so a far spot stays readable) widened by the
/// spread, the first axis along the throw direction projected on the plane.
fn ellipse_axes(n: Vec3, dir: Vec3, spread: &[Vec3], base: f32) -> (Vec3, Vec3) {
    let along = dir - n * dir.dot(n);
    let ua = if along.length_sq() > 1e-4 {
        along.normalize()
    } else {
        let helper = if n.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        n.cross(helper).normalize()
    };
    let ub = n.cross(ua).normalize();
    let ext = |u: Vec3| spread.iter().map(|o| o.dot(u).abs()).fold(0.0f32, f32::max);
    // the texture's ring sits at 0.8..0.95 of the quad: put its inner edge
    // on the furthest spread landing
    let half = |e: f32| (base * base + (e / 0.8) * (e / 0.8)).sqrt();
    (ua * half(ext(ua)), ub * half(ext(ub)))
}

/// White target: outer ring, four ticks pointing in and a centre dot.
fn ring_pic() -> Vec<u8> {
    let mut out = Vec::with_capacity(32 * 32 * 4);
    for j in 0..32 {
        for i in 0..32 {
            let (x, y) = ((i as f32 + 0.5) / 16.0 - 1.0, (j as f32 + 0.5) / 16.0 - 1.0);
            let d = (x * x + y * y).sqrt();
            let ring = (0.8..0.95).contains(&d);
            let tick = (x.abs() < 0.06 || y.abs() < 0.06) && (0.45..0.8).contains(&d);
            let dot = d < 0.14;
            out.extend(if ring || tick || dot { [255; 4] } else { [0; 4] });
        }
    }
    out
}

fn g_push(g: &mut Geometry, p: Vec3, uv: [f32; 2], color: [u8; 4]) {
    g.0.push(FteVertex { pos: p.to_array(), uv, color });
}

impl super::Host {
    /// Adds the arc for the local player when the assist is on and the
    /// grenade launcher is held (single player / local games only).
    pub(super) fn grenade_arc_frame(&mut self, view: &View, geom: &mut Geometry) {
        if self.cv("assist_grenadearc") == 0.0 || self.cv("deathmatch") != 0.0 || !self.local_autoaim_allowed() { return; }
        let cl = &self.cl;
        if cl.signon != 4 || cl.intermission != 0 || cl.stats[STAT_HEALTH] <= 0 { return; }
        let weapon = cl.model_name(cl.stats[STAT_WEAPON] as usize).map(str::to_string);
        let (pass, angles) = (cl.viewentity as u32, Vec3::from_array(cl.viewangles));
        let Some(origin) = cl.entities.get(cl.viewentity).map(|e| Vec3::from_array(e.origin)) else { return };
        let g = self.cv("sv_gravity");
        let g = if g > 0.0 { g } else { 800.0 };
        let game = self.server.core.fs.game_dirs().join(",");
        if let Some(snap) = self.server.thrown_by(pass) {
            let server = &mut self.server;
            self.arc_learn.update(&game, &snap, weapon.as_deref(), g, |a, b| {
                server.grenade_trace(a, b, pass, false).is_some_and(|(t, _)| t.fraction < 1.0 && t.ent == Some(0))
            });
        }
        // an add-on held in place of the weapon throws its own way
        let profile = match self.toy.held {
            Some(t) => {
                let now = self.server.level.as_ref().map_or(0.0, |l| l.vm.time);
                let speed = self.server.level.as_ref().map_or(600.0, |l| l.stickies.arc_speed(pass as usize - 1, now));
                super::arc_learn::toy(t, speed)
            }
            None => weapon.as_deref().and_then(|w| self.arc_learn.profile(w)),
        };
        let Some(profile) = profile else { return };
        let pvel = Vec3::from_array(self.cl.velocity);
        let server = &mut self.server;
        // full SV_Move: world, doors/lifts/platforms and monsters
        let trace = |a, b| match server.grenade_trace(a, b, pass, profile.missile) {
            Some((t, _)) if t.allsolid || t.startsolid => (0.0, a, Vec3::new(0.0, 0.0, 1.0), false),
            Some((t, boom)) => (t.fraction, t.endpos, t.plane_normal, boom),
            None => (1.0, b, Vec3::ZERO, false),
        };
        let mut trace = trace;
        let (pts, normal) = simulate(&mut trace, origin, angles, pvel, g, &profile);
        let spread = spread_offsets(&mut trace, origin, angles, pvel, g, &profile);
        let rgb = crate::reticle::CROSSHAIR_COLORS[(self.cv("assist_grenadearc_color").max(0.0) as usize).min(6)];
        let a = self.cv("assist_grenadearc_alpha").clamp(0.05, 1.0);
        let style = Style {
            dashed: self.cv("assist_grenadearc_style") != 0.0,
            color: [(rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8, (a * 255.0) as u8],
        };
        let mut arc = std::mem::take(&mut self.grenade_arc);
        arc.draw(&pts, normal, &spread, &style, view, &mut self.r, geom);
        self.grenade_arc = arc;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::arc_learn::ID_GRENADE;

    #[test]
    fn arc_rises_falls_and_lands_on_floor() {
        // floor at z = 0
        let trace = |a: Vec3, b: Vec3| {
            if b.z >= 0.0 { return (1.0, b, Vec3::ZERO, false); }
            let f = a.z / (a.z - b.z);
            (f, a + (b - a) * f, Vec3::new(0.0, 0.0, 1.0), false)
        };
        let (pts, n) = simulate(trace, Vec3::new(0.0, 0.0, 24.0), Vec3::ZERO, Vec3::ZERO, 800.0, &ID_GRENADE);
        assert_eq!(n, Some(Vec3::new(0.0, 0.0, 1.0)));
        let top = pts.iter().map(|p| p.z).fold(0.0, f32::max);
        assert!(top > 40.0, "rises: {top}");
        assert!(pts.iter().all(|p| p.z >= -0.01));
        assert!(pts.last().unwrap().x > 200.0);
        assert!(pts.len() < 2 + (ID_GRENADE.fuse / STEP) as usize * 2);
    }

    #[test]
    fn arc_bounces_off_brushes_and_stops_at_monsters() {
        // wall at x = 100: bounce if it's a door, explode if it's a monster
        let wall = |boom: bool| move |a: Vec3, b: Vec3| {
            if b.x <= 100.0 { return (1.0, b, Vec3::ZERO, false); }
            let f = (100.0 - a.x) / (b.x - a.x);
            (f, a + (b - a) * f, Vec3::new(-1.0, 0.0, 0.0), boom)
        };
        let (door, _) = simulate(wall(false), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 800.0, &ID_GRENADE);
        assert!(door.last().unwrap().x < 50.0, "bounced back");
        let (monster, _) = simulate(wall(true), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 800.0, &ID_GRENADE);
        assert!((monster.last().unwrap().x - 100.0).abs() < 0.01);
    }

    #[test]
    fn spread_widens_ring_into_ellipse_along_throw() {
        let floor = |a: Vec3, b: Vec3| {
            if b.z >= 0.0 { return (1.0, b, Vec3::ZERO, false); }
            let f = a.z / (a.z - b.z);
            (f, a + (b - a) * f, Vec3::new(0.0, 0.0, 1.0), false)
        };
        let offs = spread_offsets(floor, Vec3::new(0.0, 0.0, 24.0), Vec3::ZERO, Vec3::ZERO, 800.0, &ID_GRENADE);
        assert_eq!(offs.len(), 4);
        let (fx, sy) = offs.iter().fold((0.0f32, 0.0f32), |(x, y), o| (x.max(o.x.abs()), y.max(o.y.abs())));
        // ~0.6 s flight: ±10 ups sideways is ~6 units, range varies more
        assert!((3.0..12.0).contains(&sy), "side {sy}");
        assert!(fx > sy, "along {fx} vs side {sy}");
        let n = Vec3::new(0.0, 0.0, 1.0);
        let (a, b) = ellipse_axes(n, Vec3::new(1.0, 0.0, 0.0), &offs, 14.0);
        assert!(a.x.abs() > b.y.abs() && b.y.abs() > 14.0);
        assert!(a.x.abs() * 0.8 >= fx && b.y.abs() * 0.8 >= sy);
        let (a0, _) = ellipse_axes(n, Vec3::new(1.0, 0.0, 0.0), &[], 14.0);
        assert!((a0.length() - 14.0).abs() < 1e-4);
    }
}
