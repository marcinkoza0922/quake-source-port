//! Screen Shader page: the slang preset picker and one slider per
//! `#pragma parameter` of the loaded preset (filled in by the host).
//! Overrides live in `r_shader_params` as `name=value;name=value`.

use super::*;

/// Parses `r_shader_params`.
pub(crate) fn parse_params(s: &str) -> Vec<(String, f32)> {
    s.split(';')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((k.trim().to_string(), v.trim().parse().ok()?))
        })
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

pub(crate) fn format_params(p: &[(String, f32)]) -> String {
    p.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(";")
}

/// `r_shader_params` with `id` set to `value` (None removes the override).
pub(crate) fn with_param(cur: &str, id: &str, value: Option<f32>) -> String {
    let mut ps = parse_params(cur);
    ps.retain(|(k, _)| k != id);
    if let Some(v) = value {
        ps.push((id.to_string(), v));
    }
    format_params(&ps)
}

fn params_str(cv: &Cvars) -> &str {
    cv.by_name("r_shader_params").map_or("", |c| c.string.as_str())
}

impl Menu {
    /// Value of the k-th preset parameter (override or preset default).
    pub(super) fn shader_param(&self, k: usize, cv: &Cvars) -> Option<(&qrender::ShaderParam, f32)> {
        let p = self.shader_params.get(k)?;
        let over = parse_params(params_str(cv)).into_iter().rev().find(|(id, _)| *id == p.id);
        Some((p, over.map_or(p.initial, |o| o.1)))
    }

    pub(super) fn set_shader_param(&self, k: usize, v: f32, cv: &mut Cvars) {
        let Some(p) = self.shader_params.get(k) else { return };
        let v = if p.step > 0.0 { p.min + ((v - p.min) / p.step).round() * p.step } else { v };
        let v = (v.clamp(p.min, p.max) * 10000.0).round() / 10000.0;
        let new = with_param(params_str(cv), &p.id, (v != p.initial).then_some(v));
        cv.set("r_shader_params", &new);
    }

    /// Steps a parameter slider.
    pub(super) fn adjust_shader_param(&self, k: usize, dir: i32, cv: &mut Cvars) -> bool {
        let Some((p, v)) = self.shader_param(k, cv) else { return false };
        let step = if p.step > 0.0 { p.step } else { (p.max - p.min) / 20.0 };
        self.set_shader_param(k, v + dir as f32 * step, cv);
        true
    }

    /// Preset dropdown entries: "off" then the presets.
    pub(super) fn preset_options(&self) -> Vec<String> {
        std::iter::once("off".to_string()).chain(self.shader_presets.iter().cloned()).collect()
    }

    pub(super) fn preset_index(&self, cv: &Cvars) -> usize {
        let cur = cv.by_name("r_shader_preset").map_or("", |c| c.string.trim());
        let cur = cur.trim_end_matches(".slangp");
        if cur.is_empty() {
            return 0;
        }
        self.shader_presets.iter().position(|p| p == cur).map_or(0, |i| i + 1)
    }

    pub(super) fn set_preset(&self, i: usize, cv: &mut Cvars) {
        let name = if i == 0 { "" } else { self.shader_presets.get(i - 1).map_or("", |s| s.as_str()) };
        cv.set("r_shader_preset", name);
        // overrides belong to the old preset
        cv.set("r_shader_params", "");
    }

    /// Draws a parameter row: label from the shader's description, slider, value.
    pub(super) fn draw_shader_param(&self, c: &mut Canvas, y: f32, k: usize, cv: &Cvars) {
        let Some((p, v)) = self.shader_param(k, cv) else { return };
        let l: String = p.description.trim().chars().take(30).collect();
        c.text_right(LABEL_R, y, if l.is_empty() { &p.id } else { &l }, TXT, FG);
        let f = if p.max > p.min { ((v - p.min) / (p.max - p.min)).clamp(0.0, 1.0) } else { 0.0 };
        let (tx, tw) = (VALUE_X, 92.0);
        c.round_box(tx, y + 3.0, tw, 3.0, 1.5, [0.25, 0.22, 0.19, 1.0], None);
        c.round_box(tx, y + 3.0, (tw * f).max(3.0), 3.0, 1.5, [0.80, 0.55, 0.22, 1.0], None);
        c.round_box(tx + (tw - 8.0) * f, y + 0.5, 8.0, 8.0, 4.0, [0.95, 0.80, 0.55, 1.0], Some([0.2, 0.12, 0.05, 0.8]));
        txt(c, VALUE_X + 100.0, y, &qcore::cvar::format_float((v * 1000.0).round() / 1000.0));
    }

    /// Sets a parameter slider from a pointer x.
    pub(super) fn drag_shader_param(&self, k: usize, x: f32, cv: &mut Cvars) {
        let Some(p) = self.shader_params.get(k) else { return };
        let f = ((x - SLIDER_X.0 - 4.0) / (SLIDER_X.1 - SLIDER_X.0 - 8.0)).clamp(0.0, 1.0);
        self.set_shader_param(k, p.min + f * (p.max - p.min), cv);
    }
}
