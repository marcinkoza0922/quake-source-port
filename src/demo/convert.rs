//! Demo conversion for other ports. Our local client records with FTE
//! replacement deltas (`svcfte_updateentities`), and single-player mods may
//! add DP particle messages; ironwail can play neither. [`to_plain`] rewrites
//! a demo into plain protocol 15/666/999 messages, the way our server talks
//! to a vanilla client:
//!
//! - svc_serverinfo loses its `FTE2 <pext2>` prefix;
//! - each svcfte_updateentities becomes svc_time (first in the block) plus a
//!   U_* fast update, against the baseline, of every entity in the snapshot;
//! - svcdp_precache / trail / point particles and DP rain/snow temp
//!   entities are dropped, and per-entity trail/emit effects are lost (both
//!   counted in [`Lost`]);
//! - everything else is copied byte for byte.
//!
//! Message boundaries and entity states come from replaying the demo
//! through our own client parser (`parse_server_message_spans`).

use qclient::ClientState;
use qclient::delta::{EntState, PEXT2_PREDINFO, PEXT2_REPLACEMENTDELTAS, PROTOCOL_FTE_PEXT1, PROTOCOL_FTE_PEXT2, SVCFTE_UPDATEENTITIES};
use qnet::msg::SizeBuf;
use qnet::protocol::*;

use super::Demo;

// DP extension messages (qclient::fte).
const SVCDP_PRECACHE: u8 = 54;
const SVCDP_TRAILPARTICLES: u8 = 60;
const SVCDP_POINTPARTICLES: u8 = 61;
const SVCDP_POINTPARTICLES1: u8 = 62;
const TEDP_PARTICLERAIN: u8 = 55;
const TEDP_PARTICLESNOW: u8 = 56;
const SVCDP_UPDATESTATBYTE: u8 = 51;
const SVCFTE_UPDATESTATSTRING: u8 = 78;
const SVCFTE_UPDATESTATFLOAT: u8 = 79;
const SVCFTE_SPAWNSTATIC2: u8 = 21;
const SVCFTE_SPAWNBASELINE2: u8 = 66;

/// ironwail's MAX_MSGLEN: a longer demo block is a fatal error there.
pub const MAX_BLOCK: usize = 64000;

/// What the plain demo cannot show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lost {
    /// DP particle messages and rain/snow temp entities dropped.
    pub particle_msgs: usize,
    /// Some entity had a trail or emit effect (DP_ENT_TRAILEFFECTNUM).
    pub entity_effects: bool,
}

impl Lost {
    pub fn any(&self) -> bool {
        self.particle_msgs > 0 || self.entity_effects
    }
}

pub struct Converted {
    pub data: Vec<u8>,
    pub lost: Lost,
}

/// Converts a demo to the plain protocol. `Ok(None)`: it already is plain
/// (nothing recorded with protocol extensions).
pub fn to_plain(demo: &[u8]) -> Result<Option<Converted>, String> {
    let header_len = demo.iter().take(16).position(|&b| b == b'\n').map_or(0, |e| e + 1);
    let mut out = demo[..header_len].to_vec();
    let mut lost = Lost::default();
    let mut cl = ClientState::new();
    let mut extended = false;
    let mut spans = Vec::new();
    let mut d = Demo::new(demo.to_vec());
    while let Some(msg) = d.next_message() {
        spans.clear();
        cl.parse_server_message_spans(msg.data, &mut spans).map_err(|e| format!("demo parse failed: {e}"))?;
        drain(&mut cl);
        let block = convert_block(&cl, msg.data, &spans, &mut lost, &mut extended)?;
        out.extend((block.len() as i32).to_le_bytes());
        for a in msg.angles {
            out.extend(a.to_le_bytes());
        }
        out.extend(block);
    }
    Ok(extended.then_some(Converted { data: out, lost }))
}

/// Per-frame queues the host would drain (keeps memory flat).
fn drain(cl: &mut ClientState) {
    cl.events.clear();
    cl.weapon_impacts.clear();
    cl.shot_impacts.clear();
    cl.blood_hits.clear();
    cl.fte_queue.clear();
    cl.ack_frames.clear();
}

fn convert_block(cl: &ClientState, data: &[u8], spans: &[(u8, usize, usize)], lost: &mut Lost, extended: &mut bool) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(data.len() * 2);
    let deltas = cl.pext2 & PEXT2_REPLACEMENTDELTAS != 0;
    let predinfo = cl.pext2 & PEXT2_PREDINFO != 0;
    let has_ent_update = spans.iter().any(|s| s.0 == SVCFTE_UPDATEENTITIES) && deltas;
    if has_ent_update {
        out.push(SVC_TIME);
        out.extend((cl.mtime[0] as f32).to_le_bytes());
        if predinfo && !spans.iter().any(|s| s.0 == SVC_CLIENTDATA) {
            out.extend(synthesize_clientdata(cl));
        }
    }
    for &(cmd, start, end) in spans {
        let span = &data[start..end];
        match cmd {
            SVC_SERVERINFO => out.extend(strip_pext(span, extended)),
            SVCFTE_UPDATEENTITIES if deltas => {
                *extended = true;
                out.extend(entity_updates(cl, lost)?);
            }
            SVCDP_PRECACHE | SVCDP_TRAILPARTICLES | SVCDP_POINTPARTICLES | SVCDP_POINTPARTICLES1 => {
                *extended = true;
                lost.particle_msgs += 1;
            }
            SVC_TEMP_ENTITY if matches!(span.get(1), Some(&(TEDP_PARTICLERAIN | TEDP_PARTICLESNOW))) => {
                *extended = true;
                lost.particle_msgs += 1;
            }
            SVCDP_UPDATESTATBYTE | SVCFTE_UPDATESTATFLOAT | SVCFTE_UPDATESTATSTRING if deltas => {
                *extended = true;
                let v = match cmd {
                    SVCDP_UPDATESTATBYTE => span.get(2).map(|&b| b as i32),
                    SVCFTE_UPDATESTATFLOAT => span.get(2..6).map(|f| f32::from_le_bytes(f.try_into().unwrap()) as i32),
                    _ => None,
                };
                if let (Some(&i), Some(v)) = (span.get(1), v) {
                    out.push(SVC_UPDATESTAT);
                    out.push(i);
                    out.extend(v.to_le_bytes());
                }
            }
            SVCFTE_SPAWNSTATIC2 | SVCFTE_SPAWNBASELINE2 if deltas => {
                return Err("FTE baselines (recorded from another server) can't be converted".into());
            }
            _ => out.extend_from_slice(span),
        }
    }
    if out.len() > MAX_BLOCK {
        return Err(format!("a converted demo message is {} bytes, over the {MAX_BLOCK} other ports accept", out.len()));
    }
    Ok(out)
}

/// svc_serverinfo without the FTE extension prefixes (and without PEXT2_PREDINFO's gamedir string).
fn strip_pext(span: &[u8], extended: &mut bool) -> Vec<u8> {
    let mut rest = &span[1..];
    let mut pext2 = 0u32;
    while rest.len() >= 8 {
        let id = u32::from_le_bytes(rest[..4].try_into().unwrap());
        if id != PROTOCOL_FTE_PEXT1 && id != PROTOCOL_FTE_PEXT2 {
            break;
        }
        if id == PROTOCOL_FTE_PEXT2 {
            pext2 = u32::from_le_bytes(rest[4..8].try_into().unwrap());
        }
        *extended = true;
        rest = &rest[8..];
    }
    let mut v = vec![span[0]];
    if pext2 & PEXT2_PREDINFO != 0 && rest.len() >= 4 {
        let proto = u32::from_le_bytes(rest[..4].try_into().unwrap());
        let hdr = if proto == PROTOCOL_RMQ { 8 } else { 4 };
        if rest.len() >= hdr {
            v.extend_from_slice(&rest[..hdr]);
            let after = &rest[hdr..];
            // Skip NUL-terminated gamedir string.
            let skip = after.iter().position(|&b| b == 0).map_or(after.len(), |i| i + 1);
            v.extend_from_slice(&after[skip..]);
            return v;
        }
    }
    v.extend_from_slice(rest);
    v
}

/// Synthesize plain `svc_clientdata` from `PEXT2_PREDINFO` stats for vanilla demo playback.
fn synthesize_clientdata(cl: &ClientState) -> Vec<u8> {
    let mut msg = SizeBuf::new(256);
    let mut bits = SU_ITEMS | SU_WEAPON;
    if cl.viewheight != DEFAULT_VIEWHEIGHT {
        bits |= SU_VIEWHEIGHT;
    }
    if cl.idealpitch != 0.0 {
        bits |= SU_IDEALPITCH;
    }
    if cl.onground {
        bits |= SU_ONGROUND;
    }
    if cl.inwater {
        bits |= SU_INWATER;
    }
    for i in 0..3 {
        if cl.punchangle[i] != 0.0 {
            bits |= SU_PUNCH1 << i;
        }
        if cl.mvelocity[0][i] != 0.0 {
            bits |= SU_VELOCITY1 << i;
        }
    }
    if cl.stats[STAT_WEAPONFRAME] != 0 {
        bits |= SU_WEAPONFRAME;
    }
    if cl.stats[STAT_ARMOR] != 0 {
        bits |= SU_ARMOR;
    }
    if cl.protocol != PROTOCOL_NETQUAKE {
        for (st, b) in [
            (STAT_WEAPON, SU_WEAPON2),
            (STAT_ARMOR, SU_ARMOR2),
            (STAT_AMMO, SU_AMMO2),
            (STAT_SHELLS, SU_SHELLS2),
            (STAT_NAILS, SU_NAILS2),
            (STAT_ROCKETS, SU_ROCKETS2),
            (STAT_CELLS, SU_CELLS2),
        ] {
            if cl.stats[st] & 0xff00 != 0 {
                bits |= b;
            }
        }
        if bits & SU_WEAPONFRAME != 0 && cl.stats[STAT_WEAPONFRAME] & 0xff00 != 0 {
            bits |= SU_WEAPONFRAME2;
        }
        if cl.viewent.alpha != ENTALPHA_DEFAULT {
            bits |= SU_WEAPONALPHA;
        }
        if bits >= 65536 {
            bits |= SU_EXTEND1;
        }
        if bits >= 16_777_216 {
            bits |= SU_EXTEND2;
        }
    }
    msg.byte(SVC_CLIENTDATA as i32);
    msg.short(bits as i32);
    if bits & SU_EXTEND1 != 0 {
        msg.byte((bits >> 16) as i32);
    }
    if bits & SU_EXTEND2 != 0 {
        msg.byte((bits >> 24) as i32);
    }
    if bits & SU_VIEWHEIGHT != 0 {
        msg.char(cl.viewheight as i32);
    }
    if bits & SU_IDEALPITCH != 0 {
        msg.char(cl.idealpitch as i32);
    }
    for i in 0..3 {
        if bits & (SU_PUNCH1 << i) != 0 {
            msg.char(cl.punchangle[i] as i32);
        }
        if bits & (SU_VELOCITY1 << i) != 0 {
            msg.char((cl.mvelocity[0][i] / 16.0) as i32);
        }
    }
    msg.long(cl.items);
    if bits & SU_WEAPONFRAME != 0 {
        msg.byte(cl.stats[STAT_WEAPONFRAME]);
    }
    if bits & SU_ARMOR != 0 {
        msg.byte(cl.stats[STAT_ARMOR]);
    }
    msg.byte(cl.stats[STAT_WEAPON]);
    msg.short(cl.stats[STAT_HEALTH]);
    for st in [STAT_AMMO, STAT_SHELLS, STAT_NAILS, STAT_ROCKETS, STAT_CELLS] {
        msg.byte(cl.stats[st]);
    }
    let w = cl.stats[STAT_ACTIVEWEAPON];
    if cl.standard_quake {
        msg.byte(w);
    } else if let Some(i) = (0..32).find(|i| w & (1 << i) != 0) {
        msg.byte(i);
    } else {
        msg.byte(0);
    }
    for (st, b) in [
        (STAT_WEAPON, SU_WEAPON2),
        (STAT_ARMOR, SU_ARMOR2),
        (STAT_AMMO, SU_AMMO2),
        (STAT_SHELLS, SU_SHELLS2),
        (STAT_NAILS, SU_NAILS2),
        (STAT_ROCKETS, SU_ROCKETS2),
        (STAT_CELLS, SU_CELLS2),
        (STAT_WEAPONFRAME, SU_WEAPONFRAME2),
    ] {
        if bits & b != 0 {
            msg.byte(cl.stats[st] >> 8);
        }
    }
    if bits & SU_WEAPONALPHA != 0 {
        msg.byte(cl.viewent.alpha as i32);
    }
    msg.data
}

/// SV_WriteEntitiesToClient for the snapshot the client holds.
fn entity_updates(cl: &ClientState, lost: &mut Lost) -> Result<Vec<u8>, String> {
    let (protocol, pf) = (cl.protocol, cl.protocolflags);
    let mut msg = SizeBuf::new(MAX_BLOCK);
    for (n, e) in cl.entities.iter().enumerate().skip(1).filter(|(_, e)| e.delta_live) {
        lost.entity_effects |= e.netstate.traileffect != 0 || e.netstate.emiteffect != 0;
        write_update(&mut msg, n as u32, &e.netstate, &e.baseline, protocol, pf);
    }
    // (an overflowing buffer starts again from nothing)
    if msg.overflowed { Err("too many entities for one message".into()) } else { Ok(msg.data) }
}

fn write_update(msg: &mut SizeBuf, e: u32, s: &EntState, b: &qclient::EntityState, protocol: u32, pf: u32) {
    let mut bits = 0u32;
    for i in 0..3 {
        if !(-0.1..=0.1).contains(&(s.origin[i] - b.origin[i])) {
            bits |= U_ORIGIN1 << i;
        }
    }
    for (i, bit) in [U_ANGLE1, U_ANGLE2, U_ANGLE3].into_iter().enumerate() {
        if s.angles[i] != b.angles[i] {
            bits |= bit;
        }
    }
    if s.eflags & qclient::delta::EFLAGS_STEP != 0 {
        bits |= U_STEP;
    }
    let (mi, frame) = (s.modelindex as i32, s.frame as i32);
    if b.colormap != s.colormap as i32 {
        bits |= U_COLORMAP;
    }
    if b.skin != s.skin as i32 {
        bits |= U_SKIN;
    }
    if b.frame != frame {
        bits |= U_FRAME;
    }
    if (b.effects ^ s.effects as i32) & 0xff != 0 {
        bits |= U_EFFECTS;
    }
    if b.modelindex as i32 != mi {
        bits |= U_MODEL;
    }
    if protocol != PROTOCOL_NETQUAKE {
        if b.alpha != s.alpha {
            bits |= U_ALPHA;
        }
        if b.scale != s.scale {
            bits |= U_SCALE;
        }
        if bits & U_FRAME != 0 && frame & 0xff00 != 0 {
            bits |= U_FRAME2;
        }
        if bits & U_MODEL != 0 && mi & 0xff00 != 0 {
            bits |= U_MODEL2;
        }
        if bits >= 1 << 16 {
            bits |= U_EXTEND1;
        }
        if bits >= 1 << 24 {
            bits |= U_EXTEND2;
        }
    }
    if e >= 256 {
        bits |= U_LONGENTITY;
    }
    if bits >= 256 {
        bits |= U_MOREBITS;
    }
    msg.byte(((bits | U_SIGNAL) & 0xff) as i32);
    if bits & U_MOREBITS != 0 {
        msg.byte(((bits >> 8) & 0xff) as i32);
    }
    if bits & U_EXTEND1 != 0 {
        msg.byte(((bits >> 16) & 0xff) as i32);
    }
    if bits & U_EXTEND2 != 0 {
        msg.byte((bits >> 24) as i32);
    }
    if bits & U_LONGENTITY != 0 {
        msg.short(e as i32);
    } else {
        msg.byte(e as i32);
    }
    if bits & U_MODEL != 0 {
        msg.byte(mi);
    }
    if bits & U_FRAME != 0 {
        msg.byte(frame);
    }
    if bits & U_COLORMAP != 0 {
        msg.byte(s.colormap as i32);
    }
    if bits & U_SKIN != 0 {
        msg.byte(s.skin as i32);
    }
    if bits & U_EFFECTS != 0 {
        msg.byte(s.effects as i32 & 0xff);
    }
    for i in 0..3 {
        if bits & (U_ORIGIN1 << i) != 0 {
            msg.coord(s.origin[i], pf);
        }
        let ab = [U_ANGLE1, U_ANGLE2, U_ANGLE3][i];
        if bits & ab != 0 {
            msg.angle(s.angles[i], pf);
        }
    }
    if bits & U_ALPHA != 0 {
        msg.byte(s.alpha as i32);
    }
    if bits & U_SCALE != 0 {
        msg.byte(s.scale as i32);
    }
    if bits & U_FRAME2 != 0 {
        msg.byte(frame >> 8);
    }
    if bits & U_MODEL2 != 0 {
        msg.byte(mi >> 8);
    }
}

/// Converts a recorded `.dem` in place for other ports (the stop of
/// `record`, or `demo_convert`). When the plain demo would lose effects the
/// original is kept beside it as `.qsd` (playable here with `playdemo
/// name.qsd`). Returns a console line ("" when nothing was needed).
pub fn convert_file(path: &std::path::Path) -> String {
    let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let data = match qcore::vfs::read(path) {
        Ok(d) => d,
        Err(e) => return format!("demo convert: couldn't read {name}: {e}\n"),
    };
    let c = match to_plain(&data) {
        Ok(Some(c)) => c,
        Ok(None) => return String::new(),
        Err(e) => return format!("{name} plays only in this port: {e}\n"),
    };
    // write beside, then swap in, so the demo is never half written
    let tmp = path.with_extension("dem.tmp");
    let qsd = path.with_extension(QSD_EXT);
    let r = qcore::vfs::write(&tmp, &c.data)
        .and_then(|_| if c.lost.any() { qcore::vfs::rename(path, &qsd) } else { Ok(()) })
        .and_then(|_| qcore::vfs::rename(&tmp, path));
    if let Err(e) = r {
        let _ = qcore::vfs::remove_file(&tmp);
        return format!("demo convert: couldn't write {name}: {e}\n");
    }
    if c.lost.any() {
        let what = match (c.lost.particle_msgs > 0, c.lost.entity_effects) {
            (true, true) => "particle and trail effects",
            (true, false) => "particle effects",
            _ => "trail effects",
        };
        let full = qsd.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        format!("{name} plays in other source ports without this mod's {what}; {full} keeps them (this port only).\n")
    } else {
        format!("{name} plays in other source ports too.\n")
    }
}

/// Extension of a full-detail demo kept beside its converted `.dem`.
pub const QSD_EXT: &str = "qsd";

#[cfg(test)]
mod tests;
