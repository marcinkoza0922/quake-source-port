//! qsound — sound cache, channels, spatialization, mixer and music streaming.
//! Port of ironwail snd_dma.c / snd_mix.c / snd_mem.c / bgmusic.c, extended with
//! optional 3D binaural (HRTF) spatialization, occlusion, environmental reverb,
//! surface-aware footsteps, per-category volume mixing and music crossfading.
//! No device code: the platform audio callback calls [`Mixer::paint`]; the game
//! thread drives [`SoundSystem`], which sends commands through a short-lock queue.

pub mod music;
pub mod feedback;
pub mod category;
pub mod reverb;
pub mod hrtf;
pub mod footsteps;
pub mod gel;
pub mod cable;
pub mod hookshot;
pub mod boomerang;
pub mod portal;
pub mod rail;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use category::{CategoryVolumes, SoundCategory, classify_sound};
pub use footsteps::FootstepSurface;
pub use music::{MusicStream, cd_track_paths};
pub use reverb::ReverbParams;
#[path = "lib/mixer.rs"]
mod mixer;
pub use mixer::Mixer;
use mixer::{Channel, advance, wrap};
#[path = "lib/paint.rs"]
mod paint;
#[path = "lib/mixer_events.rs"]
mod mixer_events;
#[path = "lib/shared.rs"]
mod shared;
pub use shared::SharedMixer;

pub const MAX_DYNAMIC_CHANNELS: usize = 128;
pub const NUM_AMBIENTS: usize = 4;
pub const MAX_CHANNELS: usize = 1024;
pub const SOUND_NOMINAL_CLIP_DIST: f32 = 1000.0;
/// Ambient sound files indexed by ambient type (water, sky, slime, lava).
pub const AMBIENT_SOUNDS: [Option<&str>; NUM_AMBIENTS] = [
    Some("ambience/water1.wav"),
    Some("ambience/wind2.wav"),
    None,
    None,
];

pub type Vec3 = [f32; 3];

pub(crate) fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A cached, resampled mono sound at the output rate.
#[derive(Debug)]
pub struct Sfx {
    pub name: String,
    pub data: Vec<i16>,
    pub loop_start: Option<usize>,
}

/// Load a WAV and resample it to `out_rate` (ResampleSfx, nearest sample).
/// Stereo files are downmixed (qformats counts frames, not samples).
pub fn load_sfx(name: &str, bytes: &[u8], out_rate: u32) -> Option<Sfx> {
    let wav = qformats::Wav::parse(bytes).ok()?;
    let ch = usize::from(wav.channels.max(1));
    let w = usize::from(wav.width);
    let frames = wav.samples.min(wav.data.len() / (w * ch).max(1));
    let sample = |i: usize| -> i32 {
        let mut acc = 0i32;
        for c in 0..ch {
            let o = (i * ch + c) * w;
            acc += if w == 1 {
                (i32::from(wav.data[o]) - 128) << 8
            } else {
                i32::from(i16::from_le_bytes([wav.data[o], wav.data[o + 1]]))
            };
        }
        acc / ch as i32
    };
    let step = f64::from(wav.rate) / f64::from(out_rate);
    let out_len = (frames as f64 / step) as usize;
    let data = (0..out_len)
        .map(|i| sample(((i as f64 * step) as usize).min(frames.saturating_sub(1))) as i16)
        .collect();
    Some(Sfx {
        name: name.to_owned(),
        data,
        loop_start: wav
            .loop_start
            .map(|l| ((l as f64 / step) as usize).min(out_len)),
    })
}

/// Listener state, sent once per frame (S_Update).
#[derive(Clone, Copy, Debug, Default)]
pub struct Listener {
    pub origin: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    /// View entity; sounds from it are not spatialized.
    pub view_entity: i32,
    /// `ambient_sound_level` of the leaf the view is in; `None` outside a map.
    pub ambient_levels: Option<[u8; NUM_AMBIENTS]>,
    pub underwater: bool,
    /// Host frame time in seconds (ambient fade).
    pub frametime: f32,
    /// Split-screen: the other local players' ears and view entities. A
    /// sound plays as loud as the nearest player hears it (mono for these).
    pub others: [(Vec3, i32); MAX_OTHER_LISTENERS],
    pub num_others: usize,
}

pub const MAX_OTHER_LISTENERS: usize = 5;

/// Events mirrored from the client (the binary converts `ClientEvent`s).
#[derive(Clone, Debug)]
pub enum SoundEvent {
    Start {
        entity: i32,
        channel: i32,
        sfx: Arc<Sfx>,
        origin: Vec3,
        /// 0..1
        volume: f32,
        attenuation: f32,
    },
    Stop {
        entity: i32,
        channel: i32,
    },
    Static {
        sfx: Arc<Sfx>,
        origin: Vec3,
        volume: f32,
        attenuation: f32,
    },
    /// Local (unspatialized) sound, e.g. menu sounds.
    Local(Arc<Sfx>),
    StopAll,
    /// Entity and local sounds only; ambients and static sounds keep playing.
    StopDynamic,
    ClearStatic,
}

enum Cmd {
    Event(SoundEvent),
    Listener(Listener),
    Volume(f32),
    BgmVolume(f32),
    CategoryVolumes(CategoryVolumes),
    WaterFx(bool),
    Reverb(ReverbParams),
    Hrtf(bool),
    Occlusion(bool),
    SetOcclusion {
        entity: i32,
        channel: i32,
        value: f32,
    },
    Occlusions {
        dynamic: Vec<(i32, i32, f32)>,
        statics: Vec<(usize, f32)>,
    },
    Music(Option<Box<MusicStream>>),
    MusicPause(bool),
    MusicCrossfade(f32),
}

type Loader = Box<dyn FnMut(&str) -> Option<Vec<u8>> + Send>;
type Queue = Arc<Mutex<Vec<Cmd>>>;

/// Game-thread side: sound cache and command sender.
pub struct SoundSystem {
    rate: u32,
    loader: Loader,
    cache: HashMap<String, Option<Arc<Sfx>>>,
    queue: Queue,
    ambients: [Option<Arc<Sfx>>; NUM_AMBIENTS],
}

impl SoundSystem {
    /// `loader` fetches file bytes by game path (e.g. from `qcore::Fs`).
    pub fn new(
        rate: u32,
        loader: impl FnMut(&str) -> Option<Vec<u8>> + Send + 'static,
    ) -> (Self, Mixer) {
        let queue: Queue = Arc::default();
        let mut s = Self {
            rate,
            loader: Box::new(loader),
            cache: HashMap::new(),
            queue: queue.clone(),
            ambients: Default::default(),
        };
        s.ambients = AMBIENT_SOUNDS.map(|n| n.and_then(|n| s.precache(n)));
        let mixer = Mixer::new(rate, queue, s.ambients.clone());
        (s, mixer)
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// S_PrecacheSound: `name` is relative to `sound/`. Failures are cached too.
    pub fn precache(&mut self, name: &str) -> Option<Arc<Sfx>> {
        if let Some(s) = self.cache.get(name) {
            return s.clone();
        }
        let s = (self.loader)(&format!("sound/{name}"))
            .and_then(|b| load_sfx(name, &b, self.rate))
            .map(Arc::new);
        self.cache.insert(name.to_owned(), s.clone());
        s
    }

    fn push(&self, c: Cmd) {
        self.queue.lock().unwrap().push(c);
    }
    pub fn event(&self, e: SoundEvent) {
        self.push(Cmd::Event(e));
    }
    pub fn update(&self, l: Listener) {
        self.push(Cmd::Listener(l));
    }
    /// `volume` cvar, 0..1.
    pub fn set_volume(&self, v: f32) {
        self.push(Cmd::Volume(v.clamp(0.0, 1.0)));
    }
    /// `bgmvolume` cvar, 0..1.
    pub fn set_bgm_volume(&self, v: f32) {
        self.push(Cmd::BgmVolume(v.clamp(0.0, 1.0)));
    }
    /// Per-category SFX volume multipliers (`s_vol_*`).
    pub fn set_category_volumes(&self, v: CategoryVolumes) {
        self.push(Cmd::CategoryVolumes(v.clamped()));
    }
    /// `snd_waterfx`: underwater lowpass.
    pub fn set_water_fx(&self, on: bool) {
        self.push(Cmd::WaterFx(on));
    }
    /// Environmental reverb parameters from the host room probe.
    pub fn set_reverb(&self, params: ReverbParams) {
        self.push(Cmd::Reverb(params.clamped()));
    }
    /// `s_hrtf`: 3D binaural spatialization.
    pub fn set_hrtf(&self, on: bool) {
        self.push(Cmd::Hrtf(on));
    }
    /// `s_occlusion`: wall muffling for occluded sounds.
    pub fn set_occlusion(&self, on: bool) {
        self.push(Cmd::Occlusion(on));
    }
    /// Set immediate occlusion for a just-started channel (0 = clear, 1 = blocked).
    pub fn set_channel_occlusion(&self, entity: i32, channel: i32, value: f32) {
        self.push(Cmd::SetOcclusion {
            entity,
            channel,
            value: value.clamp(0.0, 1.0),
        });
    }
    /// Batch-update occlusion for active dynamic and static channels.
    pub fn update_occlusions(&self, dynamic: Vec<(i32, i32, f32)>, statics: Vec<(usize, f32)>) {
        self.push(Cmd::Occlusions { dynamic, statics });
    }
    /// `s_music_crossfade`: crossfade duration in seconds (0 = instant cut).
    pub fn set_music_crossfade(&self, secs: f32) {
        self.push(Cmd::MusicCrossfade(secs.clamp(0.0, 5.0)));
    }
    /// Play decoded music from raw file bytes (used by the soundtrack switcher).
    pub fn play_music_bytes(&self, bytes: Vec<u8>, ext: &str, looping: bool) -> bool {
        if let Some(m) = MusicStream::new(bytes, ext, looping, self.rate) {
            self.push(Cmd::Music(Some(Box::new(m))));
            return true;
        }
        false
    }
    /// Play `music/<name>.(ogg|mp3|flac|wav)` (or `name` with an extension).
    pub fn play_music(&mut self, name: &str, looping: bool) -> bool {
        let base = name.strip_prefix("music/").unwrap_or(name);
        let candidates: Vec<(String, &str)> = match base.rsplit_once('.') {
            Some((_, e)) if music::MUSIC_EXTS.contains(&e) => vec![(format!("music/{base}"), e)],
            _ => music::MUSIC_EXTS
                .iter()
                .map(|e| (format!("music/{base}.{e}"), *e))
                .collect(),
        };
        for (path, ext) in candidates {
            if let Some(b) = (self.loader)(&path)
                && let Some(m) = MusicStream::new(b, ext, looping, self.rate)
            {
                self.push(Cmd::Music(Some(Box::new(m))));
                return true;
            }
        }
        false
    }
    /// CD track mapping (`cd play N`, svc_cdtrack): `music/trackNN`.
    pub fn play_cd_track(&mut self, track: u8, looping: bool) -> bool {
        self.play_music(&format!("track{track:02}"), looping)
    }
    pub fn stop_music(&self) {
        self.push(Cmd::Music(None));
    }
    pub fn pause_music(&self, paused: bool) {
        self.push(Cmd::MusicPause(paused));
    }
}
