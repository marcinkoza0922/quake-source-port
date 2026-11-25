use super::*;

#[derive(Clone, Default)]
pub(super) struct Channel {
    pub(super) sfx: Option<Arc<Sfx>>,
    pub(super) category: SoundCategory,
    pub(super) entity: i32,
    pub(super) entchannel: i32,
    pub(super) origin: Vec3,
    pub(super) dist_mult: f32,
    pub(super) master_vol: i32,
    pub(super) left: i32,
    pub(super) right: i32,
    /// True if channel is spatialized in 3D space (not local / view entity).
    pub(super) spatialized: bool,
    /// Split-screen guest mono volume (kept separate so HRTF + split-screen combine).
    pub(super) guest_mono: i32,
    pub(super) hrtf_params: hrtf::HrtfParams,
    pub(super) hrtf: Option<Box<hrtf::HrtfState>>,
    /// Target occlusion (0 = clear, 1 = blocked by wall).
    pub(super) occlusion: f32,
    pub(super) occ_smooth: f32,
    pub(super) occ_lp: f32,
    pub(super) pos: usize,
    /// Static/ambient sounds loop from 0 if the sfx has no loop point.
    pub(super) force_loop: bool,
}

/// Audio-thread side. Call [`Mixer::paint`] from the device callback.
pub struct Mixer {
    pub(super) rate: u32,
    queue: Queue,
    pending: Vec<Cmd>,
    pub(super) channels: Vec<Channel>,
    pub(super) total: usize,
    pub(super) ambient_sfx: [Option<Arc<Sfx>>; NUM_AMBIENTS],
    pub(super) ambient_vol: [f32; NUM_AMBIENTS],
    pub(super) listener: Listener,
    pub(super) volume: f32,
    pub(super) bgm_volume: f32,
    pub(super) cat_vols: CategoryVolumes,
    pub(super) water_fx: bool,
    pub(super) water_mix: f32,
    pub(super) water_filter: reverb::UnderwaterFilter,
    pub(super) reverb_params: ReverbParams,
    pub(super) reverb: reverb::Reverb,
    pub(super) reverb_buf: Vec<[f32; 2]>,
    pub(super) hrtf_enabled: bool,
    pub(super) occlusion_enabled: bool,
    pub(super) music: Option<Box<MusicStream>>,
    pub(super) music_fade: Option<(Box<MusicStream>, f32, f32)>,
    pub(super) music_in: (f32, f32),
    pub(super) music_crossfade: f32,
    pub(super) buf: Vec<[i32; 2]>,
    pub(super) rng: u32,
}

impl Mixer {
    pub(super) fn new(
        rate: u32,
        queue: Queue,
        ambient_sfx: [Option<Arc<Sfx>>; NUM_AMBIENTS],
    ) -> Self {
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
            cat_vols: CategoryVolumes::default(),
            water_fx: true,
            water_mix: 0.0,
            water_filter: reverb::UnderwaterFilter::new(rate, 550.0, 0.85),
            reverb_params: ReverbParams::default(),
            reverb: reverb::Reverb::new(rate),
            reverb_buf: Vec::new(),
            hrtf_enabled: false,
            occlusion_enabled: false,
            music: None,
            music_fade: None,
            music_in: (0.0, 0.0),
            music_crossfade: 0.0,
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
            Cmd::CategoryVolumes(v) => self.cat_vols = v,
            Cmd::WaterFx(b) => self.water_fx = b,
            Cmd::Reverb(p) => self.reverb_params = p,
            Cmd::Hrtf(on) => {
                if self.hrtf_enabled != on {
                    self.hrtf_enabled = on;
                    for i in NUM_AMBIENTS..self.total {
                        if self.channels[i].sfx.is_some() {
                            self.spatialize(i);
                        }
                    }
                }
            }
            Cmd::Occlusion(on) => {
                self.occlusion_enabled = on;
                if !on {
                    for ch in &mut self.channels[NUM_AMBIENTS..self.total] {
                        ch.occlusion = 0.0;
                    }
                }
            }
            Cmd::SetOcclusion { entity, channel, value } => {
                for ch in self.channels[NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS]
                    .iter_mut()
                    .rev()
                {
                    if ch.sfx.is_some() && ch.entity == entity && ch.entchannel == channel {
                        ch.occlusion = value;
                        ch.occ_smooth = value;
                        break;
                    }
                }
            }
            Cmd::Occlusions { dynamic, statics } => {
                for (ent, chan, occ) in dynamic {
                    for ch in &mut self.channels[NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS]
                    {
                        if ch.sfx.is_some() && ch.entity == ent && ch.entchannel == chan {
                            ch.occlusion = occ.clamp(0.0, 1.0);
                        }
                    }
                }
                let base = NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS;
                for (idx, occ) in statics {
                    if let Some(ch) = self.channels.get_mut(base + idx)
                        && base + idx < self.total
                        && ch.sfx.is_some()
                    {
                        ch.occlusion = occ.clamp(0.0, 1.0);
                    }
                }
            }
            Cmd::Music(new_music) => {
                let dur = self.music_crossfade;
                if dur > 0.01 {
                    if let Some(old) = self.music.take()
                        && !old.finished
                        && !old.paused
                    {
                        self.music_fade = Some((old, 0.0, dur));
                    }
                    self.music_in = if new_music.is_some() {
                        (0.0, dur)
                    } else {
                        (0.0, 0.0)
                    };
                } else {
                    self.music_fade = None;
                    self.music_in = (0.0, 0.0);
                }
                self.music = new_music;
            }
            Cmd::MusicPause(p) => {
                if let Some(m) = &mut self.music {
                    m.paused = p;
                }
                if let Some((m, _, _)) = &mut self.music_fade {
                    m.paused = p;
                }
            }
            Cmd::MusicLoop(on) => {
                if let Some(m) = &mut self.music {
                    m.set_looping(on);
                }
            }
            Cmd::MusicCrossfade(secs) => self.music_crossfade = secs,
        }
    }

    /// SND_Spatialize.
    pub(super) fn spatialize(&mut self, i: usize) {
        let l = self.listener;
        let rate = self.rate;
        let hrtf_on = self.hrtf_enabled;
        let ch = &mut self.channels[i];
        let others = &l.others[..l.num_others.min(MAX_OTHER_LISTENERS)];
        if ch.entity == l.view_entity
            || ch.dist_mult == 0.0
            || others.iter().any(|o| o.1 == ch.entity)
        {
            ch.spatialized = false;
            ch.guest_mono = 0;
            ch.left = ch.master_vol;
            ch.right = ch.master_vol;
            return;
        }
        ch.spatialized = true;
        let mut v = sub(ch.origin, l.origin);
        let len = dot(v, v).sqrt();
        if len > 0.0 {
            v = v.map(|x| x / len);
        }
        let dist = len * ch.dist_mult;
        let d = dot(l.right, v);
        let (rscale, lscale) = (1.0 + d, 1.0 - d);
        let mv = ch.master_vol as f32;
        let atten = (mv * (1.0 - dist)).max(0.0);
        ch.right = (atten * rscale) as i32;
        ch.left = (atten * lscale) as i32;
        let mut guest_mono = 0;
        for (o, _) in others {
            let d = sub(ch.origin, *o);
            let mono = ((mv * (1.0 - dot(d, d).sqrt() * ch.dist_mult)) as i32).max(0);
            ch.left = ch.left.max(mono);
            ch.right = ch.right.max(mono);
            guest_mono = guest_mono.max(mono);
        }
        ch.guest_mono = guest_mono;
        if hrtf_on {
            ch.hrtf_params = hrtf::compute(v, l.forward, l.right, l.up, atten, rate);
        }
    }

    /// SND_PickChannel.
    pub(super) fn pick_channel(&mut self, entity: i32, entchannel: i32) -> Option<usize> {
        let view = self.listener.view_entity;
        let mut best = None;
        let mut life_left = usize::MAX;
        for (i, ch) in self.channels.iter().enumerate().skip(NUM_AMBIENTS).take(MAX_DYNAMIC_CHANNELS) {
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
pub(super) fn wrap(ch: &mut Channel, sfx: &Sfx) -> bool {
    match sfx.loop_start {
        Some(l) if l < sfx.data.len() => ch.pos = l,
        _ if ch.force_loop && !sfx.data.is_empty() => ch.pos = 0,
        _ => return false,
    }
    true
}

pub(super) fn advance(ch: &mut Channel, sfx: &Sfx, mut n: usize) -> bool {
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
