use super::*;

impl Host {
    pub fn toggle_console(&mut self) {
        if self.con.forced {
            if !self.menu.active() {
                self.menu.open(Screen::Main);
            } else {
                self.menu.screen = Screen::None;
                self.sync_console_catalog();
            }
            return;
        }
        self.con.open = !self.con.open;
        self.con.input.clear();
        self.con.cursor = 0;
        self.con.completion.clear();
        self.con.search.active = false;
        self.menu.screen = Screen::None;
        if self.con.open {
            self.sync_console_catalog();
        }
    }

    pub(super) fn sync_shell_env(&mut self) {
        let env_changed = self.con.shell_env.basedir != self.fs().basedir()
            || self.con.shell_env.gamedir != self.game_name();
        if env_changed {
            self.con.shell_env.basedir = self.fs().basedir().to_path_buf();
            self.con.shell_env.gamedir = self.game_name().to_string();
            self.con.shell_env.userdir = self.user_dir();
        }
        if self.con.shell_env.mapname != self.level.mapname {
            self.con.shell_env.mapname.clone_from(&self.level.mapname);
        }
    }

    /// Refresh the full console completion catalog (commands, cvars, maps, cfgs, demos, mods).
    pub(super) fn sync_console_catalog(&mut self) {
        self.sync_console_cvars();
        let fs = self.fs();
        let maps: Vec<String> = fs
            .list("maps/", ".bsp")
            .into_iter()
            .filter_map(|p| {
                let name = p.strip_prefix("maps/").unwrap_or(&p);
                let stem = name.strip_suffix(".bsp").unwrap_or(name);
                // Skip bmodels (b_bh10, etc.)
                (!stem.starts_with("b_")).then(|| stem.to_string())
            })
            .collect();
        let cfgs = fs.list("", ".cfg");
        let demos: Vec<String> = fs
            .list("", ".dem")
            .into_iter()
            .map(|p| p.strip_suffix(".dem").unwrap_or(&p).to_string())
            .collect();
        let mods_dir = crate::user_mods_dir();
        let mods: Vec<String> = qui::mods::scan_roots(&[fs.basedir(), &mods_dir]).into_iter().map(|m| m.dir).collect();
        let mut saves = Vec::new();
        if let Ok(rd) = qcore::vfs::read_dir(&self.user_dir()) {
            for e in rd {
                let p = e.path;
                if p.extension().is_some_and(|x| x == "sav" || x == "qsa")
                    && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                {
                    let s = stem.to_string();
                    if !saves.contains(&s) {
                        saves.push(s);
                    }
                }
            }
        }
        saves.sort();
        self.con.catalog.maps = maps;
        self.con.catalog.cfgs = cfgs;
        self.con.catalog.demos = demos;
        self.con.catalog.mods = mods;
        self.con.catalog.saves = saves;
    }

    /// Fast in-memory refresh of commands, aliases and cvar values for live autocomplete.
    pub(super) fn sync_console_cvars(&mut self) {
        let mut cmds: Vec<String> = self.cmds.cmd_names().into_iter().map(str::to_string).collect();
        cmds.extend(self.server.complete_command("").into_iter().map(str::to_string));
        cmds.sort_unstable();
        cmds.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        let aliases: Vec<(String, String)> = self
            .cmds
            .alias_list()
            .into_iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect();
        let cvars: Vec<(String, String, String)> = self
            .server
            .core
            .cvars
            .iter()
            .map(|c| (c.name.clone(), c.string.clone(), c.default.clone()))
            .collect();
        self.con.catalog.cmds = cmds;
        self.con.catalog.aliases = aliases;
        self.con.catalog.cvars = cvars;
    }

    /// Run a line that nobody typed: it never reaches the shell.
    pub fn exec_line(&mut self, line: &str) {
        self.exec_line_from(line, Origin::Script);
    }

    /// Cmd_ExecuteString with server-command forwarding and, for the user's own
    /// lines only, `!cmd` shell calls and `quake | bash` pipes.
    pub(super) fn exec_line_from(&mut self, line: &str, origin: Origin) {
        self.with_origin(origin, |h| h.exec_line_inner(line));
    }

    /// Runs `f` with `origin` in control: commands check `cmd_origin`, and
    /// `CVAR_USER` cvars only change above `Script`.
    pub(super) fn with_origin<T>(&mut self, origin: Origin, f: impl FnOnce(&mut Self) -> T) -> T {
        let outer = std::mem::replace(&mut self.cmd_origin, origin);
        let trusted = self.server.core.cvars.set_trusted(origin > Origin::Script);
        let r = f(self);
        self.server.core.cvars.set_trusted(trusted);
        self.cmd_origin = outer;
        r
    }

    /// Whether the user (console, command line, menu) gave this command;
    /// says so when it came from a server, mod, config or bind.
    pub(super) fn user_only(&mut self, cmd: &str) -> bool {
        let ok = self.cmd_origin > Origin::Script;
        if !ok {
            self.print(&format!("{cmd}: only available from the console or the menu\n"));
        }
        ok
    }

    fn exec_line_inner(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        // `delay <seconds> <command...>`: schedule the remainder of the line verbatim
        // so delayed pipes (`delay 1 cvarlist | grep ^con_`) and `!cmd` run when due.
        if let Some(after_delay) = trimmed.strip_prefix("delay").filter(|s| s.starts_with(char::is_whitespace))
            && let Some((sec_str, rest)) = after_delay.trim_start().split_once(char::is_whitespace)
        {
            let rest = rest.trim();
            if !rest.is_empty() {
                let t = self.realtime + sec_str.parse::<f64>().unwrap_or(0.0);
                let cmd = if rest.starts_with('"') && rest.ends_with('"') && rest.len() >= 2 && !rest[1..rest.len() - 1].contains('"') {
                    rest[1..rest.len() - 1].to_string()
                } else {
                    rest.to_string()
                };
                self.delayed.push((t, cmd, self.cmd_origin));
                return;
            }
        }
        // Servers, demos, QuakeC and configs all write to this buffer.
        if self.cmd_origin != Origin::User {
            self.exec_quake_single(line);
            return;
        }
        // One-shot shell escape: `!<bash command>`
        if let Some(sh) = trimmed.strip_prefix('!') {
            let sh = sh.trim();
            if !sh.is_empty() {
                self.sync_shell_env();
                self.con.run_shell_cmd(sh, None);
            }
            return;
        }
        // Pipe Quake command output into bash: `<quake_cmd> | <bash_cmd>`
        if let Some((quake_part, bash_part)) = crate::console::complete::split_unquoted_pipe(trimmed) {
            let quake_part = quake_part.trim();
            let bash_part = bash_part.trim();
            if !quake_part.is_empty() && !bash_part.is_empty() {
                self.sync_shell_env();
                self.con.begin_capture();
                let mut sub = Cbuf::new();
                sub.add_text(quake_part);
                sub.add_text("\n");
                while let Some(qline) = sub.next_line() {
                    self.exec_quake_single(&qline);
                }
                let captured = self.con.end_capture();
                self.con.run_shell_cmd(bash_part, Some(captured));
                return;
            }
        }
        self.exec_quake_single(line);
    }

    fn exec_quake_single(&mut self, line: &str) {
        let args = Args::tokenize(line);
        if args.argc() == 0 {
            return;
        }
        let name = args.argv(0).to_ascii_lowercase();
        if self.csqc.has_command(&name) && self.csqc_console_command(line) {
            return;
        }
        if self.guest_player_command(&name, line) {
            return;
        }
        let known = self.cmds.exists(&name) || self.cmds.alias(&name).is_some() || self.server.core.cvars.find(&name).is_some();
        if known {
            qcore::cmd::execute_string(self, line);
        } else if self.remote() && !matches!(name.as_str(), "map" | "maxplayers" | "restart" | "changelevel" | "map_reload") {
            // Cmd_ForwardToServer. A typed `ping` prints its reply.
            if name == "ping" {
                self.ping_expect = None;
                self.ping_next = self.realtime + 3.0;
            }
            self.cl.send_string_cmd(line);
        } else if self.server.has_command(&name) {
            self.server.command(line);
        } else if let Some(i) = self.local.filter(|_| self.server.active()) {
            self.server.client_command(i, &qcore::lang::quake_encode(line));
        } else {
            self.con.print(&qcore::ctr!("Unknown command \"{}\"\n", args.argv(0)));
        }
        self.drain_server_log();
    }

    /// Cbuf_Execute; true when a `wait` stopped it for this frame.
    pub(super) fn exec_cbuf(&mut self) -> bool {
        loop {
            // One shared Cbuf as in Quake: QC localcmd text runs here, so it
            // reaches host commands too.
            let t = self.server.take_text();
            if !t.is_empty() {
                self.cbuf.add_text(&t);
            }
            if self.cbuf.take_wait() {
                return true;
            }
            let Some((line, origin)) = self.cbuf.next_line_from() else { break };
            self.exec_line_from(&line, origin);
            // `language`/`con_translate` apply to the very next message
            self.sync_language();
            if self.quit {
                break;
            }
        }
        false
    }

    pub(super) fn drain_server_log(&mut self) {
        // Most server log lines are also sent to the client (bprint, sprint);
        // echo only diagnostics unless `developer` is set.
        let log = std::mem::take(&mut self.server.core.log);
        let dev = self.dedicated || self.cv("developer") != 0.0;
        for s in log {
            if dev || s.starts_with("map_reload:") || ["rror", "ouldn't", "not ", "nknown", "valid", "usage", "<"].iter().any(|k| s.contains(k)) {
                let s = self.server.core.loc.translate(&s);
                self.con.print(&s);
            }
        }
    }

    /// Commands from the command line (`+map e1m1 +skill 2`).
    pub(super) fn stuff_cmds(&mut self) {
        if std::mem::replace(&mut self.stuffed, true) {
            return;
        }
        let mut out = String::new();
        for a in &self.cmdline {
            if let Some(rest) = a.strip_prefix('+') {
                out.push('\n');
                out.push_str(rest);
            } else if !out.is_empty() && !a.starts_with('-') {
                out.push(' ');
                if a.contains(' ') {
                    out.push_str(&format!("\"{a}\""));
                } else {
                    out.push_str(a);
                }
            }
        }
        out.push('\n');
        // on the web the "command line" comes with the link
        let origin = if cfg!(target_arch = "wasm32") { Origin::Script } else { Origin::User };
        self.cbuf.insert_text_from(out.trim_start(), origin);
    }

    pub(super) fn key_dest(&self) -> KeyDest {
        if self.menu.active() {
            KeyDest::Menu
        } else if self.con.open || self.con.forced {
            KeyDest::Console
        } else if self.con.chat_active {
            KeyDest::Message
        } else {
            KeyDest::Game
        }
    }

    pub(super) fn sync_key_dest(&mut self) {
        let d = self.key_dest();
        if self.input.dest != d {
            if d != KeyDest::Menu { self.cancel_menu_touch(); }
            qinput::set_key_dest(self, d);
            if d == KeyDest::Game && std::mem::take(&mut self.menu_axes) {
                use qinput::GamepadAxis::*;
                for axis in [LeftX, LeftY, RightX, RightY, LeftTrigger, RightTrigger] {
                    let _ = qinput::handle_event(self, InputEvent::GamepadAxis { axis, value: 0.0 });
                }
            }
        }
        if d != KeyDest::Console {
            self.touch_console_keyboard = false;
        }
        let text = match d {
            // Sign-on forces the console briefly during map changes and loads.
            KeyDest::Console => !cfg!(target_os = "android") || self.con.open || self.touch_console_keyboard,
            KeyDest::Message => true,
            KeyDest::Menu => self.menu.wants_text(),
            KeyDest::Game => false,
        };
        self.input.touch_typing = text && d == KeyDest::Menu && self.input.last_device == Device::Touch;
        if self.text_input != Some(text)
            && let Some(w) = &self.window
        {
            let _ = w.set_text_input(text);
            self.text_input = Some(text);
        }
        self.sync_mouse_grab();
    }

    /// `play`/`playvol`: S_StartSound at the listener with a fresh entity
    /// number each time (C's `hash++`), so sounds never cut each other off.
    pub(super) fn play_console_sound(&mut self, name: &str, volume: f32) {
        let name = if name.rsplit('/').next().is_some_and(|f| f.contains('.')) { name.to_string() } else { format!("{name}.wav") };
        self.console_sound_id = self.console_sound_id.wrapping_add(1);
        let entity = 345 + self.console_sound_id as i32;
        if let Some(s) = &mut self.snd
            && let Some(sfx) = s.precache(&name)
        {
            s.event(SoundEvent::Start { entity, channel: 0, sfx, origin: Default::default(), volume: volume.clamp(0.0, 1.0), attenuation: 0.0 });
        }
    }

    pub(super) fn local_sound(&mut self, name: &str) {
        if let Some(s) = &mut self.snd
            && let Some(sfx) = s.precache(name)
        {
            s.event(SoundEvent::Local(sfx));
        }
    }
}
