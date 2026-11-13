//! CL_ParseServerMessage and helpers (cl_parse.c, cl_tent.c).

use std::fmt;

use qnet::MsgReader;
use qnet::protocol::*;

use crate::effects::Beam;
use crate::state::*;
mod server;
mod effects;

/// Host_Error equivalent: the message is unusable and the connection should drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ParseError {}

pub(crate) type R<T = ()> = Result<T, ParseError>;

pub(crate) fn err<T>(s: impl Into<String>) -> R<T> {
    Err(ParseError(s.into()))
}

impl ClientState {
    /// CL_ParseServerMessage over one complete message (reliable or datagram).
    pub fn parse_server_message(&mut self, data: &[u8]) -> R {
        let mut m = MsgReader::new(data);
        let mut lastcmd = 0;
        loop {
            if m.badread {
                return err(format!("CL_ParseServerMessage: Bad server message (after svc {lastcmd})"));
            }
            if m.at_end() {
                if !self.stuffcmdbuf.is_empty() && data.len() < 512 {
                    self.parse_stuff_text(b"\n");
                }
                return Ok(());
            }
            let cmd = m.byte() as u8;
            if cmd as u32 & U_SIGNAL != 0 {
                self.parse_update(&mut m, cmd as u32 & 127)?;
                lastcmd = cmd;
                continue;
            }
            let pf = self.protocolflags;
            match cmd {
                SVC_NOP => {}
                SVC_TIME => {
                    self.mtime[1] = self.mtime[0];
                    self.mtime[0] = m.float() as f64;
                    self.fixangle = false;
                }
                SVC_CLIENTDATA => self.parse_clientdata(&mut m),
                SVC_VERSION => {
                    let i = m.long() as u32;
                    check_protocol(i)?;
                    self.protocol = i;
                }
                SVC_DISCONNECT => {
                    self.events.push(ClientEvent::Disconnect);
                    self.signon = 0;
                    return Ok(());
                }
                SVC_PRINT => {
                    let s = qcore::text::decode(m.string());
                    self.events.push(ClientEvent::Print(s));
                }
                SVC_CENTERPRINT => {
                    let s = qcore::text::decode(m.string());
                    self.center_print(s);
                }
                SVC_STUFFTEXT => {
                    let s = m.string().to_vec();
                    self.parse_stuff_text(&s);
                }
                SVC_DAMAGE => {
                    let armor = m.byte();
                    let blood = m.byte();
                    let from = m.vec3_coord(pf);
                    self.events.push(ClientEvent::Damage { armor, blood, from });
                }
                SVC_SERVERINFO => self.parse_serverinfo(&mut m)?,
                SVC_SETANGLE => {
                    for i in 0..3 {
                        self.viewangles[i] = m.angle(pf);
                    }
                    self.fixangle = true;
                }
                SVC_SETVIEW => self.viewentity = m.short().max(0) as usize,
                SVC_LIGHTSTYLE => {
                    let i = m.byte();
                    let s = m.string_lossy();
                    if !(0..MAX_LIGHTSTYLES as i32).contains(&i) {
                        return err("svc_lightstyle > MAX_LIGHTSTYLES");
                    }
                    self.lightstyles[i as usize] = s;
                    self.events.push(ClientEvent::LightStyle(i as usize));
                }
                SVC_SOUND => self.parse_start_sound(&mut m)?,
                SVC_STOPSOUND => {
                    let i = m.short();
                    self.events.push(ClientEvent::StopSound { ent: i >> 3, channel: i & 7 });
                }
                SVC_UPDATENAME => {
                    let i = self.score_slot(m.byte())?;
                    self.scores[i].name = m.string_lossy();
                }
                SVC_UPDATEFRAGS => {
                    let i = self.score_slot(m.byte())?;
                    self.scores[i].frags = m.short();
                }
                SVC_UPDATECOLORS => {
                    let i = self.score_slot(m.byte())?;
                    self.scores[i].colors = m.byte();
                }
                SVC_PARTICLE => {
                    let org = m.vec3_coord(pf);
                    let dir = [m.char() as f32 / 16.0, m.char() as f32 / 16.0, m.char() as f32 / 16.0];
                    let msgcount = m.byte();
                    let color = m.byte();
                    let count = if msgcount == 255 { 1024 } else { msgcount };
                    if color == 73 {
                        self.blood_hits.push(org);
                    }
                    self.run_particle_effect(org, dir, color, count);
                }
                SVC_SPAWNBASELINE | SVC_SPAWNBASELINE2 => {
                    let i = m.short();
                    let b = self.parse_baseline(&mut m, if cmd == SVC_SPAWNBASELINE { 1 } else { 2 });
                    self.entity_num(i)?.baseline = b;
                }
                SVC_SPAWNSTATIC | SVC_SPAWNSTATIC2 => {
                    if self.static_entities.len() >= MAX_STATIC_ENTITIES {
                        return err("Too many static entities");
                    }
                    let b = self.parse_baseline(&mut m, if cmd == SVC_SPAWNSTATIC { 1 } else { 2 });
                    self.static_entities.push(static_entity(b));
                }
                crate::delta::SVCFTE_SPAWNBASELINE2 if self.pext2 & crate::delta::PEXT2_REPLACEMENTDELTAS != 0 => {
                    let i = crate::delta::read_entity(&mut m, self.pext2);
                    let b = self.parse_baseline_fte(&mut m)?;
                    self.entity_num(i as i32)?.baseline = b;
                }
                crate::delta::SVCFTE_SPAWNSTATIC2 if self.pext2 & crate::delta::PEXT2_REPLACEMENTDELTAS != 0 => {
                    if self.static_entities.len() >= MAX_STATIC_ENTITIES {
                        return err("Too many static entities");
                    }
                    let b = self.parse_baseline_fte(&mut m)?;
                    self.static_entities.push(static_entity(b));
                }
                crate::delta::SVCFTE_UPDATEENTITIES if self.pext2 & crate::delta::PEXT2_REPLACEMENTDELTAS != 0 => {
                    self.parse_entities_update(&mut m)?;
                }
                crate::delta::SVCDP_UPDATESTATBYTE | crate::delta::SVCFTE_UPDATESTATFLOAT | crate::delta::SVCFTE_UPDATESTATSTRING
                    if self.pext2 & crate::delta::PEXT2_REPLACEMENTDELTAS != 0 =>
                {
                    let i = m.byte();
                    let v = match cmd {
                        crate::delta::SVCDP_UPDATESTATBYTE => Some(m.byte()),
                        crate::delta::SVCFTE_UPDATESTATFLOAT => Some(m.float() as i32),
                        _ => {
                            m.string();
                            None
                        }
                    };
                    if let Some(v) = v
                        && (0..MAX_CL_STATS as i32).contains(&i)
                    {
                        self.stats[i as usize] = v;
                    }
                }
                SVC_TEMP_ENTITY => self.parse_tent(&mut m)?,
                SVC_SETPAUSE => {
                    self.paused = m.byte() != 0;
                    self.events.push(ClientEvent::Pause(self.paused));
                }
                SVC_SIGNONNUM => {
                    let i = m.byte();
                    if i <= self.signon as i32 {
                        return err(format!("Received signon {i} when at {}", self.signon));
                    }
                    self.signon = i as u32;
                    self.signon_reply();
                }
                SVC_KILLEDMONSTER => self.stats[STAT_MONSTERS] += 1,
                SVC_FOUNDSECRET => self.stats[STAT_SECRETS] += 1,
                SVC_UPDATESTAT => {
                    let i = m.byte();
                    let v = m.long();
                    if !(0..MAX_CL_STATS as i32).contains(&i) {
                        return err(format!("svc_updatestat: {i} is invalid"));
                    }
                    self.stats[i as usize] = v;
                }
                SVC_SPAWNSTATICSOUND | SVC_SPAWNSTATICSOUND2 => {
                    let origin = m.vec3_coord(pf);
                    let sound = if cmd == SVC_SPAWNSTATICSOUND2 { m.short() as u16 as usize } else { m.byte() as u8 as usize };
                    let volume = m.byte();
                    let attenuation = m.byte();
                    let s = StaticSound { origin, sound, volume, attenuation };
                    self.static_sounds.push(s);
                    self.events.push(ClientEvent::StaticSound(s));
                }
                SVC_CDTRACK => {
                    self.cdtrack = m.byte() as u8;
                    self.looptrack = m.byte() as u8;
                    self.events.push(ClientEvent::CdTrack { track: self.cdtrack, loop_track: self.looptrack });
                }
                SVC_INTERMISSION => {
                    self.intermission = 1;
                    self.completed_time = self.time;
                }
                SVC_FINALE | SVC_CUTSCENE => {
                    self.intermission = if cmd == SVC_FINALE { 2 } else { 3 };
                    self.completed_time = self.time;
                    let s = qcore::text::decode(m.string());
                    self.center_print(s);
                }
                SVC_SELLSCREEN => self.events.push(ClientEvent::SellScreen),
                SVC_SKYBOX => {
                    self.skybox = m.string_lossy();
                    self.events.push(ClientEvent::Skybox(self.skybox.clone()));
                }
                SVC_BF => self.events.push(ClientEvent::BonusFlash),
                SVC_FOG => {
                    let density = m.byte() as f32 / 255.0;
                    let color = [m.byte() as f32 / 255.0, m.byte() as f32 / 255.0, m.byte() as f32 / 255.0];
                    let time = (m.short() as f32 / 100.0).max(0.0);
                    self.fog = Fog { density, color, time };
                    self.events.push(ClientEvent::Fog(self.fog));
                }
                SVC_ACHIEVEMENT => {
                    let s = m.string_lossy();
                    self.events.push(ClientEvent::Achievement(s));
                }
                SVC_LOCALSOUND => {
                    let fm = m.byte() as u32;
                    let sound = if fm & SND_LARGESOUND != 0 { m.short() as u16 as usize } else { m.byte() as u8 as usize };
                    if sound >= MAX_SOUNDS {
                        return err(format!("CL_ParseLocalSound: {sound} > MAX_SOUNDS"));
                    }
                    self.events.push(ClientEvent::LocalSound { sound });
                }
                crate::fte::SVCDP_PRECACHE => self.parse_dp_precache(&mut m),
                crate::fte::SVCDP_TRAILPARTICLES | crate::fte::SVCDP_POINTPARTICLES | crate::fte::SVCDP_POINTPARTICLES1 => {
                    self.parse_dp_particles(&mut m, cmd)
                }
                _ => return err(format!("Illegible server message {cmd} (previous was {lastcmd})")),
            }
            lastcmd = cmd;
        }
    }

    fn center_print(&mut self, s: String) {
        self.centerprint = s.clone();
        self.centerprint_time = self.time;
        self.events.push(ClientEvent::CenterPrint(s));
    }

    fn score_slot(&self, i: i32) -> R<usize> {
        if i < 0 || i as usize >= self.maxclients {
            return err("CL_ParseServerMessage: svc_update* > MAX_SCOREBOARD");
        }
        Ok(i as usize)
    }

    /// CL_EntityNum: grows the entity array as needed.
    pub fn entity_num(&mut self, num: i32) -> R<&mut ClientEntity> {
        if num < 0 || num as usize >= MAX_EDICTS {
            return err(format!("CL_EntityNum: {num} is an invalid number"));
        }
        let n = num as usize;
        if n >= self.entities.len() {
            self.entities.resize_with(n + 1, ClientEntity::default);
        }
        Ok(&mut self.entities[n])
    }

    /// CL_ParseStuffText: split into complete lines (ProQuake 0x01 prefixes stripped).
    fn parse_stuff_text(&mut self, msg: &[u8]) {
        self.stuffcmdbuf.extend_from_slice(msg);
        while let Some(nl) = self.stuffcmdbuf.iter().position(|&c| c == b'\n') {
            let line: Vec<u8> = self.stuffcmdbuf.drain(..=nl).collect();
            if line[0] == 0x01 && self.protocol == PROTOCOL_NETQUAKE {
                continue;
            }
            // Comment transport keeps classic clients compatible and bypasses
            // command execution entirely for these narrowly parsed events.
            match line.as_slice() {
                b"//qhit 0\n" => {
                    self.events.push(ClientEvent::HitFeedback { kill: false });
                    continue;
                }
                b"//qhit 1\n" => {
                    self.events.push(ClientEvent::HitFeedback { kill: true });
                    continue;
                }
                _ => {}
            }
            if let Some(args) = line.strip_prefix(b"//qsp_portalfx ") {
                if let Some(fx) = crate::PortalEffect::parse(&String::from_utf8_lossy(args)) {
                    self.events.push(ClientEvent::PortalEffect(fx));
                }
                continue;
            }
            if let Some(args) = line.strip_prefix(b"//qsp_sticky ") {
                self.parse_sticky(&String::from_utf8_lossy(args));
                continue;
            }
            if let Some(args) = line.strip_prefix(b"//qsp_portal ") {
                self.parse_portal(&String::from_utf8_lossy(args));
                continue;
            }
            let line = line.strip_prefix(b"//").unwrap_or(&line);
            self.events.push(ClientEvent::StuffText(String::from_utf8_lossy(line).into_owned()));
        }
    }

}

/// A static entity from its baseline (CL_ParseStatic).
fn static_entity(b: EntityState) -> ClientEntity {
    ClientEntity {
        baseline: b,
        model: b.modelindex,
        lerpflags: LERP_RESETANIM,
        frame: b.frame,
        skin: b.skin,
        effects: b.effects,
        alpha: b.alpha,
        scale: b.scale,
        origin: b.origin,
        angles: b.angles,
        msg_origins: [b.origin; 2],
        msg_angles: [b.angles; 2],
        traileffect: b.traileffect,
        emiteffect: b.emiteffect,
        ..Default::default()
    }
}

/// FTE sound flags (parsed, not used).
const SND_FTE_MOREFLAGS: u32 = 1 << 2;
const SND_DP_PITCH: u32 = 1 << 5;
const SND_FTE_TIMEOFS: u32 = 1 << 6;
const SND_FTE_PITCHADJ: u32 = 1 << 7;
const SND_FTE_VELOCITY: u32 = 1 << 8;

fn check_protocol(p: u32) -> R {
    if p != PROTOCOL_NETQUAKE && p != PROTOCOL_FITZQUAKE && p != PROTOCOL_RMQ {
        return err(format!("Server returned version {p}, not 15 or 666 or 999"));
    }
    Ok(())
}
