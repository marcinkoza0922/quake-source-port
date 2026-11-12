use super::*;

impl Pm<'_, '_> {
    /// PM_Friction.
    pub(super) fn friction(&mut self) {
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
    pub(super) fn accelerate(&mut self, wishdir: Vec3, wishspeed: f32, accel: f32) {
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
    pub(super) fn air_accelerate(&mut self, wishdir: Vec3, wishspeed: f32, accel: f32) {
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
    pub(super) fn water_move(&mut self) {
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
    pub(super) fn air_move(&mut self) {
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
}
