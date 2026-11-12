use super::*;

impl Mixer {
    /// Mix into interleaved stereo `out` (len = 2 × frames).
    pub fn paint(&mut self, out: &mut [i16]) {
        self.process_commands();
        let frames = out.len() / 2;
        self.buf.clear();
        self.buf.resize(frames, [0; 2]);
        for ch in &mut self.channels[..self.total] {
            let Some(sfx) = ch.sfx.clone() else { continue };
            if ch.left == 0 && ch.right == 0 {
                // Silent: just advance (S_PaintChannels still advances position).
                if !advance(ch, &sfx, frames) {
                    ch.sfx = None;
                }
                continue;
            }
            let mut o = 0;
            while o < frames {
                if ch.pos >= sfx.data.len() && !wrap(ch, &sfx) {
                    ch.sfx = None;
                    break;
                }
                let n = (sfx.data.len() - ch.pos).min(frames - o);
                for (b, &s) in self.buf[o..o + n]
                    .iter_mut()
                    .zip(&sfx.data[ch.pos..ch.pos + n])
                {
                    let s = i32::from(s);
                    b[0] += (s * ch.left) >> 8;
                    b[1] += (s * ch.right) >> 8;
                }
                ch.pos += n;
                o += n;
            }
        }
        let vol = (self.volume * 256.0) as i32;
        for b in &mut self.buf {
            b[0] = (b[0] * vol) >> 8;
            b[1] = (b[1] * vol) >> 8;
        }
        if let Some(m) = &mut self.music {
            m.mix(&mut self.buf, self.bgm_volume);
        }
        // Underwater lowpass (snd_waterfx): fade in/out, one-pole filter.
        let target = if self.water_fx && self.listener.underwater {
            1.0
        } else {
            0.0
        };
        let fade = frames as f32 / (self.rate as f32 * 0.25);
        self.water_mix += (target - self.water_mix).clamp(-fade, fade);
        let wet = self.water_mix;
        let alpha = 1.0 - (-2.0 * std::f32::consts::PI * 1000.0 / self.rate as f32).exp();
        for (b, o) in self.buf.iter().zip(out.as_chunks_mut::<2>().0) {
            for c in 0..2 {
                let x = b[c] as f32;
                self.lp_state[c] += alpha * (x - self.lp_state[c]);
                let y = if wet > 0.0 {
                    x + (self.lp_state[c] - x) * wet
                } else {
                    x
                };
                o[c] = y.clamp(-32768.0, 32767.0) as i16;
            }
        }
    }
}
