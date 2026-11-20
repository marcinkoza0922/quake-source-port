//! CSQC 3D scene builder (`clearscene`, `addentities`, `addentity`, `setproperty`,
//! `getproperty`, `renderscene`, `dynamiclight_add`, `project`, `unproject`).

use qcore::math::{Vec3, angle_vectors};
use qrender::{DLight, RenderEntity, View};

// DP/FTE `setproperty`/`getproperty` (VF_*) constants
pub const VF_MIN: i32 = 1;
pub const VF_MIN_X: i32 = 2;
pub const VF_MIN_Y: i32 = 3;
pub const VF_SIZE: i32 = 4;
pub const VF_SIZE_X: i32 = 5;
pub const VF_SIZE_Y: i32 = 6;
pub const VF_VIEWPORT: i32 = 7;
pub const VF_FOV: i32 = 8;
pub const VF_FOVX: i32 = 9;
pub const VF_FOVY: i32 = 10;
pub const VF_ORIGIN: i32 = 11;
pub const VF_ORIGIN_X: i32 = 12;
pub const VF_ORIGIN_Y: i32 = 13;
pub const VF_ORIGIN_Z: i32 = 14;
pub const VF_ANGLES: i32 = 15;
pub const VF_ANGLES_X: i32 = 16;
pub const VF_ANGLES_Y: i32 = 17;
pub const VF_ANGLES_Z: i32 = 18;
pub const VF_DRAWWORLD: i32 = 19;
pub const VF_DRAWENGINESBAR: i32 = 20;
pub const VF_DRAWCROSSHAIR: i32 = 21;
pub const VF_CL_VIEWANGLES: i32 = 33;
pub const VF_PERSPECTIVE: i32 = 200;

pub const MASK_ENGINE: u32 = 1;
pub const MASK_VIEWMODEL: u32 = 2;

pub const RF_VIEWMODEL: u32 = 1;
pub const RF_EXTERNALMODEL: u32 = 2;
pub const RF_DEPTHHACK: u32 = 4;

/// Scene state built during `CSQC_UpdateView`.
#[derive(Default)]
pub struct SceneState {
    pub rendered: bool,
    pub draw_world: bool,
    pub draw_engine_sbar: bool,
    pub draw_crosshair: bool,
    pub include_engine_ents: bool,
    pub include_viewmodel: bool,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub fov_x: f32,
    pub fov_y: f32,
    pub min: [f32; 2],
    pub size: [f32; 2],
    pub entities: Vec<RenderEntity>,
    pub dlights: Vec<DLight>,
}

impl SceneState {
    pub fn clear(&mut self, base_view: Option<View>, w: f32, h: f32) {
        let v = base_view.unwrap_or_default();
        self.rendered = false;
        self.draw_world = true;
        self.draw_engine_sbar = false;
        self.draw_crosshair = false;
        self.include_engine_ents = false;
        self.include_viewmodel = false;
        self.origin = v.origin;
        self.angles = v.angles;
        self.fov_x = if v.fov_x > 1.0 { v.fov_x } else { 90.0 };
        self.fov_y = (self.fov_x.to_radians() * 0.5).tan() * (h.max(1.0) / w.max(1.0));
        self.fov_y = self.fov_y.atan().to_degrees() * 2.0;
        self.min = [0.0, 0.0];
        self.size = [w, h];
        self.entities.clear();
        self.dlights.clear();
    }

    pub fn view(&self, base: View) -> View {
        View {
            origin: self.origin,
            angles: self.angles,
            fov_x: self.fov_x.clamp(10.0, 170.0),
            ..base
        }
    }

    /// World point -> 2D screen `(x, y, depth)` in CSQC virtual coordinates.
    pub fn project(&self, world: [f32; 3]) -> [f32; 3] {
        let (fwd, right, up) = angle_vectors(Vec3::from_array(self.angles));
        let d = Vec3::from_array(world) - Vec3::from_array(self.origin);
        let z = d.dot(fwd);
        if z <= 1e-4 {
            return [-1.0, -1.0, z];
        }
        let x = d.dot(right);
        let y = d.dot(up);
        let tx = (self.fov_x.to_radians() * 0.5).tan().max(1e-4);
        let aspect = self.size[0].max(1.0) / self.size[1].max(1.0);
        let ty = tx / aspect;
        let ndc_x = x / (z * tx);
        let ndc_y = y / (z * ty);
        [
            self.min[0] + (ndc_x * 0.5 + 0.5) * self.size[0],
            self.min[1] + (0.5 - ndc_y * 0.5) * self.size[1],
            z,
        ]
    }

    /// 2D screen `(x, y, dist)` -> world point.
    pub fn unproject(&self, screen: [f32; 3]) -> [f32; 3] {
        let (fwd, right, up) = angle_vectors(Vec3::from_array(self.angles));
        let tx = (self.fov_x.to_radians() * 0.5).tan().max(1e-4);
        let aspect = self.size[0].max(1.0) / self.size[1].max(1.0);
        let ty = tx / aspect;
        let ndc_x = ((screen[0] - self.min[0]) / self.size[0].max(1.0)) * 2.0 - 1.0;
        let ndc_y = 1.0 - ((screen[1] - self.min[1]) / self.size[1].max(1.0)) * 2.0;
        let z = screen[2].max(0.0);
        let dir = fwd + right * (ndc_x * tx) + up * (ndc_y * ty);
        (Vec3::from_array(self.origin) + dir * z).to_array()
    }
}
