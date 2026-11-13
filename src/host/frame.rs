use super::*;

impl Host {
    /// scr_hudstyle 0: classic full-width status bar below the 3D view.
    pub(super) fn classic_hud(&self) -> bool {
        self.cv("scr_hudstyle") == 0.0
    }

    /// Toys usable this frame: add-ons on, a local non-deathmatch game and
    /// no remote clients. Empty (no allocation) when add-ons are off.
    pub(super) fn allowed_toys(&self) -> Vec<qserver::addons::Toy> {
        let cv = &self.server.core.cvars;
        if cv.value_of("addons_enable") == 0.0 || !self.server.active() {
            return Vec::new();
        }
        let all_local = qserver::addons::all_local(&self.server.core.clients);
        if !qserver::addons::allowed(cv, self.cv("deathmatch") != 0.0, all_local) {
            return Vec::new();
        }
        qserver::addons::Toy::enabled(cv).collect()
    }

    pub(super) fn view_cvars(&self) -> ViewCvars {
        let v = |n: &str| self.cv(n);
        ViewCvars {
            bob: v("cl_bob"),
            bobcycle: v("cl_bobcycle"),
            bobup: v("cl_bobup"),
            rollangle: v("cl_rollangle"),
            rollspeed: v("cl_rollspeed").max(1.0),
            kicktime: v("v_kicktime").max(0.01),
            kickroll: v("v_kickroll"),
            kickpitch: v("v_kickpitch"),
            fov: v("fov").clamp(10.0, 170.0),
            gunkick: v("v_gunkick") != 0.0,
            drawviewmodel: v("r_drawviewmodel") != 0.0,
            lerp_models: v("r_lerpmodels") != 0.0,
            lerp_move: v("r_lerpmove") != 0.0,
            flash: v("v_flashscale").clamp(0.0, 1.0),
        }
        .reduce_motion(v("a11y_reducemotion") != 0.0)
    }

    pub(super) fn move_speeds(&self) -> MoveSpeeds {
        let v = |n: &str| self.cv(n);
        MoveSpeeds {
            forward: v("cl_forwardspeed"),
            back: v("cl_backspeed"),
            side: v("cl_sidespeed"),
            up: v("cl_upspeed"),
            yaw: v("cl_yawspeed"),
            pitch: v("cl_pitchspeed"),
            movespeedkey: v("cl_movespeedkey"),
            anglespeedkey: v("cl_anglespeedkey"),
            maxpitch: v("cl_maxpitch"),
            minpitch: v("cl_minpitch"),
        }
    }

    /// Auto-aim is allowed only when every connected player is local.
    pub(super) fn local_autoaim_allowed(&self) -> bool {
        self.local.is_some() && !self.remote() && self.server.active()
            && !self.server.core.clients.iter().any(|c| c.active && !c.local)
    }

    /// Match the renderer's 4:3-adapted FOV for each local player's viewport.
    pub(super) fn autoaim_fov(&self) -> [f32; 2] {
        let (w, h) = self.r.size();
        let n = 1 + self.guests.iter().filter(|g| g.cl.signon == 4).count();
        let rect = crate::splitscreen::layout(n, w, h, self.cv("r_splitscreen_layout") != 0.0)[0];
        let aspect = rect[2].max(1) as f32 / rect[3].max(1) as f32;
        let ty = (self.cv("fov").clamp(10.0, 170.0).to_radians() * 0.5).tan() * 0.75;
        [(ty * aspect).atan().to_degrees() * 2.0, ty.atan().to_degrees() * 2.0]
    }

    /// Living loopback targets at the client's interpolated positions.
    pub(super) fn aim_targets(&self, cl: &qclient::ClientState) -> Vec<AimTarget> {
        let mut out = Vec::new();
        let Some(l) = &self.server.level else { return out };
        let vm = &l.vm;
        let (Some(fl), Some(hp), Some(org), Some(mn), Some(mx), Some(vel)) = (
            vm.field_ofs("flags"),
            vm.field_ofs("health"),
            vm.field_ofs("origin"),
            vm.field_ofs("mins"),
            vm.field_ofs("maxs"),
            vm.field_ofs("velocity"),
        ) else {
            return out;
        };
        let maxc = self.server.core.maxclients();
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || e as usize == cl.viewentity || vm.ent_f(e, hp) <= 0.0 {
                continue;
            }
            let flags = vm.ent_f(e, fl) as i32;
            let kind = if flags & 32 != 0 {
                TargetKind::Monster
            } else if (e as usize) <= maxc {
                TargetKind::Player
            } else {
                continue;
            };
            let origin = cl.entities.get(e as usize).filter(|c| c.model != 0).map_or(vm.ent_v(e, org), |c| c.origin);
            out.push(AimTarget { id: e, origin, mins: vm.ent_v(e, mn), maxs: vm.ent_v(e, mx), velocity: vm.ent_v(e, vel), kind });
        }
        out
    }

    /// One host frame of `dt` seconds.
    pub fn frame(&mut self, dt: f64, events: Vec<HostEvent>) {
        self.realtime += dt;
        self.con.realtime = self.realtime;
        for e in events {
            self.handle_event(e);
        }
        let now = self.realtime;
        let (due, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.delayed).into_iter().partition(|(t, _)| *t <= now);
        self.delayed = rest;
        for (_, c) in due {
            self.cbuf.add_text(&c);
            self.cbuf.add_text("\n");
        }
        self.exec_cbuf();
        if self.startup < 2 {
            self.startup += 1;
            if self.startup == 1 {
                self.startup_video();
                self.stuff_cmds();
                self.exec_cbuf();
            }
            if self.startup == 2 && !self.server.active() && self.cbuf.is_empty() && self.delayed.is_empty() {
                self.menu.open(Screen::Main);
            }
        }
        self.check_local_client();
        self.sync_key_dest();
        self.sync_menu_ui();
        self.menu_nav_repeat();

        // input (every frame, for smooth view angles)
        let ft = dt as f32;
        let in_game = self.cl.signon == 4 && self.cl.intermission == 0;
        self.input.autoaim_allowed = self.local_autoaim_allowed() && self.demo.is_none() && in_game && self.cl.stats[STAT_HEALTH] > 0;
        let needs_targets = self.input.gamepad_connected || (self.input.autoaim_allowed && self.cv("assist_autoaim") != 0.0);
        let targets = if in_game && self.cl.stats[STAT_HEALTH] > 0 && needs_targets { self.aim_targets(&self.cl) } else { Vec::new() };
        self.input.autoaim_fov = self.autoaim_fov();
        self.input.autoaim_players = self.cv("deathmatch") != 0.0;
        let mut inp = {
            let eye = self.cl.entities.get(self.cl.viewentity).map_or([0.0; 3], |e| {
                let mut o = e.origin;
                o[2] += self.cl.viewheight;
                o
            });
            let cm = self.server.level.as_ref().map(|l| l.world.worldmodel.clone());
            let vis = cm.as_deref().map(|cm| WorldVis::new(cm, &self.cl));
            let allowed = aim_assist_allowed(self.cv("deathmatch") != 0.0, self.cv("sv_aimassist") != 0.0);
            let ctx = vis.as_ref().map(|v| AimContext { eye, player_velocity: self.cl.velocity, targets: &targets, vis: v, allowed });
            self.input.frame(&self.server.core.cvars, self.cl.viewangles, ft, ctx.as_ref())
        };
        if self.cv("cl_alwaysrun") != 0.0 {
            inp.speed = !inp.speed;
        }
        self.pend_attack |= inp.attack;
        self.pend_attack2 |= inp.attack2;
        self.pend_jump |= inp.jump;
        let mut impulse = inp.impulse;
        let alive = self.cl.stats[STAT_HEALTH] > 0;
        let (held, aim) = (self.input.wheel_held(), self.input.wheel_aim);
        if let Some(imp) = self.wheel.update(held, in_game && alive && self.demo.is_none(), aim, &self.cl) {
            impulse = imp;
        }
        if impulse != 0 {
            self.pend_impulse = impulse;
        }
        let toys = if self.local.is_some() { self.allowed_toys() } else { Vec::new() };
        if let Some(t) = self.toy.update(&toys, inp.toy, impulse) {
            let (s, t, cols) = self.center_args(t.name());
            self.con.center_print(&s, t, cols);
        }
        self.guest_moves(ft);
        let timescale = self.wheel_timescale() as f64;
        if self.cl.signon == 4 {
            let sp = self.move_speeds();
            self.last_cmd = self.cl.base_move(&InputState { attack: false, jump: false, impulse: 0, ..inp }, &sp, ft);
        }

        // server tic
        self.accum += dt;
        let maxfps = self.cv("host_maxfps");
        let tic = if maxfps > 0.0 && maxfps <= 72.0 { 0.0 } else { SERVER_TIC };
        if self.accum >= tic {
            let sdt = self.accum.min(0.1) * timescale;
            self.accum = 0.0;
            if let Some(i) = self.local {
                let mut cmd = self.last_cmd;
                cmd.buttons = self.toy.buttons(self.pend_attack, self.pend_attack2, self.pend_jump);
                cmd.impulse = self.pend_impulse;
                // The server acks the lossless local client's frames itself;
                // only full-resend requests (-1) go back.
                for a in self.cl.ack_frames.drain(..) {
                    if a == -1 {
                        self.server.ack_frame(i, -1);
                    }
                }
                if self.cl.build_move(&cmd).is_some() {
                    self.server.set_move(i, ClientMove {
                        angles: cmd.viewangles,
                        forward: cmd.forwardmove,
                        side: cmd.sidemove,
                        up: cmd.upmove,
                        buttons: cmd.buttons,
                        impulse: cmd.impulse,
                    });
                    self.pend_attack = false;
                    self.pend_attack2 = false;
                    self.pend_jump = false;
                    self.pend_impulse = 0;
                }
            }
            for g in &mut self.guests {
                g.send(&mut self.server);
            }
            if let Some(q) = &mut self.qw_client {
                let c = self.last_cmd;
                let cmd = qnet::qw::UserCmd {
                    msec: 0,
                    angles: c.viewangles,
                    forwardmove: c.forwardmove.clamp(-32768.0, 32767.0) as i16,
                    sidemove: c.sidemove.clamp(-32768.0, 32767.0) as i16,
                    upmove: c.upmove.clamp(-32768.0, 32767.0) as i16,
                    buttons: self.pend_attack as u8 | (self.pend_jump as u8) << 1,
                    impulse: self.pend_impulse,
                };
                let active = self.cl.signon == 4;
                q.send_cmd(cmd, self.realtime);
                if active {
                    self.pend_attack = false;
                    self.pend_jump = false;
                    self.pend_impulse = 0;
                }
            }
            if self.net_client.is_some() && self.local.is_none() {
                let mut cmd = self.last_cmd;
                cmd.buttons = self.pend_attack as u8 | (self.pend_jump as u8) << 1;
                cmd.impulse = self.pend_impulse;
                if let Some(m) = self.cl.build_move(&cmd)
                    && let Some(n) = &mut self.net_client
                {
                    n.send_unreliable(&m, self.realtime);
                    self.pend_attack = false;
                    self.pend_jump = false;
                    self.pend_impulse = 0;
                }
            }
            self.forward_client_cmds();
            if let Some(n) = &mut self.net_server {
                let host = self.server.core.cvars.by_name("hostname").map_or(String::new(), |c| c.string.clone());
                n.receive(&mut self.server, self.realtime, &host);
            }
            // Keep QC localcmd text for the shared host Cbuf (see exec_cbuf).
            let t = self.server.take_text();
            self.cbuf.add_text(&t);
            // Ironwail: single player only advances while not in the menu.
            let hold = self.server.core.maxclients() == 1 && self.menu.active();
            self.server.frame_held(sdt, hold);
            if let Some(e) = self.server.error() {
                let msg = format!("Host_Error: {e}\n");
                self.con.print(&msg);
                self.server.level = None;
                self.server.core.sv.active = false;
            }
            if let Some(n) = &mut self.net_server {
                n.send(&mut self.server, self.realtime);
            }
            self.drain_server_log();
            self.feed_client();
            for g in &mut self.guests {
                g.feed(&mut self.server);
            }
            self.guest_sounds();
            self.guest_center_prints();
        }
        self.poll_remote();
        self.poll_qw();
        self.poll_lan_search();
        self.update_pings();
        self.read_demo(dt);
        self.check_local_client();
        self.sync_fte();
        if self.cl.signon > 0 {
            self.cl.frame(dt * timescale);
            self.apply_qw_prediction();
        }
        for g in &mut self.guests {
            if g.cl.signon > 0 {
                g.cl.frame(dt * timescale);
            }
        }
        self.process_client_events();
        self.track_rumble();
        self.update_death();
        self.sync_key_dest();
        self.update_autosave();
        self.con.update(ft);

        let (view, gun) = if self.cl.signon == 4 && !self.level.mapname.is_empty() {
            let mut cv = self.view_cvars();
            cv.drawviewmodel &= self.toy.held.is_none();
            let (mut v, g) = self.view.calc_refdef(&self.cl, &cv, &self.level, ft);
            self.cl.viewent.lerpflags &= !qclient::LERP_RESETANIM;
            if self.classic_hud() && self.cl.intermission == 0 {
                let (_, h) = self.r.size();
                v.reserve_bottom = (CLASSIC_SBAR_LINES * crate::draw::Canvas::new(1, h).scale) as u32;
            }
            (Some(v), g)
        } else {
            (None, None)
        };
        self.update_sound(view.as_ref(), ft);
        if self.screenshot.is_some() {
            self.r.request_capture();
        }
        if !self.dedicated {
            self.render(view, gun, ft);
        }
        if let Some(name) = self.screenshot.take() {
            self.write_screenshot(&name);
        }
    }

    pub(super) fn update_sound(&mut self, view: Option<&qrender::View>, ft: f32) {
        let vol = (self.cv("volume"), self.cv("bgmvolume"));
        let Some(snd) = &self.snd else { return };
        if vol != self.last_vol {
            snd.set_volume(vol.0);
            snd.set_bgm_volume(vol.1);
            self.last_vol = vol;
        }
        let mut l = Listener { view_entity: self.cl.viewentity as i32, frametime: ft, ..Default::default() };
        if let Some(v) = view {
            l.origin = v.origin;
            let (_, r, _) = angle_vectors(Vec3::from_array(v.angles));
            l.right = r.to_array();
            if let Some(bsp) = &self.level.bsp {
                let leaf = bsp_leaf(bsp, v.origin);
                l.ambient_levels = bsp.leafs.get(leaf).map(|lf| lf.ambient_level);
            }
            l.underwater = self.contents(v.origin) <= crate::view::CONTENTS_WATER;
        }
        // Split-screen: every other local player is a listener too.
        for g in self.guests.iter().filter(|g| g.cl.signon == 4).take(qsound::MAX_OTHER_LISTENERS) {
            let Some(e) = g.cl.entities.get(g.cl.viewentity) else { continue };
            let ear = [e.origin[0], e.origin[1], e.origin[2] + g.cl.viewheight];
            l.others[l.num_others] = (ear, g.cl.viewentity as i32);
            l.num_others += 1;
        }
        snd.set_water_fx(l.underwater);
        snd.update(l);
    }

    pub(super) fn contents(&self, p: [f32; 3]) -> i32 {
        if let Some(l) = &self.server.level {
            return l.world.point_contents(Vec3::from_array(p));
        }
        // Demo playback has no server world: use the client's BSP leaves.
        self.level.bsp.as_ref().and_then(|b| b.leafs.get(bsp_leaf(b, p))).map_or(-1, |l| l.contents)
    }
}
