//! The assault rifle's view models (`tools/build_rifle.py`): the body, the
//! magazine (its own model, so that reloading can swap it) and the tube red
//! dot sight, all in one space: X forward, Y left, Z up.
use crate::gfx::Renderer;
use qcore::math::Vec3;
use qrender::ModelId;

/// Middle of the sight's tube, its inner radius and length.
pub const LENS: Vec3 = Vec3::new(0.4, 0.0, 11.3);
pub const APERTURE: f32 = 1.75;
pub const TUBE: f32 = 3.4;
/// End of the flash hider and the ejection port.
pub const MUZZLE: Vec3 = Vec3::new(26.4, -0.1, 5.6);
pub const PORT: Vec3 = Vec3::new(-1.6, -1.9, 4.6);

/// Body, sight and magazine as models (for player bodies).
pub fn meshes(palette: &[u8]) -> [qformats::Mdl; 3] {
    use super::toy_model::parse;
    let skin: &[u8] = include_bytes!("../../assets/rifle/texture.rgb.zlib");
    [
        parse(include_bytes!("../../assets/rifle/model.mdl.zlib"), skin, palette),
        parse(include_bytes!("../../assets/rifle/sight.mdl.zlib"), include_bytes!("../../assets/rifle/sight.rgb.zlib"), palette),
        parse(include_bytes!("../../assets/rifle/mag.mdl.zlib"), skin, palette),
    ]
}

/// Body, sight and magazine.
pub fn models(lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<[ModelId; 3]> {
    const SKIN: &[u8] = include_bytes!("../../assets/rifle/texture.rgb.zlib");
    let mut get = |key, mdl, rgb| super::toy_model::get(key, mdl, rgb, false, lv, fs, r);
    Some([
        get("@rifle", include_bytes!("../../assets/rifle/model.mdl.zlib") as &[u8], SKIN)?,
        get("@rifle_sight", include_bytes!("../../assets/rifle/sight.mdl.zlib"), include_bytes!("../../assets/rifle/sight.rgb.zlib"))?,
        get("@rifle_mag", include_bytes!("../../assets/rifle/mag.mdl.zlib"), SKIN)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_load_and_the_tube_is_open_where_the_code_says() {
        let palette: Vec<u8> = (0..256).flat_map(|i| [i as u8; 3]).collect();
        let body = super::super::toy_model::parse(include_bytes!("../../assets/rifle/model.mdl.zlib"), include_bytes!("../../assets/rifle/texture.rgb.zlib"), &palette);
        assert!(body.triangles.len() > 2000 && body.triangles.len() <= 4000);
        let pose = &body.frames[0].poses[0];
        let (lo, hi) = (body.decode(pose.bboxmin), body.decode(pose.bboxmax));
        assert!((hi[0] - 26.0).abs() < 0.1 && (lo[0] + 26.0).abs() < 0.1);
        let sight = super::super::toy_model::parse(include_bytes!("../../assets/rifle/sight.mdl.zlib"), include_bytes!("../../assets/rifle/sight.rgb.zlib"), &palette);
        let pose = &sight.frames[0].poses[0];
        let at = |v: i32| Vec3::from(sight.decode(pose.verts[v as usize]));
        // no triangle of the sight crosses the line of sight
        for t in &sight.triangles {
            let p = t.verts.map(at).map(|p| p - LENS);
            let m = (p[0] + p[1] + p[2]) / 3.0;
            let near = p.iter().chain([&m]).map(|p| (p.y * p.y + p.z * p.z).sqrt()).fold(f32::MAX, f32::min);
            assert!(near > APERTURE - 0.05, "{p:?}");
        }
        // the rails lie below the sight picture
        let body_at = |v: usize| Vec3::from(body.decode(body.frames[0].poses[0].verts[v]));
        assert!((0..body.num_verts).all(|v| body_at(v).z < LENS.z - APERTURE - 1.0));
    }
}
