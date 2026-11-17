//! External world textures, as ironwail and vkQuake load them in
//! Mod_LoadTextures: `textures/<map>/<name>` then `textures/<name>`
//! (liquids `*name` as `#name`, skies never), plus `<that>_glow` or
//! `<that>_luma` as the fullbright layer of lit textures. Arcane Dimensions
//! ships many (its waterfalls among them).
//!
//! Format choice is vkQuake's Image_LoadImage: the image from the
//! highest-priority game dir wins, then png, tga, jpg, pcx, lmp. The world
//! takes images from any game dir; an external brush model (`maps/b_*.bsp`)
//! only from its own or a higher-priority one (`effective_min_path_id`).

use qcore::fs::Fs;
use qformats::bsp::{Bsp, TextureKind};
use qformats::image::Rgba;
use qrender::ExternalTexture;

const EXTS: [&str; 5] = ["png", "tga", "jpg", "pcx", "lmp"];

/// Replacements for `bsp`'s textures, indexed like `bsp.textures`.
/// `bsp_path` is its precache name (`maps/e1m1.bsp`); `world` is false for
/// external brush models.
pub fn load(fs: &Fs, bsp_path: &str, bsp: &Bsp, world: bool) -> Vec<Option<ExternalTexture>> {
    let min = if world { 0 } else { fs.priority(bsp_path).unwrap_or(0) };
    let map = bsp_path.strip_prefix("maps/").unwrap_or(bsp_path);
    let map = map.rsplit_once('.').map_or(map, |(stem, _)| stem);
    let mut pal = None;
    bsp.textures
        .iter()
        .map(|t| {
            let t = t.as_ref()?;
            let name = match t.kind {
                TextureKind::Sky => return None,
                TextureKind::Liquid => format!("#{}", t.name.get(1..)?),
                _ => t.name.clone(),
            };
            let lit = matches!(t.kind, TextureKind::Normal | TextureKind::Fence);
            let (path, base) = [format!("textures/{map}/{name}"), format!("textures/{name}")]
                .into_iter()
                .find_map(|p| Some((p.clone(), load_image(fs, &p, min, &mut pal)?)))?;
            let glow = if lit {
                ["_glow", "_luma"].iter().find_map(|s| load_image(fs, &format!("{path}{s}"), min, &mut pal))
            } else {
                None
            };
            Some(ExternalTexture { base, glow })
        })
        .collect()
}

/// Image_LoadImage: `base` with the best extension, if its game dir is at
/// least `min`. `pal` caches the game palette for `.lmp` images.
fn load_image(fs: &Fs, base: &str, min: usize, pal: &mut Option<Option<[u8; 768]>>) -> Option<Rgba> {
    let mut best: Option<(usize, &str)> = None;
    for ext in EXTS {
        if let Some(g) = fs.priority(&format!("{base}.{ext}"))
            && best.is_none_or(|(bg, _)| g > bg)
        {
            best = Some((g, ext));
        }
    }
    let (g, ext) = best?;
    if g < min {
        return None;
    }
    let path = format!("{base}.{ext}");
    let palette = if ext == "lmp" {
        *pal.get_or_insert_with(|| fs.open("gfx/palette.lmp").and_then(|d| d.get(..768)?.try_into().ok()))
    } else {
        None
    };
    let mut files = |p: &str| fs.open(p);
    let img = qpart::geom::load_image(&mut files, &path, palette.as_ref(), false)?;
    Some(Rgba { width: img.width, height: img.height, pixels: img.rgba })
}
