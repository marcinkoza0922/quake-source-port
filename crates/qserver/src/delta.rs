//! FTE replacement deltas, server side (vkQuake sv_main.c SVFTE_*): per
//! client entity snapshots, delta bits pending per entity, and the ring of
//! sent frames that acks (clcdp_ackframe) resolve.
//!
//! Each datagram carries one `svcfte_updateentities` and is logged under its
//! sequence number. An ack retires a frame; a gap in the acks marks the
//! frames in between as dropped, and their bits become pending again. The
//! local client is lossless and acks every frame as it is written.

use qnet::delta::{
    EntState, SVCFTE_UPDATEENTITIES, UF_16BIT, UF_EXTEND1, UF_RESET, calc_bits, write_record_num,
    write_update,
};

/// Server-only flags reusing bits that are never part of the delta itself.
/// The entity left the snapshot (or, on entity 0, "resend everything").
const UF_REMOVE: u32 = UF_16BIT;
/// A new entity's second reset, sent with the following frame against
/// packet loss but not tracked for resends.
const UF_RESET2: u32 = UF_EXTEND1;
/// Must be a power of two.
const NUM_FRAMES: usize = 64;
/// Bytes kept free at the end of a datagram for the terminating short.
const TAIL: usize = 2;

#[derive(Debug, Clone, Default)]
struct Frame {
    sequence: i32,
    /// (entity, bits logged for resend).
    ents: Vec<(u32, u32)>,
}

/// Delta state of one client (client_t's frames / pendingentities_bits /
/// previousentities).
#[derive(Debug, Clone)]
pub struct DeltaState {
    frames: Vec<Frame>,
    pending: Vec<u32>,
    /// Last snapshot, sorted by entity number.
    previous: Vec<(u32, EntState)>,
    last_ack: i32,
    /// Entity to continue from when a snapshot spans several datagrams.
    resume: usize,
    /// Sequence the next datagram is sent with.
    pub next_sequence: i32,
}

impl Default for DeltaState {
    /// SVFTE_SetupFrames: everything unsent, the client told to reset.
    fn default() -> Self {
        let mut pending = vec![0; 1];
        pending[0] = UF_REMOVE;
        Self {
            frames: vec![Frame { sequence: i32::MIN, ents: Vec::new() }; NUM_FRAMES],
            pending,
            previous: Vec::new(),
            last_ack: i32::MIN,
            resume: 0,
            next_sequence: 0,
        }
    }
}

impl DeltaState {
    fn frame_mut(&mut self, seq: i32) -> &mut Frame {
        &mut self.frames[seq as usize & (NUM_FRAMES - 1)]
    }

    /// SVFTE_DroppedFrame: flag a lost frame's entities for resending.
    fn dropped(&mut self, seq: i32) {
        let f = self.frame_mut(seq);
        if f.sequence != seq {
            return;
        }
        f.sequence = -1;
        let ents = std::mem::take(&mut f.ents);
        for &(num, bits) in &ents {
            if bits != 0
                && let Some(p) = self.pending.get_mut(num as usize)
            {
                *p |= bits;
            }
        }
        self.frame_mut(seq).ents = ents;
    }

    /// SVFTE_Ack. `-1` asks for a full resend (e.g. a demo started
    /// recording). Gaps before `seq` count as dropped.
    pub fn ack(&mut self, seq: i32) {
        if seq == -1 {
            self.pending[0] |= UF_REMOVE;
        }
        if seq < self.last_ack {
            return;
        }
        let mut drop = self.last_ack.wrapping_add(1);
        if self.last_ack == i32::MIN || (seq.wrapping_sub(drop) as u32) >= NUM_FRAMES as u32 {
            drop = seq.wrapping_sub(NUM_FRAMES as i32);
        }
        while drop < seq {
            self.dropped(drop);
            drop += 1;
        }
        self.last_ack = seq;
        let f = self.frame_mut(seq);
        if f.sequence == seq {
            f.sequence = -1;
        }
    }

    /// SVFTE_CalcEntityDeltas against the new snapshot (sorted by entity).
    pub fn calc(&mut self, snapshot: Vec<(u32, EntState)>, num_edicts: usize) {
        if self.pending.len() < num_edicts {
            self.pending.resize(num_edicts + 64, 0);
        }
        if self.pending[0] & UF_REMOVE != 0 {
            self.previous.clear();
            self.pending[0] = UF_REMOVE;
        }
        let (mut o, mut n) = (0, 0);
        let (olds, news) = (&self.previous, &snapshot);
        while o < olds.len() || n < news.len() {
            if n == news.len() || (o < olds.len() && olds[o].0 < news[n].0) {
                // Left the snapshot.
                self.pending[olds[o].0 as usize] = UF_REMOVE;
                o += 1;
            } else if o == olds.len() || news[n].0 < olds[o].0 {
                // New this frame: a full reset is enough.
                self.pending[news[n].0 as usize] = UF_RESET;
                n += 1;
            } else {
                let p = &mut self.pending[news[n].0 as usize];
                // Still flagged for removal after packet loss: reset instead.
                if *p & UF_REMOVE != 0 {
                    *p = (*p & !UF_REMOVE) | UF_RESET2;
                }
                *p |= calc_bits(&olds[o].1, &news[n].1);
                o += 1;
                n += 1;
            }
        }
        self.previous = snapshot;
        self.resume = 0;
    }

    /// True while a snapshot still has entities left to write.
    pub fn more(&self) -> bool {
        self.resume < self.pending.len() && self.pending[self.resume..].iter().any(|&b| b & !UF_RESET2 != 0)
    }

    /// SVFTE_WriteEntitiesToClient: one svcfte_updateentities into `msg`
    /// (up to `maxsize` bytes), logged under `next_sequence`, which then
    /// advances. `baseline(e)` is what the client resets entity `e` to.
    pub fn write(
        &mut self,
        msg: &mut qnet::SizeBuf,
        maxsize: usize,
        time: f32,
        pf: u32,
        baseline: &dyn Fn(u32) -> EntState,
    ) {
        let seq = self.next_sequence;
        self.next_sequence = seq.wrapping_add(1);
        // A frame logged under the same sequence and never acked was not
        // delivered (stale slot in the ring): resend its contents.
        if self.frame_mut(seq).sequence == seq.wrapping_sub(NUM_FRAMES as i32) {
            self.dropped(seq.wrapping_sub(NUM_FRAMES as i32));
        }
        let mut ents = std::mem::take(&mut self.frame_mut(seq).ents);
        ents.clear();
        // Records are written past the soft limit and rolled back (the
        // buffer would otherwise clear itself on overflow).
        let hard = std::mem::replace(&mut msg.maxsize, usize::MAX);
        msg.byte(SVCFTE_UPDATEENTITIES as i32);
        msg.float(time);
        let mut state = 0;
        let mut num = self.resume;
        while num < self.pending.len() {
            let bits = self.pending[num];
            if bits & !UF_RESET2 == 0 {
                num += 1;
                continue;
            }
            let rollback = msg.len();
            self.pending[num] = 0;
            let mut log = 0;
            if bits & UF_REMOVE != 0 {
                write_record_num(msg, num as u32, true);
                log = UF_REMOVE;
            } else {
                while state < self.previous.len() && (self.previous[state].0 as usize) < num {
                    state += 1;
                }
                if let Some(&(_, s)) = self.previous.get(state).filter(|x| x.0 as usize == num) {
                    let net = if bits & UF_RESET2 != 0 {
                        log = bits & !(UF_RESET | UF_RESET2);
                        UF_RESET | calc_bits(&baseline(num as u32), &s)
                    } else if bits & UF_RESET != 0 {
                        // Reset again next frame.
                        self.pending[num] = UF_RESET2;
                        log = UF_RESET;
                        UF_RESET | calc_bits(&baseline(num as u32), &s)
                    } else {
                        log = bits;
                        bits
                    };
                    write_record_num(msg, num as u32, false);
                    write_update(net, &s, msg, pf);
                }
            }
            if msg.len() + TAIL > maxsize {
                msg.data.truncate(rollback);
                self.pending[num] = bits;
                break;
            }
            ents.push((num as u32, log));
            num += 1;
        }
        msg.short(0);
        msg.maxsize = hard;
        self.resume = num;
        let f = self.frame_mut(seq);
        f.sequence = seq;
        f.ents = ents;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnet::MsgReader;
    use qnet::delta::{PEXT2_REPLACEMENTDELTAS, read_delta, read_record_num};

    /// Minimal client: applies updateentities to a state table.
    #[derive(Default)]
    struct Cl {
        ents: std::collections::BTreeMap<u32, EntState>,
    }

    impl Cl {
        fn parse(&mut self, data: &[u8]) {
            let mut m = MsgReader::new(data);
            assert_eq!(m.byte() as u8, SVCFTE_UPDATEENTITIES);
            m.float();
            while let Some((num, remove)) = read_record_num(&mut m) {
                if remove {
                    if num == 0 {
                        self.ents.clear();
                    } else {
                        self.ents.remove(&num);
                    }
                    continue;
                }
                let mut s = self.ents.get(&num).copied().unwrap_or_default();
                read_delta(&mut m, &mut s, &EntState::default(), 0, PEXT2_REPLACEMENTDELTAS).unwrap();
                self.ents.insert(num, s);
            }
            assert!(m.at_end() && !m.badread);
        }
    }

    fn st(x: f32, trail: u16) -> EntState {
        EntState { origin: [x, 0.0, 0.0], modelindex: 2, traileffect: trail, ..Default::default() }
    }

    fn frame(d: &mut DeltaState, snap: &[(u32, EntState)], maxsize: usize) -> Vec<Vec<u8>> {
        d.calc(snap.to_vec(), 16);
        let mut out = Vec::new();
        loop {
            let mut b = qnet::SizeBuf::new(maxsize);
            d.write(&mut b, maxsize, 0.0, 0, &|_| EntState::default());
            out.push(b.data);
            if !d.more() {
                return out;
            }
        }
    }

    #[test]
    fn deltas_survive_loss() {
        let mut d = DeltaState::default();
        let mut cl = Cl::default();
        // Frame 0 delivered.
        let snap = [(1, st(1.0, 0)), (3, st(3.0, 5))];
        for m in frame(&mut d, &snap, 1400) {
            cl.parse(&m);
        }
        d.ack(0);
        assert_eq!(cl.ents.get(&3), Some(&snap[1].1));
        // Frame 1 lost: entity 1 moves, 3 changes trail.
        let snap = [(1, st(2.0, 0)), (3, st(3.0, 7))];
        let _lost = frame(&mut d, &snap, 1400);
        // Frame 2 delivered, entity 1 removed; ack 2 implies 1 was dropped.
        let snap = [(3, st(3.0, 7))];
        for m in frame(&mut d, &snap, 1400) {
            cl.parse(&m);
        }
        d.ack(2);
        // Frame 3 carries the resend.
        for m in frame(&mut d, &snap, 1400) {
            cl.parse(&m);
        }
        assert_eq!(cl.ents.keys().copied().collect::<Vec<_>>(), vec![3]);
        assert_eq!(cl.ents[&3].traileffect, 7);
    }

    #[test]
    fn snapshot_split_across_datagrams() {
        let mut d = DeltaState::default();
        let mut cl = Cl::default();
        let snap: Vec<_> = (1..12).map(|i| (i, st(i as f32 * 10.0, i as u16))).collect();
        let msgs = frame(&mut d, &snap, 40);
        assert!(msgs.len() > 2);
        for m in &msgs {
            cl.parse(m);
        }
        assert_eq!(cl.ents.len(), 11);
        assert_eq!(cl.ents[&11], snap[10].1);
    }
}
