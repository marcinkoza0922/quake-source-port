//! External world textures: `textures/[<map>/]<name>` images that replace a
//! BSP texture, as ironwail and vkQuake load them (Mod_LoadTextures).

use super::{Palette, downsample_rgba, world_texture};
use qformats::bsp::{MipTex, TextureKind};
use qformats::image::Rgba;

/// A replacement for one BSP texture: the image at any size, plus an
/// optional `_glow`/`_luma` fullbright layer (never for liquids).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalTexture {
    pub base: Rgba,
    pub glow: Option<Rgba>,
}

/// One BSP handed to `Renderer::load_level`: the map or an external brush
/// model, its `.lit` / HDR `.lit2` / `.dlit` deluxemap and its replacement
/// textures (indexed like `bsp.textures`; missing or shorter means none).
#[derive(Clone, Copy)]
pub struct LevelBsp<'a> {
    pub bsp: &'a qformats::bsp::Bsp,
    pub lit: Option<&'a [u8]>,
    pub hdr_lit: Option<&'a [u32]>,
    pub deluxe: Option<&'a [u8]>,
    pub external: &'a [Option<ExternalTexture>],
}

impl<'a> LevelBsp<'a> {
    pub fn new(bsp: &'a qformats::bsp::Bsp, lit: Option<&'a [u8]>) -> Self {
        LevelBsp { bsp, lit, hdr_lit: None, deluxe: None, external: &[] }
    }

    pub(crate) fn external(&self, i: usize) -> Option<&'a ExternalTexture> {
        self.external.get(i).and_then(|e| e.as_ref())
    }
}

/// One texture ready for upload: size and RGBA mip levels.
pub struct TexImage {
    pub width: u32,
    pub height: u32,
    pub mips: Vec<Vec<u8>>,
}

impl TexImage {
    pub fn mip_refs(&self) -> Vec<&[u8]> {
        self.mips.iter().map(|m| m.as_slice()).collect()
    }
}

/// Base and fullbright images for a BSP texture, taken from `ext` when
/// there is one and the miptex otherwise. Texture coordinates stay in
/// miptex texels either way (the material carries the miptex size), so a
/// replacement of a different size covers the face the same way.
pub fn world_texture_images(pal: &Palette, t: &MipTex, ext: Option<&ExternalTexture>) -> (TexImage, Option<TexImage>) {
    if let Some(e) = ext.filter(|e| valid(&e.base)) {
        let fence = t.kind == TextureKind::Fence;
        let lit = matches!(t.kind, TextureKind::Normal | TextureKind::Fence);
        let base = mip_chain(&e.base, fence, !fence);
        let fb = e.glow.as_ref().filter(|g| lit && valid(g)).map(|g| mip_chain(g, fence, false));
        return (base, fb);
    }
    let (base, fb) = world_texture(pal, t);
    let img = |mips| TexImage { width: t.width, height: t.height, mips };
    (img(base), fb.map(img))
}

/// `width | height << 16` of the miptex, which world shaders divide texel
/// coordinates by.
pub fn packed_size(t: &MipTex) -> u32 {
    (t.width.min(0xffff)) | (t.height.min(0xffff) << 16)
}

fn valid(img: &Rgba) -> bool {
    img.width > 0 && img.height > 0 && img.pixels.len() >= (img.width * img.height * 4) as usize
}

/// Full mip chain down to 1×1 (TEXPREF_MIPMAP). Fence textures keep their
/// alpha and average colour over opaque texels; with `opaque`, alpha is
/// forced to 255, since only fence textures use it.
fn mip_chain(img: &Rgba, fence: bool, opaque: bool) -> TexImage {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut top = img.pixels[..w * h * 4].to_vec();
    if opaque {
        for p in top.as_chunks_mut::<4>().0 {
            p[3] = 255;
        }
    }
    let mut mips = vec![top];
    let (mut cw, mut ch) = (w, h);
    while cw > 1 || ch > 1 {
        let next = downsample_rgba(mips.last().unwrap(), cw, ch, fence);
        mips.push(next);
        cw = (cw / 2).max(1);
        ch = (ch / 2).max(1);
    }
    TexImage { width: img.width, height: img.height, mips }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn miptex(name: &str, kind: TextureKind, w: u32, h: u32) -> MipTex {
        MipTex { name: name.into(), width: w, height: h, kind, mips: vec![vec![0; (w * h) as usize]], shift: 0 }
    }

    fn rgba(w: u32, h: u32, px: [u8; 4]) -> Rgba {
        Rgba { width: w, height: h, pixels: px.repeat((w * h) as usize) }
    }

    #[test]
    fn replacement_keeps_its_size_and_gets_full_mips() {
        let pal = [[0u8; 3]; 256];
        let t = miptex("+0wfall", TextureKind::Normal, 16, 32);
        let e = ExternalTexture { base: rgba(64, 128, [10, 20, 30, 0]), glow: Some(rgba(8, 8, [1, 2, 3, 4])) };
        let (b, fb) = world_texture_images(&pal, &t, Some(&e));
        assert_eq!((b.width, b.height, b.mips.len()), (64, 128, 8));
        assert_eq!(b.mips.last().unwrap().len(), 4);
        assert_eq!(&b.mips[0][..4], &[10, 20, 30, 255], "alpha is ignored outside fences");
        let fb = fb.expect("glow layer");
        assert_eq!((fb.width, fb.height, fb.mips.len()), (8, 8, 4));
        assert_eq!(packed_size(&t), 16 | 32 << 16);
    }

    #[test]
    fn fences_keep_alpha_and_liquids_get_no_glow() {
        let pal = [[0u8; 3]; 256];
        let e = ExternalTexture { base: rgba(4, 4, [9, 9, 9, 0]), glow: Some(rgba(4, 4, [5; 4])) };
        let (b, _) = world_texture_images(&pal, &miptex("{grate", TextureKind::Fence, 4, 4), Some(&e));
        assert_eq!(b.mips[0][3], 0);
        let (_, fb) = world_texture_images(&pal, &miptex("*water", TextureKind::Liquid, 4, 4), Some(&e));
        assert!(fb.is_none());
    }

    #[test]
    fn without_replacement_uses_the_miptex() {
        let pal = [[7u8; 3]; 256];
        let t = miptex("wall", TextureKind::Normal, 8, 8);
        let (b, fb) = world_texture_images(&pal, &t, None);
        assert_eq!((b.width, b.height, b.mips.len()), (8, 8, 1));
        assert!(fb.is_none());
        let broken = ExternalTexture { base: Rgba { width: 4, height: 4, pixels: vec![] }, glow: None };
        assert_eq!(world_texture_images(&pal, &t, Some(&broken)).0.width, 8);
    }
}
