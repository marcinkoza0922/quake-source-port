//! QuakeWorld player movement (QW/client pmove.c, pmovetst.c), used by the
//! QW client to predict the local player and other players.

use crate::hull::{CollisionModel, HPlane, Hull, Trace, box_planes};
use crate::{CONTENTS_EMPTY, CONTENTS_SOLID, CONTENTS_WATER, Vec3};

pub const PLAYER_MINS: Vec3 = Vec3::new(-16.0, -16.0, -24.0);
pub const PLAYER_MAXS: Vec3 = Vec3::new(16.0, 16.0, 32.0);
const STEPSIZE: f32 = 18.0;
const BUTTON_JUMP: u8 = 2;
const STOP_EPSILON: f32 = 0.1;
const MAX_CLIP_PLANES: usize = 5;

/// movevars_t (from svc_serverdata, svc_maxspeed, svc_entgravity).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveVars {
    pub gravity: f32,
    pub stopspeed: f32,
    pub maxspeed: f32,
    pub spectatormaxspeed: f32,
    pub accelerate: f32,
    pub airaccelerate: f32,
    pub wateraccelerate: f32,
    pub friction: f32,
    pub waterfriction: f32,
    pub entgravity: f32,
}

impl Default for MoveVars {
    fn default() -> Self {
        Self {
            gravity: 800.0,
            stopspeed: 100.0,
            maxspeed: 320.0,
            spectatormaxspeed: 500.0,
            accelerate: 10.0,
            airaccelerate: 0.7,
            wateraccelerate: 10.0,
            friction: 4.0,
            waterfriction: 1.0,
            entgravity: 1.0,
        }
    }
}

/// physent_t: a brush model (submodel `sub` of `model`) or a box.
#[derive(Debug, Clone)]
pub enum PhysShape<'a> {
    Model(&'a CollisionModel, usize),
    Box { mins: Vec3, maxs: Vec3 },
}

#[derive(Debug, Clone)]
pub struct PhysEnt<'a> {
    pub origin: Vec3,
    pub shape: PhysShape<'a>,
}

/// The pmove input/output (playermove_t minus the physents).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayerState {
    pub origin: Vec3,
    pub velocity: Vec3,
    pub oldbuttons: u8,
    pub waterjumptime: f32,
    pub onground: bool,
}

/// One command's worth of movement input.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MoveCmd {
    pub msec: u8,
    pub angles: [f32; 3],
    pub forwardmove: f32,
    pub sidemove: f32,
    pub upmove: f32,
    pub buttons: u8,
}

struct PmTrace {
    allsolid: bool,
    startsolid: bool,
    fraction: f32,
    endpos: Vec3,
    normal: Vec3,
    /// Physent index, -1 for none.
    ent: i32,
}

struct Pm<'a, 'b> {
    ents: &'b [PhysEnt<'a>],
    mv: &'b MoveVars,
    s: PlayerState,
    cmd: MoveCmd,
    angles: [f32; 3],
    dead: bool,
    frametime: f32,
    forward: Vec3,
    right: Vec3,
    onground: i32,
    waterlevel: i32,
    watertype: i32,
}

fn angle_vectors(a: [f32; 3]) -> (Vec3, Vec3, Vec3) {
    let r = |d: f32| (d * std::f32::consts::PI * 2.0 / 360.0).sin_cos();
    let (sy, cy) = r(a[1]);
    let (sp, cp) = r(a[0]);
    let (sr, cr) = r(a[2]);
    let f = Vec3::new(cp * cy, cp * sy, -sp);
    let rt = Vec3::new(-sr * sp * cy + -cr * -sy, -sr * sp * sy + -cr * cy, -sr * cp);
    let u = Vec3::new(cr * sp * cy + -sr * -sy, cr * sp * sy + -sr * cy, cr * cp);
    (f, rt, u)
}

fn normalize(v: &mut Vec3) -> f32 {
    let l = v.length();
    if l != 0.0 {
        *v *= 1.0 / l;
    }
    l
}

/// PM_ClipVelocity.
fn clip_velocity(inv: Vec3, normal: Vec3, overbounce: f32) -> Vec3 {
    let backoff = inv.dot(normal) * overbounce;
    let mut out = inv - normal * backoff;
    for i in 0..3 {
        if out[i] > -STOP_EPSILON && out[i] < STOP_EPSILON {
            out[i] = 0.0;
        }
    }
    out
}

/// PM_PlayerMove over a set of physents (hull 1 for brush models, a box
/// hull expanded by the player size otherwise).
fn player_trace(ents: &[PhysEnt<'_>], start: Vec3, end: Vec3) -> PmTrace {
    let mut total = PmTrace { allsolid: false, startsolid: false, fraction: 1.0, endpos: end, normal: Vec3::ZERO, ent: -1 };
    for (i, pe) in ents.iter().enumerate() {
        let planes: [HPlane; 6];
        let hull = match &pe.shape {
            PhysShape::Model(m, sub) => m.hull(*sub, 1),
            PhysShape::Box { mins, maxs } => {
                planes = box_planes(*mins - PLAYER_MAXS, *maxs - PLAYER_MINS);
                Hull::for_box(&planes)
            }
        };
        let (sl, el) = (start - pe.origin, end - pe.origin);
        let mut t = Trace::new(end);
        hull.recursive_check(hull.first, 0.0, 1.0, sl, el, &mut t);
        if t.allsolid {
            t.startsolid = true;
        }
        if t.startsolid {
            t.fraction = 0.0;
        }
        if t.fraction < total.fraction {
            // Trace::new set endpos to `end` (world space); a hit sets it hull-local.
            let endpos = if t.fraction < 1.0 { t.endpos + pe.origin } else { t.endpos };
            total = PmTrace {
                allsolid: t.allsolid,
                startsolid: t.startsolid,
                fraction: t.fraction,
                endpos,
                normal: t.plane_normal,
                ent: i as i32,
            };
        }
    }
    total
}

/// PM_TestPlayerPosition.
pub fn test_player_position(ents: &[PhysEnt<'_>], pos: Vec3) -> bool {
    for pe in ents {
        let planes: [HPlane; 6];
        let hull = match &pe.shape {
            PhysShape::Model(m, sub) => m.hull(*sub, 1),
            PhysShape::Box { mins, maxs } => {
                planes = box_planes(*mins - PLAYER_MAXS, *maxs - PLAYER_MINS);
                Hull::for_box(&planes)
            }
        };
        if hull.point_contents(hull.first, pos - pe.origin) == CONTENTS_SOLID {
            return false;
        }
    }
    true
}

impl Pm<'_, '_> {
    fn trace(&self, a: Vec3, b: Vec3) -> PmTrace {
        player_trace(self.ents, a, b)
    }

    /// PM_PointContents: world hull 0.
    fn point_contents(&self, p: Vec3) -> i32 {
        match self.ents.first().map(|e| &e.shape) {
            Some(PhysShape::Model(m, _)) => m.point_contents(p),
            _ => CONTENTS_EMPTY,
        }
    }

    /// PM_FlyMove.
    fn fly_move(&mut self) -> i32 {
        let mut blocked = 0;
        let original_velocity = self.s.velocity;
        let primal_velocity = self.s.velocity;
        let mut planes = [Vec3::ZERO; MAX_CLIP_PLANES];
        let mut numplanes = 0;
        let mut time_left = self.frametime;
        for _ in 0..4 {
            let end = self.s.origin + self.s.velocity * time_left;
            let tr = self.trace(self.s.origin, end);
            if tr.startsolid || tr.allsolid {
                self.s.velocity = Vec3::ZERO;
                return 3;
            }
            if tr.fraction > 0.0 {
                self.s.origin = tr.endpos;
                numplanes = 0;
            }
            if tr.fraction == 1.0 {
                break;
            }
            if tr.normal.z > 0.7 {
                blocked |= 1;
            }
            if tr.normal.z == 0.0 {
                blocked |= 2;
            }
            time_left -= time_left * tr.fraction;
            if numplanes >= MAX_CLIP_PLANES {
                self.s.velocity = Vec3::ZERO;
                break;
            }
            planes[numplanes] = tr.normal;
            numplanes += 1;
            let mut i = 0;
            while i < numplanes {
                self.s.velocity = clip_velocity(original_velocity, planes[i], 1.0);
                let ok = (0..numplanes).all(|j| j == i || self.s.velocity.dot(planes[j]) >= 0.0);
                if ok {
                    break;
                }
                i += 1;
            }
            if i == numplanes {
                if numplanes != 2 {
                    self.s.velocity = Vec3::ZERO;
                    break;
                }
                let dir = planes[0].cross(planes[1]);
                let d = dir.dot(self.s.velocity);
                self.s.velocity = dir * d;
            }
            if self.s.velocity.dot(primal_velocity) <= 0.0 {
                self.s.velocity = Vec3::ZERO;
                break;
            }
        }
        if self.s.waterjumptime != 0.0 {
            self.s.velocity = primal_velocity;
        }
        blocked
    }

    /// PM_GroundMove.
    fn ground_move(&mut self) {
        self.s.velocity.z = 0.0;
        if self.s.velocity == Vec3::ZERO {
            return;
        }
        let mut dest = self.s.origin + self.s.velocity * self.frametime;
        dest.z = self.s.origin.z;
        let tr = self.trace(self.s.origin, dest);
        if tr.fraction == 1.0 {
            self.s.origin = tr.endpos;
            return;
        }
        let original = self.s.origin;
        let originalvel = self.s.velocity;
        self.fly_move();
        let down = self.s.origin;
        let downvel = self.s.velocity;
        self.s.origin = original;
        self.s.velocity = originalvel;
        let mut dest = self.s.origin;
        dest.z += STEPSIZE;
        let tr = self.trace(self.s.origin, dest);
        if !tr.startsolid && !tr.allsolid {
            self.s.origin = tr.endpos;
        }
        self.fly_move();
        let mut dest = self.s.origin;
        dest.z -= STEPSIZE;
        let tr = self.trace(self.s.origin, dest);
        if tr.normal.z < 0.7 {
            self.s.origin = down;
            self.s.velocity = downvel;
            return;
        }
        if !tr.startsolid && !tr.allsolid {
            self.s.origin = tr.endpos;
        }
        let up = self.s.origin;
        let d2 = |p: Vec3| (p.x - original.x).powi(2) + (p.y - original.y).powi(2);
        if d2(down) > d2(up) {
            self.s.origin = down;
            self.s.velocity = downvel;
        } else {
            self.s.velocity.z = downvel.z;
        }
    }

    /// PM_Friction.
    fn friction(&mut self) {
        if self.s.waterjumptime != 0.0 {
            return;
        }
        let vel = self.s.velocity;
        let speed = vel.length();
        if speed < 1.0 {
            self.s.velocity.x = 0.0;
            self.s.velocity.y = 0.0;
            return;
        }
        let mut friction = self.mv.friction;
        if self.onground != -1 {
            let mut start = self.s.origin + Vec3::new(vel.x / speed * 16.0, vel.y / speed * 16.0, 0.0);
            start.z = self.s.origin.z + PLAYER_MINS.z;
            let mut stop = start;
            stop.z = start.z - 34.0;
            if self.trace(start, stop).fraction == 1.0 {
                friction *= 2.0;
            }
        }
        let mut drop = 0.0;
        if self.waterlevel >= 2 {
            drop += speed * self.mv.waterfriction * self.waterlevel as f32 * self.frametime;
        } else if self.onground != -1 {
            let control = if speed < self.mv.stopspeed { self.mv.stopspeed } else { speed };
            drop += control * friction * self.frametime;
        }
        let newspeed = (speed - drop).max(0.0) / speed;
        self.s.velocity = vel * newspeed;
    }

    /// PM_Accelerate.
    fn accelerate(&mut self, wishdir: Vec3, wishspeed: f32, accel: f32) {
        if self.dead || self.s.waterjumptime != 0.0 {
            return;
        }
        let addspeed = wishspeed - self.s.velocity.dot(wishdir);
        if addspeed <= 0.0 {
            return;
        }
        let accelspeed = (accel * self.frametime * wishspeed).min(addspeed);
        self.s.velocity += wishdir * accelspeed;
    }

    /// PM_AirAccelerate.
    fn air_accelerate(&mut self, wishdir: Vec3, wishspeed: f32, accel: f32) {
        if self.dead || self.s.waterjumptime != 0.0 {
            return;
        }
        let wishspd = wishspeed.min(30.0);
        let addspeed = wishspd - self.s.velocity.dot(wishdir);
        if addspeed <= 0.0 {
            return;
        }
        let accelspeed = (accel * wishspeed * self.frametime).min(addspeed);
        self.s.velocity += wishdir * accelspeed;
    }

    /// PM_WaterMove.
    fn water_move(&mut self) {
        let c = self.cmd;
        let mut wishvel = self.forward * c.forwardmove + self.right * c.sidemove;
        if c.forwardmove == 0.0 && c.sidemove == 0.0 && c.upmove == 0.0 {
            wishvel.z -= 60.0;
        } else {
            wishvel.z += c.upmove;
        }
        let mut wishdir = wishvel;
        let mut wishspeed = normalize(&mut wishdir);
        if wishspeed > self.mv.maxspeed {
            wishspeed = self.mv.maxspeed;
        }
        wishspeed *= 0.7;
        self.accelerate(wishdir, wishspeed, self.mv.wateraccelerate);
        let dest = self.s.origin + self.s.velocity * self.frametime;
        let mut start = dest;
        start.z += STEPSIZE + 1.0;
        let tr = self.trace(start, dest);
        if !tr.startsolid && !tr.allsolid {
            self.s.origin = tr.endpos;
            return;
        }
        self.fly_move();
    }

    /// PM_AirMove.
    fn air_move(&mut self) {
        let (fmove, smove) = (self.cmd.forwardmove, self.cmd.sidemove);
        self.forward.z = 0.0;
        self.right.z = 0.0;
        normalize(&mut self.forward);
        normalize(&mut self.right);
        let mut wishvel = self.forward * fmove + self.right * smove;
        wishvel.z = 0.0;
        let mut wishdir = wishvel;
        let mut wishspeed = normalize(&mut wishdir);
        if wishspeed > self.mv.maxspeed {
            wishspeed = self.mv.maxspeed;
        }
        let g = self.mv.entgravity * self.mv.gravity * self.frametime;
        if self.onground != -1 {
            self.s.velocity.z = 0.0;
            self.accelerate(wishdir, wishspeed, self.mv.accelerate);
            self.s.velocity.z -= g;
            self.ground_move();
        } else {
            self.air_accelerate(wishdir, wishspeed, self.mv.accelerate);
            self.s.velocity.z -= g;
            self.fly_move();
        }
    }

    /// PM_CatagorizePosition.
    fn categorize_position(&mut self) {
        let mut point = self.s.origin;
        point.z -= 1.0;
        if self.s.velocity.z > 180.0 {
            self.onground = -1;
        } else {
            let tr = self.trace(self.s.origin, point);
            self.onground = if tr.normal.z < 0.7 { -1 } else { tr.ent };
            if self.onground != -1 {
                self.s.waterjumptime = 0.0;
                if !tr.startsolid && !tr.allsolid {
                    self.s.origin = tr.endpos;
                }
            }
        }
        self.waterlevel = 0;
        self.watertype = CONTENTS_EMPTY;
        let mut p = self.s.origin;
        p.z = self.s.origin.z + PLAYER_MINS.z + 1.0;
        let cont = self.point_contents(p);
        if cont <= CONTENTS_WATER {
            self.watertype = cont;
            self.waterlevel = 1;
            p.z = self.s.origin.z + (PLAYER_MINS.z + PLAYER_MAXS.z) * 0.5;
            if self.point_contents(p) <= CONTENTS_WATER {
                self.waterlevel = 2;
                p.z = self.s.origin.z + 22.0;
                if self.point_contents(p) <= CONTENTS_WATER {
                    self.waterlevel = 3;
                }
            }
        }
    }

    /// JumpButton.
    fn jump_button(&mut self) {
        if self.dead {
            self.s.oldbuttons |= BUTTON_JUMP;
            return;
        }
        if self.s.waterjumptime != 0.0 {
            self.s.waterjumptime = (self.s.waterjumptime - self.frametime).max(0.0);
            return;
        }
        if self.waterlevel >= 2 {
            self.onground = -1;
            self.s.velocity.z = match self.watertype {
                crate::CONTENTS_WATER => 100.0,
                crate::CONTENTS_SLIME => 80.0,
                _ => 50.0,
            };
            return;
        }
        if self.onground == -1 || self.s.oldbuttons & BUTTON_JUMP != 0 {
            return;
        }
        self.onground = -1;
        self.s.velocity.z += 270.0;
        self.s.oldbuttons |= BUTTON_JUMP;
    }

    /// CheckWaterJump.
    fn check_water_jump(&mut self) {
        if self.s.waterjumptime != 0.0 || self.s.velocity.z < -180.0 {
            return;
        }
        let mut flat = Vec3::new(self.forward.x, self.forward.y, 0.0);
        normalize(&mut flat);
        let mut spot = self.s.origin + flat * 24.0;
        spot.z += 8.0;
        if self.point_contents(spot) != CONTENTS_SOLID {
            return;
        }
        spot.z += 24.0;
        if self.point_contents(spot) != CONTENTS_EMPTY {
            return;
        }
        self.s.velocity = flat * 50.0;
        self.s.velocity.z = 310.0;
        self.s.waterjumptime = 2.0;
        self.s.oldbuttons |= BUTTON_JUMP;
    }

    /// NudgePosition.
    fn nudge_position(&mut self) {
        let base = self.s.origin;
        const SIGN: [f32; 3] = [0.0, -1.0, 1.0];
        for z in SIGN {
            for x in SIGN {
                for y in SIGN {
                    let p = base + Vec3::new(x, y, z) * 0.125;
                    if test_player_position(self.ents, p) {
                        self.s.origin = p;
                        return;
                    }
                }
            }
        }
        self.s.origin = base;
    }

    /// SpectatorMove.
    fn spectator_move(&mut self) {
        let speed = self.s.velocity.length();
        if speed < 1.0 {
            self.s.velocity = Vec3::ZERO;
        } else {
            let friction = self.mv.friction * 1.5;
            let control = if speed < self.mv.stopspeed { self.mv.stopspeed } else { speed };
            let newspeed = (speed - control * friction * self.frametime).max(0.0) / speed;
            self.s.velocity *= newspeed;
        }
        normalize(&mut self.forward);
        normalize(&mut self.right);
        let mut wishvel = self.forward * self.cmd.forwardmove + self.right * self.cmd.sidemove;
        wishvel.z += self.cmd.upmove;
        let mut wishdir = wishvel;
        let mut wishspeed = normalize(&mut wishdir);
        if wishspeed > self.mv.spectatormaxspeed {
            wishspeed = self.mv.spectatormaxspeed;
        }
        let addspeed = wishspeed - self.s.velocity.dot(wishdir);
        if addspeed > 0.0 {
            let accelspeed = (self.mv.accelerate * self.frametime * wishspeed).min(addspeed);
            self.s.velocity += wishdir * accelspeed;
        }
        self.s.origin += self.s.velocity * self.frametime;
    }
}

/// PlayerMove for one command; physents[0] must be the world.
pub fn player_move(ents: &[PhysEnt<'_>], mv: &MoveVars, from: &PlayerState, cmd: &MoveCmd, dead: bool, spectator: bool) -> PlayerState {
    let (forward, right, _) = angle_vectors(cmd.angles);
    let mut pm = Pm {
        ents,
        mv,
        s: *from,
        cmd: *cmd,
        angles: cmd.angles,
        dead,
        frametime: cmd.msec as f32 * 0.001,
        forward,
        right,
        onground: -1,
        waterlevel: 0,
        watertype: CONTENTS_EMPTY,
    };
    let _ = pm.angles;
    if spectator {
        pm.spectator_move();
        pm.s.oldbuttons = cmd.buttons;
        return pm.s;
    }
    pm.nudge_position();
    pm.categorize_position();
    if pm.waterlevel == 2 {
        pm.check_water_jump();
    }
    if pm.s.velocity.z < 0.0 {
        pm.s.waterjumptime = 0.0;
    }
    if cmd.buttons & BUTTON_JUMP != 0 {
        pm.jump_button();
    } else {
        pm.s.oldbuttons &= !BUTTON_JUMP;
    }
    pm.friction();
    if pm.waterlevel >= 2 {
        pm.water_move();
    } else {
        pm.air_move();
    }
    pm.categorize_position();
    let mut s = pm.s;
    s.onground = pm.onground != -1;
    // CL_PredictUsercmd: to->oldbuttons = pmove.cmd.buttons.
    s.oldbuttons = cmd.buttons;
    s
}

/// CL_PredictUsercmd: long moves are split in halves of at most 50 ms.
pub fn predict(ents: &[PhysEnt<'_>], mv: &MoveVars, from: &PlayerState, cmd: &MoveCmd, dead: bool, spectator: bool) -> PlayerState {
    if cmd.msec > 50 {
        let split = MoveCmd { msec: cmd.msec / 2, ..*cmd };
        let mid = predict(ents, mv, from, &split, dead, spectator);
        return predict(ents, mv, &mid, &split, dead, spectator);
    }
    player_move(ents, mv, from, cmd, dead, spectator)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world that is solid below z = 0 (one clip node on the z plane).
    fn floor() -> CollisionModel {
        use crate::hull::{CNode, SubModel};
        CollisionModel {
            planes: vec![HPlane { normal: Vec3::new(0.0, 0.0, 1.0), dist: 0.0, kind: 2 }],
            hull0: vec![CNode { plane: 0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] }],
            // Hull 1 is the floor raised by the player's mins (24 units).
            clipnodes: vec![CNode { plane: 1, children: [CONTENTS_EMPTY, CONTENTS_SOLID] }],
            submodels: vec![SubModel { mins: Vec3::splat(-4096.0), maxs: Vec3::splat(4096.0), origin: Vec3::ZERO, headnode: [0, 0, 0, 0] }],
            ..Default::default()
        }
        .with_plane(HPlane { normal: Vec3::new(0.0, 0.0, 1.0), dist: 24.0, kind: 2 })
    }

    trait WithPlane {
        fn with_plane(self, p: HPlane) -> Self;
    }
    impl WithPlane for CollisionModel {
        fn with_plane(mut self, p: HPlane) -> Self {
            self.planes.push(p);
            self
        }
    }

    #[test]
    fn falls_lands_and_runs() {
        let w = floor();
        let ents = [PhysEnt { origin: Vec3::ZERO, shape: PhysShape::Model(&w, 0) }];
        let mv = MoveVars::default();
        let mut s = PlayerState { origin: Vec3::new(0.0, 0.0, 100.0), ..Default::default() };
        let idle = MoveCmd { msec: 13, ..Default::default() };
        for _ in 0..200 {
            s = predict(&ents, &mv, &s, &idle, false, false);
        }
        assert!(s.onground && (s.origin.z - 24.0).abs() < 0.1, "{s:?}");
        let run = MoveCmd { msec: 13, forwardmove: 400.0, ..Default::default() };
        for _ in 0..100 {
            s = predict(&ents, &mv, &s, &run, false, false);
        }
        // Capped at maxspeed along +x (yaw 0).
        assert!((s.velocity.x - 320.0).abs() < 1.0 && s.velocity.y.abs() < 0.01, "{s:?}");
        let jump = MoveCmd { buttons: BUTTON_JUMP, ..run };
        let j = predict(&ents, &mv, &s, &jump, false, false);
        assert!(!j.onground && j.velocity.z > 200.0, "{j:?}");
        // Holding jump doesn't pogo.
        let mut s = j;
        for _ in 0..120 {
            s = predict(&ents, &mv, &s, &jump, false, false);
        }
        assert!(s.onground, "{s:?}");
        let again = predict(&ents, &mv, &s, &jump, false, false);
        assert!(again.onground);
    }
}
