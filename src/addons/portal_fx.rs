//! Bounded cosmetic shot timeline; particles and sound share the impact instant.
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, Renderer, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;
use qclient::PortalEffect;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);
struct Shot { event: PortalEffect, time: f64, impact: bool }
pub struct Presentation {
    shots: Vec<Shot>,
    last: [Option<(usize, f64)>; 256],
    rate: u32,
    cues: Vec<Arc<Sfx>>,
    pic: Option<PicId>,
}

impl Default for Presentation {
    fn default() -> Self {
        Self { shots: Vec::new(), last: [None; 256], rate: 0, cues: Vec::new(), pic: None }
    }
}
impl Presentation {
    pub fn clear(&mut self) { self.shots.clear(); self.last.fill(None); }
    pub fn receive(&mut self, event: PortalEffect, time: f64, snd: Option<&SoundSystem>) {
        if self.shots.len() >= 64 { self.shots.remove(0); }
        if event.kind != 2 {
            self.last[event.owner as usize] = Some((event.slot as usize,time));
        }
        if event.kind == 2 {
            self.play(3, &event, event.end, snd);
            if (Vec3::from(event.end)-Vec3::from(event.start)).length() > 128.0 {
                self.play(3, &event, event.start, snd);
            }
        } else {
            self.play(0, &event, event.start, snd);
        }
        self.shots.push(Shot { event, time, impact: false });
    }
    fn play(&mut self, cue: usize, e: &PortalEffect, origin: [f32;3], snd: Option<&SoundSystem>) {
        let Some(snd) = snd else { return };
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..4).flat_map(|k| (0..2).map(move |s| Arc::new(qsound::portal::render(k,s,snd.rate())))).collect();
        }
        snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx: self.cues[cue*2+e.slot as usize].clone(),
            origin, volume: 0.8, attenuation: 0.7 });
    }
    pub fn weapon(&self, owner: usize, time: f64, view: View, lv: &mut crate::assets::Level,
        fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<RenderEntity> {
        let key = "@portal_projector";
        if !lv.by_name.contains_key(key) {
            let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
            let model = r.load_alias(&super::portal_model::model(&palette)).ok().map(|id|
                crate::assets::Model { id, kind: crate::assets::Kind::Alias, flags: 0 });
            lv.by_name.insert(key.into(),model);
        }
        let model = lv.by_name.get(key).copied().flatten()?;
        let (slot, shot) = self.last[owner.min(255)].unwrap_or((0,-100.0));
        let age = (time-shot).max(0.0) as f32;
        let recoil = (-age*18.0).exp();
        let (f,r,u) = angle_vectors(view.angles.into());
        let bob = (time as f32 * 2.5).sin()*0.15;
        Some(RenderEntity { model: model.id, origin: (Vec3::from(view.origin) + f*(23.0-recoil*2.5) + r*9.0 - u*(8.0+bob)).to_array(),
            angles: [view.angles[0], view.angles[1]+12.0, view.angles[2]], skin: ((owner%4)*2+slot) as u32, viewmodel: true, ..Default::default() })
    }
    pub fn frame(&mut self, time: f64, view: &View, r: &mut Renderer, snd: Option<&SoundSystem>, geom: &mut Geometry) {
        self.shots.retain(|s| time >= s.time && time-s.time < 0.8);
        let impacts: Vec<_> = self.shots.iter_mut().filter_map(|s| {
            if !s.impact && time-s.time >= 0.12 && s.event.kind != 2 {
                s.impact = true; Some(s.event)
            } else { None }
        }).collect();
        for e in impacts { self.play(if e.kind == 1 {1} else {2}, &e, e.end, snd); }
        if self.shots.is_empty() { return; }
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let mut pixels = Vec::new();
                for y in 0..16 { for x in 0..16 {
                    let d = ((x as f32-7.5).powi(2)+(y as f32-7.5).powi(2)).sqrt()/7.5;
                    pixels.extend([255,255,255,((1.0-d).max(0.0).powi(2)*255.0) as u8]);
                }}
                let Ok(p) = r.register_pic_rgba_linear(16,16,&pixels) else { return };
                self.pic = Some(p); p
            }
        };
        let (_,right,up) = angle_vectors(view.angles.into());
        let first = geom.1.len() as u32;
        for shot in &self.shots {
            let e = shot.event;
            let age = (time-shot.time) as f32;
            let color = super::portal_draw::colour(e.owner as usize,(e.slot ^ u8::from(e.kind == 2)) as usize);
            let start = Vec3::from(e.start);
            let end = Vec3::from(e.end);
            if age < 0.12 && e.kind != 2 {
                let dir = (end-start).normalize();
                let axis = if dir.z.abs() < 0.9 { Vec3::new(0.0,0.0,1.0) } else { Vec3::new(0.0,1.0,0.0) };
                let a = dir.cross(axis).normalize(); let b = dir.cross(a);
                for i in 0..32 {
                    let t = (age/0.12-i as f32*0.006).max(0.0);
                    let phase = i as f32*0.65+age*65.0;
                    let pos = start+(end-start)*t+(a*phase.cos()+b*phase.sin())*2.0;
                    dot(geom,pos,right,up,if i==0 {7.0} else {2.5},color,1.0-i as f32/35.0);
                }
            } else {
                let t = if e.kind == 2 { age } else { age-0.12 };
                let fade = (1.0-t/0.55).max(0.0);
                for i in 0..40 {
                    let theta = i as f32*2.39996;
                    let z = 1.0-2.0*i as f32/40.0;
                    let radial = (1.0-z*z).sqrt();
                    let dir = Vec3::new(theta.cos()*radial,theta.sin()*radial,z);
                    let size = if e.kind == 0 { 2.0 } else { 3.5 };
                    let spread = if e.kind == 0 { 12.0*(1.0-t/0.55) } else { 8.0+t*75.0 };
                    dot(geom,end+dir*spread,right,up,size,color,fade);
                }
            }
        }
        geom.2.push(FteBatch { pic, blend: FteBlend::AddA, lines: false, first_index: first,
            num_indices: geom.1.len() as u32-first, soft: false });
    }
}
fn dot(g: &mut Geometry, p: Vec3, r: Vec3, u: Vec3, size: f32, rgb: [u8;3], alpha: f32) {
    let base = g.0.len() as u32;
    for (x,y,uv) in [(-1.0,-1.0,[0.0,0.0]),(1.0,-1.0,[1.0,0.0]),(1.0,1.0,[1.0,1.0]),(-1.0,1.0,[0.0,1.0])] {
        g.0.push(FteVertex { pos: (p+r*x*size+u*y*size).to_array(), uv, color: [rgb[0],rgb[1],rgb[2],(alpha*255.0) as u8] });
    }
    g.1.extend([base,base+1,base+2,base,base+2,base+3]);
}
