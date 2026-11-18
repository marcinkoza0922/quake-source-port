//! Reading and patching `.dem`/`.qsd` files for parity stills.
//!
//! A demo is a text line (the CD track) and then blocks of `i32 length,
//! 3 × f32 view angles, length bytes of server messages`. The stills put an
//! `svc_stufftext "screenshot\n"` at the end of chosen blocks, and every
//! engine runs it while playing the demo: under `timedemo` each block is
//! drawn exactly once, so all three engines shoot the same demo frame.

/// `svc_stufftext`, the same number in every protocol.
const SVC_STUFFTEXT: u8 = 9;
/// ironwail's MAX_MSGLEN: a longer block is fatal there.
const MAX_BLOCK: usize = 64000;

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub angles: [f32; 3],
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Demo {
    /// The CD track line, `\n` included.
    pub header: Vec<u8>,
    pub blocks: Vec<Block>,
}

impl Demo {
    pub fn parse(bytes: &[u8]) -> Result<Demo, String> {
        let nl = bytes.iter().take(16).position(|&b| b == b'\n').ok_or("no CD track line")?;
        let header = bytes[..=nl].to_vec();
        let mut blocks = Vec::new();
        let mut pos = nl + 1;
        while pos < bytes.len() {
            let rest = &bytes[pos..];
            if rest.len() < 16 {
                return Err(format!("truncated block header at byte {pos}"));
            }
            let word = |i: usize| <[u8; 4]>::try_from(&rest[i * 4..i * 4 + 4]).unwrap();
            let len = i32::from_le_bytes(word(0));
            if len < 0 || rest.len() < 16 + len as usize {
                return Err(format!("bad block length {len} at byte {pos}"));
            }
            let angles = [1, 2, 3].map(|i| f32::from_le_bytes(word(i)));
            blocks.push(Block { angles, data: rest[16..16 + len as usize].to_vec() });
            pos += 16 + len as usize;
        }
        Ok(Demo { header, blocks })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.header.clone();
        for b in &self.blocks {
            v.extend((b.data.len() as i32).to_le_bytes());
            for a in b.angles {
                v.extend(a.to_le_bytes());
            }
            v.extend(&b.data);
        }
        v
    }

    /// Appends `svc_stufftext <text>` to block `i`.
    pub fn stuff(&mut self, i: usize, text: &str) -> Result<(), String> {
        let b = self.blocks.get_mut(i).ok_or_else(|| format!("no block {i}"))?;
        if b.data.len() + text.len() + 2 > MAX_BLOCK {
            return Err(format!("block {i} would exceed {MAX_BLOCK} bytes"));
        }
        b.data.push(SVC_STUFFTEXT);
        b.data.extend(text.as_bytes());
        b.data.push(0);
        Ok(())
    }

    /// For each view (in recording order) given as `(separator, view)`
    /// angles, the block to shoot: three quarters into the first run of at
    /// least `min_run` blocks with the view's angles that directly follows a
    /// run with its separator's angles, after the previous view. The
    /// separator tells a view from the spawn or an earlier view that shares
    /// its angles; three quarters in, the camera has settled and the view
    /// stays on screen a while longer, so a frame early or late in some
    /// engine still shows it.
    pub fn shot_blocks(&self, views: &[([f32; 3], [f32; 3])], min_run: usize) -> Result<Vec<usize>, String> {
        // Runs of equal angles: (angles, first block, length).
        let mut runs: Vec<([f32; 3], usize, usize)> = Vec::new();
        for (i, b) in self.blocks.iter().enumerate() {
            match runs.last_mut() {
                Some(r) if same_angles(r.0, b.angles) => r.2 += 1,
                _ => runs.push((b.angles, i, 1)),
            }
        }
        let mut shots = Vec::new();
        let mut from = 0;
        for (n, (sep, view)) in views.iter().enumerate() {
            let j = (from..runs.len().saturating_sub(1))
                .find(|&j| same_angles(runs[j].0, *sep) && same_angles(runs[j + 1].0, *view) && runs[j + 1].2 >= min_run)
                .ok_or_else(|| format!("view {n}: no run of {min_run} blocks with angles {view:?} after {sep:?}"))?;
            let (_, start, len) = runs[j + 1];
            shots.push(start + len * 3 / 4);
            from = j + 2;
        }
        Ok(shots)
    }
}

/// Angles equal within 0.05°, yaw and roll modulo 360.
fn same_angles(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| {
        let d = (a[i] - b[i]).rem_euclid(360.0);
        d.min(360.0 - d) < 0.05
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo(angles: &[[f32; 3]]) -> Demo {
        Demo { header: b"-1\n".to_vec(), blocks: angles.iter().map(|&a| Block { angles: a, data: vec![1] }).collect() }
    }

    #[test]
    fn round_trips_and_stuffs_text() {
        let mut d = demo(&[[0.0; 3], [0.0, 90.0, 0.0]]);
        let bytes = d.to_bytes();
        assert_eq!(Demo::parse(&bytes).unwrap(), d);
        d.stuff(1, "screenshot\n").unwrap();
        let back = Demo::parse(&d.to_bytes()).unwrap();
        assert_eq!(back.blocks[1].data, b"\x01\x09screenshot\n\x00");
        assert!(Demo::parse(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn shots_land_inside_each_settled_view() {
        let a = [0.0, 90.0, 0.0];
        let sa = [0.0, 135.0, 0.0];
        let b = [0.0, -90.0, 0.0];
        let sb = [0.0, -45.0, 0.0];
        // A long spawn hold with a's angles, then separator+a, separator+b
        // (b written as 270 == -90 modulo 360), and a again.
        let mut seq = vec![a; 10];
        seq.extend([sa; 2]);
        seq.extend([a; 8]); // 12..20
        seq.extend([[0.0, 315.0, 0.0]; 2]);
        seq.extend([[0.0, 270.0, 0.0]; 8]); // 22..30
        seq.extend([sa; 2]);
        seq.extend([a; 8]); // 32..40
        let d = demo(&seq);
        assert_eq!(d.shot_blocks(&[(sa, a), (sb, b), (sa, a)], 4).unwrap(), vec![12 + 6, 22 + 6, 32 + 6]);
        assert!(d.shot_blocks(&[(sa, a), (sa, a), (sa, a), (sa, a)], 4).is_err());
        assert!(d.shot_blocks(&[(sa, a)], 9).is_err());
    }
}
