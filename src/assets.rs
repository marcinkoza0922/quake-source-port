//! Per-level assets: models/sounds from the client precache lists loaded
//! into qrender/qsound, plus the parsed BSP (ambient levels, worldspawn keys).

use std::collections::HashMap;
use std::sync::Arc;

use qclient::ClientState;
use qcore::fs::Fs;
use qformats::{Bsp, Mdl, Spr, parse_entities};
use crate::gfx::Renderer;
use qrender::{Fog, LevelBsp, ModelId};
use qsound::{Sfx, SoundSystem};

mod external_textures;
pub(crate) mod md3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Brush,
    Alias,
    Sprite,
}

#[derive(Clone, Copy, Debug)]
pub struct Model {
    pub id: ModelId,
    pub kind: Kind,
    pub flags: u32,
    pub md3: bool,
}

#[derive(Default)]
pub struct Level {
    pub mapname: String,
    pub enhanced_models: bool,
    /// Indexed by client model index.
    pub models: Vec<Option<Model>>,
    /// Indexed by client sound index.
    pub sounds: Vec<Option<Arc<Sfx>>>,
    /// Models by name (temp entities, beams).
    pub by_name: HashMap<String, Option<Model>>,
    pub bsp: Option<Bsp>,
    /// Lazily initialized BSP collision model shared across client FX systems.
    pub cm: std::sync::OnceLock<qworld::CollisionModel>,
    pub world_fog: Fog,
    pub world_sky: String,
    /// Skybox in use (worldspawn, svc_skybox or the `sky` command).
    pub sky: String,
    /// The skybox's wind (`gfx/env/<sky>wind.cfg`, `skywind` commands).
    pub sky_wind: crate::skywind::SkyWind,
    /// Worldspawn `[_]wateralpha`/`lavaalpha`/`slimealpha`/`telealpha`
    /// (R_ParseWorldspawn), in that order.
    pub liquid_alpha: [Option<f32>; 4],
    /// Visible-weapon rig and models.
    pub vwep: crate::vwep::Cache,
}

fn load_model(fs: &Fs, r: &mut Renderer, name: &str, enhanced_models: bool) -> Option<Model> {
    let data = fs.open(name)?;
    if data.len() >= 4 && &data[..4] == b"IDPO" {
        let m = Mdl::parse(&data).ok()?;
        let flags = m.flags as u32;
        let replacement = enhanced_models
            .then(|| {
                md3::load_iqm_replacement(fs, r, name, &m)
                    .or_else(|| md3::load_replacement(fs, r, name, &m))
                    .or_else(|| md3::load_md5_replacement(fs, r, name, &m))
            })
            .flatten();
        if let Some(id) = replacement {
            return Some(Model { id, kind: Kind::Alias, flags, md3: true });
        }
        let id = r.load_alias(&m).ok()?;
        return Some(Model { id, kind: Kind::Alias, flags, md3: false });
    }
    if data.len() >= 16 && &data[..16] == qformats::iqm::IQM_MAGIC {
        let m = qformats::Iqm::parse(&data).ok()?;
        let id = md3::load_iqm(fs, r, name, &m, false)?;
        return Some(Model { id, kind: Kind::Alias, flags: 0, md3: true });
    }
    if data.len() >= 4 && &data[..4] == b"IDP3" {
        let m = qformats::Md3::parse(&data).ok()?;
        let id = md3::load_md3(fs, r, name, &m, false)?;
        return Some(Model { id, kind: Kind::Alias, flags: 0, md3: true });
    }
    if data.len() >= 4 && &data[..4] == b"IDSP" {
        let s = Spr::parse(&data).ok()?;
        let id = r.load_sprite(&s).ok()?;
        return Some(Model { id, kind: Kind::Sprite, flags: 0, md3: false });
    }
    None
}

pub struct LevelLoad<'a> {
    pub colored: bool,
    pub enhanced_models: bool,
    pub external_entities: Option<&'a str>,
    pub reload_bsp: Option<Bsp>,
}

impl Level {
    /// Lazily builds and caches the BSP collision model for this level.
    pub fn collision_model(&self) -> Option<&qworld::CollisionModel> {
        self.bsp
            .as_ref()
            .map(|bsp| self.cm.get_or_init(|| qworld::CollisionModel::from_bsp(bsp)))
    }

    /// Loads everything in `cl`'s precache lists; sets `cl.model_flags`.
    pub fn load(
        fs: &Fs,
        r: &mut Renderer,
        snd: Option<&mut SoundSystem>,
        cl: &mut ClientState,
        options: LevelLoad<'_>,
        print: &mut dyn FnMut(&str),
    ) -> Level {
        let LevelLoad { colored, enhanced_models, external_entities, reload_bsp } = options;
        let mut lv = Level { mapname: cl.mapname.clone(), enhanced_models, ..Default::default() };
        let world = cl.model_precache.get(1).cloned().unwrap_or_default();
        let mut bsp = reload_bsp.or_else(|| {
            fs.open(&world).and_then(|d| Bsp::parse(&d).map_err(|e| print(&format!("{world}: {e}\n"))).ok())
        });
        if let (Some(bsp), Some(ents)) = (&mut bsp, external_entities) {
            bsp.entities = ents.to_owned();
        }
        let mut subs = Vec::new();
        // External brush models (maps/b_*.bsp) load together with the world.
        let mut brush_names = Vec::new();
        let mut brushes = Vec::new();
        for name in cl.model_precache.iter().skip(2) {
            if name.starts_with('*') || !name.trim_end_matches(' ').ends_with(".bsp") {
                continue;
            }
            let Some(d) = fs.open(name) else { continue };
            if d.starts_with(b"IDPO") || d.starts_with(b"IDSP") || d.starts_with(b"IDP3") || d.starts_with(qformats::iqm::IQM_MAGIC) {
                continue;
            }
            match Bsp::parse(&d) {
                Ok(b) => {
                    let light = load_lighting(fs, name, &b, colored);
                    let ext = external_textures::load(fs, name, &b, false);
                    brush_names.push(name.clone());
                    brushes.push((b, light, ext));
                }
                Err(e) => print(&format!("{name}: {e}\n")),
            }
        }
        let mut brush_models = HashMap::new();
        if let Some(b) = &bsp {
            let light = load_lighting(fs, &world, b, colored);
            let ext = external_textures::load(fs, &world, b, true);
            let level = LevelBsp {
                bsp: b,
                lit: light.rgb.as_deref(),
                hdr_lit: light.hdr.as_deref(),
                deluxe: light.deluxe.as_deref(),
                external: &ext,
            };
            let refs: Vec<_> = brushes
                .iter()
                .map(|(b, l, e)| LevelBsp {
                    bsp: b,
                    lit: l.rgb.as_deref(),
                    hdr_lit: l.hdr.as_deref(),
                    deluxe: l.deluxe.as_deref(),
                    external: e,
                })
                .collect();
            match r.load_level(level, &refs) {
                Ok((s, ext)) => {
                    subs = s;
                    brush_models = brush_names.into_iter().zip(ext).collect();
                }
                Err(e) => print(&format!("load_world: {e:?}\n")),
            }
            if let Ok(ents) = parse_entities(&b.entities)
                && let Some(w) = ents.first()
            {
                if let Some(f) = w.get("fog") {
                    lv.world_fog = qrender::parse_fog(f);
                }
                for (i, k) in ["wateralpha", "lavaalpha", "slimealpha", "telealpha"].iter().enumerate() {
                    if let Some(v) = w.get(k).or_else(|| w.get(&format!("_{k}"))) {
                        lv.liquid_alpha[i] = Some(qcore::cvar::q_atof(v));
                    }
                }
                if let Some(s) = w.get("sky").or(w.get("skyname")) {
                    lv.world_sky = s.to_string();
                }
            }
        }
        lv.bsp = bsp;
        let sky = if cl.skybox.is_empty() { lv.world_sky.clone() } else { cl.skybox.clone() };
        lv.set_sky(fs, r, &sky);
        lv.models = vec![None; cl.model_precache.len()];
        for (i, name) in cl.model_precache.iter().enumerate().skip(2) {
            let m = if let Some(n) = name.strip_prefix('*') {
                n.parse::<usize>().ok().and_then(|n| subs.get(n.wrapping_sub(1))).map(|&id| Model {
                    id,
                    kind: Kind::Brush,
                    flags: 0,
                    md3: false,
                })
            } else if let Some(&id) = brush_models.get(name) {
                Some(Model { id, kind: Kind::Brush, flags: 0, md3: false })
            } else {
                let m = load_model(fs, r, name, enhanced_models);
                if m.is_none() && !name.trim_end_matches(' ').ends_with(".bsp") {
                    print(&format!("couldn't load model {name}\n"));
                }
                m
            };
            if let Some(m) = m {
                cl.model_flags[i] = m.flags;
            }
            lv.models[i] = m;
            lv.by_name.insert(name.clone(), m);
        }
        if let Some(s) = snd {
            lv.sounds = cl.sound_precache.iter().map(|n| if n.is_empty() { None } else { s.precache(n) }).collect();
        }
        lv
    }

    pub fn set_sky(&mut self, fs: &Fs, r: &mut Renderer, name: &str) {
        let faces = if name.is_empty() { None } else { qrender::load_skybox(fs, name) };
        let _ = r.set_skybox(faces.as_ref());
        let sky = if faces.is_some() { name.to_string() } else { String::new() };
        // Sky_LoadSkyBox keeps a loaded box's (possibly tuned) wind
        if sky != self.sky || sky.is_empty() {
            self.sky_wind = self.load_sky_wind(fs, &sky);
        }
        self.sky = sky;
    }

    /// Skywind_Load_f: the skybox's wind config, or still air.
    pub fn load_sky_wind(&self, fs: &Fs, sky: &str) -> crate::skywind::SkyWind {
        if sky.is_empty() {
            return Default::default();
        }
        fs.open(&crate::skywind::cfg_path(sky))
            .and_then(|d| crate::skywind::SkyWind::parse(&String::from_utf8_lossy(&d)))
            .unwrap_or_default()
    }

    /// A model precached after the level loaded (svcdp_precache). Brush models
    /// load with the world only.
    pub fn late_model(&mut self, fs: &Fs, r: &mut Renderer, cl: &mut ClientState, index: usize) {
        if self.mapname.is_empty() {
            return; // `load` takes it
        }
        let Some(name) = cl.model_precache.get(index).cloned() else { return };
        let m = self.named(fs, r, &name);
        if self.models.len() <= index {
            self.models.resize(index + 1, None);
        }
        self.models[index] = m;
        if let (Some(m), Some(f)) = (m, cl.model_flags.get_mut(index)) {
            *f = m.flags;
        }
    }

    /// Model by name, loading on first use (temp entity models).
    pub fn named(&mut self, fs: &Fs, r: &mut Renderer, name: &str) -> Option<Model> {
        if let Some(m) = self.by_name.get(name) {
            return *m;
        }
        let m = load_model(fs, r, name, self.enhanced_models);
        self.by_name.insert(name.to_string(), m);
        m
    }
}

struct LoadedLighting {
    rgb: Option<Vec<u8>>,
    hdr: Option<Vec<u32>>,
    deluxe: Option<Vec<u8>>,
}

/// Mod_LoadLighting: loads external `.lit2` / `.lit` (v1 RGB8 or HDR E5BGR9)
/// and `.dlit` / `.lux` deluxemaps when they come from the same or a
/// higher-priority game dir than the map. Falls back to embedded BSPX lumps
/// (`LIGHTING_E5BGR9`, `RGBLIGHTING`, `LIGHTINGDIR`).
fn load_lighting(
    fs: &qcore::fs::Fs,
    bsp_path: &str,
    b: &Bsp,
    colored: bool,
) -> LoadedLighting {
    let stem = bsp_path.trim_end_matches(".bsp");
    let expected = if !b.lighting.is_empty() {
        b.lighting.len()
    } else if let Some(h) = &b.hdr_lighting {
        h.len()
    } else {
        b.rgb_lighting.as_ref().map_or(0, |l| l.len() / 3)
    };
    let mut rgb = None;
    let mut hdr = None;
    if colored {
        // Prefer external .lit2 (HDR) > .lit (HDR or RGB8) > embedded BSPX HDR > embedded RGB.
        for ext in [".lit2", ".lit"] {
            if let Some(data) = fs.open_companion(bsp_path, &format!("{stem}{ext}"))
                && let Ok(parsed) = qformats::bsp::parse_lit_any(&data)
            {
                match parsed {
                    qformats::bsp::LitData::HdrE5Bgr9(v) if expected == 0 || v.len() == expected => {
                        hdr = Some(v);
                        break;
                    }
                    qformats::bsp::LitData::Rgb8(v) if expected == 0 || v.len() == expected * 3 => {
                        rgb = Some(v);
                        break;
                    }
                    _ => {}
                }
            }
        }
        if hdr.is_none() {
            hdr = b.hdr_lighting.clone();
        }
        if hdr.is_none() && rgb.is_none() {
            rgb = b.rgb_lighting.clone();
        }
    }
    let dlx_expected = if expected > 0 {
        expected
    } else {
        hdr.as_ref().map_or_else(|| rgb.as_ref().map_or(0, |r| r.len() / 3), Vec::len)
    };
    let deluxe = [".dlit", ".lux"]
        .into_iter()
        .find_map(|ext| {
            fs.open_companion(bsp_path, &format!("{stem}{ext}"))
                .and_then(|d| qformats::bsp::parse_dlit(&d).ok())
                .filter(|d| dlx_expected == 0 || d.len() == dlx_expected * 3)
        })
        .or_else(|| b.deluxemap.clone());
    LoadedLighting { rgb, hdr, deluxe }
}
