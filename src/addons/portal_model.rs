//! The owner's Meshy projector, converted offline to native alias geometry.
use qformats::mdl::Mdl;

pub fn model(palette: &[u8]) -> Mdl {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "../../assets/portal-gun/projector.mdl.zlib"
    ))
    .expect("bundled portal geometry");
    let mut model = Mdl::parse(&bytes).expect("bundled portal MDL");
    model.skins = super::portal_skin::skins(palette, &model.skins[0].images[0]);
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcore::math::Vec3;

    #[test]
    fn imported_asset_has_valid_geometry_uvs_and_owner_skins() {
        let palette: Vec<u8> = (0..256).flat_map(|i| [i as u8; 3]).collect();
        let m = model(&palette);
        assert!(m.triangles.len() > 500 && m.triangles.len() <= 4_000);
        assert_eq!((m.skin_width, m.skin_height), (512, 512));
        assert_eq!(m.skins.len(), 8);
        assert_eq!(m.num_verts, m.stverts.len());
        assert!(
            m.stverts
                .iter()
                .all(|v| v.onseam == 0 && (0..512).contains(&v.s) && (0..512).contains(&v.t))
        );
        let pose = &m.frames[0].poses[0];
        assert_eq!(pose.verts.len(), m.num_verts);
        assert!(pose.verts.iter().all(|v| v.normal < 162));
        for tri in &m.triangles {
            assert!(
                tri.verts
                    .iter()
                    .all(|&v| v >= 0 && (v as usize) < m.num_verts)
            );
            let p = tri
                .verts
                .map(|i| Vec3::from(m.decode(pose.verts[i as usize])));
            assert!((p[1] - p[0]).cross(p[2] - p[0]).length() > 0.0);
        }
        // The model extends forward along +X and stays clear of the camera.
        let lo = m.decode(pose.bboxmin);
        let hi = m.decode(pose.bboxmax);
        assert!((lo[0] + 12.0).abs() < 0.1 && (hi[0] - 24.0).abs() < 0.1);
        for skin in &m.skins {
            assert_eq!(skin.images[0].len(), 512 * 512);
            assert!(skin.images[0].iter().all(|&p| p < 255));
        }
    }
}
