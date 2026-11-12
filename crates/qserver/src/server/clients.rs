use super::*;

impl Server {
    /// SV_CreateBaseline.
    pub(super) fn create_baseline(&mut self) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let vm = &l.vm;
        let f = c.sv.fields.clone();
        let maxclients = c.maxclients() as u32;
        let player = c.model_index(b"progs/player.mdl");
        for e in 0..vm.num_edicts() as u32 {
            if vm.is_free(e) {
                continue;
            }
            if e > maxclients && vm.ent_f(e, f.modelindex) == 0.0 {
                continue;
            }
            let protocol = c.sv.protocol;
            let mut b = Baseline {
                origin: vm.ent_v(e, f.origin),
                angles: vm.ent_v(e, f.angles),
                frame: vm.ent_f(e, f.frame) as i32,
                skin: vm.ent_f(e, f.skin) as i32,
                scale: ENTSCALE_DEFAULT,
                ..Baseline::default()
            };
            if e > 0 && e <= maxclients {
                b.colormap = e as i32;
                b.modelindex = player;
                b.alpha = ENTALPHA_DEFAULT;
            } else {
                b.modelindex = c.model_index(vm.ent_str(e, f.model).unwrap_or(b""));
                b.alpha = match f.alpha {
                    Some(o) => entalpha_encode(vm.ent_f(e, o)),
                    None => c.ext(e).alpha,
                };
                if protocol == PROTOCOL_RMQ
                    && let Some(o) = f.scale
                {
                    b.scale = entscale_encode(vm.ent_f(e, o));
                }
            }
            let mut bits = 0;
            if protocol == PROTOCOL_NETQUAKE {
                if b.modelindex & 0xff00 != 0 {
                    b.modelindex = 0;
                }
                if b.frame & 0xff00 != 0 {
                    b.frame = 0;
                }
                b.alpha = ENTALPHA_DEFAULT;
                b.scale = ENTSCALE_DEFAULT;
            } else {
                if b.modelindex & 0xff00 != 0 {
                    bits |= B_LARGEMODEL;
                }
                if b.frame & 0xff00 != 0 {
                    bits |= B_LARGEFRAME;
                }
                if b.alpha != ENTALPHA_DEFAULT {
                    bits |= B_ALPHA;
                }
                if b.scale != ENTSCALE_DEFAULT {
                    bits |= B_SCALE;
                }
            }
            c.ext(e).baseline = b;
            let pf = c.sv.protocolflags;
            let m = c.signon(35);
            m.byte(if bits != 0 { SVC_SPAWNBASELINE2 } else { SVC_SPAWNBASELINE });
            m.short(e as i32);
            if bits != 0 {
                m.byte(bits);
            }
            if bits & B_LARGEMODEL != 0 { m.short(b.modelindex) } else { m.byte(b.modelindex) }
            if bits & B_LARGEFRAME != 0 { m.short(b.frame) } else { m.byte(b.frame) }
            m.byte(b.colormap);
            m.byte(b.skin);
            for k in 0..3 {
                m.coord(b.origin[k], pf);
                m.angle(b.angles[k], pf);
            }
            if bits & B_ALPHA != 0 {
                m.byte(b.alpha as i32);
            }
            if bits & B_SCALE != 0 {
                m.byte(b.scale as i32);
            }
        }
    }

    /// SV_SaveSpawnparms.
    pub(crate) fn save_spawn_parms(&mut self) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let g = c.sv.globals.clone();
        c.serverflags = l.vm.g_f(g.serverflags);
        for i in 0..c.maxclients() {
            if !c.clients[i].active {
                continue;
            }
            l.vm.set_self(i as u32 + 1);
            { let fnum = l.vm.g_i(g.f_set_change_parms); exec(&mut l.vm, c, &mut l.world, fnum); }
            for k in 0..NUM_SPAWN_PARMS {
                c.clients[i].spawn_parms[k] = l.vm.g_f(g.parm1 + k);
            }
        }
    }

    /// Host_Spawn_f for client `i`.
    pub(crate) fn client_spawn(&mut self, i: usize) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        if c.clients[i].spawned {
            c.print("Spawn not valid -- already spawned\n");
            return;
        }
        let g = c.sv.globals.clone();
        let f = c.sv.fields.clone();
        let e = i as u32 + 1;
        let vm = &mut l.vm;
        if c.sv.loadgame {
            c.sv.paused = false;
        } else {
            vm.edict_mut(e).fill(0);
            vm.set_ent_f(e, f.colormap, e as f32);
            vm.set_ent_f(e, f.team, ((c.clients[i].colors & 15) + 1) as f32);
            let h = vm.strings.engine(c.clients[i].name.as_bytes());
            vm.set_ent_i(e, f.netname, h);
            for k in 0..NUM_SPAWN_PARMS {
                vm.set_g_f(g.parm1 + k, c.clients[i].spawn_parms[k]);
            }
            vm.set_g_f(g.time, vm.time as f32);
            vm.set_self(e);
            exec(vm, c, &mut l.world, vm.g_i(g.f_client_connect));
            exec(vm, c, &mut l.world, vm.g_i(g.f_put_client_in_server));
        }

        let pf = c.sv.protocolflags;
        let mut m = std::mem::take(&mut c.clients[i].message);
        m.clear();
        m.byte(SVC_TIME);
        m.float(vm.time as f32);
        for (k, cl) in c.clients.iter().enumerate() {
            m.byte(SVC_UPDATENAME);
            m.byte(k as i32);
            m.string(cl.name.as_bytes());
            m.byte(SVC_UPDATEFRAGS);
            m.byte(k as i32);
            m.short(cl.old_frags);
            m.byte(SVC_UPDATECOLORS);
            m.byte(k as i32);
            m.byte(cl.colors);
        }
        for (k, s) in c.sv.lightstyles.iter().enumerate() {
            m.byte(SVC_LIGHTSTYLE);
            m.byte(k as i32);
            m.string(s);
        }
        for (stat, ofs) in [
            (STAT_TOTALSECRETS, g.total_secrets),
            (STAT_TOTALMONSTERS, g.total_monsters),
            (STAT_SECRETS, g.found_secrets),
            (STAT_MONSTERS, g.killed_monsters),
        ] {
            m.byte(SVC_UPDATESTAT);
            m.byte(stat);
            m.long(vm.g_f(ofs) as i32);
        }
        m.byte(SVC_SETANGLE);
        let a = vm.ent_v(e, f.angles);
        m.angle(a[0], pf);
        m.angle(a[1], pf);
        m.angle(0.0, pf);
        crate::send::write_clientdata(vm, c, &mut l.world, e, &mut m);
        m.byte(SVC_SIGNONNUM);
        m.byte(3);
        let cl = &mut c.clients[i];
        cl.message = m;
        cl.sendsignon = Signon::Flush;
    }

    /// Process a client string command (SV_ReadClientMessage's clc_stringcmd).
    pub fn client_command(&mut self, i: usize, s: &str) {
        if !self.core.clients.get(i).is_some_and(|c| c.active) {
            return;
        }
        let low = s.trim_start().to_ascii_lowercase();
        if low == "pext" || low.starts_with("pext ") {
            self.client_pext(i, &low);
            return;
        }
        let is_signon = low.starts_with("spawn") || low.starts_with("begin") || low.starts_with("prespawn");
        if !is_signon
            && let (Some(fnum), Some(l)) = (self.core.sv.parse_client_command, &mut self.level)
        {
            let c = &mut self.core;
            let h = l.vm.strings.temp(s.as_bytes());
            l.vm.set_g_i(ofs_parm(0), h);
            l.vm.set_g_f(c.sv.globals.time, l.vm.time as f32);
            l.vm.set_self(i as u32 + 1);
            exec(&mut l.vm, c, &mut l.world, fnum as i32);
            self.process_queued_client_cmds();
            return;
        }
        self.exec_client_string(i, s);
    }

    /// SV_Pext_f: the client's `key value` list of protocol extensions, in
    /// reply to `cmd pext`. Answers once per connection with the serverinfo.
    pub(super) fn client_pext(&mut self, i: usize, args: &str) {
        let cl = &mut self.core.clients[i];
        if cl.pext_known || cl.spawned {
            return;
        }
        let num = |s: &str| {
            let s = s.trim();
            match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                Some(h) => u32::from_str_radix(h, 16).ok(),
                None => s.parse::<i64>().ok().map(|v| v as u32),
            }
            .unwrap_or(0)
        };
        let words: Vec<&str> = args.split_whitespace().skip(1).collect();
        for kv in words.chunks(2) {
            if num(kv[0]) == qnet::delta::PROTOCOL_FTE_PEXT2 {
                cl.pext2 = kv.get(1).map_or(0, |v| num(v)) & qnet::delta::PEXT2_SUPPORTED;
            }
        }
        cl.pext_known = true;
        self.send_serverinfo(i);
    }

    /// clcdp_ackframe from client `i` (`-1` requests a full entity resend).
    pub fn ack_frame(&mut self, i: usize, seq: i32) {
        if let Some(d) = self.core.clients.get_mut(i).and_then(|c| c.delta.as_mut()) {
            d.ack(seq);
        }
    }

    /// The network sequence the next datagram of client `i` goes out with
    /// (replacement deltas log frames by it).
    pub fn set_datagram_sequence(&mut self, i: usize, seq: u32) {
        if let Some(d) = self.core.clients.get_mut(i).and_then(|c| c.delta.as_mut()) {
            d.next_sequence = seq as i32;
        }
    }

    /// Active PEXT2_* extensions of client `i`.
    pub fn client_pext2(&self, i: usize) -> u32 {
        self.core.clients.get(i).map_or(0, |c| c.pext2)
    }

    /// Cmd_ExecuteString(s, src_client) restricted to the commands clients may run.
    pub(crate) fn exec_client_string(&mut self, i: usize, s: &str) {
        const ALLOWED: &[&str] = &[
            "status", "god", "notarget", "fly", "name", "noclip", "setpos", "say", "say_team", "tell",
            "color", "kill", "pause", "spawn", "begin", "prespawn", "kick", "ping", "give", "ban",
        ];
        let low = s.trim_start().to_ascii_lowercase();
        if !ALLOWED.iter().any(|a| low.starts_with(a)) {
            self.core.print(&format!("client {i} tried to {s}\n"));
            return;
        }
        let old = self.core.cmd_client.replace(i);
        qcore::cmd::execute_string(self, s);
        self.core.cmd_client = old;
    }
}
