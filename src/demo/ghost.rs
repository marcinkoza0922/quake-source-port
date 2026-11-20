//! Personal-best ghost track: locates a map's PB demo (in `ghosts/` or kept
//! by `cl_autodemo_pb`) and extracts a compact, time-indexed timeline of the
//! player's world pose and animation frames for translucent ghost playback.

use std::path::{Path, PathBuf};

use qclient::{ClientEvent, ClientState};

use super::Demo;

/// Subdirectory under the mod's user dir holding PB ghost demos.
pub const GHOST_DIR: &str = "ghosts";

/// Skill names shared with PB demo filenames (`level_time`).
pub const SKILL_NAMES: [&str; 4] = ["easy", "normal", "hard", "nightmare"];

/// Distance (units) above which a single-step jump is treated as a teleport.
const TELEPORT_DELTA: f32 = 100.0;

/// One sampled player pose from a PB demo.
#[derive(Clone, Debug, PartialEq)]
pub struct GhostSample {
    pub time: f64,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub frame: u32,
    pub prev_frame: u32,
    pub frame_time: f64,
    pub skin: u32,
    /// Index into [`GhostTrack::models`].
    pub model: u16,
    pub colormap: Option<[u8; 2]>,
}

/// Interpolated ghost pose for rendering a single frame.
#[derive(Clone, Debug, PartialEq)]
pub struct GhostPose {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub frame: u32,
    pub prev_frame: u32,
    pub lerp: f32,
    pub skin: u32,
    pub model: String,
    pub colormap: Option<[u8; 2]>,
}

/// Time-sorted player poses extracted from a PB demo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GhostTrack {
    pub models: Vec<String>,
    pub samples: Vec<GhostSample>,
}

/// Sanitizes a map name for filenames (same rule as `autodemo` / `level_time`).
pub fn sanitize_map(mapname: &str) -> String {
    mapname.chars().map(|c| if c.is_alphanumeric() || "-_".contains(c) { c } else { '_' }).collect()
}

/// Dedicated ghost demo path: `<user_dir>/ghosts/<map>_<any|100>_<skill>.dem`.
pub fn ghost_path(user_dir: &Path, mapname: &str, skill: usize, full: bool) -> PathBuf {
    let map = sanitize_map(mapname).to_ascii_lowercase();
    let cat = if full { "100" } else { "any" };
    let sk = SKILL_NAMES.get(skill).copied().unwrap_or("normal");
    user_dir.join(GHOST_DIR).join(format!("{map}_{cat}_{sk}.dem"))
}

/// Parses `<M>m<SS>s<CC>` (with optional `_<n>` suffix) into centiseconds.
pub fn parse_file_time(s: &str) -> Option<u64> {
    let base = match s.rsplit_once('_') {
        Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => s,
    };
    let (m_str, rest) = base.split_once('m')?;
    let (s_str, c_str) = rest.split_once('s')?;
    if m_str.is_empty() || s_str.len() != 2 || c_str.len() != 2 {
        return None;
    }
    let mins: u64 = m_str.parse().ok()?;
    let secs: u64 = s_str.parse().ok()?;
    let cs: u64 = c_str.parse().ok()?;
    if secs >= 60 || cs >= 100 {
        return None;
    }
    Some(mins * 6000 + secs * 100 + cs)
}

/// Finds the PB demo for `(mapname, skill, full)`: prefers the dedicated
/// `ghosts/` demo, otherwise picks the fastest matching `<map>_pb_*` demo
/// kept in `user_dir` by `cl_autodemo_pb`.
pub fn find_pb_demo(user_dir: &Path, mapname: &str, skill: usize, full: bool) -> Option<PathBuf> {
    if mapname.is_empty() || skill >= SKILL_NAMES.len() {
        return None;
    }
    let dedicated = ghost_path(user_dir, mapname, skill, full);
    if qcore::vfs::exists(&dedicated) {
        return Some(dedicated);
    }
    let map = sanitize_map(mapname).to_ascii_lowercase();
    let sk = SKILL_NAMES[skill];
    let prefixes = if full {
        [format!("{map}_pb_100_{sk}_"), format!("{map}_pb_any-100_{sk}_")]
    } else {
        [format!("{map}_pb_any_{sk}_"), format!("{map}_pb_any-100_{sk}_")]
    };
    let entries = qcore::vfs::read_dir(user_dir).ok()?;
    let mut best: Option<(u64, PathBuf)> = None;
    for e in entries {
        if e.is_dir || !e.path.extension().is_some_and(|x| x.eq_ignore_ascii_case("dem")) {
            continue;
        }
        let stem = e.path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let lower = stem.to_ascii_lowercase();
        let Some(rest) = prefixes.iter().find_map(|p| lower.strip_prefix(p.as_str())) else {
            continue;
        };
        let Some(cs) = parse_file_time(rest) else {
            continue;
        };
        if best.as_ref().is_none_or(|(b, bp)| cs < *b || (cs == *b && e.path < *bp)) {
            best = Some((cs, e.path));
        }
    }
    best.map(|(_, p)| p)
}

impl GhostTrack {
    /// Replays `data` (a `.dem` file) and extracts the player's pose timeline
    /// for `expected_map` (or the first map in the demo when `None`).
    pub fn extract(data: &[u8], expected_map: Option<&str>) -> Self {
        let mut demo = Demo::new(data.to_vec());
        let mut cl = ClientState::new();
        cl.demoplayback = true;
        let mut track = GhostTrack::default();
        let mut in_map = false;
        let mut anim: Option<(u16, u32, u32, f64)> = None;

        while let Some(msg) = demo.next_message() {
            if cl.parse_server_message(msg.data).is_err() {
                break;
            }
            let new_map = cl.events.iter().any(|e| matches!(e, ClientEvent::NewMap));
            let disconnect = cl.events.iter().any(|e| matches!(e, ClientEvent::Disconnect));
            drain(&mut cl);

            if new_map {
                if !track.samples.is_empty() {
                    break;
                }
                in_map = expected_map.is_none_or(|m| cl.mapname.eq_ignore_ascii_case(m));
                anim = None;
            }
            if in_map && cl.signon == 4 {
                if cl.intermission != 0 {
                    break;
                }
                let t = cl.mtime[0];
                let me = cl.viewentity;
                if me > 0
                    && let Some(e) = cl.entities.get(me)
                    && e.model != 0
                    && e.msgtime == t
                {
                    let name = cl
                        .model_precache
                        .get(e.model)
                        .filter(|s| !s.is_empty())
                        .map_or("progs/player.mdl", String::as_str);
                    let model = track.intern_model(name);
                    let colormap = (e.colormap > 0 && (e.colormap as usize) <= cl.scores.len()).then(|| {
                        let c = cl.scores[e.colormap as usize - 1].colors;
                        [((c >> 4) & 15) as u8, (c & 15) as u8]
                    });
                    let frame = e.frame.max(0) as u32;
                    let (prev_frame, frame_time) = match anim {
                        None => (frame, t - 1.0),
                        Some((am, _, _, _)) if am != model => (frame, t - 1.0),
                        Some((_, af, _, _)) if af != frame => (af, t),
                        Some((_, _, ap, at)) => (ap, at),
                    };
                    anim = Some((model, frame, prev_frame, frame_time));
                    let sample = GhostSample {
                        time: t,
                        origin: e.msg_origins[0],
                        angles: e.msg_angles[0],
                        frame,
                        prev_frame,
                        frame_time,
                        skin: e.skin.max(0) as u32,
                        model,
                        colormap,
                    };
                    if let Some(last) = track.samples.last_mut() {
                        if t < last.time {
                            continue;
                        }
                        if (t - last.time).abs() <= 1e-9 {
                            *last = sample;
                            continue;
                        }
                    }
                    track.samples.push(sample);
                }
            }
            if disconnect {
                break;
            }
        }
        track
    }

    fn intern_model(&mut self, name: &str) -> u16 {
        if let Some(i) = self.models.iter().position(|m| m == name) {
            return i as u16;
        }
        let i = self.models.len().min(u16::MAX as usize) as u16;
        if (i as usize) == self.models.len() {
            self.models.push(name.to_string());
        }
        i
    }

    fn model_name(&self, idx: u16) -> String {
        self.models.get(idx as usize).cloned().unwrap_or_else(|| "progs/player.mdl".to_string())
    }

    /// Evaluates the ghost's pose at level time `time`. Returns `None` before
    /// the track starts (with a 0.5 s grace window at spawn) or after it ends.
    pub fn sample(&self, time: f64, lerp_models: bool) -> Option<GhostPose> {
        let first = self.samples.first()?;
        let last = self.samples.last()?;
        if time > last.time {
            return None;
        }
        if time < first.time {
            if first.time - time > 0.5 {
                return None;
            }
            return Some(GhostPose {
                origin: first.origin,
                angles: first.angles,
                frame: first.frame,
                prev_frame: first.frame,
                lerp: 1.0,
                skin: first.skin,
                model: self.model_name(first.model),
                colormap: first.colormap,
            });
        }
        let idx = self.samples.partition_point(|s| s.time <= time);
        if idx == 0 {
            return Some(self.pose_at(first, time, lerp_models));
        }
        if idx >= self.samples.len() {
            return Some(self.pose_at(last, time, lerp_models));
        }
        let a = &self.samples[idx - 1];
        let b = &self.samples[idx];
        let dt = b.time - a.time;
        let frac = if dt > 1e-6 { ((time - a.time) / dt).clamp(0.0, 1.0) as f32 } else { 1.0 };
        let teleported = (0..3).any(|j| (b.origin[j] - a.origin[j]).abs() > TELEPORT_DELTA);
        if teleported {
            let pick = if frac < 0.5 { a } else { b };
            return Some(self.pose_at(pick, time, lerp_models));
        }
        let mut origin = [0.0; 3];
        let mut angles = [0.0; 3];
        for j in 0..3 {
            origin[j] = a.origin[j] + (b.origin[j] - a.origin[j]) * frac;
            let mut d = b.angles[j] - a.angles[j];
            if d > 180.0 {
                d -= 360.0;
            } else if d < -180.0 {
                d += 360.0;
            }
            angles[j] = a.angles[j] + d * frac;
        }
        let lerp = if lerp_models { ((time - a.frame_time) / 0.1).clamp(0.0, 1.0) as f32 } else { 1.0 };
        Some(GhostPose {
            origin,
            angles,
            frame: a.frame,
            prev_frame: a.prev_frame,
            lerp,
            skin: a.skin,
            model: self.model_name(a.model),
            colormap: a.colormap,
        })
    }

    fn pose_at(&self, s: &GhostSample, time: f64, lerp_models: bool) -> GhostPose {
        let lerp = if lerp_models { ((time - s.frame_time) / 0.1).clamp(0.0, 1.0) as f32 } else { 1.0 };
        GhostPose {
            origin: s.origin,
            angles: s.angles,
            frame: s.frame,
            prev_frame: s.prev_frame,
            lerp,
            skin: s.skin,
            model: self.model_name(s.model),
            colormap: s.colormap,
        }
    }
}

fn drain(cl: &mut ClientState) {
    cl.events.clear();
    cl.weapon_impacts.clear();
    cl.shot_impacts.clear();
    cl.blood_hits.clear();
    cl.fte_queue.clear();
    cl.ack_frames.clear();
    cl.take_reliable_out();
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnet::msg::SizeBuf;
    use qnet::protocol::*;

    fn block(msg: &[u8]) -> Vec<u8> {
        let mut out = (msg.len() as i32).to_le_bytes().to_vec();
        out.extend([0u8; 12]);
        out.extend(msg);
        out
    }

    fn synthetic_demo() -> Vec<u8> {
        let mut dem = b"-1\n".to_vec();
        // Serverinfo + signon 4 + player updates
        let mut m = SizeBuf::new(2048);
        m.byte(SVC_SERVERINFO.into());
        m.long(PROTOCOL_NETQUAKE as i32);
        m.byte(1); // maxclients
        m.byte(0); // gametype
        m.string(b"Slipgate Complex");
        m.string(b"maps/e1m1.bsp");
        m.string(b"progs/player.mdl");
        m.string(b"");
        m.string(b"");
        m.byte(SVC_SETVIEW.into());
        m.short(1);
        m.byte(SVC_UPDATECOLORS.into());
        m.byte(0);
        m.byte(0x34); // shirt 3, pants 4
        m.byte(SVC_SIGNONNUM.into());
        m.byte(4);
        dem.extend(block(&m.data));

        let push_frame = |dem: &mut Vec<u8>, time: f32, origin: [f32; 3], yaw: f32, frame: i32| {
            let mut b = SizeBuf::new(256);
            b.byte(SVC_TIME.into());
            b.float(time);
            let bits: u32 = U_SIGNAL | U_MOREBITS | U_MODEL | U_FRAME | U_COLORMAP | U_ORIGIN1 | U_ORIGIN2 | U_ORIGIN3 | U_ANGLE2;
            b.byte((bits & 0xff) as i32);
            b.byte(((bits >> 8) & 0xff) as i32);
            b.byte(1); // ent 1
            b.byte(2); // model 2 = progs/player.mdl
            b.byte(frame);
            b.byte(1); // colormap 1
            b.coord(origin[0], 0);
            b.coord(origin[1], 0);
            b.angle(yaw, 0);
            b.coord(origin[2], 0);
            dem.extend(block(&b.data));
        };

        push_frame(&mut dem, 1.0, [0.0, 0.0, 0.0], 350.0, 6);
        push_frame(&mut dem, 1.1, [100.0, 0.0, 0.0], 10.0, 7);
        // Teleporter jump (> 100 units)
        push_frame(&mut dem, 1.2, [500.0, 200.0, 64.0], 90.0, 7);
        // Intermission cuts off the track
        let mut inter = SizeBuf::new(32);
        inter.byte(SVC_INTERMISSION.into());
        dem.extend(block(&inter.data));
        push_frame(&mut dem, 1.3, [999.0, 0.0, 0.0], 0.0, 8);
        dem
    }

    #[test]
    fn extracts_and_interpolates_with_wrap_and_teleport() {
        let dem = synthetic_demo();
        // Wrong map name produces an empty track
        assert!(GhostTrack::extract(&dem, Some("e1m2")).samples.is_empty());
        let track = GhostTrack::extract(&dem, Some("E1M1"));
        assert_eq!(track.samples.len(), 3);
        assert_eq!(track.models, ["progs/player.mdl"]);

        // Halfway between 1.0 and 1.1: x ≈ 50, yaw wraps 350 -> 10 across 0/360
        let mid = track.sample(1.05, true).unwrap();
        assert!((mid.origin[0] - 50.0).abs() < 1.0, "{mid:?}");
        let yaw = mid.angles[1].rem_euclid(360.0);
        assert!(yaw < 3.0 || yaw > 357.0, "expected wrapped yaw near 0, got {yaw}");
        assert_eq!(mid.frame, 6);
        assert_eq!(mid.colormap, Some([3, 4]));

        // Frame transition 6 -> 7 happened at t = 1.1; at t = 1.15 lerp is ~0.5
        let anim = track.sample(1.14, true).unwrap();
        assert_eq!((anim.frame, anim.prev_frame), (7, 6));
        assert!((anim.lerp - 0.4).abs() < 0.05);
        // Teleporter jump between 1.1 and 1.2 snaps rather than crossing the map
        assert!((anim.origin[0] - 100.0).abs() < 1.0);
        let after_tp = track.sample(1.16, true).unwrap();
        assert!((after_tp.origin[0] - 500.0).abs() < 1.0);

        // Before grace window / after end -> None
        assert!(track.sample(0.2, true).is_none());
        assert!(track.sample(0.8, true).is_some());
        assert!(track.sample(1.25, true).is_none());
    }

    #[test]
    fn parse_file_time_and_find_pb_demo() {
        assert_eq!(parse_file_time("1m05s53"), Some(6553));
        assert_eq!(parse_file_time("0m42s10_2"), Some(4210));
        assert_eq!(parse_file_time("bogus"), None);

        let dir = std::env::temp_dir().join(format!("qsp-ghost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "e1m1_pb_any_normal_0m50s00.dem",
            "e1m1_pb_any_normal_0m42s10.dem",
            "e1m1_pb_any-100_normal_0m45s00.dem",
            "e1m1_pb_100_normal_0m40s00.dem",
            "e1m1_pb_any_hard_0m30s00.dem",
        ] {
            std::fs::write(dir.join(f), b"-1\n").unwrap();
        }
        // any% normal picks 0m42s10 (ignores 100%-only 0m40s00 and hard 0m30s00)
        assert_eq!(find_pb_demo(&dir, "e1m1", 1, false), Some(dir.join("e1m1_pb_any_normal_0m42s10.dem")));
        // 100% normal picks 0m40s00
        assert_eq!(find_pb_demo(&dir, "e1m1", 1, true), Some(dir.join("e1m1_pb_100_normal_0m40s00.dem")));
        // any% hard picks 0m30s00; 100% hard has none
        assert_eq!(find_pb_demo(&dir, "e1m1", 2, false), Some(dir.join("e1m1_pb_any_hard_0m30s00.dem")));
        assert_eq!(find_pb_demo(&dir, "e1m1", 2, true), None);

        // Dedicated ghosts/ file takes priority once saved
        let g = ghost_path(&dir, "e1m1", 1, false);
        std::fs::create_dir_all(g.parent().unwrap()).unwrap();
        std::fs::write(&g, b"-1\n").unwrap();
        assert_eq!(find_pb_demo(&dir, "e1m1", 1, false), Some(g));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
