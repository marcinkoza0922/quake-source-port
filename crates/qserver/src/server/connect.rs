use super::*;

impl Server {
    /// Client-side half of the signon handshake for an in-process client.
    pub(super) fn local_signon(&mut self, i: usize) {
        self.flush_client(i);
        self.client_command(i, "prespawn");
        self.flush_client(i);
        self.client_command(i, "spawn");
        self.flush_client(i);
        self.client_command(i, "begin");
    }

    /// SV_SendServerinfo.
    pub(crate) fn send_serverinfo(&mut self, i: usize) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let sv = &c.sv;
        let mut m = std::mem::take(&mut c.clients[i].message);
        // FTE extension handshake (SV_SendServerinfo): ask a remote client
        // with `cmd pext` first; it answers with `pext`, which resends this.
        let offer = c.cvars.value_of("sv_protocol_pext2") as u32 & qnet::delta::PEXT2_SUPPORTED;
        let cl = &mut c.clients[i];
        if offer == 0 {
            cl.pext_known = false;
            cl.pext2 = 0;
        } else if cl.local {
            cl.pext_known = true;
            cl.pext2 = offer;
        } else if !cl.pext_known {
            m.byte(SVC_STUFFTEXT);
            m.string(b"cmd pext\n");
            cl.message = m;
            cl.sendsignon = Signon::Flush;
            cl.spawned = false;
            return;
        }
        cl.pext2 &= offer;
        let pext2 = cl.pext2;
        cl.delta = (pext2 & qnet::delta::PEXT2_REPLACEMENTDELTAS != 0).then(Box::default);
        m.byte(SVC_PRINT);
        m.string(format!("\x02\nFITZQUAKE 0.85 SERVER ({} CRC)\n", l.vm.progs.crc).as_bytes());
        m.byte(SVC_SERVERINFO);
        if pext2 != 0 {
            m.long(qnet::delta::PROTOCOL_FTE_PEXT2 as i32);
            m.long(pext2 as i32);
        }
        m.long(sv.protocol as i32);
        if sv.protocol == PROTOCOL_RMQ {
            m.long(sv.protocolflags as i32);
        }
        m.byte(c.clients.len() as i32);
        let dm = c.cvars.value_of("coop") == 0.0 && c.cvars.value_of("deathmatch") != 0.0;
        m.byte(if dm { GAME_DEATHMATCH } else { GAME_COOP });
        let msg_ofs = l.vm.field_ofs("message");
        let message = msg_ofs.and_then(|o| l.vm.ent_str(0, o).ok()).unwrap_or(b"").to_vec();
        m.string(&message);
        for (k, s) in sv.model_precache.iter().enumerate().skip(1) {
            if sv.protocol != PROTOCOL_NETQUAKE || k < 256 {
                m.string(s.as_bytes());
            }
        }
        m.byte(0);
        for (k, s) in sv.sound_precache.iter().enumerate().skip(1) {
            if sv.protocol != PROTOCOL_NETQUAKE || k < 256 {
                m.string(s.as_bytes());
            }
        }
        m.byte(0);
        if c.fte_particles() {
            for (k, name) in sv.particle_precache.iter().enumerate().skip(1) {
                crate::particles::write_precache(&mut m, k, name);
            }
        }
        let sounds = l.vm.field_ofs("sounds").map_or(0, |o| l.vm.ent_f(0, o) as i32);
        m.byte(SVC_CDTRACK);
        m.byte(sounds);
        m.byte(sounds);
        m.byte(SVC_SETVIEW);
        m.short(i as i32 + 1);
        m.byte(SVC_SIGNONNUM);
        m.byte(1);
        let cl = &mut c.clients[i];
        cl.message = m;
        cl.sendsignon = Signon::Flush;
        cl.spawned = false;
    }

    /// SV_ConnectClient for slot `i`.
    pub fn connect_client(&mut self, i: usize) {
        self.connect_slot(i, false);
    }

    pub(super) fn connect_slot(&mut self, i: usize, local: bool) {
        if i >= self.core.maxclients() || self.level.is_none() {
            return;
        }
        let cl = &mut self.core.clients[i];
        *cl = Client { active: true, local, name: "unconnected".into(), ..Client::default() };
        let l = self.level.as_mut().unwrap();
        let c = &mut self.core;
        let g = c.sv.globals.clone();
        if c.sv.loadgame {
            c.clients[i].spawn_parms = c.sv.loadgame_parms;
        } else {
            { let fnum = l.vm.g_i(g.f_set_new_parms); exec(&mut l.vm, c, &mut l.world, fnum); }
            for k in 0..NUM_SPAWN_PARMS {
                c.clients[i].spawn_parms[k] = l.vm.g_f(g.parm1 + k);
            }
        }
        self.send_serverinfo(i);
    }

    /// Connect an in-process client and run the whole signon sequence
    /// (prespawn/spawn/begin). Returns the slot.
    pub fn connect_local(&mut self) -> Option<usize> {
        self.connect_local_as("player", 0)
    }

    /// [`Server::connect_local`] with the name and colours set before the
    /// signon, so ClientConnect announces the right name.
    pub fn connect_local_as(&mut self, name: &str, colors: i32) -> Option<usize> {
        let i = self.core.clients.iter().position(|c| !c.active)?;
        self.connect_slot(i, true);
        if !self.core.clients[i].active {
            return None;
        }
        self.core.clients[i].name = name.into();
        self.core.clients[i].colors = colors;
        self.local_signon(i);
        Some(i)
    }

    /// Set the input applied each frame (clc_move).
    pub fn set_move(&mut self, i: usize, mv: ClientMove) {
        if let Some(c) = self.core.clients.get_mut(i) {
            c.cmd = mv;
            c.new_move = true;
        }
    }

    /// SV_ReadClientMove's ping: `client_time` is the server time the client
    /// had last received (cl.mtime[0]) when it sent the move.
    pub fn record_ping(&mut self, i: usize, client_time: f32) {
        let now = self.level.as_ref().map_or(0.0, |l| l.vm.time) as f32;
        if let Some(c) = self.core.clients.get_mut(i) {
            let k = c.num_pings % crate::client::NUM_PING_TIMES;
            c.ping_times[k] = (now - client_time).max(0.0);
            c.num_pings += 1;
        }
    }

    /// Queue a clc_stringcmd from client `i` (processed next frame, or now via [`Server::client_command`]).
    pub fn queue_client_command(&mut self, i: usize, s: &str) {
        if let Some(c) = self.core.clients.get_mut(i) {
            c.stringcmds.push_back(s.to_string());
        }
    }

    /// Drain the delivered reliable messages of client `i`, in order. Each is
    /// one network message (never merged, so demos stay under MAX_MSGLEN).
    pub fn take_reliable(&mut self, i: usize) -> Vec<Vec<u8>> {
        self.core.clients.get_mut(i).map(|c| std::mem::take(&mut c.reliable_out)).unwrap_or_default()
    }

    /// Drain the delivered unreliable datagrams of client `i`.
    pub fn take_datagrams(&mut self, i: usize) -> Vec<Vec<u8>> {
        self.core.clients.get_mut(i).map(|c| c.datagrams.drain(..).collect()).unwrap_or_default()
    }

    /// Edict of client `i` (i + 1).
    pub fn client_edict(i: usize) -> u32 {
        i as u32 + 1
    }

    /// SV_DropClient.
    pub fn drop_client(&mut self, i: usize, crash: bool) {
        if !self.core.clients.get(i).is_some_and(|c| c.active) {
            return;
        }
        if !crash {
            let cl = &mut self.core.clients[i];
            cl.message.byte(SVC_DISCONNECT);
            let m = std::mem::take(&mut cl.message.data);
            cl.reliable_out.push(m);
            if cl.spawned
                && let Some(l) = &mut self.level
            {
                let c = &mut self.core;
                let g = c.sv.globals.clone();
                let saveself = l.vm.self_ent();
                l.vm.set_self(i as u32 + 1);
                { let fnum = l.vm.g_i(g.f_client_disconnect); exec(&mut l.vm, c, &mut l.world, fnum); }
                l.vm.set_self(saveself);
            }
        }
        let cl = &mut self.core.clients[i];
        cl.active = false;
        cl.spawned = false;
        cl.name.clear();
        cl.old_frags = -999999;
    }
}
