//! Stub presentation: filled in by its feature branch.
use crate::gfx::Renderer;
use qclient::guns::GunEvent;
use qrender::{FteBatch, FteVertex, RenderEntity, View};
use qsound::SoundSystem;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

#[derive(Default)]
pub struct Presentation {}

impl Presentation {
    /// Drops every effect in flight (new map).
    pub fn clear(&mut self) {}

    /// One `//qsp_gun` event of this toy.
    pub fn receive(&mut self, _e: &GunEvent, _time: f64, _snd: Option<&SoundSystem>) {}

    /// `owner`'s view model entities while the toy is held.
    #[allow(clippy::too_many_arguments)]
    pub fn weapons(&mut self, _owner: usize, _cl: &qclient::ClientState, _time: f64, _view: View, _lv: &mut crate::assets::Level, _fs: &qcore::fs::Fs, _r: &mut Renderer) -> Vec<RenderEntity> {
        Vec::new()
    }

    /// Effect geometry (tracers, flashes, smoke, casings).
    pub fn frame(&mut self, _time: f64, _view: &View, _r: &mut Renderer, _geom: &mut Geometry) {}
}
