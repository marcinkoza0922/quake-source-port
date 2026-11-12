use super::*;

impl Host {
    /// Cmd_ExecuteString with server-command forwarding.
    pub fn exec_line(&mut self, line: &str) {
        let args = Args::tokenize(line);
        if args.argc() == 0 {
            return;
        }
        let name = args.argv(0).to_ascii_lowercase();
        let known = self.cmds.exists(&name) || self.cmds.alias(&name).is_some() || self.server.core.cvars.find(&name).is_some();
        if known {
            qcore::cmd::execute_string(self, line);
        } else if self.remote() && !matches!(name.as_str(), "map" | "maxplayers" | "restart" | "changelevel") {
            // Cmd_ForwardToServer. A typed `ping` prints its reply.
            if name == "ping" {
                self.ping_expect = None;
                self.ping_next = self.realtime + 3.0;
            }
            self.cl.send_string_cmd(line);
        } else if self.server.has_command(&name) {
            self.server.command(line);
        } else if let Some(i) = self.local.filter(|_| self.server.active()) {
            self.server.client_command(i, line);
        } else {
            self.con.print(&format!("Unknown command \"{}\"\n", args.argv(0)));
        }
        self.drain_server_log();
    }

    pub(super) fn exec_cbuf(&mut self) {
        loop {
            // One shared Cbuf as in Quake: QC localcmd text runs here, so it
            // reaches host commands too.
            let t = self.server.take_text();
            if !t.is_empty() {
                self.cbuf.add_text(&t);
            }
            if self.cbuf.take_wait() {
                break;
            }
            let Some(line) = self.cbuf.next_line() else { break };
            self.exec_line(&line);
            if self.quit {
                break;
            }
        }
    }

    pub(super) fn drain_server_log(&mut self) {
        // Most server log lines are also sent to the client (bprint, sprint);
        // echo only diagnostics unless `developer` is set.
        let log = std::mem::take(&mut self.server.core.log);
        let dev = self.dedicated || self.cv("developer") != 0.0;
        for s in log {
            if dev || ["rror", "ouldn't", "not ", "nknown", "valid", "usage", "<"].iter().any(|k| s.contains(k)) {
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
        self.cbuf.insert_text(out.trim_start());
    }

    pub(super) fn key_dest(&self) -> KeyDest {
        if self.menu.active() {
            KeyDest::Menu
        } else if self.con.open || self.con.forced {
            KeyDest::Console
        } else {
            KeyDest::Game
        }
    }

    pub(super) fn sync_key_dest(&mut self) {
        let d = self.key_dest();
        if self.input.dest != d {
            qinput::set_key_dest(self, d);
            if let Some(w) = &self.window {
                let _ = w.set_relative_mouse(d == KeyDest::Game);
                let _ = w.set_text_input(d == KeyDest::Console);
            }
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
