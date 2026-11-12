//! qsound — sound cache, channels, spatialization, mixer and music streaming.
//! Port of ironwail snd_dma.c / snd_mix.c / snd_mem.c / bgmusic.c. No device code:
//! the platform audio callback calls [`Mixer::paint`]; the game thread drives
//! [`SoundSystem`], which sends commands through a short-lock queue.

pub mod music;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use music::{MusicStream, cd_track_paths};
#[path = "lib/paint.rs"]
mod paint;
#[path = "lib/mixer_events.rs"]
mod mixer_events;

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

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f32 {
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
    pub right: Vec3,
    /// View entity; sounds from it are not spatialized.
    pub view_entity: i32,
    /// `ambient_sound_level` of the leaf the view is in; `None` outside a map.
    pub ambient_levels: Option<[u8; NUM_AMBIENTS]>,
    pub underwater: bool,
    /// Host frame time in seconds (ambient fade).
    pub frametime: f32,
}

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
    ClearStatic,
}

enum Cmd {
    Event(SoundEvent),
    Listener(Listener),
    Volume(f32),
    BgmVolume(f32),
    WaterFx(bool),
    Music(Option<Box<MusicStream>>),
    MusicPause(bool),
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
    /// `snd_waterfx`: underwater lowpass.
    pub fn set_water_fx(&self, on: bool) {
        self.push(Cmd::WaterFx(on));
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

#[derive(Clone, Default)]
struct Channel {
    sfx: Option<Arc<Sfx>>,
    entity: i32,
    entchannel: i32,
    origin: Vec3,
    dist_mult: f32,
    master_vol: i32,
    left: i32,
    right: i32,
    pos: usize,
    /// Static/ambient sounds loop from 0 if the sfx has no loop point.
    force_loop: bool,
}

/// Audio-thread side. Call [`Mixer::paint`] from the device callback.
pub struct Mixer {
    rate: u32,
    queue: Queue,
    pending: Vec<Cmd>,
    channels: Vec<Channel>,
    total: usize,
    ambient_sfx: [Option<Arc<Sfx>>; NUM_AMBIENTS],
    ambient_vol: [f32; NUM_AMBIENTS],
    listener: Listener,
    volume: f32,
    bgm_volume: f32,
    water_fx: bool,
    water_mix: f32,
    lp_state: [f32; 2],
    music: Option<Box<MusicStream>>,
    buf: Vec<[i32; 2]>,
    rng: u32,
}

impl Mixer {
    fn new(rate: u32, queue: Queue, ambient_sfx: [Option<Arc<Sfx>>; NUM_AMBIENTS]) -> Self {
        Self {
            rate,
            queue,
            pending: Vec::new(),
            channels: vec![Channel::default(); MAX_CHANNELS],
            total: NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS,
            ambient_sfx,
            ambient_vol: [0.0; NUM_AMBIENTS],
            listener: Listener::default(),
            volume: 0.7,
            bgm_volume: 1.0,
            water_fx: true,
            water_mix: 0.0,
            lp_state: [0.0; 2],
            music: None,
            buf: Vec::new(),
            rng: 0x1234_5678,
        }
    }

    /// Number of channels currently playing.
    pub fn active_channels(&self) -> usize {
        self.channels[..self.total]
            .iter()
            .filter(|c| c.sfx.is_some())
            .count()
    }

    /// Entities and entity channels of playing dynamic channels.
    pub fn dynamic_channels(&self) -> Vec<(i32, i32, String)> {
        self.channels[NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS]
            .iter()
            .filter_map(|c| {
                c.sfx
                    .as_ref()
                    .map(|s| (c.entity, c.entchannel, s.name.clone()))
            })
            .collect()
    }

    /// Current (left, right) volume of the dynamic channel playing on `entity`/`entchannel`.
    pub fn channel_volume(&self, entity: i32, entchannel: i32) -> Option<(i32, i32)> {
        self.channels[NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS]
            .iter()
            .find(|c| c.sfx.is_some() && c.entity == entity && c.entchannel == entchannel)
            .map(|c| (c.left, c.right))
    }

    pub fn music_playing(&self) -> bool {
        self.music.as_ref().is_some_and(|m| !m.finished)
    }

    /// Apply queued commands without painting.
    pub fn process_commands(&mut self) {
        // Short lock: swap the queue out.
        if let Ok(mut q) = self.queue.lock() {
            std::mem::swap(&mut *q, &mut self.pending);
        }
        let mut cmds = std::mem::take(&mut self.pending);
        for c in cmds.drain(..) {
            self.apply(c);
        }
        self.pending = cmds;
    }

    fn apply(&mut self, c: Cmd) {
        match c {
            Cmd::Event(e) => self.event(e),
            Cmd::Listener(l) => {
                self.listener = l;
                self.update_ambients();
                for i in NUM_AMBIENTS..self.total {
                    if self.channels[i].sfx.is_some() {
                        self.spatialize(i);
                    }
                }
            }
            Cmd::Volume(v) => self.volume = v,
            Cmd::BgmVolume(v) => self.bgm_volume = v,
            Cmd::WaterFx(b) => self.water_fx = b,
            Cmd::Music(m) => self.music = m,
            Cmd::MusicPause(p) => {
                if let Some(m) = &mut self.music {
                    m.paused = p;
                }
            }
        }
    }

    /// SND_Spatialize.
    fn spatialize(&mut self, i: usize) {
        let l = self.listener;
        let ch = &mut self.channels[i];
        if ch.entity == l.view_entity || ch.dist_mult == 0.0 {
            ch.left = ch.master_vol;
            ch.right = ch.master_vol;
            return;
        }
        let mut v = sub(ch.origin, l.origin);
        let len = dot(v, v).sqrt();
        if len > 0.0 {
            v = v.map(|x| x / len);
        }
        let dist = len * ch.dist_mult;
        let d = dot(l.right, v);
        let (rscale, lscale) = (1.0 + d, 1.0 - d);
        let mv = ch.master_vol as f32;
        ch.right = ((mv * (1.0 - dist) * rscale) as i32).max(0);
        ch.left = ((mv * (1.0 - dist) * lscale) as i32).max(0);
    }

    /// SND_PickChannel.
    fn pick_channel(&mut self, entity: i32, entchannel: i32) -> Option<usize> {
        let view = self.listener.view_entity;
        let mut best = None;
        let mut life_left = usize::MAX;
        for i in NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS {
            let ch = &self.channels[i];
            // channel 0 never overrides
            if entchannel != 0
                && ch.entity == entity
                && (ch.entchannel == entchannel || entchannel == -1)
            {
                best = Some(i);
                break;
            }
            // don't let monster sounds override player sounds
            if ch.entity == view && entity != view && ch.sfx.is_some() {
                continue;
            }
            let left = ch
                .sfx
                .as_ref()
                .map_or(0, |s| s.data.len().saturating_sub(ch.pos));
            if left < life_left {
                life_left = left;
                best = Some(i);
            }
        }
        if let Some(i) = best {
            self.channels[i].sfx = None;
        }
        best
    }

}

/// Wrap a channel at end of data. Returns false if it should stop.
fn wrap(ch: &mut Channel, sfx: &Sfx) -> bool {
    match sfx.loop_start {
        Some(l) if l < sfx.data.len() => ch.pos = l,
        _ if ch.force_loop && !sfx.data.is_empty() => ch.pos = 0,
        _ => return false,
    }
    true
}

fn advance(ch: &mut Channel, sfx: &Sfx, mut n: usize) -> bool {
    while n > 0 {
        if ch.pos >= sfx.data.len() && !wrap(ch, sfx) {
            return false;
        }
        let k = (sfx.data.len() - ch.pos).min(n);
        ch.pos += k;
        n -= k;
    }
    true
}
