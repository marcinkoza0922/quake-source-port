//! Cached routes: the usercmd stream of a run that finished its level by game
//! means only. Runs are deterministic, so replaying the stream repeats the run
//! without building a nav graph or planning. Checkpoints (the bot's origin
//! once a second) detect when the engine now behaves differently; the bot then
//! takes over from where it is.
//!
//! File: "QBR1", key u64, then counted sections (u32 count each): moves as
//! (repeat u16, angles 3×f32, forward, side, up f32, buttons u8, impulse u8),
//! checkpoints (frame u32, origin 3×f32), exit activations (frame u32, ent u32).

use qcore::math::Vec3;
use qserver::ClientMove;
use std::path::Path;

const MAGIC: &[u8; 4] = b"QBR1";
/// Bump when the recording changes meaning.
const VERSION: u64 = 1;

#[derive(Default)]
pub struct Route {
    pub key: u64,
    pub moves: Vec<ClientMove>,
    /// Bot origin at a frame.
    pub checkpoints: Vec<(u32, Vec3)>,
    /// Exit trigger made touchable at a frame.
    pub exits: Vec<(u32, u32)>,
}

/// Cache key: map, progs and skill (and the format version).
pub fn key(bsp: &[u8], progs: &[u8], skill: i32) -> u64 {
    use std::hash::Hasher;
    let mut h = crate::nav::Fx::default();
    h.write_u64(VERSION);
    h.write_u64(skill as u64);
    for d in [bsp, progs] {
        h.write_u64(d.len() as u64);
        for c in d.chunks(8) {
            let mut b = [0u8; 8];
            b[..c.len()].copy_from_slice(c);
            h.write_u64(u64::from_le_bytes(b));
        }
    }
    h.finish()
}

fn same(a: &ClientMove, b: &ClientMove) -> bool {
    a.angles.map(f32::to_bits) == b.angles.map(f32::to_bits)
        && a.forward.to_bits() == b.forward.to_bits()
        && a.side.to_bits() == b.side.to_bits()
        && a.up.to_bits() == b.up.to_bits()
        && a.buttons == b.buttons
        && a.impulse == b.impulse
}

impl Route {
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut o = Vec::new();
        o.extend_from_slice(MAGIC);
        o.extend_from_slice(&self.key.to_le_bytes());
        let mut runs: Vec<(u16, &ClientMove)> = Vec::new();
        for m in &self.moves {
            match runs.last_mut() {
                Some((n, last)) if *n < u16::MAX && same(last, m) => *n += 1,
                _ => runs.push((1, m)),
            }
        }
        o.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (n, m) in runs {
            o.extend_from_slice(&n.to_le_bytes());
            for f in [m.angles[0], m.angles[1], m.angles[2], m.forward, m.side, m.up] {
                o.extend_from_slice(&f.to_le_bytes());
            }
            o.push(m.buttons);
            o.push(m.impulse);
        }
        o.extend_from_slice(&(self.checkpoints.len() as u32).to_le_bytes());
        for (f, p) in &self.checkpoints {
            o.extend_from_slice(&f.to_le_bytes());
            for x in p.to_array() {
                o.extend_from_slice(&x.to_le_bytes());
            }
        }
        o.extend_from_slice(&(self.exits.len() as u32).to_le_bytes());
        for (f, e) in &self.exits {
            o.extend_from_slice(&f.to_le_bytes());
            o.extend_from_slice(&e.to_le_bytes());
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, o)
    }

    /// The route at `path` if it exists, parses and matches `key`.
    pub fn load(path: &Path, key: u64) -> Option<Route> {
        let d = std::fs::read(path).ok()?;
        let mut r = Reader { d: &d, i: 0 };
        if r.take(4)? != MAGIC || r.u64()? != key {
            return None;
        }
        let mut route = Route { key, ..Default::default() };
        for _ in 0..r.u32()? {
            let n = r.u16()?;
            let mut f = [0f32; 6];
            for x in &mut f {
                *x = r.f32()?;
            }
            let (buttons, impulse) = (r.u8()?, r.u8()?);
            let m = ClientMove { angles: [f[0], f[1], f[2]], forward: f[3], side: f[4], up: f[5], buttons, impulse };
            route.moves.extend(std::iter::repeat_n(m, n as usize));
        }
        for _ in 0..r.u32()? {
            let f = r.u32()?;
            route.checkpoints.push((f, Vec3::new(r.f32()?, r.f32()?, r.f32()?)));
        }
        for _ in 0..r.u32()? {
            route.exits.push((r.u32()?, r.u32()?));
        }
        Some(route)
    }
}

struct Reader<'a> {
    d: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.d.get(self.i..self.i + n)?;
        self.i += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let m = |f: f32| ClientMove { angles: [1.0, f, 0.0], forward: 320.0, side: -0.5, up: 0.0, buttons: 3, impulse: 9 };
        let r = Route { key: 42, moves: vec![m(1.0), m(1.0), m(2.0)], checkpoints: vec![(72, Vec3::new(1.0, 2.0, 3.0))], exits: vec![(100, 7)] };
        let dir = std::env::temp_dir().join(format!("qbot-route-{}", std::process::id()));
        let p = dir.join("x.qbr");
        r.save(&p).unwrap();
        let l = Route::load(&p, 42).unwrap();
        assert!(Route::load(&p, 43).is_none());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(l.moves.len(), 3);
        assert!(same(&l.moves[1], &m(1.0)) && same(&l.moves[2], &m(2.0)));
        assert_eq!(l.checkpoints, r.checkpoints);
        assert_eq!(l.exits, r.exits);
    }
}
