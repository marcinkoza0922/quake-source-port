//! CSQC 3D scene builtins: QC entity submission, view properties and dynamic lights.

use qrender::{DLight, RenderEntity};
use qvm::Vm;

use super::super::scene::*;
use super::{CsqcHost, p};

impl CsqcHost<'_, '_> {
    fn add_qc_entity(&mut self, vm: &Vm, e: u32) {
        if e == 0 || vm.is_free(e) {
            return;
        }
        let f = self.state.fields;
        let ef = |ofs: usize, def: f32| if ofs != usize::MAX { vm.ent_f(e, ofs) } else { def };
        let ev = |ofs: usize| if ofs != usize::MAX { vm.ent_v(e, ofs) } else { [0.0; 3] };

        let mi = ef(f.modelindex, 0.0) as i32;
        let model_id = if mi > 0 {
            self.env.level.models.get(mi as usize).and_then(|m| *m).map(|m| m.id)
        } else if mi < 0 {
            let name = self.model_name_for_index(mi).to_string();
            self.resolve_model(&name)
        } else if f.model != usize::MAX {
            let h = vm.ent_i(e, f.model);
            let name = vm.strings.get(h).ok().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
            self.resolve_model(&name)
        } else {
            None
        };
        let Some(model) = model_id else { return };

        let rf = ef(f.renderflags, 0.0) as u32;
        if (rf & RF_EXTERNALMODEL) != 0 {
            return;
        }
        let alpha = ef(f.alpha, 0.0);
        let alpha = if alpha == 0.0 { 1.0 } else { alpha.clamp(0.0, 1.0) };
        let frame = ef(f.frame, 0.0).max(0.0) as u32;
        let frame2 = if f.frame2 != usize::MAX { ef(f.frame2, frame as f32).max(0.0) as u32 } else { frame };
        let lerp = if f.lerpfrac != usize::MAX { (1.0 - ef(f.lerpfrac, 0.0)).clamp(0.0, 1.0) } else { 1.0 };
        let cm = ef(f.colormap, 0.0) as i32;
        let colormap = if cm > 0 && cm <= self.env.cl.maxclients as i32 {
            self.env.cl.scores.get((cm - 1) as usize).map(|s| [(s.colors >> 4) as u8, (s.colors & 15) as u8])
        } else {
            None
        };
        let effects = ef(f.effects, 0.0) as u32;
        self.state.scene.entities.push(RenderEntity {
            model,
            origin: ev(f.origin),
            angles: ev(f.angles),
            frame,
            prev_frame: frame2,
            lerp,
            skin: ef(f.skin, 0.0).max(0.0) as u32,
            colormap,
            alpha,
            viewmodel: (rf & (RF_VIEWMODEL | RF_DEPTHHACK)) != 0,
            fullbright: (effects & 128) != 0,
            ..Default::default()
        });
    }

    fn set_property(&mut self, vm: &Vm) {
        let prop = vm.g_f(p(0)) as i32;
        let sc = &mut self.state.scene;
        match prop {
            VF_MIN => { let v = vm.g_v(p(1)); sc.min = [v[0], v[1]]; }
            VF_MIN_X => sc.min[0] = vm.g_f(p(1)),
            VF_MIN_Y => sc.min[1] = vm.g_f(p(1)),
            VF_SIZE => { let v = vm.g_v(p(1)); sc.size = [v[0], v[1]]; }
            VF_SIZE_X => sc.size[0] = vm.g_f(p(1)),
            VF_SIZE_Y => sc.size[1] = vm.g_f(p(1)),
            VF_VIEWPORT => {
                let mn = vm.g_v(p(1));
                let sz = vm.g_v(p(2));
                sc.min = [mn[0], mn[1]];
                sc.size = [sz[0], sz[1]];
            }
            VF_FOV => {
                let v = vm.g_v(p(1));
                if v[0] > 1.0 {
                    sc.fov_x = v[0];
                }
                if v[1] > 1.0 {
                    sc.fov_y = v[1];
                }
            }
            VF_FOVX => sc.fov_x = vm.g_f(p(1)),
            VF_FOVY => sc.fov_y = vm.g_f(p(1)),
            VF_ORIGIN => sc.origin = vm.g_v(p(1)),
            VF_ORIGIN_X => sc.origin[0] = vm.g_f(p(1)),
            VF_ORIGIN_Y => sc.origin[1] = vm.g_f(p(1)),
            VF_ORIGIN_Z => sc.origin[2] = vm.g_f(p(1)),
            VF_ANGLES | VF_CL_VIEWANGLES => sc.angles = vm.g_v(p(1)),
            VF_ANGLES_X => sc.angles[0] = vm.g_f(p(1)),
            VF_ANGLES_Y => sc.angles[1] = vm.g_f(p(1)),
            VF_ANGLES_Z => sc.angles[2] = vm.g_f(p(1)),
            VF_DRAWWORLD => sc.draw_world = vm.g_f(p(1)) != 0.0,
            VF_DRAWENGINESBAR => sc.draw_engine_sbar = vm.g_f(p(1)) != 0.0,
            VF_DRAWCROSSHAIR => sc.draw_crosshair = vm.g_f(p(1)) != 0.0,
            VF_PERSPECTIVE => {}
            _ => {}
        }
    }

    fn get_property(&self, vm: &mut Vm) {
        let prop = vm.g_f(p(0)) as i32;
        let sc = &self.state.scene;
        match prop {
            VF_MIN => vm.ret_v([sc.min[0], sc.min[1], 0.0]),
            VF_MIN_X => vm.ret_f(sc.min[0]),
            VF_MIN_Y => vm.ret_f(sc.min[1]),
            VF_SIZE => vm.ret_v([sc.size[0], sc.size[1], 0.0]),
            VF_SIZE_X => vm.ret_f(sc.size[0]),
            VF_SIZE_Y => vm.ret_f(sc.size[1]),
            VF_FOV => vm.ret_v([sc.fov_x, sc.fov_y, 0.0]),
            VF_FOVX => vm.ret_f(sc.fov_x),
            VF_FOVY => vm.ret_f(sc.fov_y),
            VF_ORIGIN => vm.ret_v(sc.origin),
            VF_ORIGIN_X => vm.ret_f(sc.origin[0]),
            VF_ORIGIN_Y => vm.ret_f(sc.origin[1]),
            VF_ORIGIN_Z => vm.ret_f(sc.origin[2]),
            VF_ANGLES | VF_CL_VIEWANGLES => vm.ret_v(sc.angles),
            VF_ANGLES_X => vm.ret_f(sc.angles[0]),
            VF_ANGLES_Y => vm.ret_f(sc.angles[1]),
            VF_ANGLES_Z => vm.ret_f(sc.angles[2]),
            VF_DRAWWORLD => vm.ret_f(if sc.draw_world { 1.0 } else { 0.0 }),
            VF_DRAWENGINESBAR => vm.ret_f(if sc.draw_engine_sbar { 1.0 } else { 0.0 }),
            VF_DRAWCROSSHAIR => vm.ret_f(if sc.draw_crosshair { 1.0 } else { 0.0 }),
            VF_PERSPECTIVE => vm.ret_f(1.0),
            _ => vm.ret_v([0.0; 3]),
        }
    }

    /// Handles the scene builtins; returns `false` if `name` isn't one of them.
    pub(super) fn ext_scene(&mut self, vm: &mut Vm, name: &str) -> bool {
        match name {
            // ---- 3D Scene ----
            "clearscene" => {
                self.state.scene.clear(self.env.base_view, self.env.virt[0], self.env.virt[1]);
            }
            "addentities" => {
                let mask = vm.g_f(p(0)) as u32;
                if mask & MASK_ENGINE != 0 {
                    self.state.scene.include_engine_ents = true;
                }
                if mask & MASK_VIEWMODEL != 0 {
                    self.state.scene.include_viewmodel = true;
                }
                // Also add any CSQC-owned edicts whose drawmask matches
                let dm_ofs = self.state.fields.drawmask;
                if dm_ofs != usize::MAX {
                    let n = vm.num_edicts() as u32;
                    for e in 1..n {
                        if !vm.is_free(e) && ((vm.ent_f(e, dm_ofs) as u32) & mask) != 0 {
                            self.add_qc_entity(vm, e);
                        }
                    }
                }
            }
            "addentity" => {
                let e = vm.g_i(p(0)) as u32;
                self.add_qc_entity(vm, e);
            }
            "setproperty" | "setviewprop" => {
                self.set_property(vm);
                vm.ret_f(1.0);
            }
            "getproperty" | "getviewprop" => self.get_property(vm),
            "renderscene" => {
                self.state.scene.rendered = true;
            }
            "dynamiclight_add" => {
                let org = vm.g_v(p(0));
                let radius = vm.g_f(p(1));
                let color = vm.g_v(p(2));
                self.state.scene.dlights.push(DLight {
                    origin: org,
                    radius,
                    minlight: 0.0,
                    color,
                    key: 0,
                });
            }
            "project" => {
                let v = self.state.scene.project(vm.g_v(p(0)));
                vm.ret_v(v);
            }
            "unproject" => {
                let v = self.state.scene.unproject(vm.g_v(p(0)));
                vm.ret_v(v);
            }

            _ => return false,
        }
        true
    }
}
