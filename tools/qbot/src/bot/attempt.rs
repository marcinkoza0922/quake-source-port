//! What happened during one attempt at a goal, so a goal that times out
//! says why ("never reached", "no clear shot", "shots don't hurt it", ...)
//! instead of just "timed out". Failure lines start with a fixed phrase
//! (before any numbers) so a sweep can group them.

use super::*;

/// Within this of the spot counts as having reached it.
const REACHED: f32 = 32.0;

pub(super) struct Attempt {
    /// Closest the bot came to where it wanted to be.
    closest: f32,
    /// Frames a spot (firing or touch) was known, out of all frames.
    spot_frames: u32,
    frames: u32,
    /// Frames with a clear shot at the target, and frames spent firing.
    clear: u32,
    fired: u32,
    /// Frames the player's box overlapped the target.
    inside: u32,
    /// Target health when the attempt started.
    health: f32,
    pub(super) warps: u32,
}

impl Attempt {
    pub(super) fn new(health: f32) -> Attempt {
        Attempt { closest: f32::INFINITY, spot_frames: 0, frames: 0, clear: 0, fired: 0, inside: 0, health, warps: 0 }
    }

    /// One frame of the attempt: distance to the wanted spot, if any.
    pub(super) fn frame(&mut self, dist: Option<f32>) {
        self.frames += 1;
        if let Some(d) = dist {
            self.spot_frames += 1;
            self.closest = self.closest.min(d);
        }
    }

    pub(super) fn shot(&mut self, clear: bool, firing: bool) {
        self.clear += clear as u32;
        self.fired += firing as u32;
    }

    pub(super) fn touching(&mut self, inside: bool) {
        self.inside += inside as u32;
    }

    pub(super) fn inside_secs(&self) -> f64 {
        Attempt::secs(self.inside)
    }

    fn reached(&self) -> bool {
        self.closest < REACHED
    }

    fn secs(n: u32) -> f64 {
        n as f64 * DT
    }
}

impl Bot<'_> {
    /// Why goal `i` got nowhere in its attempt.
    pub(super) fn diagnose(&self, i: usize) -> String {
        let a = &self.att;
        let g = &self.goals[i];
        let vm = self.vm();
        let health = vm.ent_f(g.ent, self.o.health);
        let shootable = health > 0.0 && vm.ent_f(g.ent, self.o.takedamage) != 0.0;
        let far = || format!("closest {:.0}u", a.closest);
        let mut why = match g.kind {
            Kind::Monster | Kind::Shoot => self.shot_why(health),
            Kind::Button if shootable => self.shot_why(health),
            Kind::Button => {
                let state = self.o.state.map_or(0.0, |s| vm.ent_f(g.ent, s));
                if a.spot_frames == 0 {
                    "no free spot touching it".to_string()
                } else if !a.reached() {
                    format!("never reached a touch spot ({})", far())
                } else {
                    format!("pushed {} times, state stayed {state}", self.pushes)
                }
            }
            Kind::Secret | Kind::Trigger | Kind::Pickup | Kind::Exit => {
                if a.inside > 0 {
                    format!("inside it {:.1}s, touch never fired", Attempt::secs(a.inside))
                } else if a.reached() {
                    "reached its spot but never overlapped it".to_string()
                } else {
                    format!("never reached it ({})", far())
                }
            }
        };
        if a.warps > 0 {
            why += &format!(" [warped {}x]", a.warps);
        }
        why
    }

    fn shot_why(&self, health: f32) -> String {
        let a = &self.att;
        if a.spot_frames == 0 {
            "no open spot with a clear shot".to_string()
        } else if !a.reached() {
            format!("never reached its firing spot (closest {:.0}u)", a.closest)
        } else if a.clear == 0 {
            "at its firing spot but never a clear shot".to_string()
        } else if a.fired == 0 {
            "clear shot but never fired".to_string()
        } else if health >= a.health {
            format!("shots do no damage (fired {:.1}s, health {health})", Attempt::secs(a.fired))
        } else {
            format!("damaged but not killed (health {} -> {health})", a.health)
        }
    }
}
