use super::*;

impl QwState {
    /// CL_ParseServerData.
    pub(super) fn parse_serverdata(&mut self, m: &mut MsgReader<'_>, out: &mut Parsed) -> R {
        self.clear_level();
        // FTE/MVD extension headers come before the version; we ask for none.
        let protover = m.long();
        if protover != PROTOCOL_VERSION {
            return err(format!("Server returned version {protover}, not {PROTOCOL_VERSION}"));
        }
        self.servercount = m.long();
        self.gamedir = m.string_lossy();
        let pn = m.byte();
        self.spectator = pn & 128 != 0;
        self.playernum = (pn & 127) as usize;
        if self.playernum >= MAX_CLIENTS {
            return err(format!("bad playernum {}", self.playernum));
        }
        self.levelname = m.string_lossy();
        let mv = &mut self.movevars;
        for f in [
            &mut mv.gravity,
            &mut mv.stopspeed,
            &mut mv.maxspeed,
            &mut mv.spectatormaxspeed,
            &mut mv.accelerate,
            &mut mv.airaccelerate,
            &mut mv.wateraccelerate,
            &mut mv.friction,
            &mut mv.waterfriction,
            &mut mv.entgravity,
        ] {
            *f = m.float();
        }
        self.stage = Stage::Precache;
        out.actions.push(QwAction::Cmd(format!("soundlist {} 0", self.servercount)));
        Ok(())
    }

    /// CL_ParseSoundlist / CL_ParseModellist.
    pub(super) fn parse_list(&mut self, m: &mut MsgReader<'_>, models: bool, out: &mut Parsed) -> R {
        let mut n = m.byte().max(0) as usize;
        let list = if models { &mut self.model_names } else { &mut self.sound_names };
        loop {
            let s = m.string_lossy();
            if s.is_empty() || m.badread {
                break;
            }
            n += 1;
            if n >= 256 {
                return err("Server sent too many precaches");
            }
            if list.len() <= n {
                list.resize(n + 1, String::new());
            }
            list[n] = s;
        }
        let next = m.byte();
        if next > 0 {
            let what = if models { "modellist" } else { "soundlist" };
            out.actions.push(QwAction::Cmd(format!("{what} {} {next}", self.servercount)));
            return Ok(());
        }
        if !models {
            out.actions.push(QwAction::Cmd(format!("modellist {} 0", self.servercount)));
            return Ok(());
        }
        let find = |name: &str| self.model_names.iter().position(|n| n == name).unwrap_or(0);
        self.playerindex = find("progs/player.mdl");
        self.spikeindex = find("progs/spike.mdl");
        out.actions.push(QwAction::Precache);
        Ok(())
    }

    /// Files the host must have before [`QwState::finish_precache`]:
    /// models (not inline `*N` ones) then sounds.
    pub fn needed_files(&self) -> Vec<String> {
        let mut v: Vec<String> = self.model_names.iter().skip(1).filter(|n| !n.is_empty() && !n.starts_with('*')).cloned().collect();
        v.extend(self.sound_names.iter().skip(1).filter(|n| !n.is_empty()).map(|n| format!("sound/{n}")));
        v
    }

    /// Lists and downloads are done: the NQ serverinfo (signon 1) for the
    /// client, and the `prespawn` command with the map's checksum2.
    pub fn finish_precache(&mut self, map_checksum2: u32) -> (Vec<u8>, String) {
        let mut b = Nq::new();
        b.byte(nq::SVC_SERVERINFO);
        b.0.long(nq::PROTOCOL_RMQ as i32);
        b.0.long(NQ_FLAGS as i32);
        b.byte(MAX_CLIENTS as u8);
        b.byte(if self.deathmatch() { nq::GAME_DEATHMATCH } else { nq::GAME_COOP });
        b.string(&self.levelname);
        for n in self.model_names.iter().skip(1) {
            b.string(n);
        }
        b.string("");
        for n in self.sound_names.iter().skip(1) {
            b.string(n);
        }
        b.string("");
        self.nq_ready = true;
        self.emit_cdtrack(&mut b);
        b.byte(nq::SVC_SETVIEW);
        b.0.short(self.playernum as i32 + 1);
        for i in 0..MAX_CLIENTS {
            self.emit_player(&mut b, i);
            self.emit_frags(&mut b, i);
        }
        self.nq_signon = 1;
        b.byte(nq::SVC_SIGNONNUM);
        b.byte(1);
        self.stage = Stage::Signon;
        (b.0.data, format!("prespawn {} 0 {}", self.servercount, map_checksum2 as i32))
    }
}
