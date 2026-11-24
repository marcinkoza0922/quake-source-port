//! One server frame of the bot, and what it notices about it: teleports,
//! being held in place, keys handed out by impulse 9.

use super::*;

impl Bot<'_> {
    /// Escape a spot QC keeps putting the bot back to (see `step`).
    pub(super) fn unpin(&mut self, at: Vec3) {
        for i in 0..self.goals.len() {
            let g = &self.goals[i];
            if g.kind == Kind::Trigger && !g.done && !self.vm().is_free(g.ent) {
                let (a, b) = self.bounds(g.ent);
                if (at.max(a).min(b) - at).length() < 64.0 {
                    self.goals[i].done = true;
                    if self.cur == Some(i) {
                        self.cur = None;
                    }
                }
            }
        }
        let to = self.cur.map_or(at + Vec3::new(0.0, 0.0, 128.0), |i| self.center(self.goals[i].ent));
        self.note(&format!("pinned at {at:?}, setpos to {to:?}"));
        self.sv.command(&format!("setpos {} {} {}", to.x, to.y, to.z));
    }

    pub(super) fn step(&mut self, mut mv: ClientMove) {
        // impulse 9 also hands out every key in id1: strip the keys it
        // added, so key doors stay locked until the key is picked up.
        let keys_before = (mv.impulse == 9).then(|| self.o.items.map(|it| (it, self.vm().ent_f(self.me, it) as i32))).flatten();
        let before = self.me_origin();
        let walking = self.begin_walk(&mut mv);
        self.sv.set_move(self.c, mv);
        self.sv.frame(DT);
        self.drain_log();
        if self.sv.level.is_none() {
            return;
        }
        if walking {
            self.end_walk();
        }
        // A teleporter moves the bot away before it is ever seen inside:
        // a jump further than flight goes counts for triggers it was at.
        let reach = SPEED * DT as f32 + 32.0;
        let moved = (self.me_origin() - before).length();
        // A teleporter whose destination lies inside it puts the bot back
        // every frame: count its triggers and jump out to the goal.
        let flying = self.vm().ent_f(self.me, self.o.movetype) as i32 == MOVETYPE_NOCLIP;
        self.pinned = if flying && moved < 1.0 && (mv.forward != 0.0 || mv.side != 0.0 || mv.up != 0.0) { self.pinned + 1 } else { 0 };
        if self.pinned > 20 {
            self.pinned = 0;
            self.unpin(before);
        }
        if moved > reach {
            for i in 0..self.goals.len() {
                let g = &self.goals[i];
                if g.kind == Kind::Trigger && !g.done && !self.vm().is_free(g.ent) {
                    let (a, b) = self.bounds(g.ent);
                    let p = before.max(a).min(b);
                    if (p - before).length() < reach {
                        self.goals[i].done = true;
                        if self.cur == Some(i) {
                            self.cur = None;
                        }
                    }
                }
            }
        }
        if let Some((it, before)) = keys_before
            && let Some(l) = self.sv.level.as_mut()
        {
            let now = l.vm.ent_f(self.me, it) as i32;
            let added = now & (IT_KEY1 | IT_KEY2) & !before;
            if added != 0 {
                l.vm.set_ent_f(self.me, it, (now & !added) as f32);
            }
        }
        let _ = self.sv.take_reliable(self.c);
        let _ = self.sv.take_datagrams(self.c);
    }
}
