//! What the bot does each frame for its current goal: noclip flight to a
//! spot, aiming and firing, and walking into buttons.

use super::*;

/// Yaw/pitch (Quake view convention: positive pitch looks down) towards `d`.
fn look_angles(d: Vec3) -> [f32; 3] {
    let yaw = d.y.atan2(d.x).to_degrees();
    let pitch = -(d.z.atan2((d.x * d.x + d.y * d.y).sqrt())).to_degrees();
    [pitch, yaw, 0.0]
}

/// Fill in `mv`'s move so the bot flies towards `dest` whatever it looks at
/// (classic noclip: horizontal velocity from forward/side and the view,
/// vertical velocity straight from the up move).
fn fly_towards(mv: &mut ClientMove, here: Vec3, dest: Vec3) {
    let d = dest - here;
    let len = d.length();
    if len < 1.0 {
        return;
    }
    let w = d * (SPEED.min(len / DT as f32) / len);
    let (sy, cy) = mv.angles[1].to_radians().sin_cos();
    let cp = mv.angles[0].to_radians().cos().max(0.2);
    mv.forward = (w.x * cy + w.y * sy) / cp;
    mv.side = w.x * sy - w.y * cy;
    mv.up = w.z;
}

impl Bot<'_> {
    pub(super) fn act(&mut self, i: usize, t: f64) -> ClientMove {
        let (ent, kind) = (self.goals[i].ent, self.goals[i].kind);
        let here = self.me_origin();
        if t >= self.pushing_until && self.vm().ent_f(self.me, self.o.movetype) as i32 != MOVETYPE_NOCLIP {
            self.set_noclip(true);
        }
        let mut mv = ClientMove::default();
        let shootable = self.vm().ent_f(ent, self.o.health) > 0.0 && self.vm().ent_f(ent, self.o.takedamage) != 0.0;
        if kind == Kind::Monster && self.stuck_down(ent) {
            self.fail(i, "knocked down inside the world: can never get up or be hit");
            return mv;
        }
        match kind {
            Kind::Monster | Kind::Shoot => self.shoot(&mut mv, ent, t),
            Kind::Button if shootable => self.shoot(&mut mv, ent, t),
            Kind::Button => {
                if !self.press(&mut mv, i, t) {
                    self.fail(i, "pushing into it did nothing");
                }
            }
            Kind::Secret | Kind::Trigger | Kind::Pickup | Kind::Exit => {
                self.fire_at_will(&mut mv);
                if self.spot.is_none() || t - self.spot_at > 1.0 {
                    self.spot = Some(self.touch_point(ent));
                    self.spot_at = t;
                }
                let c = self.spot.unwrap();
                self.att.frame(Some((c - here).length()));
                let inside = self.inside(ent);
                self.att.touching(inside);
                self.warp_if_slow(t, c);
                fly_towards(&mut mv, here, c);
            }
        }
        mv
    }

    /// A point where the player's box overlaps trigger `e` but no other
    /// trigger (a secret wrapped around a teleporter), else its centre.
    fn touch_point(&self, e: u32) -> Vec3 {
        let (lo, hi) = self.bounds(e);
        let c = (lo + hi) * 0.5;
        let vm = self.vm();
        let others: Vec<(Vec3, Vec3)> = (1..vm.num_edicts() as u32)
            .filter(|&o| o != e && !vm.is_free(o) && vm.ent_f(o, self.o.solid) as i32 == SOLID_TRIGGER)
            .map(|o| self.bounds(o))
            .filter(|(a, b)| a.x < hi.x + 16.0 && b.x > lo.x - 16.0 && a.y < hi.y + 16.0 && b.y > lo.y - 16.0 && a.z < hi.z + 32.0 && b.z > lo.z - 24.0)
            .collect();
        if others.is_empty() {
            return c;
        }
        let clear = |p: Vec3| {
            others.iter().all(|(a, b)| !(p.x + 16.0 > a.x && p.x - 16.0 < b.x && p.y + 16.0 > a.y && p.y - 16.0 < b.y && p.z + 32.0 > a.z && p.z - 24.0 < b.z))
        };
        let mut best: Option<(f32, Vec3)> = None;
        // The player's box just touching each part of the trigger.
        let (a, b) = (lo - Vec3::new(15.0, 15.0, 31.0), hi + Vec3::new(15.0, 15.0, 23.0));
        for i in 0..=8 {
            for j in 0..=8 {
                for k in 0..=4 {
                    let f = Vec3::new(i as f32 / 8.0, j as f32 / 8.0, k as f32 / 4.0);
                    let p = a + (b - a).mul_elem(f);
                    let d = (p - c).length_sq();
                    if best.is_none_or(|x| d < x.0) && clear(p) {
                        best = Some((d, p));
                    }
                }
            }
        }
        best.map_or(c, |x| x.1)
    }

    /// Flying there hasn't worked for a while (a teleporter in the way keeps
    /// sending the bot off): put it there directly, every few seconds.
    fn warp_if_slow(&mut self, t: f64, to: Vec3) {
        if t - self.goal_start > 5.0 && t - self.warped_at > 5.0 {
            self.warped_at = t;
            self.att.warps += 1;
            self.note(&format!("warp to {to:?}"));
            self.sv.command(&format!("setpos {} {} {}", to.x, to.y, to.z));
        }
    }

    /// A knocked-down zombie is not solid (shots and splash pass it by) and
    /// gets up only when `walkmove(0,0)` succeeds; one placed in the floor
    /// (e1m7's closets, "walkmonster in wall") never does. Unkillable in
    /// vanilla too: a level finding, not something to keep shooting at.
    fn stuck_down(&self, ent: u32) -> bool {
        let o = v(self.vm().ent_v(ent, self.o.origin));
        self.vm().ent_f(ent, self.o.solid) as i32 == SOLID_NOT && self.cm.hull(0, 1).trace(o, o).startsolid
    }

    /// Fly to a spot with a clear shot at `ent` and fire at it.
    fn shoot(&mut self, mv: &mut ClientMove, ent: u32, t: f64) {
        if self.spot.is_none() || t - self.spot_at > 1.0 {
            let found = self.firing_spot(ent);
            self.found_spot = found.is_some();
            let (spot, aim) = found.unwrap_or((self.center(ent), Vec3::ZERO));
            self.spot = Some(spot);
            self.aim_point = aim;
            self.spot_at = t;
        }
        // Keep the aim on the same part of the target as it moves.
        let c = self.center(ent) + self.aim_point;
        self.aim(mv, ent, c);
        let spot = self.spot.unwrap();
        let dist = (spot - self.me_origin()).length();
        let clear = self.clear_shot(self.me_origin(), c, ent);
        if dist < 32.0 || clear {
            mv.buttons |= 1;
        }
        self.att.frame(self.found_spot.then_some(dist));
        self.att.shot(clear, mv.buttons & 1 != 0);
        fly_towards(mv, self.me_origin(), spot);
    }

    /// Fire at the nearest live monster in sight while flying elsewhere.
    fn fire_at_will(&mut self, mv: &mut ClientMove) {
        let (eye, here) = (self.eye(), self.me_origin());
        let mut cands: Vec<(f32, u32, Vec3)> = self
            .goals
            .iter()
            .filter(|g| g.kind == Kind::Monster && !g.done && !self.dormant(g))
            .map(|g| {
                let c = self.center(g.ent);
                ((c - eye).length_sq(), g.ent, c)
            })
            .filter(|x| x.0 < 1200.0 * 1200.0)
            .collect();
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(_, e, c) in cands.iter().take(6) {
            if self.clear_shot(here, c, e) {
                self.aim(mv, e, c);
                mv.buttons |= 1;
                break;
            }
        }
    }

    /// Look at `c` and pick the weapon for target `e`.
    fn aim(&mut self, mv: &mut ClientMove, e: u32, c: Vec3) {
        let d = c - self.eye();
        mv.angles = look_angles(d);
        // Zombies only die when gibbed: rockets. Otherwise the lightning
        // gun in its range (hitscan, cheap to simulate), the super nailgun
        // beyond it. Never the lightning gun underwater: it discharges
        // instead of firing (e1m1's secret panel is shot from the pool).
        let wet = self.vm().ent_f(self.me, self.o.waterlevel) > 1.0;
        let w = if self.vm().classname(e).contains("zombie") {
            7
        } else if d.length() < 560.0 && !wet {
            8
        } else {
            5
        };
        if w != self.weapon {
            self.weapon = w;
            self.impulses.push(w);
        }
    }

    /// Buttons fire on contact, which noclip never makes: fly next to one,
    /// switch noclip off and walk into it. `false` once that failed 3 times.
    fn press(&mut self, mv: &mut ClientMove, i: usize, t: f64) -> bool {
        let ent = self.goals[i].ent;
        let c = self.center(ent);
        let here = self.me_origin();
        if t < self.pushing_until {
            // Look straight at it: underwater, swimming follows the pitch.
            mv.angles = look_angles(c - self.eye());
            mv.forward = 400.0;
            mv.up = (c.z - here.z).clamp(-1.0, 1.0) * 200.0;
            return true;
        }
        if self.pushes >= 3 {
            return false;
        }
        if self.goals[i].again && !self.fires_mover(ent) && self.movers_moving() {
            // Stay put until the doors the other buttons moved have arrived.
            self.goal_start = t;
            return true;
        }
        if self.spot.is_none() || t - self.spot_at > 1.0 {
            // Try another face on each attempt. Nothing free (a monster in
            // the way): clear the area and look again.
            let spots = self.touch_spots(ent);
            self.spot = spots.get(self.pushes as usize).or(spots.first()).copied();
            self.spot_at = t;
        }
        self.fire_at_will(mv);
        self.att.frame(self.spot.map(|s| (s - here).length()));
        let Some(spot) = self.spot else {
            fly_towards(mv, here, c);
            return true;
        };
        self.warp_if_slow(t, spot);
        fly_towards(mv, here, spot);
        if (spot - here).length() < 4.0 {
            self.note(&format!("pushing button #{ent} from {spot:?}"));
            self.set_noclip(false);
            self.pushing_until = t + 1.5;
            self.pushes += 1;
            self.spot = None;
        }
        true
    }

    /// The nearest open spot with a clear shot at some part of `ent` (its
    /// centre, else the middle of a face: an altar's trigger can wrap world
    /// brushes), and that part as an offset from the centre.
    fn firing_spot(&mut self, ent: u32) -> Option<(Vec3, Vec3)> {
        let here = self.me_origin();
        let (lo, hi) = self.bounds(ent);
        let c = (lo + hi) * 0.5;
        let h = (hi - lo) * 0.5 - Vec3::splat(2.0);
        let aims = [
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, h.z),
            Vec3::new(h.x, 0.0, 0.0),
            Vec3::new(-h.x, 0.0, 0.0),
            Vec3::new(0.0, h.y, 0.0),
            Vec3::new(0.0, -h.y, 0.0),
            Vec3::new(0.0, 0.0, -h.z),
        ];
        let mut best: Option<(f32, Vec3, Vec3)> = None;
        for r in [48.0f32, 96.0, 192.0, 320.0] {
            let r = r + h.x.max(h.y);
            for k in 0..12 {
                let a = k as f32 * std::f32::consts::TAU / 12.0;
                for dz in [0.0, 64.0, -48.0] {
                    let p = c + Vec3::new(a.cos() * r, a.sin() * r, dz);
                    let d = (p - here).length_sq();
                    if best.is_some_and(|b| b.0 <= d) || !self.fits(p, 0, false) {
                        continue;
                    }
                    if let Some(&aim) = aims.iter().find(|&&o| self.clear_shot(p, c + o, ent)) {
                        best = Some((d, p, aim));
                    }
                }
            }
            if best.is_some() {
                break;
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Where a player stands touching brush entity `e`: just off one of its
    /// faces with nothing of the world in between, best first (standing on
    /// a floor before floating).
    fn touch_spots(&self, e: u32) -> Vec<Vec3> {
        let (lo, hi) = self.bounds(e);
        let c = (lo + hi) * 0.5;
        let mut out: Vec<(bool, Vec3)> = Vec::new();
        // Standing on it (floor buttons) ...
        let top = Vec3::new(c.x, c.y, hi.z + 25.0);
        if self.fits(top, e, true) {
            out.push((true, top));
        }
        // ... or next to one of its faces, at a few heights and distances.
        let faces = [
            (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(lo.x, c.y, c.z)),
            (Vec3::new(1.0, 0.0, 0.0), Vec3::new(hi.x, c.y, c.z)),
            (Vec3::new(0.0, -1.0, 0.0), Vec3::new(c.x, lo.y, c.z)),
            (Vec3::new(0.0, 1.0, 0.0), Vec3::new(c.x, hi.y, c.z)),
        ];
        for (n, face) in faces {
            'face: for dist in [17.0, 24.0, 32.0] {
                // The last: standing on a floor flush with its top (a sill).
                for z in [c.z, lo.z + 24.0, hi.z - 32.0, hi.z + 25.0] {
                    let p = Vec3::new(face.x, face.y, z) + n * dist;
                    // The face must look into open space, not through a wall.
                    if !self.fits(p, e, true) || self.cm.hull(0, 0).trace(p, Vec3::new(face.x, face.y, z.clamp(lo.z, hi.z))).fraction < 1.0 {
                        continue;
                    }
                    // Lowest origin whose body still reaches it (body -24..+32).
                    let tr = self.cm.hull(0, 1).trace(p, Vec3::new(p.x, p.y, lo.z - 31.0));
                    let floor = tr.fraction < 1.0 && !tr.startsolid && self.fits(tr.endpos, e, true);
                    out.push(if floor { (true, tr.endpos) } else { (false, p) });
                    break 'face;
                }
            }
        }
        out.sort_by_key(|x| !x.0);
        out.into_iter().map(|x| x.1).collect()
    }

    /// Whether a player fits at `p`: world, brush entities where they are
    /// now (other than `except`), and other bodies if `bodies`.
    fn fits(&self, p: Vec3, except: u32, bodies: bool) -> bool {
        if self.cm.hull(0, 1).trace(p, p).startsolid {
            return false;
        }
        let l = self.sv.level.as_ref().unwrap();
        let vm = &l.vm;
        let (pmin, pmax) = (p + Vec3::new(-16.0, -16.0, -24.0), p + Vec3::new(16.0, 16.0, 32.0));
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || e == self.me || e == except {
                continue;
            }
            let solid = vm.ent_f(e, self.o.solid) as i32;
            if solid == SOLID_NOT || solid == SOLID_TRIGGER {
                continue;
            }
            let (a, b) = self.bounds(e);
            if a.x > pmax.x || b.x < pmin.x || a.y > pmax.y || b.y < pmin.y || a.z > pmax.z || b.z < pmin.z {
                continue;
            }
            if solid != SOLID_BSP {
                if bodies {
                    return false;
                }
                continue;
            }
            if let Some(mi) = self.o.modelindex
                && let Some((m, sub)) = l.world.model(vm.ent_f(e, mi) as usize)
            {
                let o = v(vm.ent_v(e, self.o.origin));
                if m.hull(*sub, 1).trace(p - o, p - o).startsolid {
                        return false;
                }
            }
        }
        true
    }

    /// Whether a shot by a player standing at `origin` hits `target` when
    /// aimed at `b`, by the engine's own traces (monsters and brush entities
    /// in the way stop it). Both lines must reach: weapons fire from
    /// `origin + '0 0 16'`, but the lightning gun's damage traces start at
    /// the origin itself (e1m1's panel over the pool clips the ledge).
    fn clear_shot(&mut self, origin: Vec3, b: Vec3, target: u32) -> bool {
        let me = self.me;
        // A downed zombie is not solid, so no trace hits it, but rocket
        // splash does: then only the world and brushes may be in the way.
        if self.vm().ent_f(target, self.o.solid) as i32 == SOLID_NOT {
            let a = origin + Vec3::new(0.0, 0.0, 16.0);
            return self.sv.traceline(a, b, true, me).is_some_and(|tr| tr.fraction == 1.0);
        }
        [origin + Vec3::new(0.0, 0.0, 16.0), origin].into_iter().all(|a| {
            // Run through `b`: an aim point on a face ends a trace short.
            let end = a + (b - a) * 1.25;
            self.sv.traceline(a, end, false, me).is_some_and(|tr| tr.ent == Some(target))
        })
    }
}
