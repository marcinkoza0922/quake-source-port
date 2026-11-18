//! Sky wind (ironwail/vkQuake gl_sky.c): a skybox's clouds drift by
//! sampling it twice along a wind direction and cross-fading the two over a
//! looping period. Set per skybox by `gfx/env/<sky>wind.cfg` (`skywind
//! distance yaw period pitch`), tuned live with the `skywind*` commands and
//! scaled by `r_skywind` (0 off, 2 twice as fast).

use qcore::cmd::com_parse;

/// Sky_LoadSkyBox's config name: `gfx/env/<sky>wind.cfg` (sky names end in
/// `_`, so mods ship `redcld_wind.cfg`).
pub fn cfg_path(sky: &str) -> String {
    format!("gfx/env/{sky}wind.cfg")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyWind {
    /// Drift amplitude, -2..=2 (0 = still).
    pub dist: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// Seconds per loop.
    pub period: f32,
}

impl Default for SkyWind {
    /// Skywind_Clear.
    fn default() -> Self {
        SkyWind { dist: 0.0, yaw: 45.0, pitch: 0.0, period: 30.0 }
    }
}

/// C fmod: the result has the sign of `a`.
fn fmod(a: f64, b: f64) -> f32 {
    (a % b) as f32
}

impl SkyWind {
    pub fn set_dist(&mut self, v: f64) {
        self.dist = v.clamp(-2.0, 2.0) as f32;
    }
    pub fn set_yaw(&mut self, v: f64) {
        self.yaw = fmod(v, 360.0);
    }
    pub fn set_pitch(&mut self, v: f64) {
        self.pitch = fmod(v + 90.0, 180.0) - 90.0;
    }

    /// Skywind_Load_f's parse: `skywind [distance [yaw [period [pitch]]]]`;
    /// `None` unless the first token is `skywind`.
    pub fn parse(text: &str) -> Option<SkyWind> {
        let (first, mut rest) = com_parse(text)?;
        if first != "skywind" {
            return None;
        }
        let mut w = SkyWind::default();
        let mut next = || {
            let (t, r) = com_parse(rest)?;
            rest = r;
            Some(qcore::cvar::q_atof(t) as f64)
        };
        if let Some(v) = next() {
            w.set_dist(v);
        }
        if let Some(v) = next() {
            w.set_yaw(v);
        }
        if let Some(v) = next() {
            w.period = v as f32;
        }
        if let Some(v) = next() {
            w.set_pitch(v);
        }
        Some(w)
    }

    /// Skywind_Save_f's file.
    pub fn save_text(&self) -> String {
        format!("// distance yaw period pitch\nskywind {} {} {} {}\n", self.dist, self.yaw, self.period, self.pitch)
    }

    /// Sky_SetupFrame: the wind vector in Quake world space (xyz) and the
    /// loop phase in [-0.5, 0.5) (w) at client time `t`; zero when still.
    pub fn uniform(&self, t: f64, r_skywind: f32) -> [f32; 4] {
        if r_skywind == 0.0 || self.dist == 0.0 {
            return [0.0; 4];
        }
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let dist = self.dist.clamp(-2.0, 2.0);
        let period = self.period / r_skywind;
        let mut phase = if period != 0.0 { t * 0.5 / f64::from(period) } else { 0.5 };
        phase -= phase.floor() + 0.5;
        // ironwail's cubemap axes are (-y, z, x) of Quake's: its winddir
        // (dist*cp*sy, dist*sp, -dist*cp*cy) in world space
        [-dist * cp * cy, -dist * cp * sy, dist * sp, phase as f32]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_shipped_configs() {
        // dmd's redcld_wind.cfg (CRLF), qbj3's comment says "angle"
        let w = SkyWind::parse("// distance yaw period pitch\r\nskywind 1.3 127 15 -7\r\n").unwrap();
        assert_eq!(w, SkyWind { dist: 1.3, yaw: 127.0, period: 15.0, pitch: -7.0 });
        // out of range values clamp/wrap like C
        let w = SkyWind::parse("skywind 50 400 12 100").unwrap();
        assert_eq!((w.dist, w.yaw, w.pitch), (2.0, 40.0, -80.0));
        // missing fields keep Skywind_Clear's values
        assert_eq!(SkyWind::parse("skywind 1").unwrap(), SkyWind { dist: 1.0, ..SkyWind::default() });
        assert!(SkyWind::parse("fog 1").is_none());
        assert_eq!(SkyWind::parse(&w.save_text()), Some(w));
    }

    #[test]
    fn uniform_direction_and_phase() {
        let w = SkyWind { dist: 1.0, yaw: 0.0, pitch: 0.0, period: 10.0 };
        // yaw 0: ironwail's cubemap -z, which is world -x
        let u = w.uniform(0.0, 1.0);
        assert!((u[0] + 1.0).abs() < 1e-6 && u[1].abs() < 1e-6 && u[2].abs() < 1e-6);
        assert_eq!(u[3], -0.5);
        // half a loop later (period / r_skywind = 10 s, phase t*0.5/period)
        assert!((w.uniform(10.0, 1.0)[3]).abs() < 1e-6);
        assert!((w.uniform(5.0, 2.0)[3]).abs() < 1e-6);
        assert_eq!(w.uniform(3.0, 0.0), [0.0; 4]);
        assert_eq!(SkyWind::default().uniform(3.0, 1.0), [0.0; 4]);
    }
}
