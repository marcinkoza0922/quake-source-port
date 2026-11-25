//! QuakeWorld `.qwd` and Multi-View Demo `.mvd` playback support.
//!
//! Translates `.qwd` (single-POV client netchan recording) and `.mvd`
//! (server-side all-player recording with `dem_all`, `dem_multiple`,
//! `dem_single`, `dem_stats`) into the NetQuake `.dem` block stream consumed
//! by [`super::Demo`], [`super::index`], [`super::keyframes`] and [`qclient::ClientState`].
//! For `.mvd` demos, [`MvdInfo`] retains the raw MVD bytes and player roster
//! so the active player POV can be switched at any point during playback.

use std::sync::Arc;

use qclient::qw::QwState;
use qnet::qw::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QwDemoFormat {
    Qwd,
    Mvd,
}

#[derive(Clone, Debug)]
pub struct MvdInfo {
    pub raw: Arc<[u8]>,
    pub pov: usize,
    pub players: Vec<(usize, String)>,
}

/// Checks if `payload` begins with a QuakeWorld `SVC_SERVERDATA` header
/// (optionally preceded by `SVC_NOP` or `SVC_PRINT`, and with optional
/// `FTEX`/`FTE2`/`MVD1` protocol extension pairs before `PROTOCOL_VERSION`).
fn starts_with_qw_serverdata(mut payload: &[u8]) -> bool {
    while let Some(&b) = payload.first() {
        if b == SVC_NOP {
            payload = &payload[1..];
        } else {
            break;
        }
    }
    if payload.first() != Some(&SVC_SERVERDATA) {
        return false;
    }
    let mut p = &payload[1..];
    for _ in 0..4 {
        if p.len() < 4 {
            return false;
        }
        let v = i32::from_le_bytes(p[..4].try_into().unwrap());
        if v == PROTOCOL_VERSION {
            return true;
        }
        if matches!(v, PROTOCOL_VERSION_FTE | PROTOCOL_VERSION_FTE2 | PROTOCOL_VERSION_MVD1) {
            if p.len() < 8 {
                return false;
            }
            p = &p[8..];
            continue;
        }
        return false;
    }
    false
}

/// Auto-detects whether `data` is a `.qwd` or `.mvd` file (`None` for `.dem`).
pub fn detect_format(data: &[u8]) -> Option<QwDemoFormat> {
    // 1. Check for .qwd: records of [f32 time, u8 cmd (0..=2), ...].
    if data.len() >= 18 {
        let mut pos = 0usize;
        for _ in 0..4 {
            if pos + 5 > data.len() {
                break;
            }
            let t = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            let cmd = data[pos + 4];
            if !t.is_finite() || !(-1000.0..=1_000_000.0).contains(&t) {
                break;
            }
            pos += 5;
            match cmd {
                DEM_CMD => {
                    if pos + 36 > data.len() {
                        break;
                    }
                    pos += 36;
                }
                DEM_SET => {
                    if pos + 8 > data.len() {
                        break;
                    }
                    pos += 8;
                }
                DEM_READ => {
                    if pos + 4 > data.len() {
                        break;
                    }
                    let len = i32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                    if !(9..=96_000).contains(&len) || pos + 4 + len as usize > data.len() {
                        break;
                    }
                    let pkt = &data[pos + 4..pos + 4 + len as usize];
                    if starts_with_qw_serverdata(&pkt[8..]) {
                        return Some(QwDemoFormat::Qwd);
                    }
                    pos += 4 + len as usize;
                }
                _ => break,
            }
        }
    }

    // 2. Check for .mvd: records of [u8 msec, u8 cmd, (optional u32 mask), u32 len, payload].
    if data.len() >= 11 {
        let mut pos = 0usize;
        for _ in 0..4 {
            if pos + 2 > data.len() {
                break;
            }
            let cmd = data[pos + 1] & DEM_MASK;
            pos += 2;
            match cmd {
                DEM_SET => {
                    if pos + 8 > data.len() {
                        break;
                    }
                    pos += 8;
                }
                DEM_MULTIPLE => {
                    if pos + 8 > data.len() {
                        break;
                    }
                    let mask = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                    let len = i32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
                    if !(0..=96_000).contains(&len) || pos + 8 + len as usize > data.len() {
                        break;
                    }
                    let pkt = &data[pos + 8..pos + 8 + len as usize];
                    if mask != 0 && starts_with_qw_serverdata(pkt) {
                        return Some(QwDemoFormat::Mvd);
                    }
                    pos += 8 + len as usize;
                }
                DEM_ALL | DEM_READ | DEM_SINGLE | DEM_STATS => {
                    if pos + 4 > data.len() {
                        break;
                    }
                    let len = i32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                    if !(0..=96_000).contains(&len) || pos + 4 + len as usize > data.len() {
                        break;
                    }
                    let pkt = &data[pos + 4..pos + 4 + len as usize];
                    if starts_with_qw_serverdata(pkt) {
                        return Some(QwDemoFormat::Mvd);
                    }
                    pos += 4 + len as usize;
                }
                _ => break,
            }
        }
    }
    None
}

fn push_dem_block(out: &mut Vec<u8>, angles: [f32; 3], msg: &[u8]) {
    out.extend((msg.len() as i32).to_le_bytes());
    for a in angles {
        out.extend(a.to_le_bytes());
    }
    out.extend_from_slice(msg);
}

/// Most a demo may grow to: every packet becomes a whole frame, so a crafted
/// or very long one would take gigabytes. The rest of it isn't played.
const MAX_OUT: usize = if cfg!(target_arch = "wasm32") { 64 << 20 } else { 256 << 20 };

/// Transcodes a `.qwd` file into an in-memory `.dem` byte stream.
pub fn transcode_qwd(data: &[u8]) -> Option<Vec<u8>> {
    transcode_qwd_within(data, MAX_OUT)
}

fn transcode_qwd_within(data: &[u8], max: usize) -> Option<Vec<u8>> {
    let mut qw = QwState::new();
    qw.demo_mode = true;
    let mut out = b"-1\n".to_vec();
    let mut pos = 0usize;
    let mut base_time: Option<f64> = None;
    let mut last_time = 0.01f64;
    let mut cur_angles = [0.0f32; 3];

    while pos + 5 <= data.len() && out.len() < max {
        let raw_t = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
        let cmd = data[pos + 4];
        pos += 5;
        let t0 = *base_time.get_or_insert(raw_t as f64);
        let rel_t = ((raw_t as f64 - t0).max(0.0) + 0.01).max(last_time);
        last_time = rel_t;

        match cmd {
            DEM_CMD => {
                if pos + 36 > data.len() {
                    break;
                }
                let r = &data[pos..pos + 36];
                pos += 36;
                let f = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
                let s = |o: usize| i16::from_le_bytes(r[o..o + 2].try_into().unwrap());
                let ucmd = UserCmd {
                    msec: r[0],
                    angles: [f(4), f(8), f(12)],
                    forwardmove: s(16),
                    sidemove: s(18),
                    upmove: s(20),
                    buttons: r[22],
                    impulse: r[23],
                };
                cur_angles = [f(24), f(28), f(32)];
                let seq = qw.outgoing_sequence;
                let fi = (seq & UPDATE_MASK) as usize;
                qw.frames[fi].cmd = ucmd;
                qw.frames[fi].senttime = rel_t;
                qw.outgoing_sequence = qw.outgoing_sequence.wrapping_add(1);
            }
            DEM_SET => {
                if pos + 8 > data.len() {
                    break;
                }
                let out_seq = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                pos += 8;
                qw.outgoing_sequence = out_seq;
            }
            DEM_READ => {
                if pos + 4 > data.len() {
                    break;
                }
                let len = i32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                if len < 0 || pos + 4 + len as usize > data.len() {
                    break;
                }
                let pkt = &data[pos + 4..pos + 4 + len as usize];
                pos += 4 + len as usize;
                if pkt.len() >= 4 && i32::from_le_bytes(pkt[..4].try_into().unwrap()) == -1 {
                    if pkt.get(4) == Some(&SVC_DISCONNECT) {
                        break;
                    }
                    continue;
                }
                if pkt.len() < 8 {
                    continue;
                }
                let seq = u32::from_le_bytes(pkt[0..4].try_into().unwrap()) & 0x7fff_ffff;
                let ack = u32::from_le_bytes(pkt[4..8].try_into().unwrap()) & 0x7fff_ffff;
                let Ok(parsed) = qw.parse(&pkt[8..], ack, seq, rel_t) else {
                    break;
                };
                for msg in parsed.nq {
                    push_dem_block(&mut out, cur_angles, &msg);
                }
            }
            _ => break,
        }
    }
    (out.len() > 3).then_some(out)
}

/// Transcodes an `.mvd` file into an in-memory `.dem` byte stream for `pov`
/// (or auto-selects the first active player when `pov` is `None`).
pub fn transcode_mvd(raw: Arc<[u8]>, pov: Option<usize>) -> Option<(Vec<u8>, MvdInfo)> {
    transcode_mvd_within(raw, pov, MAX_OUT)
}

fn transcode_mvd_within(raw: Arc<[u8]>, pov: Option<usize>, max: usize) -> Option<(Vec<u8>, MvdInfo)> {
    let data = &raw[..];
    let mut qw = QwState::new();
    qw.demo_mode = true;
    qw.mvd = true;
    if let Some(slot) = pov {
        qw.playernum = slot.min(MAX_CLIENTS - 1);
        qw.pov_locked = true;
    }

    let mut out = b"-1\n".to_vec();
    let mut pos = 0usize;
    let mut demotime = 0.01f64;
    let mut seq = 1u32;

    while pos + 2 <= data.len() && out.len() < max {
        let msec = data[pos];
        let cmd_byte = data[pos + 1];
        pos += 2;
        if msec > 0 {
            demotime += f64::from(msec) * 0.001;
        }
        let kind = cmd_byte & DEM_MASK;
        let mut target_mask = u32::MAX;
        let payload: &[u8] = match kind {
            DEM_CMD => {
                if pos + 36 > data.len() {
                    break;
                }
                pos += 36;
                continue;
            }
            DEM_SET => {
                if pos + 8 > data.len() {
                    break;
                }
                pos += 8;
                continue;
            }
            DEM_MULTIPLE => {
                if pos + 8 > data.len() {
                    break;
                }
                target_mask = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                let len = i32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap());
                if len < 0 || pos + 8 + len as usize > data.len() {
                    break;
                }
                let p = &data[pos + 8..pos + 8 + len as usize];
                pos += 8 + len as usize;
                if target_mask == 0 {
                    // Hidden MVD extension block (MVD_PEXT1_HIDDEN_MESSAGES).
                    continue;
                }
                p
            }
            DEM_SINGLE | DEM_STATS => {
                let slot = (cmd_byte >> 3) as u32;
                target_mask = 1u32.checked_shl(slot).unwrap_or(0);
                if pos + 4 > data.len() {
                    break;
                }
                let len = i32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                if len < 0 || pos + 4 + len as usize > data.len() {
                    break;
                }
                let p = &data[pos + 4..pos + 4 + len as usize];
                pos += 4 + len as usize;
                p
            }
            DEM_READ | DEM_ALL => {
                if pos + 4 > data.len() {
                    break;
                }
                let len = i32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                if len < 0 || pos + 4 + len as usize > data.len() {
                    break;
                }
                let p = &data[pos + 4..pos + 4 + len as usize];
                pos += 4 + len as usize;
                p
            }
            _ => break,
        };
        if payload.is_empty() {
            continue;
        }

        // In MVD, SV_SendDemoMessage writes svc_packetentities in one DEM_ALL
        // packet and SV_MVDWritePacketsEx writes svc_playerinfo in the next
        // DEM_ALL packet with msec == 0. Coalesce consecutive DEM_ALL packets
        // at the same timestamp once active so entities and players share one frame.
        let mut combined: Option<Vec<u8>> = None;
        if kind == DEM_ALL && qw.stage == qclient::qw::Stage::Active {
            while pos + 6 <= data.len() && data[pos] == 0 && (data[pos + 1] & DEM_MASK) == DEM_ALL {
                let next_len = i32::from_le_bytes(data[pos + 2..pos + 6].try_into().unwrap());
                if next_len < 0 || pos + 6 + next_len as usize > data.len() {
                    break;
                }
                let next_p = &data[pos + 6..pos + 6 + next_len as usize];
                if starts_with_qw_serverdata(next_p) {
                    break;
                }
                let buf = combined.get_or_insert_with(|| payload.to_vec());
                buf.extend_from_slice(next_p);
                pos += 6 + next_len as usize;
            }
        }
        let slice = combined.as_deref().unwrap_or(payload);
        qw.target_mask = target_mask;
        qw.outgoing_sequence = seq;
        let Ok(parsed) = qw.parse(slice, seq, seq, demotime) else {
            break;
        };
        seq = seq.wrapping_add(1);
        let angles = qw.pov_angles();
        for msg in parsed.nq {
            push_dem_block(&mut out, angles, &msg);
        }
    }

    if out.len() <= 3 {
        return None;
    }
    let info = MvdInfo {
        raw,
        pov: qw.playernum,
        players: qw.active_players(),
    };
    Some((out, info))
}

#[cfg(test)]
#[path = "qw_tests.rs"]
mod tests;
