//! MD3 model loading and skin/texture resolution (`ironwail` rules).

use qcore::fs::Fs;
use qformats::image::{parse_pcx, parse_pic, parse_tga};
use qformats::md3::{Md3, parse_skin};
use qformats::mdl::MF_HOLEY;
use qformats::Mdl;
use crate::gfx::Renderer;
use qrender::{Md3SkinImage, Md3SurfaceSkins, ModelId};

const MAX_SKINS: usize = 32;
const MAX_SKIN_FRAMES: usize = 4;

enum DecodedTex {
    Rgba { width: u32, height: u32, pixels: Vec<u8> },
    Indexed { width: u32, height: u32, pixels: Vec<u8> },
}

fn strip_ext(path: &str) -> &str {
    let slash = path.rfind('/').unwrap_or(0);
    match path.rfind('.') {
        Some(dot) if dot > slash => &path[..dot],
        _ => path,
    }
}

fn load_image(fs: &Fs, raw_path: &str) -> Option<(DecodedTex, String)> {
    let normalized = raw_path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return None;
    }
    let stem = strip_ext(&normalized);
    for (ext, kind) in [("tga", 0), ("lmp", 1), ("pcx", 2)] {
        let candidate = format!("{stem}.{ext}");
        let Some(data) = fs.open(&candidate) else { continue };
        let tex = match kind {
            0 => parse_tga(&data).ok().map(|r| DecodedTex::Rgba {
                width: r.width,
                height: r.height,
                pixels: r.pixels,
            }),
            1 => parse_pic(&data).ok().map(|p| DecodedTex::Indexed {
                width: p.width,
                height: p.height,
                pixels: p.pixels,
            }),
            _ => parse_pcx(&data).ok().map(|(p, pal)| DecodedTex::Rgba {
                width: p.width,
                height: p.height,
                pixels: p.pixels
                    .iter()
                    .flat_map(|&idx| {
                        let c = pal[idx as usize];
                        [c[0], c[1], c[2], 255]
                    })
                    .collect(),
            }),
        };
        if let Some(t) = tex {
            return Some((t, stem.to_string()));
        }
    }
    None
}

fn load_glow_luma(r: &Renderer, fs: &Fs, stem: &str) -> Option<(u32, u32, Vec<u8>)> {
    for suffix in ["_glow", "_luma"] {
        let candidate = format!("{stem}{suffix}");
        let Some((tex, _)) = load_image(fs, &candidate) else { continue };
        return Some(match tex {
            DecodedTex::Rgba { width, height, pixels } => (width, height, pixels),
            DecodedTex::Indexed { width, height, pixels } => {
                let skin = r.md3_indexed_skin(width, height, &pixels, false);
                (width, height, skin.base)
            }
        });
    }
    None
}

fn to_skin_image(
    r: &Renderer,
    fs: &Fs,
    tex: DecodedTex,
    stem: &str,
    holey: bool,
) -> Md3SkinImage {
    match tex {
        DecodedTex::Indexed { width, height, pixels } => {
            r.md3_indexed_skin(width, height, &pixels, holey)
        }
        DecodedTex::Rgba { width, height, pixels } => {
            let fb = load_glow_luma(r, fs, stem);
            Md3SkinImage {
                width,
                height,
                base: pixels,
                fb,
                cmap: None,
            }
        }
    }
}

fn intervals_for(len: usize) -> Vec<f32> {
    if len <= 1 {
        Vec::new()
    } else {
        (1..=len).map(|i| i as f32 * 0.1).collect()
    }
}

fn is_glow_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("_glow") || lower.contains("_luma")
}

fn load_skin_manifests(
    fs: &Fs,
    md3_path: &str,
    base_path: &str,
    surface_names: &[&str],
) -> Vec<Option<Vec<(String, String)>>> {
    let mut manifests = Vec::new();
    for skinnum in 0..MAX_SKINS {
        let mut parsed = None;
        let mut found_any_file = false;
        for candidate in [
            format!("{md3_path}_{skinnum}.skin"),
            format!("{base_path}_{skinnum}.skin"),
        ] {
            let Some(bytes) = fs.open(&candidate) else { continue };
            found_any_file = true;
            if let Some(entries) = parse_skin(&String::from_utf8_lossy(&bytes), surface_names) {
                parsed = Some(entries);
                break;
            }
        }
        if !found_any_file && skinnum > 0 {
            break;
        }
        manifests.push(parsed);
    }
    manifests
}

fn skin_from_manifest(
    r: &Renderer,
    fs: &Fs,
    entries: &[(String, String)],
    surface_name: &str,
    holey: bool,
) -> Vec<Md3SkinImage> {
    let mut frames = Vec::new();
    let mut explicit_fbs = Vec::new();
    for (surf, tex_path) in entries {
        if surf != surface_name {
            continue;
        }
        if is_glow_path(tex_path) {
            if let Some((tex, _)) = load_image(fs, tex_path) {
                explicit_fbs.push(match tex {
                    DecodedTex::Rgba { width, height, pixels } => (width, height, pixels),
                    DecodedTex::Indexed { width, height, pixels } => {
                        let s = r.md3_indexed_skin(width, height, &pixels, false);
                        (width, height, s.base)
                    }
                });
            }
            continue;
        }
        if frames.len() >= MAX_SKIN_FRAMES {
            continue;
        }
        if let Some((tex, stem)) = load_image(fs, tex_path) {
            frames.push(to_skin_image(r, fs, tex, &stem, holey));
        }
    }
    for (frame, fb) in frames.iter_mut().zip(explicit_fbs) {
        if frame.fb.is_none() {
            frame.fb = Some(fb);
        }
    }
    frames
}

fn try_prefixes(
    r: &Renderer,
    fs: &Fs,
    rel: &str,
    holey: bool,
) -> Option<Md3SkinImage> {
    for prefix in ["progs/", "textures/"] {
        let path = format!("{prefix}{rel}");
        if let Some((tex, stem)) = load_image(fs, &path) {
            return Some(to_skin_image(r, fs, tex, &stem, holey));
        }
    }
    None
}

fn load_fallback_skins(
    r: &Renderer,
    fs: &Fs,
    surface_name: &str,
    shaders: &[String],
    holey: bool,
) -> Vec<(Vec<Md3SkinImage>, Vec<f32>)> {
    let mut out = Vec::new();
    for skinnum in 0..MAX_SKINS {
        let mut frames = Vec::new();
        for f in 0..MAX_SKIN_FRAMES {
            let rel = format!("{surface_name}_{skinnum}_{f}");
            let Some(img) = try_prefixes(r, fs, &rel, holey) else { break };
            frames.push(img);
        }
        if !frames.is_empty() {
            let iv = intervals_for(frames.len());
            out.push((frames, iv));
            continue;
        }
        let rel = format!("{surface_name}_{skinnum}");
        if let Some(img) = try_prefixes(r, fs, &rel, holey) {
            out.push((vec![img], Vec::new()));
            continue;
        }
        if skinnum == 0 {
            if let Some(img) = try_prefixes(r, fs, surface_name, holey) {
                out.push((vec![img], Vec::new()));
                continue;
            }
            if let Some(img) = shaders
                .iter()
                .find_map(|s| load_image(fs, s).map(|(tex, stem)| to_skin_image(r, fs, tex, &stem, holey)))
            {
                out.push((vec![img], Vec::new()));
                continue;
            }
        }
        break;
    }
    out
}

fn dummy_skin() -> Md3SkinImage {
    Md3SkinImage {
        width: 1,
        height: 1,
        base: vec![128, 128, 128, 255],
        fb: None,
        cmap: None,
    }
}

/// Loads an MD3 model (`md3_path`) with skin manifests and fallback textures.
pub fn load_md3(
    fs: &Fs,
    r: &mut Renderer,
    md3_path: &str,
    md3: &Md3,
    holey: bool,
) -> Option<ModelId> {
    let base_path = strip_ext(md3_path);
    let surface_names: Vec<&str> = md3.surfaces.iter().map(|s| s.name.as_str()).collect();
    let manifests = load_skin_manifests(fs, md3_path, base_path, &surface_names);
    let mut surface_skins = Vec::with_capacity(md3.surfaces.len());
    for surf in &md3.surfaces {
        let mut skins: Vec<(Vec<Md3SkinImage>, Vec<f32>)> = Vec::new();
        for manifest in &manifests {
            let Some(entries) = manifest else { continue };
            let frames = skin_from_manifest(r, fs, entries, &surf.name, holey);
            if !frames.is_empty() {
                let iv = intervals_for(frames.len());
                skins.push((frames, iv));
            } else if let Some(first) = skins.first().cloned() {
                skins.push(first);
            }
        }
        if skins.is_empty() {
            skins = load_fallback_skins(r, fs, &surf.name, &surf.shaders, holey);
        }
        if skins.is_empty() {
            skins.push((vec![dummy_skin()], Vec::new()));
        }
        surface_skins.push(Md3SurfaceSkins { skins, holey });
    }
    r.load_md3(md3, &surface_skins).ok()
}

/// Loads an IQM v2 skeletal model (`iqm_path`) with skin manifests and material/fallback textures.
pub fn load_iqm(
    fs: &Fs,
    r: &mut Renderer,
    iqm_path: &str,
    iqm: &qformats::Iqm,
    holey: bool,
) -> Option<ModelId> {
    let base_path = strip_ext(iqm_path);
    let mesh_names: Vec<&str> = iqm.meshes.iter().map(|m| m.name.as_str()).collect();
    let manifests = load_skin_manifests(fs, iqm_path, base_path, &mesh_names);
    let mut surface_skins = Vec::with_capacity(iqm.meshes.len());
    for mesh in &iqm.meshes {
        let mut skins: Vec<(Vec<Md3SkinImage>, Vec<f32>)> = Vec::new();
        for manifest in &manifests {
            let Some(entries) = manifest else { continue };
            let frames = skin_from_manifest(r, fs, entries, &mesh.name, holey);
            if !frames.is_empty() {
                let iv = intervals_for(frames.len());
                skins.push((frames, iv));
            } else if let Some(first) = skins.first().cloned() {
                skins.push(first);
            }
        }
        if skins.is_empty() {
            let shaders: Vec<String> = if mesh.material.is_empty() {
                Vec::new()
            } else {
                vec![mesh.material.clone()]
            };
            skins = load_fallback_skins(r, fs, &mesh.name, &shaders, holey);
        }
        if skins.is_empty() {
            skins.push((vec![dummy_skin()], Vec::new()));
        }
        surface_skins.push(Md3SurfaceSkins { skins, holey });
    }
    r.load_iqm(iqm, &surface_skins).ok()
}

/// Attempts to load a companion `.iqm` replacement for `mdl_name`.
pub fn load_iqm_replacement(
    fs: &Fs,
    r: &mut Renderer,
    mdl_name: &str,
    mdl: &Mdl,
) -> Option<ModelId> {
    let stem = strip_ext(mdl_name);
    let iqm_name = format!("{stem}.iqm");
    let data = fs.open_companion(mdl_name, &iqm_name)?;
    if data.get(..16) != Some(qformats::iqm::IQM_MAGIC.as_slice()) {
        return None;
    }
    let iqm = qformats::Iqm::parse(&data).ok()?;
    let holey = (mdl.flags & MF_HOLEY) != 0;
    load_iqm(fs, r, &iqm_name, &iqm, holey)
}

/// Attempts to load a companion `.md3` replacement for `mdl_name`.
pub fn load_replacement(
    fs: &Fs,
    r: &mut Renderer,
    mdl_name: &str,
    mdl: &Mdl,
) -> Option<ModelId> {
    let stem = strip_ext(mdl_name);
    let md3_name = format!("{stem}.md3");
    let data = fs.open_companion(mdl_name, &md3_name)?;
    if data.len() < 4 || &data[..4] != b"IDP3" {
        return None;
    }
    let md3 = Md3::parse(&data).ok()?;
    let holey = (mdl.flags & MF_HOLEY) != 0;
    load_md3(fs, r, &md3_name, &md3, holey)
}

/// Attempts to load the 2021 re-release's `.md5mesh`/`.md5anim` companion of
/// `mdl_name`, baked into an MD3 (see `qformats::md5`).
pub fn load_md5_replacement(fs: &Fs, r: &mut Renderer, mdl_name: &str, mdl: &Mdl) -> Option<ModelId> {
    let md5 = parse_md5(fs, mdl_name)?;
    let skins = md5_skins(fs, r, mdl_name, &md5, (mdl.flags & MF_HOLEY) != 0);
    r.load_md3(&md5.md3, &skins).ok()
}

/// The `.md5mesh`/`.md5anim` companion of `mdl_name`, when one is found.
pub fn parse_md5(fs: &Fs, mdl_name: &str) -> Option<qformats::md5::Md5> {
    let stem = strip_ext(mdl_name);
    let mesh = fs.open_companion(mdl_name, &format!("{stem}.md5mesh"))?;
    let anim = fs.open(&format!("{stem}.md5anim"));
    match qformats::md5::Md5::parse(&mesh, anim.as_deref()) {
        Ok(m) => Some(m),
        Err(e) => {
            eprintln!("{stem}.md5mesh: {e}; using the .mdl");
            None
        }
    }
}

/// Surface skins of `mdl_name`'s MD5 companion. They follow the re-release
/// naming, `<dir>/<shader>_SS_FF` (skin, animated skin frame).
pub fn md5_skins(fs: &Fs, r: &mut Renderer, mdl_name: &str, md5: &qformats::md5::Md5, holey: bool) -> Vec<Md3SurfaceSkins> {
    let dir = mdl_name.rfind('/').map_or("", |i| &mdl_name[..=i]);
    md5.shaders
        .iter()
        .map(|shader| {
            let mut skins = Vec::new();
            for s in 0..MAX_SKINS {
                let frames: Vec<_> = (0..MAX_SKIN_FRAMES)
                    .map_while(|f| load_image(fs, &format!("{dir}{shader}_{s:02}_{f:02}")))
                    .map(|(tex, stem)| to_skin_image(r, fs, tex, &stem, holey))
                    .collect();
                if frames.is_empty() {
                    break;
                }
                let iv = intervals_for(frames.len());
                skins.push((frames, iv));
            }
            if skins.is_empty() {
                skins.push((vec![dummy_skin()], Vec::new()));
            }
            Md3SurfaceSkins { skins, holey }
        })
        .collect()
}
