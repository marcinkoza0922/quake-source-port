//! `qworld::Edicts` on top of the QC edicts.

use qvm::Vm;
use qworld::{E, Edicts, Ent, F, V, Vec3, World};

use crate::server::{Core, exec};

pub(crate) struct Ents<'a> {
    pub vm: &'a mut Vm,
    pub c: &'a mut Core,
}

impl Ents<'_> {
    #[inline]
    fn ok(&self, e: Ent) -> bool {
        (e as usize) < self.vm.num_edicts()
    }
}

impl Edicts for Ents<'_> {
    fn num_edicts(&self) -> u32 {
        self.vm.num_edicts() as u32
    }
    fn is_free(&self, e: Ent) -> bool {
        self.vm.is_free(e)
    }
    fn v(&self, e: Ent, f: V) -> Vec3 {
        if !self.ok(e) {
            return Vec3::ZERO;
        }
        Vec3::from_array(self.vm.ent_v(e, self.c.sv.fields.v(f)))
    }
    fn set_v(&mut self, e: Ent, f: V, v: Vec3) {
        if self.ok(e) {
            let o = self.c.sv.fields.v(f);
            self.vm.set_ent_v(e, o, v.to_array());
        }
    }
    fn f(&self, e: Ent, f: F) -> f32 {
        match self.c.sv.fields.f(f) {
            Some(o) if self.ok(e) => self.vm.ent_f(e, o),
            _ => 0.0,
        }
    }
    fn set_f(&mut self, e: Ent, f: F, v: f32) {
        if let Some(o) = self.c.sv.fields.f(f)
            && self.ok(e)
        {
            self.vm.set_ent_f(e, o, v);
        }
    }
    fn e(&self, e: Ent, f: E) -> Ent {
        if !self.ok(e) {
            return 0;
        }
        let v = self.vm.ent_i(e, self.c.sv.fields.e(f));
        if v < 0 || v as usize >= self.vm.num_edicts() { 0 } else { v as Ent }
    }
    fn set_e(&mut self, e: Ent, f: E, v: Ent) {
        if self.ok(e) {
            let o = self.c.sv.fields.e(f);
            self.vm.set_ent_i(e, o, v as i32);
        }
    }
    fn has_touch(&self, e: Ent) -> bool {
        self.ok(e) && self.vm.ent_i(e, self.c.sv.fields.touch) != 0
    }
    fn touch(&mut self, w: &mut World, ent: Ent, other: Ent) {
        if !self.ok(ent) || !self.ok(other) {
            return;
        }
        let g = &self.c.sv.globals;
        let (gs, go, gt) = (g.self_, g.other, g.time);
        let (oself, oother) = (self.vm.g_i(gs), self.vm.g_i(go));
        let f = self.vm.ent_i(ent, self.c.sv.fields.touch);
        self.vm.set_g_i(gs, ent as i32);
        self.vm.set_g_i(go, other as i32);
        self.vm.set_g_f(gt, self.vm.time as f32);
        exec(self.vm, self.c, w, f);
        self.vm.set_g_i(gs, oself);
        self.vm.set_g_i(go, oother);
    }
    fn blocked(&mut self, w: &mut World, pusher: Ent, other: Ent) {
        if !self.ok(pusher) {
            return;
        }
        let f = self.vm.ent_i(pusher, self.c.sv.fields.blocked);
        if f == 0 {
            return;
        }
        let g = &self.c.sv.globals;
        let (gs, go) = (g.self_, g.other);
        self.vm.set_g_i(gs, pusher as i32);
        self.vm.set_g_i(go, other as i32);
        exec(self.vm, self.c, w, f);
    }
    fn think(&mut self, w: &mut World, ent: Ent, time: f64) {
        if !self.ok(ent) {
            return;
        }
        let fr = self.vm.ent_f(ent, self.c.sv.fields.frame);
        let x = self.c.ext(ent);
        x.oldthinktime = time as f32;
        x.oldframe = fr;
        let g = &self.c.sv.globals;
        let (gs, go, gt) = (g.self_, g.other, g.time);
        let f = self.vm.ent_i(ent, self.c.sv.fields.think);
        self.vm.set_g_f(gt, time as f32);
        self.vm.set_g_i(gs, ent as i32);
        self.vm.set_g_i(go, 0);
        if f == 0 {
            let msg = format!("PR_ExecuteProgram: NULL function (think of {})", self.vm.classname(ent));
            self.c.host_error(qvm::VmError::new(msg));
            return;
        }
        exec(self.vm, self.c, w, f);
    }
    fn set_leafs(&mut self, e: Ent, leafs: &[u32]) {
        let x = self.c.ext(e);
        x.leafs.clear();
        x.leafs.extend_from_slice(leafs);
    }
    fn start_sound(&mut self, e: Ent, sample: &str) {
        crate::send::start_sound(self.vm, self.c, e, 0, sample.as_bytes(), 255, 1.0);
    }
    fn rand(&mut self) -> i32 {
        self.vm.rand15() as i32
    }
}
