use std::f32::consts::PI;

use super::*;

impl Mixer {
    /// Mix into interleaved stereo `out` (len = 2 × frames).
    pub fn paint(&mut self, out: &mut [i16]) {
        self.process_commands();
        let frames = out.len() / 2;
        self.buf.clear();
        self.buf.resize(frames, [0; 2]);

        let reverb_active = self.reverb.is_active(self.reverb_params);
        if reverb_active {
            self.reverb_buf.clear();
            self.reverb_buf.resize(frames, [0.0; 2]);
        }

        let rate_f = self.rate.max(8000) as f32;
        let occ_slew = (frames as f32 / (rate_f * 0.08)).clamp(0.02, 1.0);
        let hrtf_on = self.hrtf_enabled;
        let occ_on = self.occlusion_enabled;
        let cat_vols = self.cat_vols;

        for ch in &mut self.channels[..self.total] {
            let Some(sfx) = ch.sfx.clone() else { continue };
            let cat_gain = cat_vols.gain(ch.category);
            if (ch.left == 0 && ch.right == 0) || cat_gain <= 0.0001 {
                // Silent: just advance (S_PaintChannels still advances position).
                if !advance(ch, &sfx, frames) {
                    ch.sfx = None;
                }
                continue;
            }

            // Smooth per-channel occlusion.
            let target_occ = if occ_on && ch.spatialized { ch.occlusion } else { 0.0 };
            ch.occ_smooth += (target_occ - ch.occ_smooth) * occ_slew;
            if ch.occ_smooth < 0.001 && target_occ == 0.0 {
                ch.occ_smooth = 0.0;
            }
            let use_occ = ch.occ_smooth > 0.001;
            let occ_gain = 1.0 - ch.occ_smooth * 0.56;
            let occ_alpha = if use_occ {
                let cutoff = 14000.0 * (650.0 / 14000.0_f32).powf(ch.occ_smooth);
                (1.0 - (-2.0 * PI * cutoff / rate_f).exp()).clamp(0.02, 1.0)
            } else {
                1.0
            };

            let use_hrtf = hrtf_on && ch.spatialized;
            if use_hrtf && ch.hrtf.is_none() {
                ch.hrtf = Some(Box::new(hrtf::HrtfState::default()));
            }
            let send_reverb = reverb_active && ch.category != SoundCategory::Ui;

            let mut o = 0;
            while o < frames {
                if ch.pos >= sfx.data.len() && !wrap(ch, &sfx) {
                    ch.sfx = None;
                    break;
                }
                let n = (sfx.data.len() - ch.pos).min(frames - o);
                if !use_hrtf && !use_occ && !send_reverb && (cat_gain - 1.0).abs() < 0.0001 {
                    // Classic zero-overhead integer path.
                    for (b, &s) in self.buf[o..o + n]
                        .iter_mut()
                        .zip(&sfx.data[ch.pos..ch.pos + n])
                    {
                        let s = i32::from(s);
                        b[0] += (s * ch.left) >> 8;
                        b[1] += (s * ch.right) >> 8;
                    }
                } else if use_hrtf {
                    let hrtf = ch.hrtf.as_mut().unwrap();
                    let params = ch.hrtf_params;
                    let guest = (ch.guest_mono as f32 / 256.0) * cat_gain;
                    let scale = cat_gain * occ_gain;
                    let rev_scale = if send_reverb {
                        // Occluded sounds keep slightly more diffuse reverb energy.
                        0.85 + ch.occ_smooth * 0.25
                    } else {
                        0.0
                    };
                    for idx in 0..n {
                        let raw = f32::from(sfx.data[ch.pos + idx]);
                        let s = if use_occ {
                            ch.occ_lp += occ_alpha * (raw - ch.occ_lp);
                            ch.occ_lp
                        } else {
                            ch.occ_lp = raw;
                            raw
                        };
                        let [hl, hr] = hrtf.process_sample(s, &params);
                        let mut fl = hl * scale;
                        let mut fr = hr * scale;
                        if guest > 0.0 {
                            let gm = raw * guest;
                            fl = if fl.abs() >= gm.abs() { fl } else { gm };
                            fr = if fr.abs() >= gm.abs() { fr } else { gm };
                        }
                        self.buf[o + idx][0] += fl as i32;
                        self.buf[o + idx][1] += fr as i32;
                        if send_reverb {
                            self.reverb_buf[o + idx][0] += fl * rev_scale;
                            self.reverb_buf[o + idx][1] += fr * rev_scale;
                        }
                    }
                } else {
                    let gl = (ch.left as f32 / 256.0) * cat_gain * occ_gain;
                    let gr = (ch.right as f32 / 256.0) * cat_gain * occ_gain;
                    let rev_scale = if ch.spatialized { 1.0 } else { 0.45 };
                    for idx in 0..n {
                        let raw = f32::from(sfx.data[ch.pos + idx]);
                        let s = if use_occ {
                            ch.occ_lp += occ_alpha * (raw - ch.occ_lp);
                            ch.occ_lp
                        } else {
                            ch.occ_lp = raw;
                            raw
                        };
                        let fl = s * gl;
                        let fr = s * gr;
                        self.buf[o + idx][0] += fl as i32;
                        self.buf[o + idx][1] += fr as i32;
                        if send_reverb {
                            self.reverb_buf[o + idx][0] += fl * rev_scale;
                            self.reverb_buf[o + idx][1] += fr * rev_scale;
                        }
                    }
                }
                ch.pos += n;
                o += n;
            }
        }

        if reverb_active {
            self.reverb.process_add(&self.reverb_buf, &mut self.buf, self.reverb_params);
        }

        let vol = (self.volume * 256.0) as i32;
        for b in &mut self.buf {
            b[0] = (b[0] * vol) >> 8;
            b[1] = (b[1] * vol) >> 8;
        }

        self.mix_music(frames);

        // Underwater 2-pole Butterworth lowpass (snd_waterfx): smooth 0.25 s fade in/out.
        let target = if self.water_fx && self.listener.underwater {
            1.0
        } else {
            0.0
        };
        let fade = frames as f32 / (rate_f * 0.25);
        self.water_mix += (target - self.water_mix).clamp(-fade, fade);
        let wet = self.water_mix;
        for (b, o) in self.buf.iter().zip(out.as_chunks_mut::<2>().0) {
            for c in 0..2 {
                let x = b[c] as f32;
                let lp = self.water_filter.process_sample(c, x);
                let y = if wet > 0.0 {
                    // Gently attenuate overall submerged level slightly while replacing highs with muffled low-end.
                    let submerged = lp * 0.86;
                    x + (submerged - x) * wet
                } else {
                    x
                };
                o[c] = y.clamp(-32768.0, 32767.0) as i16;
            }
        }
    }

    /// Mix current music stream and any outgoing crossfade stream in smooth sub-chunks.
    fn mix_music(&mut self, frames: usize) {
        if self.music.is_none() && self.music_fade.is_none() {
            return;
        }
        let rate_f = self.rate.max(8000) as f32;
        let fading = self.music_fade.is_some() || self.music_in.1 > 0.0;
        let chunk = if fading { 256 } else { frames.max(1) };
        let mut o = 0;
        while o < frames {
            let n = (frames - o).min(chunk);
            let dt = n as f32 / rate_f;
            if let Some(m) = &mut self.music {
                let gain = if self.music_in.1 > 0.0 {
                    self.music_in.0 += dt;
                    let t = (self.music_in.0 / self.music_in.1).clamp(0.0, 1.0);
                    if t >= 1.0 {
                        self.music_in = (0.0, 0.0);
                    }
                    t
                } else {
                    1.0
                };
                m.mix(&mut self.buf[o..o + n], self.bgm_volume * gain);
            }
            if let Some((old, elapsed, dur)) = &mut self.music_fade {
                *elapsed += dt;
                let t = (*elapsed / (*dur).max(0.01)).clamp(0.0, 1.0);
                let gain = 1.0 - t;
                if gain > 0.001 && !old.finished {
                    old.mix(&mut self.buf[o..o + n], self.bgm_volume * gain);
                }
                if t >= 1.0 || old.finished {
                    self.music_fade = None;
                }
            }
            o += n;
        }
    }
}
