use super::*;
use crate::menu::SearchStatus;
use crate::net::browser::ServerTab;

impl Host {
    /// Connects the local client when a server is running without one.
    pub(super) fn check_local_client(&mut self) {
        self.update_listen();
        if self.remote() {
            if !self.server.active() {
                return;
            }
            // A local `map` ends the remote game.
            self.disconnect_client();
        }
        if !self.server.active() {
            if self.demo.is_some() {
                return;
            }
            if self.local.take().is_some() || self.cl.signon != 0 {
                self.disconnect_client();
            }
            return;
        }
        self.sync_guests();
        let ok = self.dedicated || self.local.is_some_and(|i| self.server.core.clients.get(i).is_some_and(|c| c.active));
        if !ok {
            // Host_Map_f: a local game stops demo playback and the demo loop.
            if self.demo.take().is_some() {
                self.demonum = -1;
            }
            // CL_Disconnect stops a recording; `record <name> <map>` starts here.
            self.stop_recording();
            self.demo_head.clear();
            if let Some((path, track)) = self.pending_record.take() {
                self.start_recording(&path, track);
            }
            self.cl = ClientState::new();
            self.cl.name = self.server.core.cvars.by_name("_cl_name").map_or("player".into(), |c| c.string.clone());
            self.local = self.server.connect_local();
            self.drain_server_log();
            self.feed_client();
        }
    }

    pub(super) fn disconnect_client(&mut self) {
        if self.input.touch.any_finger() {
            qinput::release_touch(self);
        }
        self.touch_quickload_probe = 0.0;
        self.touch_console_keyboard = false;
        if let Some(mut n) = self.net_client.take() {
            n.disconnect(self.realtime);
        }
        if let Some(mut q) = self.qw_client.take() {
            q.disconnect();
        }
        if self.demo.take().is_some() {
            self.finish_timedemo();
        }
        self.stop_recording();
        self.autodemo_finish();
        self.ghost_reset();
        self.view.reset_effects();
        self.autosave.reset();
        self.bullet_time = Default::default();
        self.freecam.cam = None;
        self.hitstop = Default::default();
        self.pad_gestures.clear_session();
        self.pend_dodge = false;
        self.death_handled = false;
        self.feedback = Default::default();
        self.training = Default::default();
        self.demo_head.clear();
        self.cl = ClientState::new();
        self.local = None;
        self.guests.clear();
        self.csqc.clear();
        self.level = Level::default();
        self.audio_fx.clear_map();
        self.audio_fx.soundtracks.clear_track();
        self.cd.playing = None;
        self.cd.paused = false;
        self.zoom = (0.0, 0.0);
        self.con.end_chat();
        if let Some(s) = &self.snd {
            s.event(SoundEvent::StopAll);
            s.event(SoundEvent::ClearStatic);
            s.stop_music();
        }
        self.con.forced = true;
    }

    /// Delivers the server's reliable stream and datagrams to the client.
    pub(super) fn feed_client(&mut self) {
        let Some(i) = self.local else { return };
        let mut msgs = self.server.take_reliable(i);
        msgs.extend(self.server.take_datagrams(i));
        self.deliver(msgs);
    }

    /// Parse server messages (loopback or network) into the client.
    pub(super) fn deliver(&mut self, msgs: Vec<Vec<u8>>) {
        self.deliver_seq(msgs.into_iter().map(|m| (m, None)).collect());
    }

    /// [`Self::deliver`] with each datagram's network sequence (acks).
    pub(super) fn deliver_seq(&mut self, msgs: Vec<(Vec<u8>, Option<u32>)>) {
        for (m, seq) in msgs {
            self.cl.net_sequence = seq;
            if let Some(w) = &mut self.recording
                && !w.is_qwd()
                && let Err(e) = w.write(self.cl.viewangles, &m)
            {
                self.con.print(&format!("demo write failed: {e}\n"));
                self.recording = None;
            }
            let before = self.cl.signon;
            let events_before = self.cl.events.len();
            self.cl.defer_impact_effects();
            let r = self.cl.parse_server_message(&m);
            let new_map = self.cl.events[events_before.min(self.cl.events.len())..].iter().any(|e| matches!(e, ClientEvent::NewMap));
            self.autodemo_message(&m, new_map);
            // demo_head: what a client needs to reach signon 2. A new
            // serverinfo (changelevel) starts it over.
            let after = self.cl.signon;
            if before >= 2 && after < 2 {
                self.demo_head.clear();
            }
            if before < 2 || after < 2 {
                self.demo_head.push(m);
                self.demo_head_signon = after;
            }
            if let Err(e) = r {
                self.con.print(&format!("Host_Error: {e:?}\n"));
                if !self.remote() {
                    self.server.level = None;
                }
                self.disconnect_client();
                return;
            }
        }
        self.process_client_events();
    }

    /// Open the listen socket while a multiplayer server runs; close it
    /// when back to single player.
    pub(super) fn update_listen(&mut self) {
        let want = self.server.core.maxclients() > 1 && self.server.active();
        if want && self.net_server.is_none() {
            match crate::net::NetServer::listen(self.net_port) {
                Ok(n) => {
                    self.con.print(&qcore::ctr!("UDP listening on port {}\n", n.port()));
                    self.net_server = Some(n);
                }
                Err(e) => {
                    self.con.print(&qcore::ctr!("UDP bind on port {} failed: {}\n", self.net_port, e));
                    self.net_port = 0; // don't retry every frame; next try takes any port
                }
            }
        } else if !want && let Some(mut n) = self.net_server.take() {
            n.shutdown(&mut self.server, self.realtime);
        }
    }

    /// Read the remote server's messages (client side).
    pub(super) fn poll_remote(&mut self) {
        let now = self.realtime;
        let Some(n) = &mut self.net_client else { return };
        let was = n.connected();
        match n.poll(now) {
            crate::net::ClientPoll::Messages(m) => {
                if !was && n.connected() {
                    let a = n.server;
                    self.con.print(&qcore::ctr!("Connection accepted by {}\n", a));
                }
                self.deliver_seq(m);
            }
            crate::net::ClientPoll::Failed(why) => {
                self.con.print(&qcore::ctr!("Connection failed: {}", why));
                self.disconnect_client();
            }
        }
    }

    /// Scoreboard pings while it is shown in multiplayer: straight from a
    /// local server, else by asking a remote one every 2 s.
    pub(super) fn update_pings(&mut self) {
        if self.ping_expect.is_some_and(|t| self.realtime > t) {
            self.ping_expect = None;
        }
        if !self.show_scores || self.cl.maxclients < 2 || self.cl.signon != 4 {
            return;
        }
        if self.net_client.is_none() {
            if self.server.active() {
                for (k, s) in self.cl.scores.iter_mut().enumerate() {
                    s.ping = self.server.core.clients.get(k).map_or(0, |c| (c.ping() * 1000.0).round() as i32);
                }
            }
            return;
        }
        if self.realtime >= self.ping_next && self.ping_expect.is_none() {
            self.cl.send_string_cmd("ping");
            self.ping_next = self.realtime + 2.0;
            self.ping_expect = Some(self.realtime + 1.5);
        }
    }

    /// Consume a `ping` reply (header and "  ms name" lines, in one print or
    /// one per line). False when `s` isn't part of it, which ends the reply.
    pub(super) fn take_ping_reply(&mut self, s: &str) -> bool {
        let mut any = false;
        for line in s.lines() {
            if line.starts_with("Client ping times:") {
                any = true;
                continue;
            }
            let t = line.trim_start();
            let Some((ms, name)) = t.split_once(' ') else { break };
            let Ok(ms) = ms.parse::<i32>() else { break };
            let name = name.trim();
            if let Some(sc) = self.cl.scores.iter_mut().find(|sc| sc.name.trim() == name) {
                sc.ping = ms;
            }
            any = true;
        }
        if !any {
            self.ping_expect = None;
        }
        any
    }

    pub(super) fn start_server_search(&mut self, tab: ServerTab) {
        if self.server_searches[tab.index()].is_some() {
            return;
        }
        match crate::net::browser::ServerSearch::start(
            self.net_port,
            tab,
            self.cvs("net_masters"),
            self.cvs("net_qwmasters"),
            &self.saved_servers,
            self.realtime,
        ) {
            Ok(search) => {
                self.print(&qcore::ctr!("Looking for Quake servers...\n"));
                self.server_searches[tab.index()] = Some(search);
                self.menu.set_server_search_status(tab, SearchStatus::Searching);
            }
            Err(e) => {
                self.menu.set_server_search_status(tab, SearchStatus::Failed);
                self.print(&format!("{}: {e}\n", tab.command()));
            }
        }
    }

    pub(super) fn poll_server_searches(&mut self) {
        for tab in ServerTab::ALL {
            let slot = &mut self.server_searches[tab.index()];
            let Some(search) = slot else { continue };
            let changed = search.poll(self.realtime);
            let done = search.done(self.realtime);
            if changed || done {
                self.menu.replace_server_results(tab, search.found.clone(), &self.saved_servers);
            }
            if !done {
                continue;
            }
            let search = slot.take().expect("finished search");
            self.menu.set_server_search_status(tab, SearchStatus::Complete);
            let mut out = String::new();
            for s in &search.found {
                let ping = s.ping_ms.map_or_else(|| " --".into(), |ms| format!("{ms:>3}ms"));
                out.push_str(&format!(
                    "{:<15.15} {:<10.10} {:<4} {:>2}/{:<2} {:>5} {}\n",
                    s.hostname,
                    s.map,
                    s.protocol.tag(),
                    s.players,
                    s.max_players,
                    ping,
                    s.connect_address(),
                ));
            }
            out.push_str(&format!("{}: {} server(s) found\n", tab.command(), search.found.len()));
            self.con.print(&out);
        }
    }

    pub(super) fn record_recent_server(&mut self, addr: &str, qw: bool) {
        let bare = addr.trim().strip_prefix("qw://").unwrap_or(addr.trim());
        let existing = self.menu.find_server(bare).cloned();
        self.saved_servers.record_recent(addr, qw, existing.as_ref());
        self.saved_servers.save(&crate::userdata::root().join("servers.txt"));
        self.menu.update_saved_servers(&self.saved_servers);
    }

    pub(super) fn toggle_favorite_server(&mut self, addr: &str) {
        let bare = addr.trim().strip_prefix("qw://").unwrap_or(addr.trim());
        if bare.is_empty() {
            return;
        }
        let qw = addr.trim().starts_with("qw://");
        let entry = self
            .menu
            .find_server(bare)
            .cloned()
            .unwrap_or_else(|| crate::menu::ServerEntry {
                address: bare.to_string(),
                hostname: bare.to_string(),
                protocol: if qw {
                    crate::net::browser::ServerProto::QuakeWorld
                } else {
                    crate::net::browser::ServerProto::NetQuake
                },
                gamedir: if qw { "qw".into() } else { "id1".into() },
                ..Default::default()
            });
        let now_fav = self.saved_servers.toggle_favorite(&entry);
        self.saved_servers.save(&crate::userdata::root().join("servers.txt"));
        self.menu.update_saved_servers(&self.saved_servers);
        if now_fav {
            self.print(&format!("Added {} to favorites\n", entry.connect_address()));
        } else {
            self.print(&format!("Removed {} from favorites\n", entry.connect_address()));
        }
    }

    /// `connect <address>`.
    pub(super) fn connect_remote(&mut self, addr: &str) {
        let Some(a) = crate::net::resolve(addr) else {
            self.con.print(&qcore::ctr!("Couldn't resolve {}\n", addr));
            return;
        };
        self.stop_game();
        // CL_EstablishConnection: no demo loop while connected.
        self.demonum = -1;
        self.cl = ClientState::new();
        self.cl.name = self.cvs("_cl_name").to_string();
        self.cl.color = self.cv("_cl_color") as i32;
        self.con.print(&qcore::ctr!("Connecting to {}...\n", a));
        match crate::net::NetClient::connect(a, self.realtime) {
            Ok(n) => {
                self.net_client = Some(n);
                self.last_server = Some((a, false));
                self.record_recent_server(&a.to_string(), false);
            }
            Err(e) => self.con.print(&qcore::ctr!("Connection failed: {}\n", e)),
        }
    }

    /// Connected (or connecting) to a remote NetQuake or QuakeWorld server.
    pub(super) fn remote(&self) -> bool {
        self.net_client.is_some() || self.qw_client.is_some()
    }

    /// `connect qw://<address>` / `qwconnect <address>`.
    pub(super) fn connect_qw(&mut self, addr: &str) {
        let Some(a) = crate::qw::resolve(addr) else {
            self.con.print(&qcore::ctr!("Couldn't resolve {}\n", addr));
            return;
        };
        self.stop_game();
        self.demonum = -1;
        self.cl = ClientState::new();
        self.cl.name = self.cvs("_cl_name").to_string();
        self.cl.color = self.cv("_cl_color") as i32;
        let mut ui = String::new();
        let color = self.cl.color;
        for (k, v) in [
            ("name", self.cl.name.clone()),
            ("topcolor", (color >> 4).to_string()),
            ("bottomcolor", (color & 15).to_string()),
            ("rate", "25000".into()),
            ("msg", "1".into()),
        ] {
            qnet::qw::info_set(&mut ui, k, &v);
        }
        self.con.print(&qcore::ctr!("Connecting to QuakeWorld server {}...\n", a));
        match crate::qw::QwClient::connect(a, ui, self.realtime) {
            Ok(q) => {
                self.qw_client = Some(q);
                self.last_server = Some((a, true));
                self.record_recent_server(&format!("qw://{a}"), true);
            }
            Err(e) => self.con.print(&qcore::ctr!("Connection failed: {}\n", e)),
        }
    }

    /// Read the QuakeWorld server's messages.
    pub(super) fn poll_qw(&mut self) {
        let now = self.realtime;
        let Some(q) = &mut self.qw_client else { return };
        q.recording_qwd = self.recording.as_ref().is_some_and(|w| w.is_qwd());
        let res = q.poll(&mut self.server.core.fs, now);
        let tape = q.take_qwd_tape();
        if !tape.is_empty()
            && let Some(w) = &mut self.recording
            && w.is_qwd()
            && let Err(e) = w.write_raw(&tape)
        {
            self.con.print(&format!("demo write failed: {e}\n"));
            self.recording = None;
        }
        match res {
            crate::qw::QwPoll::Messages(m, prints) => {
                for p in prints {
                    self.con.print(&p);
                }
                self.deliver(m);
            }
            crate::qw::QwPoll::Failed(why) => {
                self.con.print(&qcore::ctr!("Disconnected: {}", why));
                self.disconnect_client();
            }
        }
    }

    /// NQ (`PEXT2_PREDINFO`): replays unacknowledged client moves against
    /// the loaded BSP collision model (respecting `cl_nopred`).
    pub(super) fn apply_nq_prediction(&mut self) {
        if self.net_client.is_none() || self.local.is_some() || self.demo.is_some() {
            return;
        }
        let Some(cm) = self.level.collision_model() else {
            return;
        };
        let nopred = self.cv("cl_nopred") != 0.0;
        let pending = (self.accum > 0.001 && self.freecam.cam.is_none()).then(|| {
            let mut cmd = self.last_cmd;
            cmd.buttons = self.pend_attack as u16 | (self.pend_jump as u16) << 1;
            cmd.impulse = self.pend_impulse;
            let msec = (self.accum * 1000.0).round().clamp(1.0, 250.0) as u8;
            (cmd, msec)
        });
        self.cl.predict_move(cm, nopred, pending);
    }

    /// QW: the predicted local player replaces the server's position, and
    /// the scoreboard takes the server's pings.
    pub(super) fn apply_qw_prediction(&mut self) {
        let nopred = self.cv("cl_nopred") != 0.0;
        let Some(q) = &mut self.qw_client else { return };
        q.predict(self.realtime, nopred);
        let qw = &q.qw;
        for (i, s) in self.cl.scores.iter_mut().enumerate() {
            s.ping = qw.ping(i);
        }
        if !qw.predicted {
            return;
        }
        if let Some(e) = self.cl.entities.get_mut(self.cl.viewentity) {
            e.origin = qw.simorg;
        }
        self.cl.velocity = qw.simvel;
        self.cl.onground = qw.sim_onground;
    }

    /// Client string commands (clc_stringcmd) back to the server.
    pub(super) fn forward_client_cmds(&mut self) {
        let out = self.cl.take_reliable_out();
        if let Some(q) = &mut self.qw_client {
            for s in string_cmds(&out) {
                let low = s.trim_start().to_ascii_lowercase();
                let word = low.split_whitespace().next().unwrap_or("");
                // The NQ signon replies; QW runs its own signon, and name
                // and colour are in the userinfo (changes go through
                // send_userinfo as setinfo).
                if matches!(word, "prespawn" | "spawn" | "begin" | "name" | "color") {
                    continue;
                }
                q.stringcmd(s.trim_end());
            }
            return;
        }
        if let Some(n) = &mut self.net_client {
            if !out.is_empty() {
                n.send_reliable(&out, self.realtime);
            }
            return;
        }
        let Some(i) = self.local else { return };
        let mut p = 0;
        while p < out.len() {
            if out[p] as i32 != CLC_STRINGCMD as i32 {
                break;
            }
            let end = out[p + 1..].iter().position(|&b| b == 0).map_or(out.len(), |e| p + 1 + e);
            let s = String::from_utf8_lossy(&out[p + 1..end]).into_owned();
            p = end + 1;
            let low = s.trim_start().to_ascii_lowercase();
            if ["prespawn", "spawn", "begin", "name", "color"].iter().any(|p| low.starts_with(p)) {
                continue; // connect_local runs the signon itself
            }
            self.server.client_command(i, s.trim_end());
        }
    }

    pub(super) fn process_client_events(&mut self) {
        let evs = self.cl.take_events();
        let mut feedback = None;
        for e in evs {
            if self.demo_seeking && Self::seek_skips(&e) {
                continue;
            }
            match e {
                ClientEvent::PortalEffect(_) | ClientEvent::RailEffect(_) | ClientEvent::Hook(_) | ClientEvent::Cable(_) | ClientEvent::Boomerang(_)
                | ClientEvent::Gel(_) | ClientEvent::Water(_) | ClientEvent::Gun(_) => {
                    self.toy_event(e);
                }
                ClientEvent::HitStop { kill, damage, class, health } => {
                    self.receive_hitstop(super::hitstop::Hit { kill, damage, class, health });
                }
                ClientEvent::HitFeedback { kill } => {
                    feedback = Some(feedback.unwrap_or(false) || kill);
                }
                ClientEvent::Print(s) => {
                    if self.ping_expect.is_some() && self.take_ping_reply(&s) {
                        continue;
                    }
                    let s = self.server.core.loc.translate(&s);
                    if !self.csqc_parse_print(&s) {
                        self.con.print(&s);
                    }
                }
                ClientEvent::CenterPrint(s) => {
                    if !self.csqc_parse_centerprint(&s) {
                        let (s, t, cols) = self.center_args(&s);
                        self.con.center_print(&s, t, cols);
                    }
                }
                ClientEvent::ModelPrecache(i) => {
                    let fs = &self.server.core.fs;
                    self.level.late_model(fs, &mut self.r, &mut self.cl, i);
                }
                ClientEvent::SoundPrecache(i) => {
                    if let (Some(s), Some(n)) = (&mut self.snd, self.cl.sound_precache.get(i))
                        && !self.level.mapname.is_empty()
                    {
                        if self.level.sounds.len() <= i {
                            self.level.sounds.resize(i + 1, None);
                        }
                        self.level.sounds[i] = s.precache(n);
                    }
                }
                ClientEvent::StuffText(s) => {
                    if !self.csqc_parse_stuffcmd(&s) {
                        self.cbuf.add_text(&s);
                    }
                }
                ClientEvent::CsqcEvent(data) => {
                    self.csqc_parse_event(&data);
                }
                ClientEvent::Sound { ent, channel, sound, origin, volume, attenuation } => {
                    self.view.vwep.sound(&self.cl, ent, sound);
                    if self.cv("cl_captions") != 0.0
                        && let Some(n) = self.cl.sound_precache.get(sound)
                    {
                        self.captions.sound(n, self.realtime);
                    }
                    if let (Some(snd), Some(Some(sfx))) = (&self.snd, self.level.sounds.get(sound)) {
                        let dur = sfx.data.len() as f64 / snd.rate().max(8000) as f64;
                        snd.event(SoundEvent::Start { entity: ent, channel, sfx: sfx.clone(), origin, volume, attenuation });
                        let mut brushes = None;
                        self.audio_fx.on_sound_start(
                            &self.server.core.cvars,
                            &self.cl,
                            &self.level,
                            &mut brushes,
                            self.last_audio_view.as_ref(),
                            ent,
                            channel,
                            origin,
                            dur,
                            self.realtime,
                            snd,
                        );
                    }
                }
                ClientEvent::TempSound { name, origin } => {
                    if self.cv("cl_captions") != 0.0 {
                        self.captions.sound(name, self.realtime);
                    }
                    if let Some(snd) = &mut self.snd
                        && let Some(sfx) = snd.precache(name)
                    {
                        let dur = sfx.data.len() as f64 / snd.rate().max(8000) as f64;
                        snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin, volume: 1.0, attenuation: 1.0 });
                        let mut brushes = None;
                        self.audio_fx.on_sound_start(
                            &self.server.core.cvars,
                            &self.cl,
                            &self.level,
                            &mut brushes,
                            self.last_audio_view.as_ref(),
                            -1,
                            0,
                            origin,
                            dur,
                            self.realtime,
                            snd,
                        );
                    }
                }
                ClientEvent::LocalSound { sound } => {
                    if let (Some(snd), Some(Some(sfx))) = (&self.snd, self.level.sounds.get(sound)) {
                        snd.event(SoundEvent::Local(sfx.clone()));
                    }
                }
                ClientEvent::StopSound { ent, channel } => {
                    if let Some(snd) = &self.snd {
                        snd.event(SoundEvent::Stop { entity: ent, channel });
                    }
                }
                ClientEvent::StaticSound(s) => {
                    if let (Some(snd), Some(Some(sfx))) = (&self.snd, self.level.sounds.get(s.sound)) {
                        snd.event(SoundEvent::Static {
                            sfx: sfx.clone(),
                            origin: s.origin,
                            volume: s.volume as f32 / 255.0,
                            attenuation: s.attenuation as f32 / 64.0,
                        });
                    }
                }
                ClientEvent::CdTrack { track, .. } => {
                    // an attract demo's track doesn't replace the menu theme
                    if self.menu_music && self.demo.is_some() {
                        continue;
                    }
                    self.menu_music = false;
                    self.play_cd_track(track, true);
                }
                ClientEvent::Damage { armor, blood, from } => {
                    let cv = self.view_cvars();
                    self.view.parse_damage(&self.cl, &cv, armor, blood, from);
                    let s = ((armor + blood) as f32 / 30.0).min(1.0);
                    self.add_rumble(0.4 + 0.6 * s, 0.2 + 0.3 * s, 120 + (s * 250.0) as u32);
                    self.add_touch_haptic(0.35 + 0.45 * s, 45 + (s * 65.0) as u32);
                }
                ClientEvent::Pause(p) => {
                    if let Some(snd) = &self.snd {
                        snd.pause_music(p);
                    }
                }
                ClientEvent::Skybox(name) => {
                    let fs = &self.server.core.fs;
                    self.level.set_sky(fs, &mut self.r, &name);
                }
                ClientEvent::BonusFlash => self.view.bonus_flash(),
                ClientEvent::NewMap => {
                    self.training = Default::default();
                    self.clear_toy_fx();
                    self.audio_fx.clear_map();
                    self.view.reset_effects();
                    self.bullet_time = Default::default();
                    self.hitstop = Default::default();
                    self.pend_attack = false;
                    self.pend_attack2 = false;
                    self.pend_reload = false;
                    self.pend_jump = false;
                    self.pend_dodge = false;
                    self.pend_impulse = 0;
                    self.death_handled = false;
                    feedback = None;
                    self.feedback = Default::default();
                    self.stats_recorded = false;
                    self.pb_new_map();
                    self.ghost_reset();
                    self.autosave.reset();
                    self.new_map();
                    self.demo_new_map();
                }
                ClientEvent::Signon(4) => {
                    self.con.forced = false;
                    self.con.open = false;
                    self.con.clear_notify();
                    // A demo plays behind the menu; only a real game closes it.
                    if self.demo.is_none() {
                        self.menu.screen = Screen::None;
                    }
                    self.sync_key_dest();
                }
                // Host_EndGame during playback moves on to the next demo.
                ClientEvent::Disconnect if self.demo.is_some() => {
                    self.demo_reached_end();
                    return;
                }
                ClientEvent::Disconnect => {
                    self.con.print(&qcore::ctr!("Server disconnected\n"));
                    if self.remote() {
                        self.disconnect_client();
                        return;
                    }
                }
                _ => {}
            }
        }
        if let Some(kill) = feedback {
            self.receive_feedback(kill);
        }
        if self.cl.intermission != 0 && !self.stats_recorded {
            self.stats_recorded = true;
            self.record_level_stats();
        }
    }
}
