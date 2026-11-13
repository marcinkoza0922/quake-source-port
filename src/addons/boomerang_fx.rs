//! Client presentation of the boomerang (`qserver::addons::boomerang`):
//! parses `//qsp_boom` lines, draws the held and flying boomerang, numbered
//! target markers, circling stun stars, and plays the synthesised cues.
use super::boomerang_model::{self as bm, RING, STAR};
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, Renderer, View};
use qsound::boomerang::{CATCH, STUN, SWISH, THROW};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

#[derive(Clone, Copy, Debug)]
struct Fly {
    pos: Vec3,
    prev: Vec3,
    at: f64,
    swish: f64,
}

#[derive(Default)]
pub struct Presentation {
    /// Per owner: queued (entity, head height).
    marks: Vec<Vec<(usize, f32)>>,
    flights: Vec<Option<Fly>>,
    /// (entity, start, end, head height)
    stuns: Vec<(usize, f64, f64, f32)>,
    rate: u32,
    cues: Vec<Arc<Sfx>>,
    pic: Option<PicId>,
}

fn nums(words: &[&str]) -> Vec<f32> {
    words
        .iter()
        .filter_map(|w| w.parse::<f32>().ok())
        .filter(|v| v.is_finite() && v.abs() < 1e6)
        .collect()
}

impl Presentation {
    pub fn clear(&mut self) {
        self.marks.clear();
        self.flights.clear();
        self.stuns.clear();
    }

    fn owner(&mut self, o: f32) -> Option<usize> {
        let o = o as usize;
        if !(0.0..256.0).contains(&(o as f32)) {
            return None;
        }
        if self.marks.len() <= o {
            self.marks.resize(o + 1, Vec::new());
            self.flights.resize(o + 1, None);
        }
        Some(o)
    }

    /// One `//qsp_boom` line (prefix stripped).
    pub fn receive(
        &mut self,
        line: &str,
        time: f64,
        cl: &qclient::ClientState,
        snd: Option<&SoundSystem>,
    ) {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some((&kind, rest)) = words.split_first() else {
            return;
        };
        let n = nums(rest);
        match (kind, n.as_slice()) {
            ("m", [o, list @ ..]) => {
                let Some(o) = self.owner(*o) else { return };
                self.marks[o] = list
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| (c[0] as usize, c[1]))
                    .collect();
            }
            ("t" | "f", [o, x, y, z]) => {
                let Some(o) = self.owner(*o) else { return };
                let pos = Vec3::new(*x, *y, *z);
                let prev = self.flights[o].map_or(pos, |f| f.pos);
                let swish = self.flights[o].map_or(time, |f| f.swish);
                self.flights[o] = Some(Fly {
                    pos,
                    prev,
                    at: time,
                    swish,
                });
                if kind == "t" {
                    self.play(THROW, pos, snd);
                }
            }
            ("c", [o]) => {
                let Some(o) = self.owner(*o) else { return };
                if let Some(f) = self.flights[o].take() {
                    self.play(CATCH, f.pos, snd);
                }
            }
            ("s", [e, secs, head]) => {
                let e = *e as usize;
                let end = time + secs.clamp(0.0, 60.0) as f64;
                match self.stuns.iter_mut().find(|s| s.0 == e) {
                    Some(s) => s.2 = s.2.max(end),
                    None => self.stuns.push((e, time, end, *head)),
                }
                if let Some(ent) = cl.entities.get(e) {
                    self.play(STUN, Vec3::from(ent.origin), snd);
                }
            }
            _ => {}
        }
    }

    fn play(&mut self, cue: usize, origin: Vec3, snd: Option<&SoundSystem>) {
        let Some(snd) = snd else { return };
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..4)
                .map(|k| Arc::new(qsound::boomerang::render(k, snd.rate())))
                .collect();
        }
        snd.event(SoundEvent::Start {
            entity: -1,
            channel: 0,
            sfx: self.cues[cue].clone(),
            origin: origin.to_array(),
            volume: if cue == SWISH { 0.5 } else { 0.9 },
            attenuation: 0.8,
        });
    }

    /// This owner's boomerang is in the air (no view model then).
    pub fn flying(&self, owner: usize) -> bool {
        self.flights.get(owner).is_some_and(|f| f.is_some())
    }

    fn model(
        lv: &mut crate::assets::Level,
        fs: &qcore::fs::Fs,
        r: &mut Renderer,
    ) -> Option<crate::assets::Model> {
        Self::load("@boomerang", bm::model, lv, fs, r)
    }

    fn load(
        key: &str,
        build: fn(&[u8]) -> qformats::mdl::Mdl,
        lv: &mut crate::assets::Level,
        fs: &qcore::fs::Fs,
        r: &mut Renderer,
    ) -> Option<crate::assets::Model> {
        if !lv.by_name.contains_key(key) {
            let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
            let model = r
                .load_alias(&build(&palette))
                .ok()
                .map(|id| crate::assets::Model {
                    id,
                    kind: crate::assets::Kind::Alias,
                    flags: 0,
                });
            lv.by_name.insert(key.into(), model);
        }
        lv.by_name.get(key).copied().flatten()
    }

    /// The held boomerang as a view weapon (None while it is thrown).
    pub fn weapon(
        &self,
        owner: usize,
        time: f64,
        view: View,
        lv: &mut crate::assets::Level,
        fs: &qcore::fs::Fs,
        r: &mut Renderer,
    ) -> Option<RenderEntity> {
        if self.flying(owner) {
            return None;
        }
        let model = Self::load("@boomerang_view", bm::view_model, lv, fs, r)?;
        let (fwd, right, up) = angle_vectors(view.angles.into());
        let bob = (time as f32 * 2.0).sin() * 0.2;
        let origin = Vec3::from(view.origin) + fwd * 26.0 + right * 7.0 - up * (11.0 + bob);
        let a = view.angles;
        Some(RenderEntity {
            model: model.id,
            origin: origin.to_array(),
            // flat face to the camera, one arm raised: reads as a V in the hand
            angles: [-a[0] - 40.0, a[1] + 70.0, a[2] + 80.0],
            viewmodel: true,
            ..Default::default()
        })
    }

    /// Flying boomerangs, spinning flat, and the swish while they fly.
    pub fn entities(
        &mut self,
        time: f64,
        lv: &mut crate::assets::Level,
        fs: &qcore::fs::Fs,
        r: &mut Renderer,
        snd: Option<&SoundSystem>,
        ents: &mut Vec<RenderEntity>,
    ) {
        if self.flights.iter().all(Option::is_none) {
            return;
        }
        let Some(model) = Self::model(lv, fs, r) else {
            return;
        };
        let mut swishes = Vec::new();
        for f in self.flights.iter_mut().flatten() {
            // ease between the last two server positions
            let k = ((time - f.at) * 72.0).clamp(0.0, 1.0) as f32;
            let pos = f.prev + (f.pos - f.prev) * k;
            ents.push(RenderEntity {
                model: model.id,
                origin: pos.to_array(),
                angles: [0.0, (time * 1080.0 % 360.0) as f32, 12.0],
                ..Default::default()
            });
            if time - f.swish > 0.2 || time < f.swish {
                f.swish = time;
                swishes.push(pos);
            }
        }
        for p in swishes {
            self.play(SWISH, p, snd);
        }
    }

    /// Target markers with queue numbers, and stun stars.
    pub fn frame(
        &mut self,
        time: f64,
        cl: &qclient::ClientState,
        view: &View,
        r: &mut Renderer,
        geom: &mut Geometry,
    ) {
        self.stuns
            .retain(|s| time < s.2 && time >= s.1 - 0.1 && s.0 < cl.entities.len());
        if self.stuns.is_empty() && self.marks.iter().all(Vec::is_empty) {
            return;
        }
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let side = (bm::CELL * bm::GRID) as u32;
                let Ok(p) = r.register_pic_rgba(side, side, &bm::atlas()) else {
                    return;
                };
                self.pic = Some(p);
                p
            }
        };
        let (_, right, up) = angle_vectors(view.angles.into());
        let first = geom.1.len() as u32;
        let head = |e: usize, h: f32| Vec3::from(cl.entities[e].origin) + Vec3::new(0.0, 0.0, h);
        for list in &self.marks {
            for (i, &(e, h)) in list.iter().enumerate() {
                if e >= cl.entities.len() {
                    continue;
                }
                let p = head(e, h) + Vec3::new(0.0, 0.0, 4.0);
                let pulse = 1.0 + 0.12 * (time as f32 * 8.0).sin();
                quad(geom, p, right, up, 9.0 * pulse, RING, [255, 60, 40], 1.0);
                let digits = (i + 1).to_string();
                let w = digits.len() as f32 * 7.0;
                for (k, d) in digits.bytes().enumerate() {
                    let x = -w * 0.5 + 3.5 + k as f32 * 7.0;
                    quad(
                        geom,
                        p + right * x + up * 14.0,
                        right,
                        up,
                        5.0,
                        (d - b'0') as usize,
                        [255, 230, 90],
                        1.0,
                    );
                }
            }
        }
        for &(e, start, end, h) in &self.stuns {
            let fade = ((end - time) / 0.5).clamp(0.0, 1.0) as f32
                * ((time - start) / 0.15).clamp(0.0, 1.0) as f32;
            let c = head(e, h) + Vec3::new(0.0, 0.0, 8.0);
            for k in 0..5 {
                let a = time as f32 * 4.0 + k as f32 * std::f32::consts::TAU / 5.0;
                let p = c + Vec3::new(a.cos() * 13.0, a.sin() * 13.0, (a * 2.0).sin() * 2.5);
                quad(geom, p, right, up, 3.5, STAR, [255, 235, 60], fade);
            }
        }
        geom.2.push(FteBatch {
            pic,
            blend: FteBlend::Blend,
            lines: false,
            first_index: first,
            num_indices: geom.1.len() as u32 - first,
            soft: false,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn quad(
    g: &mut Geometry,
    p: Vec3,
    r: Vec3,
    u: Vec3,
    size: f32,
    cell: usize,
    rgb: [u8; 3],
    alpha: f32,
) {
    let ([u0, v0], [u1, v1]) = bm::cell_uv(cell);
    let base = g.0.len() as u32;
    for (x, y, uv) in [
        (-1.0, 1.0, [u0, v0]),
        (1.0, 1.0, [u1, v0]),
        (1.0, -1.0, [u1, v1]),
        (-1.0, -1.0, [u0, v1]),
    ] {
        g.0.push(FteVertex {
            pos: (p + r * x * size + u * y * size).to_array(),
            uv,
            color: [rgb[0], rgb[1], rgb[2], (alpha * 255.0) as u8],
        });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_update_marks_flights_and_stuns() {
        let mut p = Presentation::default();
        let mut cl = qclient::ClientState::new();
        cl.entities.resize(10, Default::default());
        p.receive("m 0 5 24 7 32", 1.0, &cl, None);
        assert_eq!(p.marks[0], vec![(5, 24.0), (7, 32.0)]);
        p.receive("t 0 1 2 3", 1.0, &cl, None);
        assert!(p.flying(0));
        p.receive("f 0 4 5 6", 1.1, &cl, None);
        p.receive("c 0", 1.2, &cl, None);
        assert!(!p.flying(0));
        p.receive("s 5 3 24", 1.0, &cl, None);
        p.receive("s 5 1 24", 1.5, &cl, None);
        assert_eq!(p.stuns.len(), 1);
        assert_eq!(p.stuns[0].2, 4.0);
        // malformed lines are ignored
        p.receive("t 0 nan 1", 1.0, &cl, None);
        p.receive("x", 1.0, &cl, None);
        p.receive("", 1.0, &cl, None);
        assert!(!p.flying(0));
    }
}
