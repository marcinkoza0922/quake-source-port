use super::*;

impl Pm<'_, '_> {
    /// PM_CatagorizePosition.
    pub(super) fn categorize_position(&mut self) {
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
    pub(super) fn jump_button(&mut self) {
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
    pub(super) fn check_water_jump(&mut self) {
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
    pub(super) fn nudge_position(&mut self) {
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
    pub(super) fn spectator_move(&mut self) {
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
