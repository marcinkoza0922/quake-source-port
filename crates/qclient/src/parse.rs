//! CL_ParseServerMessage and helpers (cl_parse.c, cl_tent.c).

use std::fmt;

use qnet::MsgReader;
use qnet::protocol::*;

use crate::effects::Beam;
use crate::state::*;

/// Host_Error equivalent: the message is unusable and the connection should drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ParseError {}

type R<T = ()> = Result<T, ParseError>;

fn err<T>(s: impl Into<String>) -> R<T> {
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
                    let s = m.string_lossy();
                    self.events.push(ClientEvent::Print(s));
                }
                SVC_CENTERPRINT => {
                    let s = m.string_lossy();
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
                    self.static_entities.push(ClientEntity {
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
                        ..Default::default()
                    });
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
                    let s = m.string_lossy();
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
            let line = line.strip_prefix(b"//").unwrap_or(&line);
            self.events.push(ClientEvent::StuffText(String::from_utf8_lossy(line).into_owned()));
        }
    }

    /// CL_ParseServerInfo.
    fn parse_serverinfo(&mut self, m: &mut MsgReader<'_>) -> R {
        self.clear_state();
        let p = m.long() as u32;
        check_protocol(p)?;
        self.protocol = p;
        self.protocolflags = if p == PROTOCOL_RMQ { m.long() as u32 } else { 0 };
        let mc = m.byte();
        if !(1..=MAX_SCOREBOARD as i32).contains(&mc) {
            return err(format!("Bad maxclients ({mc}) from server"));
        }
        self.maxclients = mc as usize;
        self.scores = vec![Score::default(); self.maxclients];
        self.gametype = m.byte() as u8;
        self.levelname = m.string_lossy();
        for (list, max, what) in [(&mut self.model_precache, MAX_MODELS, "model"), (&mut self.sound_precache, MAX_SOUNDS, "sound")] {
            list.truncate(1);
            loop {
                let s = m.string();
                if s.is_empty() || m.badread {
                    break;
                }
                if list.len() == max {
                    return err(format!("Server sent too many {what} precaches"));
                }
                list.push(String::from_utf8_lossy(s).into_owned());
            }
        }
        if m.badread {
            return err("bad serverinfo");
        }
        self.model_flags = vec![0; self.model_precache.len()];
        let world = self.model_precache.get(1).cloned().unwrap_or_default();
        let base = world.rsplit('/').next().unwrap_or("");
        self.mapname = base.rsplit_once('.').map_or(base, |(a, _)| a).to_string();
        self.entities[0].model = 1;
        // A new serverinfo starts a fresh signon (changelevel / reconnect).
        self.signon = 0;
        self.events.push(ClientEvent::NewMap);
        Ok(())
    }

    /// CL_ParseBaseline.
    fn parse_baseline(&self, m: &mut MsgReader<'_>, version: i32) -> EntityState {
        let pf = self.protocolflags;
        let bits = if version == 2 { m.byte() as u32 } else { 0 };
        let modelindex = if bits & B_LARGEMODEL != 0 { m.short() as u16 as usize } else { m.byte() as u8 as usize };
        let frame = if bits & B_LARGEFRAME != 0 { m.short() as u16 as i32 } else { m.byte() };
        let colormap = m.byte();
        let skin = m.byte();
        let mut origin = [0.0; 3];
        let mut angles = [0.0; 3];
        for i in 0..3 {
            origin[i] = m.coord(pf);
            angles[i] = m.angle(pf);
        }
        let alpha = if bits & B_ALPHA != 0 { m.byte() as u8 } else { ENTALPHA_DEFAULT };
        let scale = if bits & B_SCALE != 0 { m.byte() as u8 } else { ENTSCALE_DEFAULT };
        EntityState { origin, angles, modelindex, frame, colormap, skin, effects: 0, alpha, scale }
    }

    /// CL_ParseUpdate.
    fn parse_update(&mut self, m: &mut MsgReader<'_>, mut bits: u32) -> R {
        if self.signon == SIGNONS - 1 {
            self.signon = SIGNONS;
            self.signon_reply();
        }
        if bits & U_MOREBITS != 0 {
            bits |= (m.byte() as u32 & 0xff) << 8;
        }
        let fitz = self.protocol != PROTOCOL_NETQUAKE;
        if fitz {
            if bits & U_EXTEND1 != 0 {
                bits |= (m.byte() as u32 & 0xff) << 16;
            }
            if bits & U_EXTEND2 != 0 {
                bits |= (m.byte() as u32 & 0xff) << 24;
            }
        }
        let num = if bits & U_LONGENTITY != 0 { m.short() as u16 as i32 } else { m.byte() };
        let (pf, protocol, mtime, nmodels) = (self.protocolflags, self.protocol, self.mtime, self.model_precache.len());
        let ent = self.entity_num(num)?;
        let mut forcelink = ent.msgtime != mtime[1];
        if ent.msgtime + 0.2 < mtime[0] {
            ent.lerpflags |= LERP_RESETANIM;
        }
        ent.msgtime = mtime[0];
        let b = ent.baseline;
        let mut modnum = if bits & U_MODEL != 0 { m.byte() as u8 as usize } else { b.modelindex };
        let prevframe = ent.frame;
        ent.frame = if bits & U_FRAME != 0 { m.byte() } else { b.frame };
        ent.colormap = if bits & U_COLORMAP != 0 { m.byte() } else { b.colormap };
        ent.skin = if bits & U_SKIN != 0 { m.byte() } else { b.skin };
        ent.effects = if bits & U_EFFECTS != 0 { m.byte() } else { b.effects };
        ent.msg_origins[1] = ent.msg_origins[0];
        ent.msg_angles[1] = ent.msg_angles[0];
        let (o1, a1, o2, a2, o3, a3) = (U_ORIGIN1, U_ANGLE1, U_ORIGIN2, U_ANGLE2, U_ORIGIN3, U_ANGLE3);
        for (i, (ob, ab)) in [(o1, a1), (o2, a2), (o3, a3)].into_iter().enumerate() {
            ent.msg_origins[0][i] = if bits & ob != 0 { m.coord(pf) } else { b.origin[i] };
            ent.msg_angles[0][i] = if bits & ab != 0 { m.angle(pf) } else { b.angles[i] };
        }
        if bits & U_STEP != 0 {
            ent.lerpflags |= LERP_MOVESTEP;
            ent.forcelink = true;
        } else {
            ent.lerpflags &= !LERP_MOVESTEP;
        }
        if fitz {
            ent.alpha = if bits & U_ALPHA != 0 { m.byte() as u8 } else { b.alpha };
            ent.scale = if bits & U_SCALE != 0 { m.byte() as u8 } else { b.scale };
            if bits & U_FRAME2 != 0 {
                ent.frame = (ent.frame & 0xff) | ((m.byte() & 0xff) << 8);
            }
            if bits & U_MODEL2 != 0 {
                modnum = (modnum & 0xff) | (((m.byte() & 0xff) as usize) << 8);
            }
            if bits & U_LERPFINISH != 0 {
                ent.lerpfinish = ent.msgtime + m.byte() as f64 / 255.0;
                ent.lerpflags |= LERP_FINISH;
            } else {
                ent.lerpflags &= !LERP_FINISH;
            }
        } else if protocol == PROTOCOL_NETQUAKE {
            if bits & U_TRANS != 0 {
                // Nehahra alpha.
                let a = m.float();
                let alpha = m.float();
                if a == 2.0 {
                    m.float();
                }
                ent.alpha = entalpha_encode(alpha);
            } else {
                ent.alpha = b.alpha;
            }
            ent.scale = b.scale;
        }
        if modnum >= MAX_MODELS {
            return err("CL_ParseModel: bad modnum");
        }
        // Unknown precache index behaves like a NULL model.
        let model = if modnum < nmodels { modnum } else { 0 };
        if model != ent.model {
            ent.model = model;
            if model == 0 {
                forcelink = true;
            } else {
                ent.syncbase = 0.0;
            }
            ent.lerpflags |= LERP_RESETANIM;
        }
        let _ = prevframe; // syncbase for ST_FRAMETIME models is the renderer's concern
        if forcelink {
            ent.msg_origins[1] = ent.msg_origins[0];
            ent.origin = ent.msg_origins[0];
            ent.msg_angles[1] = ent.msg_angles[0];
            ent.angles = ent.msg_angles[0];
            ent.forcelink = true;
        }
        if m.badread {
            return err("CL_ParseUpdate: truncated");
        }
        Ok(())
    }

    /// CL_ParseClientdata.
    fn parse_clientdata(&mut self, m: &mut MsgReader<'_>) {
        let mut bits = m.short() as u16 as u32;
        if bits & SU_EXTEND1 != 0 {
            bits |= (m.byte() as u32 & 0xff) << 16;
        }
        if bits & SU_EXTEND2 != 0 {
            bits |= (m.byte() as u32 & 0xff) << 24;
        }
        self.viewheight = if bits & SU_VIEWHEIGHT != 0 { m.char() as f32 } else { DEFAULT_VIEWHEIGHT };
        self.idealpitch = if bits & SU_IDEALPITCH != 0 { m.char() as f32 } else { 0.0 };
        self.mvelocity[1] = self.mvelocity[0];
        for i in 0..3 {
            self.punchangle[i] = if bits & (SU_PUNCH1 << i) != 0 { m.char() as f32 } else { 0.0 };
            self.mvelocity[0][i] = if bits & (SU_VELOCITY1 << i) != 0 { m.char() as f32 * 16.0 } else { 0.0 };
        }
        if self.punchangles[0] != self.punchangle {
            self.punchangles[1] = self.punchangles[0];
            self.punchangles[0] = self.punchangle;
            self.punchtime = self.time;
        }
        let items = m.long();
        if self.items != items {
            for j in 0..32 {
                if items & (1 << j) != 0 && self.items & (1 << j) == 0 {
                    self.item_gettime[j] = self.time;
                }
            }
            self.items = items;
            self.stats[STAT_ITEMS] = items;
        }
        self.onground = bits & SU_ONGROUND != 0;
        self.inwater = bits & SU_INWATER != 0;
        let s = &mut self.stats;
        s[STAT_WEAPONFRAME] = if bits & SU_WEAPONFRAME != 0 { m.byte() } else { 0 };
        s[STAT_ARMOR] = if bits & SU_ARMOR != 0 { m.byte() } else { 0 };
        s[STAT_WEAPON] = if bits & SU_WEAPON != 0 { m.byte() } else { 0 };
        s[STAT_HEALTH] = m.short();
        s[STAT_AMMO] = m.byte();
        for i in 0..4 {
            s[STAT_SHELLS + i] = m.byte();
        }
        let w = m.byte();
        s[STAT_ACTIVEWEAPON] = if self.standard_quake { w } else { 1i32.wrapping_shl(w as u32) };
        for (bit, stat) in [
            (SU_WEAPON2, STAT_WEAPON),
            (SU_ARMOR2, STAT_ARMOR),
            (SU_AMMO2, STAT_AMMO),
            (SU_SHELLS2, STAT_SHELLS),
            (SU_NAILS2, STAT_NAILS),
            (SU_ROCKETS2, STAT_ROCKETS),
            (SU_CELLS2, STAT_CELLS),
            (SU_WEAPONFRAME2, STAT_WEAPONFRAME),
        ] {
            if bits & bit != 0 {
                s[stat] |= (m.byte() & 0xff) << 8;
            }
        }
        self.viewent.alpha = if bits & SU_WEAPONALPHA != 0 { m.byte() as u8 } else { ENTALPHA_DEFAULT };
        let wm = self.stats[STAT_WEAPON].max(0) as usize;
        if self.viewent.model != wm {
            self.viewent.model = wm;
            self.viewent.lerpflags |= LERP_RESETANIM;
        }
        self.viewent.frame = self.stats[STAT_WEAPONFRAME];
    }

    /// CL_ParseStartSoundPacket.
    fn parse_start_sound(&mut self, m: &mut MsgReader<'_>) -> R {
        let fm = m.byte() as u32;
        let volume = if fm & SND_VOLUME != 0 { m.byte() } else { DEFAULT_SOUND_PACKET_VOLUME };
        let attenuation = if fm & SND_ATTENUATION != 0 { m.byte() as f32 / 64.0 } else { DEFAULT_SOUND_PACKET_ATTENUATION };
        let (ent, channel) = if fm & SND_LARGEENTITY != 0 {
            let e = m.short() as u16 as i32;
            (e, m.byte())
        } else {
            let c = m.short() as u16 as i32;
            (c >> 3, c & 7)
        };
        let sound = if fm & SND_LARGESOUND != 0 { m.short() as u16 as usize } else { m.byte() as u8 as usize };
        if sound >= MAX_SOUNDS {
            return err(format!("CL_ParseStartSoundPacket: {sound} > MAX_SOUNDS"));
        }
        if ent as usize > MAX_EDICTS {
            return err(format!("CL_ParseStartSoundPacket: ent = {ent}"));
        }
        let origin = m.vec3_coord(self.protocolflags);
        self.events.push(ClientEvent::Sound { ent, channel, sound, origin, volume: volume as f32 / 255.0, attenuation });
        Ok(())
    }

    /// CL_ParseBeam.
    fn parse_beam(&mut self, m: &mut MsgReader<'_>, model: &'static str) {
        let pf = self.protocolflags;
        let entity = m.short();
        let start = m.vec3_coord(pf);
        let end = m.vec3_coord(pf);
        let b = Beam { entity, model, start, end, starttime: self.time - 0.001, endtime: self.time + 0.2 };
        let t = self.time;
        let slice = self.beams.as_mut_slice();
        if let Some(x) = slice.iter_mut().find(|x| x.entity == entity) {
            *x = b;
        } else if let Some(x) = slice.iter_mut().find(|x| x.starttime > t || x.endtime < t) {
            *x = b;
        } else {
            let _ = self.beams.insert(b); // "Beam list overflow" when full
        }
    }

    /// CL_ParseTEnt.
    fn parse_tent(&mut self, m: &mut MsgReader<'_>) -> R {
        let pf = self.protocolflags;
        let ty = m.byte() as u8;
        let pos_of = |m: &mut MsgReader<'_>| m.vec3_coord(pf);
        match ty {
            TE_WIZSPIKE => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_WIZSPIKE", pos, 1.0) {
                    self.run_particle_effect(pos, [0.0; 3], 20, 30);
                }
                self.tsound("wizard/hit.wav", pos);
            }
            TE_KNIGHTSPIKE => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_KNIGHTSPIKE", pos, 1.0) {
                    self.run_particle_effect(pos, [0.0; 3], 226, 20);
                }
                self.tsound("hknight/hit.wav", pos);
            }
            TE_SPIKE | TE_SUPERSPIKE => {
                let pos = pos_of(m);
                let name = if ty == TE_SPIKE { "TE_SPIKE" } else { "TE_SUPERSPIKE" };
                if !self.fte_effect(name, pos, 1.0) {
                    self.run_particle_effect(pos, [0.0; 3], 0, if ty == TE_SPIKE { 10 } else { 20 });
                }
                if self.rng.rand() % 5 != 0 {
                    self.tsound("weapons/tink1.wav", pos);
                } else {
                    let s = match self.rng.rand() & 3 {
                        1 => "weapons/ric1.wav",
                        2 => "weapons/ric2.wav",
                        _ => "weapons/ric3.wav",
                    };
                    self.tsound(s, pos);
                }
            }
            TE_GUNSHOT => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_GUNSHOT", pos, 20.0) {
                    self.run_particle_effect(pos, [0.0; 3], 0, 20);
                }
            }
            TE_EXPLOSION => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_EXPLOSION", pos, 1.0) {
                    self.particle_explosion(pos);
                }
                self.explosion_light(pos);
                self.tsound("weapons/r_exp3.wav", pos);
            }
            TE_TAREXPLOSION => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_TAREXPLOSION", pos, 1.0) {
                    self.blob_explosion(pos);
                }
                self.tsound("weapons/r_exp3.wav", pos);
            }
            TE_LIGHTNING1 => self.parse_beam(m, "progs/bolt.mdl"),
            TE_LIGHTNING2 => self.parse_beam(m, "progs/bolt2.mdl"),
            TE_LIGHTNING3 => self.parse_beam(m, "progs/bolt3.mdl"),
            TE_BEAM => self.parse_beam(m, "progs/beam.mdl"),
            TE_LAVASPLASH => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_LAVASPLASH", pos, 1.0) {
                    self.lava_splash(pos);
                }
            }
            TE_TELEPORT => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_TELEPORT", pos, 1.0) {
                    self.teleport_splash(pos);
                }
            }
            TE_EXPLOSION2 => {
                let pos = pos_of(m);
                let start = m.byte();
                let len = m.byte();
                if !self.fte_effect(&format!("TE_EXPLOSION2_{start}_{len}"), pos, 1.0) {
                    self.particle_explosion2(pos, start, len);
                }
                self.explosion_light(pos);
                self.tsound("weapons/r_exp3.wav", pos);
            }
            crate::fte::TEDP_PARTICLERAIN => self.parse_dp_weather(m, false),
            crate::fte::TEDP_PARTICLESNOW => self.parse_dp_weather(m, true),
            _ => return err(format!("CL_ParseTEnt: bad type {ty}")),
        }
        Ok(())
    }

    fn tsound(&mut self, name: &'static str, origin: [f32; 3]) {
        self.events.push(ClientEvent::TempSound { name, origin });
    }

    fn explosion_light(&mut self, pos: [f32; 3]) {
        let die = self.time + 0.5;
        let dl = self.alloc_dlight(0);
        dl.origin = pos;
        dl.radius = 350.0;
        dl.die = die;
        dl.decay = 300.0;
    }
}

fn check_protocol(p: u32) -> R {
    if p != PROTOCOL_NETQUAKE && p != PROTOCOL_FITZQUAKE && p != PROTOCOL_RMQ {
        return err(format!("Server returned version {p}, not 15 or 666 or 999"));
    }
    Ok(())
}
