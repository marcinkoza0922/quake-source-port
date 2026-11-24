use super::*;

impl Mixer {
    pub(super) fn event(&mut self, e: SoundEvent) {
        match e {
            SoundEvent::Start {
                entity,
                channel,
                sfx,
                origin,
                volume,
                attenuation,
            } => {
                let Some(i) = self.pick_channel(entity, channel) else {
                    return;
                };
                let category = classify_sound(&sfx.name);
                let ch = &mut self.channels[i];
                *ch = Channel {
                    category,
                    entity,
                    entchannel: channel,
                    origin,
                    dist_mult: attenuation / SOUND_NOMINAL_CLIP_DIST,
                    master_vol: (volume * 255.0) as i32,
                    ..Default::default()
                };
                self.spatialize(i);
                let ch = &self.channels[i];
                if ch.left == 0 && ch.right == 0 {
                    return; // not audible at all
                }
                // Skew sounds started in the same frame with the same sfx (S_StartSound).
                let mut skew = 0;
                for (j, c) in self.channels[NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS]
                    .iter()
                    .enumerate()
                {
                    if j + NUM_AMBIENTS != i
                        && c.pos == 0
                        && c.sfx.as_ref().is_some_and(|s| Arc::ptr_eq(s, &sfx))
                    {
                        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
                        let r = (self.rng >> 16) as usize % (self.rate as usize / 10).max(1);
                        skew = r.min(sfx.data.len());
                        break;
                    }
                }
                let ch = &mut self.channels[i];
                ch.pos = skew;
                ch.sfx = Some(sfx);
            }
            SoundEvent::Local(sfx) => {
                let view = self.listener.view_entity;
                let Some(i) = self.pick_channel(view, -1) else {
                    return;
                };
                let category = match classify_sound(&sfx.name) {
                    SoundCategory::Ui => SoundCategory::Ui,
                    // Local sounds from menus/feedback stay dry UI; others keep their mix bucket.
                    _ => SoundCategory::Ui,
                };
                let ch = &mut self.channels[i];
                *ch = Channel {
                    sfx: Some(sfx),
                    category,
                    entity: view,
                    entchannel: -1,
                    origin: self.listener.origin,
                    dist_mult: 0.0,
                    master_vol: 255,
                    left: 255,
                    right: 255,
                    ..Default::default()
                };
            }
            SoundEvent::Stop { entity, channel } => {
                for ch in &mut self.channels[NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS] {
                    if ch.entity == entity && ch.entchannel == channel {
                        ch.sfx = None;
                    }
                }
            }
            SoundEvent::Static {
                sfx,
                origin,
                volume,
                attenuation,
            } => {
                if self.total >= MAX_CHANNELS {
                    return;
                }
                let i = self.total;
                self.total += 1;
                self.channels[i] = Channel {
                    sfx: Some(sfx),
                    category: SoundCategory::Ambient,
                    entity: -1,
                    origin,
                    dist_mult: attenuation / SOUND_NOMINAL_CLIP_DIST,
                    master_vol: (volume * 255.0) as i32,
                    force_loop: true,
                    ..Default::default()
                };
                self.spatialize(i);
            }
            SoundEvent::StopAll => {
                for ch in &mut self.channels {
                    *ch = Channel::default();
                }
                self.total = NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS;
                self.ambient_vol = [0.0; NUM_AMBIENTS];
                self.reverb.clear();
            }
            SoundEvent::StopDynamic => {
                for ch in &mut self.channels[NUM_AMBIENTS..NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS] {
                    *ch = Channel::default();
                }
            }
            SoundEvent::ClearStatic => {
                for ch in &mut self.channels[NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS..] {
                    *ch = Channel::default();
                }
                self.total = NUM_AMBIENTS + MAX_DYNAMIC_CHANNELS;
            }
        }
    }

    /// S_UpdateAmbientSounds.
    pub(super) fn update_ambients(&mut self) {
        let l = self.listener;
        for a in 0..NUM_AMBIENTS {
            let ch = &mut self.channels[a];
            ch.category = SoundCategory::Ambient;
            let Some(levels) = l.ambient_levels.filter(|_| l.ambient_level != 0.0) else {
                ch.sfx = None;
                self.ambient_vol[a] = 0.0;
                continue;
            };
            ch.sfx.clone_from(&self.ambient_sfx[a]);
            ch.force_loop = true;
            let mut target = (l.ambient_level * f32::from(levels[a])).trunc().min(255.0);
            if target < 8.0 {
                target = 0.0;
            }
            let v = &mut self.ambient_vol[a];
            let step = l.frametime * l.ambient_fade;
            if *v < target {
                *v = (*v + step).min(target);
            } else if *v > target {
                *v = (*v - step).max(target);
            }
            ch.master_vol = *v as i32;
            ch.left = ch.master_vol;
            ch.right = ch.master_vol;
        }
    }
}
