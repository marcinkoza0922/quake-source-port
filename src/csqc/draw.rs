//! CSQC 2D drawing & picture cache builtins (`drawpic`, `drawsubpic`, `drawfill`,
//! `drawstring`, `drawrawstring`, `drawcharacter`, `drawline`, `drawsetcliparea`,
//! `precache_pic`, `drawgetimagesize`, `freepic`).

use std::collections::{HashMap, HashSet};

use qcore::fs::Fs;
use qformats::Wad;
use qformats::image::{parse_pcx, parse_pic, parse_tga};
use crate::gfx::Renderer;
use qrender::{Draw2D, PicId};

use super::markup;

/// CSQC-owned 2D picture cache (separate from `Pics` so `freepic` and custom
/// paths like `gfx/hud/icon.tga` or `num_0` WAD lumps resolve per DP/FTE rules).
#[derive(Default)]
pub struct CsqcPics {
    wad: Option<Wad>,
    wad_loaded: bool,
    /// Every picture registered so far: the renderer never frees one, so these
    /// outlive `freepic` and the VM (each map load starts a new one).
    cache: HashMap<String, Option<(PicId, u32, u32)>>,
    /// What the running VM has precached.
    live: HashSet<String>,
}

impl CsqcPics {
    fn ensure_wad(&mut self, fs: &Fs) {
        if !self.wad_loaded {
            self.wad_loaded = true;
            self.wad = fs.open("gfx.wad").and_then(|d| Wad::parse(&d).ok());
        }
    }

    pub fn is_cached(&self, name: &str) -> bool {
        self.live.contains(name) && self.cache.get(name).is_some_and(Option::is_some)
    }

    /// A new VM has precached nothing; missing files get another look.
    pub fn new_vm(&mut self) {
        self.live.clear();
        self.cache.retain(|_, p| p.is_some());
    }

    pub fn free(&mut self, name: &str) {
        self.live.remove(name);
    }

    /// Loads or retrieves a CSQC picture by name (supports WAD lumps, `.lmp`,
    /// `.tga`, `.pcx`, with or without extension and `gfx/` prefix).
    pub fn precache(&mut self, fs: &Fs, r: &mut Renderer, raw: &str) -> Option<(PicId, u32, u32)> {
        if raw.is_empty() {
            return None;
        }
        self.live.insert(raw.to_string());
        if let Some(&entry) = self.cache.get(raw) {
            return entry;
        }
        self.ensure_wad(fs);
        let found = self.load_pic(fs, r, raw);
        self.cache.insert(raw.to_string(), found);
        found
    }

    fn load_pic(&self, fs: &Fs, r: &mut Renderer, raw: &str) -> Option<(PicId, u32, u32)> {
        let clean = raw.trim().replace('\\', "/");
        // Try external image formats first (exact path, then stem + ext, then gfx/ prefix)
        let stem = clean
            .strip_suffix(".tga")
            .or_else(|| clean.strip_suffix(".lmp"))
            .or_else(|| clean.strip_suffix(".pcx"))
            .unwrap_or(&clean);

        let mut candidates = Vec::with_capacity(8);
        candidates.push(clean.clone());
        for ext in [".tga", ".lmp", ".pcx"] {
            candidates.push(format!("{stem}{ext}"));
        }
        if !clean.contains('/') {
            for ext in [".tga", ".lmp", ".pcx"] {
                candidates.push(format!("gfx/{stem}{ext}"));
            }
        }
        for path in candidates {
            let Some(data) = fs.open(&path) else { continue };
            let low = path.to_ascii_lowercase();
            if low.ends_with(".tga")
                && let Ok(img) = parse_tga(&data)
                && img.width > 0
                && img.height > 0
                && let Ok(id) = r.register_pic_rgba_linear(img.width, img.height, &img.pixels)
            {
                return Some((id, img.width, img.height));
            }
            if low.ends_with(".pcx")
                && let Ok((pic, pal)) = parse_pcx(&data)
                && pic.width > 0
                && pic.height > 0
            {
                let rgba: Vec<u8> = pic
                    .pixels
                    .iter()
                    .flat_map(|&i| {
                        if i == 255 {
                            [0, 0, 0, 0]
                        } else {
                            let c = pal[i as usize];
                            [c[0], c[1], c[2], 255]
                        }
                    })
                    .collect();
                if let Ok(id) = r.register_pic_rgba(pic.width, pic.height, &rgba) {
                    return Some((id, pic.width, pic.height));
                }
            }
            if let Ok(pic) = parse_pic(&data)
                && pic.width > 0
                && pic.height > 0
                && let Ok(id) = r.register_pic(&pic)
            {
                return Some((id, pic.width, pic.height));
            }
        }
        // Fallback: gfx.wad lump
        let lump_name = stem.rsplit('/').next().unwrap_or(stem);
        if let Some(lump) = self.wad.as_ref().and_then(|w| w.get(lump_name))
            && let Ok(pic) = parse_pic(&lump.data)
            && pic.width > 0
            && pic.height > 0
            && let Ok(id) = r.register_pic(&pic)
        {
            return Some((id, pic.width, pic.height));
        }
        None
    }
}

/// Virtual-to-framebuffer scaling parameters for one CSQC draw pass.
#[derive(Clone, Copy)]
pub struct DrawSpace {
    pub fb_w: f32,
    pub fb_h: f32,
    pub virt_w: f32,
    pub virt_h: f32,
}

impl DrawSpace {
    #[inline]
    pub fn sx(self) -> f32 {
        self.fb_w / self.virt_w.max(1.0)
    }
    #[inline]
    pub fn sy(self) -> f32 {
        self.fb_h / self.virt_h.max(1.0)
    }
}

pub fn draw_fill(list: &mut Vec<Draw2D>, sp: DrawSpace, pos: [f32; 2], size: [f32; 2], rgb: [f32; 3], alpha: f32) {
    if alpha <= 0.0 || size[0] <= 0.0 || size[1] <= 0.0 {
        return;
    }
    list.push(Draw2D::Fill {
        x: pos[0] * sp.sx(),
        y: pos[1] * sp.sy(),
        w: size[0] * sp.sx(),
        h: size[1] * sp.sy(),
        color: [rgb[0], rgb[1], rgb[2], alpha.clamp(0.0, 1.0)],
    });
}

pub fn draw_pic(
    list: &mut Vec<Draw2D>,
    sp: DrawSpace,
    pos: [f32; 2],
    size: [f32; 2],
    pic: PicId,
    uv: [f32; 4],
    rgba: [f32; 4],
) {
    if rgba[3] <= 0.0 || size[0] == 0.0 || size[1] == 0.0 {
        return;
    }
    list.push(Draw2D::Pic {
        x: pos[0] * sp.sx(),
        y: pos[1] * sp.sy(),
        w: size[0] * sp.sx(),
        h: size[1] * sp.sy(),
        pic,
        uv,
        color: [rgba[0], rgba[1], rgba[2], rgba[3].clamp(0.0, 1.0)],
    });
}

pub fn draw_line(
    list: &mut Vec<Draw2D>,
    sp: DrawSpace,
    width: f32,
    p0: [f32; 2],
    p1: [f32; 2],
    rgb: [f32; 3],
    alpha: f32,
) {
    if alpha <= 0.0 {
        return;
    }
    list.push(Draw2D::Line {
        x0: p0[0] * sp.sx(),
        y0: p0[1] * sp.sy(),
        x1: p1[0] * sp.sx(),
        y1: p1[1] * sp.sy(),
        width: (width * sp.sx().min(sp.sy())).max(1.0),
        color: [rgb[0], rgb[1], rgb[2], alpha.clamp(0.0, 1.0)],
    });
}

pub fn draw_char(
    list: &mut Vec<Draw2D>,
    sp: DrawSpace,
    pos: [f32; 2],
    ch: u8,
    scale: [f32; 2],
    rgb: [f32; 3],
    alpha: f32,
) {
    if ch == b' ' || alpha <= 0.0 || scale[0] <= 0.0 || scale[1] <= 0.0 {
        return;
    }
    let size = (scale[0] * sp.sx()).min(scale[1] * sp.sy()).max(1.0);
    list.push(Draw2D::Char {
        x: pos[0] * sp.sx(),
        y: pos[1] * sp.sy(),
        size,
        ch,
        color: [rgb[0], rgb[1], rgb[2], alpha.clamp(0.0, 1.0)],
    });
}

pub fn draw_string(
    list: &mut Vec<Draw2D>,
    sp: DrawSpace,
    pos: [f32; 2],
    text: &[u8],
    scale: [f32; 2],
    rgba: [f32; 4],
    colored: bool,
) {
    let alpha = rgba[3].clamp(0.0, 1.0);
    if alpha <= 0.0 || scale[0] <= 0.0 || scale[1] <= 0.0 {
        return;
    }
    let step_x = scale[0] * sp.sx();
    let glyph_sz = step_x.min(scale[1] * sp.sy()).max(1.0);
    let mut x = pos[0] * sp.sx();
    let y = pos[1] * sp.sy();
    if !colored {
        let color = [rgba[0], rgba[1], rgba[2], alpha];
        for &ch in text {
            if ch != b' ' {
                list.push(Draw2D::Char { x, y, size: glyph_sz, ch, color });
            }
            x += step_x;
        }
        return;
    }
    for (run, color) in markup::parse_colored_runs(text, [rgba[0], rgba[1], rgba[2]], alpha) {
        for ch in run {
            if ch != b' ' {
                list.push(Draw2D::Char { x, y, size: glyph_sz, ch, color });
            }
            x += step_x;
        }
    }
}

pub fn set_clip(list: &mut Vec<Draw2D>, sp: DrawSpace, x: f32, y: f32, w: f32, h: f32) {
    list.push(Draw2D::Clip(Some([
        x * sp.sx(),
        y * sp.sy(),
        w.max(0.0) * sp.sx(),
        h.max(0.0) * sp.sy(),
    ])));
}

pub fn reset_clip(list: &mut Vec<Draw2D>) {
    list.push(Draw2D::Clip(None));
}

#[cfg(test)]
mod pic_tests {
    use super::*;

    /// The renderer never frees pictures: a map load or `freepic` must not
    /// register them again.
    #[test]
    fn pictures_outlive_the_vm() {
        let Ok(mut r) = Renderer::headless(64, 64, &[[0; 3]; 256]) else { return };
        let dir = std::env::temp_dir().join(format!("qsp-csqcpics-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("id1/gfx")).unwrap();
        // a 1x1 qpic
        std::fs::write(dir.join("id1/gfx/x.lmp"), [1, 0, 0, 0, 1, 0, 0, 0, 7]).unwrap();
        let mut fs = Fs::new(&dir);
        fs.add_game_dir("id1");
        let mut p = CsqcPics::default();
        let first = p.precache(&fs, &mut r, "gfx/x.lmp").expect("loads");
        assert!(p.is_cached("gfx/x.lmp"));
        p.free("gfx/x.lmp");
        assert!(!p.is_cached("gfx/x.lmp"));
        assert_eq!(p.precache(&fs, &mut r, "gfx/x.lmp"), Some(first));
        assert!(p.precache(&fs, &mut r, "gfx/none.lmp").is_none());
        p.new_vm();
        assert!(!p.is_cached("gfx/x.lmp"));
        assert_eq!(p.precache(&fs, &mut r, "gfx/x.lmp"), Some(first));
        std::fs::write(dir.join("id1/gfx/none.lmp"), [1, 0, 0, 0, 1, 0, 0, 0, 7]).unwrap();
        assert!(p.precache(&fs, &mut r, "gfx/none.lmp").is_some(), "a missing file is looked for again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
