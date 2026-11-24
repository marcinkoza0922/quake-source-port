use super::*;

impl Ctx<'_> {
    pub(super) fn traceline(&mut self, vm: &mut Vm) -> Result<()> {
        let fix = |v: [f32; 3]| if v.iter().any(|x| x.is_nan()) { [0.0; 3] } else { v };
        let v1 = Vec3::from_array(fix(vm.g_v(p(0))));
        let v2 = Vec3::from_array(fix(vm.g_v(p(1))));
        let nomonsters = vm.g_f(p(2)) as i32;
        let pass = ent_arg(vm, 3)?;
        if crate::addons::shotgun::launch(vm, self.c, self.w, v1, v2, nomonsters, pass)? {
            // QC still owns spread, ammo, sound and refire; defer each pellet
            // collision and damage to missile physics.
            self.set_trace_globals(vm, &qworld::Trace { allsolid: false, inopen: true, ..qworld::Trace::new(v2) });
            return Ok(());
        }
        // a bolt's damage is traced along the bolt, not straight to its end
        let redirect = self.c.portal_bolts.damage_line(vm.time, pass, v1, v2);
        let (v1, v2) = redirect.unwrap_or((v1, v2));
        let mut entered = Vec::new();
        let tr = {
            let (ents, w) = self.ents(vm);
            let mt = MoveType::from(nomonsters);
            let active = !ents.c.linked_portals.is_empty() && crate::addons::active(ents.c);
            let mut from = v1;
            let mut to = v2;
            let mut pass_ent = Some(pass);
            let mut total_frac = 0.0f32;
            let mut hops = 0u32;
            loop {
                let mut tr = w.trace(&ents, from, Vec3::ZERO, Vec3::ZERO, to, mt, pass_ent);
                if active
                    && hops < 4
                    && let Some(hit) = crate::addons::portal_move::intersect_segment(&ents.c.linked_portals, from, to, &tr, 0.5)
                {
                    total_frac += (1.0 - total_frac) * hit.t;
                    from = hit.exit_pt;
                    to = hit.exit_pt + hit.rem;
                    pass_ent = None;
                    hops += 1;
                    entered.push(hit.a);
                    continue;
                }
                if hops > 0 {
                    tr.fraction = (total_frac + (1.0 - total_frac) * tr.fraction).min(1.0);
                }
                break tr;
            }
        };
        if !entered.is_empty() && redirect.is_none() {
            self.c.portal_bolts.record(crate::addons::portal_bolt::Crossing {
                time: vm.time,
                pass,
                start: v1,
                unfolded: v1 + (v2 - v1) * tr.fraction,
                end: tr.endpos,
                entered,
                bolt: false,
            });
        }
        self.set_trace_globals(vm, &tr);
        Ok(())
    }

    pub(super) fn set_trace_globals(&self, vm: &mut Vm, tr: &qworld::Trace) {
        let g = &self.c.sv.globals;
        vm.set_g_f(g.trace_allsolid, tr.allsolid as i32 as f32);
        vm.set_g_f(g.trace_startsolid, tr.startsolid as i32 as f32);
        vm.set_g_f(g.trace_fraction, tr.fraction);
        vm.set_g_f(g.trace_inwater, tr.inwater as i32 as f32);
        vm.set_g_f(g.trace_inopen, tr.inopen as i32 as f32);
        vm.set_g_v(g.trace_endpos, tr.endpos.to_array());
        vm.set_g_v(g.trace_plane_normal, tr.plane_normal.to_array());
        vm.set_g_f(g.trace_plane_dist, tr.plane_dist);
        vm.set_g_i(g.trace_ent, tr.ent.unwrap_or(0) as i32);
    }

    pub(super) fn new_check_client(&mut self, vm: &Vm, check: u32) -> u32 {
        let maxc = self.c.maxclients() as u32;
        let f = &self.c.sv.fields;
        let check = check.clamp(1, maxc);
        let mut i = if check == maxc { 1 } else { check + 1 };
        loop {
            if i == maxc + 1 {
                i = 1;
            }
            if i == check {
                break;
            }
            if !vm.is_free(i) && vm.ent_f(i, f.health) > 0.0 && (vm.ent_f(i, f.flags) as i32) & FL_NOTARGET == 0 {
                break;
            }
            i += 1;
        }
        let org = Vec3::from_array(vm.ent_v(i, f.origin)) + Vec3::from_array(vm.ent_v(i, f.view_ofs));
        let leaf = self.c.sv.vis.point_in_leaf(org);
        self.c.sv.checkpvs = self.c.sv.vis.leaf_pvs(leaf);
        i
    }

    pub(super) fn checkclient(&mut self, vm: &mut Vm) {
        if vm.time - self.c.sv.lastchecktime >= 0.1 {
            let lc = self.c.sv.lastcheck;
            self.c.sv.lastcheck = self.new_check_client(vm, lc);
            self.c.sv.lastchecktime = vm.time;
        }
        let f = &self.c.sv.fields;
        let ent = self.c.sv.lastcheck;
        if (ent as usize) >= vm.num_edicts() || vm.is_free(ent) || vm.ent_f(ent, f.health) <= 0.0 {
            vm.ret_e(0);
            return;
        }
        let s = vm.self_ent().min(vm.num_edicts() as u32 - 1);
        let view = Vec3::from_array(vm.ent_v(s, f.origin)) + Vec3::from_array(vm.ent_v(s, f.view_ofs));
        let l = self.c.sv.vis.point_in_leaf(view) as i64 - 1;
        let vis = l >= 0 && leafs_in_pvs(&[l as u32], &self.c.sv.checkpvs);
        vm.ret_e(if vis { ent } else { 0 });
    }
}
