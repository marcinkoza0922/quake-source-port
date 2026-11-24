//! Touching things for real. Some progs (Copper and what builds on it, qbj)
//! ignore players in noclip: items, triggers, secrets and exits only react
//! to one that walks. There the bot stands still and walks for a single
//! frame when it first overlaps such a goal, then flies on. With other progs
//! it only tries that on what hasn't reacted to it.

use super::*;

/// Between walking frames inside the current goal (an item that refuses).
const RETRY: f64 = 0.25;
/// Inside an item or exit this long without it reacting: walk.
const REFUSED: f64 = 1.0;

/// Whether touch code tests `other.movetype == MOVETYPE_NOCLIP`. Code for
/// pits, liquids and projectiles (alkaline) doesn't count: it isn't about
/// what the bot has to touch.
pub(super) fn ignores_noclip(p: &qvm::Progs) -> bool {
    let (Some(other), Some(mt)) = (p.find_global("other"), p.find_field("movetype")) else { return false };
    let int = |ofs: u16| p.globals.get(ofs as usize).map(|&g| g as i32);
    let is_noclip = |ofs: u16| p.globals.get(ofs as usize).is_some_and(|&g| f32::from_bits(g) == MOVETYPE_NOCLIP as f32);
    let mut starts: Vec<(usize, usize)> =
        p.functions.iter().enumerate().filter(|(_, f)| f.first_statement > 0).map(|(i, f)| (f.first_statement as usize, i)).collect();
    starts.sort_unstable();
    p.statements.iter().enumerate().any(|(i, s)| {
        // LOAD_* other.movetype, then EQ_* or NE_* with 8
        (24..=29).contains(&s.op)
            && s.a == other.ofs
            && int(s.b) == Some(mt.ofs as i32)
            && p.statements[i + 1..].iter().take(6).any(|c| {
                (10..=19).contains(&c.op) && (c.a == s.c && is_noclip(c.b) || c.b == s.c && is_noclip(c.a))
            })
            && {
                let f = starts[starts.partition_point(|x| x.0 <= i).saturating_sub(1)].1;
                let name = p.function_name(f);
                !["void", "liquid", "projectile"].iter().any(|n| name.contains(n))
            }
    })
}

impl Bot<'_> {
    /// Whether this frame must be a walking one; makes it so. Called just
    /// before the server frame.
    pub(super) fn begin_walk(&mut self, mv: &mut ClientMove) -> bool {
        let due = (0..self.goals.len()).any(|i| self.walk_due(i));
        let mt = self.vm().ent_f(self.me, self.o.movetype) as i32;
        if !due || mt != MOVETYPE_NOCLIP && mt != qworld::MOVETYPE_WALK {
            return false;
        }
        // Already walking (into a button), or a pit that kills walkers
        // whatever god mode says: the overlap has to do, as before.
        if mt == MOVETYPE_NOCLIP && !self.over_pit() {
            (mv.forward, mv.side, mv.up) = (0.0, 0.0, 0.0);
            self.set_noclip(false);
            let (me, at) = (self.me, self.me_origin());
            let vm = &mut self.sv.level.as_mut().unwrap().vm;
            for (name, val) in [("velocity", Vec3::ZERO), ("oldorigin", at)] {
                // SV_CheckStuck puts a stuck walker back at `oldorigin`,
                // which noclip never updates.
                if let Some(f) = vm.field_ofs(name) {
                    vm.set_ent_v(me, f, [val.x, val.y, val.z]);
                }
            }
        }
        true
    }

    /// Whether goal `i` waits for the bot to touch it walking.
    fn walk_due(&self, i: usize) -> bool {
        let g = &self.goals[i];
        let vm = self.vm();
        let t = self.sv.time();
        if g.done || !matches!(g.kind, Kind::Secret | Kind::Trigger | Kind::Pickup | Kind::Exit) {
            return false;
        }
        let now = self.cur == Some(i) && t - g.walked_at > RETRY;
        let wanted = match g.kind {
            _ if self.walks => !g.walked || now,
            Kind::Secret => !g.walked,
            Kind::Pickup | Kind::Exit => now && self.att.inside_secs() > REFUSED,
            _ => false,
        };
        wanted
            && !vm.is_free(g.ent)
            && vm.ent_f(g.ent, self.o.solid) as i32 == SOLID_TRIGGER
            && vm.ent_i(g.ent, self.o.touch) != 0
            && self.inside(g.ent)
    }

    /// The frame ran: what the bot overlaps has been touched. Back to
    /// noclip, unless QC took the movement over (a cutscene, a death).
    pub(super) fn end_walk(&mut self) {
        let t = self.sv.time();
        for i in 0..self.goals.len() {
            let (ent, done) = (self.goals[i].ent, self.goals[i].done);
            if !done && !self.vm().is_free(ent) && self.inside(ent) {
                (self.goals[i].walked, self.goals[i].walked_at) = (true, t);
            }
        }
        if t >= self.pushing_until && self.vm().ent_f(self.me, self.o.movetype) as i32 == qworld::MOVETYPE_WALK {
            self.set_noclip(true);
        }
    }

    /// Walking with a pit just below: fly off before falling in.
    pub(super) fn falling_into_pit(&self) -> bool {
        self.pit_below(96.0)
    }

    /// Inside one of Copper's bottomless pits (func_void, trigger_void,
    /// trigger_plunge), which kill a walking player outright.
    fn over_pit(&self) -> bool {
        self.pit_below(0.0)
    }

    /// A pit overlaps the player's box stretched down by `down`.
    fn pit_below(&self, down: f32) -> bool {
        let vm = self.vm();
        let (p, o) = (self.me_origin(), &self.o);
        (1..vm.num_edicts() as u32).any(|e| {
            if vm.is_free(e) || vm.ent_f(e, o.solid) as i32 != SOLID_TRIGGER || !is_pit(&vm.classname(e)) {
                return false;
            }
            let (a, b) = self.bounds(e);
            p.x + 16.0 > a.x && p.x - 16.0 < b.x && p.y + 16.0 > a.y && p.y - 16.0 < b.y && p.z + 32.0 > a.z && p.z - 24.0 - down < b.z
        })
    }
}

fn is_pit(cls: &str) -> bool {
    cls.contains("void") || cls.contains("plunge")
}

#[cfg(test)]
mod tests {
    #[test]
    fn pits() {
        assert!(["func_void", "trigger_void", "trigger_plunge"].iter().all(|c| super::is_pit(c)));
        assert!(!super::is_pit("trigger_once"));
    }
}
