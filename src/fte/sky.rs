use super::*;

/// Sky-triangle rain emitters of the world and its inline brush models
/// (PScript_RecalculateSkyTris), keyed by submodel index: worldspawn
/// `[_]texpart_<texture>` keys, else effects named `tex_<texture>`.
pub(super) fn sky_tris(bsp: &Bsp, reg: &mut Registry, f: &mut dyn qpart::FileSource) -> Vec<(usize, Vec<qpart::SkyTri>)> {
    let mut remap: HashMap<usize, qpart::TypeId> = HashMap::new();
    let mut s = bsp.entities.as_str();
    if let Some(("{", rest)) = qcore::cmd::com_parse(s) {
        s = rest;
        while let Some((k, rest)) = qcore::cmd::com_parse(s) {
            if k == "}" {
                break;
            }
            let Some((v, rest)) = qcore::cmd::com_parse(rest) else { break };
            s = rest;
            let key = k.strip_prefix('_').unwrap_or(k).trim_end();
            if key.len() > 8 && key[..8].eq_ignore_ascii_case("texpart_") {
                for (t, tex) in bsp.textures.iter().enumerate() {
                    if tex.as_ref().is_some_and(|m| m.name.eq_ignore_ascii_case(&key[8..]))
                        && let Some(ty) = reg.find(v, f)
                    {
                        remap.insert(t, ty);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut by_tex: HashMap<usize, Option<qpart::TypeId>> = HashMap::new();
    for (sub, m) in bsp.models.iter().enumerate() {
        let tris = model_sky_tris(bsp, m, &remap, &mut by_tex, reg, f);
        if !tris.is_empty() {
            out.push((sub, tris));
        }
    }
    out
}

pub(super) fn model_sky_tris(
    bsp: &Bsp,
    m: &qformats::bsp::Model,
    remap: &HashMap<usize, qpart::TypeId>,
    by_tex: &mut HashMap<usize, Option<qpart::TypeId>>,
    reg: &mut Registry,
    f: &mut dyn qpart::FileSource,
) -> Vec<qpart::SkyTri> {
    let mut out = Vec::new();
    for fi in m.first_face.max(0) as usize..(m.first_face + m.num_faces).max(0) as usize {
        let Some(face) = bsp.faces.get(fi) else { continue };
        let Some(ti) = bsp.texinfo.get(face.texinfo as usize) else { continue };
        let t = ti.miptex.max(0) as usize;
        let ty = *by_tex.entry(t).or_insert_with(|| {
            remap.get(&t).copied().or_else(|| {
                let name = bsp.textures.get(t).and_then(|x| x.as_ref())?.name.clone();
                reg.find(&format!("tex_{name}"), f)
            })
        });
        let Some(ty) = ty else { continue };
        let mut verts = Vec::new();
        for e in face.first_edge..face.first_edge + face.num_edges {
            let Some(&se) = bsp.surfedges.get(e as usize) else { break };
            let v = if se >= 0 { bsp.edges.get(se as usize).map(|e| e[0]) } else { bsp.edges.get((-se) as usize).map(|e| e[1]) };
            if let Some(p) = v.and_then(|v| bsp.vertexes.get(v as usize)) {
                verts.push(Vec3::from(*p));
            }
        }
        let pn = bsp.planes.get(face.plane as usize).map_or(Vec3::ZERO, |p| Vec3::from(p.normal));
        let normal = if face.side != 0 { pn * -1.0 } else { pn };
        out.extend(qpart::sky_tris_for_polygon(&verts, normal, ty));
    }
    out
}

/// Uploads textures added to the registry since the last call.
pub(super) fn upload_textures(reg: &Registry, pics: &mut Vec<Option<PicId>>, r: &mut Renderer, f: &mut dyn qpart::FileSource) {
    while pics.len() < reg.textures.len() {
        let t = &reg.textures[pics.len()];
        let img = match t {
            TexRef::Builtin(b) => Some(geom::builtin_image(*b)),
            TexRef::File { path, premul, .. } => geom::load_image(f, path, reg.palette.as_deref(), *premul),
        };
        let nearest = matches!(t, TexRef::File { nearest: true, .. });
        let pic = img.and_then(|i| {
            if nearest {
                r.register_pic_rgba(i.width, i.height, &i.rgba).ok()
            } else {
                r.register_pic_rgba_linear(i.width, i.height, &i.rgba).ok()
            }
        });
        pics.push(pic);
    }
}
