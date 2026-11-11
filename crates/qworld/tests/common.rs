//! Mock edict storage + game data helpers shared by tests.
#![allow(dead_code)]

use qworld::{E, Edicts, Ent, F, V, Vec3, World};
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct Ed {
    pub free: bool,
    pub v: [Vec3; 14],
    pub f: [f32; 15],
    pub e: [Ent; 4],
    pub touch: bool,
}

#[derive(Default)]
pub struct Ents {
    pub ed: Vec<Ed>,
    pub touches: Vec<(Ent, Ent)>,
    pub thinks: Vec<Ent>,
    pub blocked: Vec<(Ent, Ent)>,
}

impl Ents {
    pub fn spawn(&mut self) -> Ent {
        self.ed.push(Ed::default());
        (self.ed.len() - 1) as Ent
    }
}

impl Edicts for Ents {
    fn num_edicts(&self) -> u32 {
        self.ed.len() as u32
    }
    fn is_free(&self, e: Ent) -> bool {
        self.ed[e as usize].free
    }
    fn v(&self, e: Ent, f: V) -> Vec3 {
        self.ed[e as usize].v[f as usize]
    }
    fn set_v(&mut self, e: Ent, f: V, v: Vec3) {
        self.ed[e as usize].v[f as usize] = v;
        if matches!(f, V::Mins | V::Maxs) {
            let d = &mut self.ed[e as usize];
            d.v[V::Size as usize] = d.v[V::Maxs as usize] - d.v[V::Mins as usize];
        }
    }
    fn f(&self, e: Ent, f: F) -> f32 {
        self.ed[e as usize].f[f as usize]
    }
    fn set_f(&mut self, e: Ent, f: F, v: f32) {
        self.ed[e as usize].f[f as usize] = v;
    }
    fn e(&self, e: Ent, f: E) -> Ent {
        self.ed[e as usize].e[f as usize]
    }
    fn set_e(&mut self, e: Ent, f: E, v: Ent) {
        self.ed[e as usize].e[f as usize] = v;
    }
    fn has_touch(&self, e: Ent) -> bool {
        self.ed[e as usize].touch
    }
    fn touch(&mut self, _w: &mut World, ent: Ent, other: Ent) {
        self.touches.push((ent, other));
    }
    fn blocked(&mut self, _w: &mut World, pusher: Ent, other: Ent) {
        self.blocked.push((pusher, other));
    }
    fn think(&mut self, _w: &mut World, ent: Ent, _time: f64) {
        self.thinks.push(ent);
    }
}

pub fn basedir() -> Option<PathBuf> {
    let d = std::env::var("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    d.join("id1").is_dir().then_some(d)
}

pub fn fs(game: Option<&str>) -> Option<qcore::fs::Fs> {
    let mut fs = qcore::fs::Fs::new(basedir()?);
    fs.add_game_dir("id1");
    if let Some(g) = game {
        fs.add_game_dir(g);
    }
    Some(fs)
}

/// Tiny xorshift RNG.
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32 * (hi - lo)
    }
}
