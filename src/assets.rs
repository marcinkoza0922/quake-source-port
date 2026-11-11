//! Per-level assets: models/sounds from the client precache lists loaded
//! into qrender/qsound, plus the parsed BSP (ambient levels, worldspawn keys).

use std::collections::HashMap;
use std::sync::Arc;

use qclient::ClientState;
use qcore::fs::Fs;
use qformats::{Bsp, Mdl, Spr, parse_entities};
use qrender::{Fog, ModelId, Renderer};
use qsound::{Sfx, SoundSystem};

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
}

#[derive(Default)]
pub struct Level {
    pub mapname: String,
    /// Indexed by client model index.
    pub models: Vec<Option<Model>>,
    /// Indexed by client sound index.
    pub sounds: Vec<Option<Arc<Sfx>>>,
    /// Models by name (temp entities, beams).
    pub by_name: HashMap<String, Option<Model>>,
    pub bsp: Option<Bsp>,
    pub world_fog: Fog,
    pub world_sky: String,
}

fn load_model(fs: &Fs, r: &mut Renderer, name: &str) -> Option<Model> {
    let data = fs.open(name)?;
    if data.len() >= 4 && &data[..4] == b"IDPO" {
        let m = Mdl::parse(&data).ok()?;
        let id = r.load_alias(&m).ok()?;
        return Some(Model { id, kind: Kind::Alias, flags: m.flags as u32 });
    }
    if data.len() >= 4 && &data[..4] == b"IDSP" {
        let s = Spr::parse(&data).ok()?;
        let id = r.load_sprite(&s).ok()?;
        return Some(Model { id, kind: Kind::Sprite, flags: 0 });
    }
    None
}

impl Level {
    /// Loads everything in `cl`'s precache lists; sets `cl.model_flags`.
    pub fn load(fs: &Fs, r: &mut Renderer, snd: Option<&mut SoundSystem>, cl: &mut ClientState, print: &mut dyn FnMut(&str)) -> Level {
        let mut lv = Level { mapname: cl.mapname.clone(), ..Default::default() };
        let world = cl.model_precache.get(1).cloned().unwrap_or_default();
        let bsp = fs.open(&world).and_then(|d| Bsp::parse(&d).map_err(|e| print(&format!("{world}: {e}\n"))).ok());
        let mut subs = Vec::new();
        // External brush models (maps/b_*.bsp) load together with the world.
        let mut brush_names = Vec::new();
        let mut brushes = Vec::new();
        for name in cl.model_precache.iter().skip(2) {
            if name.starts_with('*') || !name.ends_with(".bsp") {
                continue;
            }
            let Some(d) = fs.open(name) else { continue };
            if d.starts_with(b"IDPO") || d.starts_with(b"IDSP") {
                continue;
            }
            match Bsp::parse(&d) {
                Ok(b) => {
                    let lit = fs.open(&format!("{}.lit", name.trim_end_matches(".bsp"))).and_then(|d| qformats::bsp::parse_lit(&d).ok()).or_else(|| b.rgb_lighting.clone());
                    brush_names.push(name.clone());
                    brushes.push((b, lit));
                }
                Err(e) => print(&format!("{name}: {e}\n")),
            }
        }
        let mut brush_models = HashMap::new();
        if let Some(b) = &bsp {
            let base = world.trim_end_matches(".bsp");
            let lit = fs.open(&format!("{base}.lit")).and_then(|d| qformats::bsp::parse_lit(&d).ok()).or_else(|| b.rgb_lighting.clone());
            let refs: Vec<_> = brushes.iter().map(|(b, l)| (b, l.as_deref())).collect();
            match r.load_level(b, lit.as_deref(), &refs) {
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
                })
            } else if let Some(&id) = brush_models.get(name) {
                Some(Model { id, kind: Kind::Brush, flags: 0 })
            } else {
                let m = load_model(fs, r, name);
                if m.is_none() && !name.ends_with(".bsp") {
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
    }

    /// Model by name, loading on first use (temp entity models).
    pub fn named(&mut self, fs: &Fs, r: &mut Renderer, name: &str) -> Option<Model> {
        if let Some(m) = self.by_name.get(name) {
            return *m;
        }
        let m = load_model(fs, r, name);
        self.by_name.insert(name.to_string(), m);
        m
    }
}
