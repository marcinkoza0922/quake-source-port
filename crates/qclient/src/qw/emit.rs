use super::*;

impl QwState {
    pub(super) fn emit_cdtrack(&self, b: &mut Nq) {
        b.byte(nq::SVC_CDTRACK);
        b.byte(self.cdtrack);
        b.byte(self.cdtrack);
    }

    pub(super) fn emit_frags(&self, b: &mut Nq, i: usize) {
        if self.nq_ready {
            b.byte(nq::SVC_UPDATEFRAGS);
            b.byte(i as u8);
            b.0.short(self.players[i].frags);
        }
    }

    pub(super) fn emit_player(&self, b: &mut Nq, i: usize) {
        if !self.nq_ready {
            return;
        }
        let p = &self.players[i];
        b.byte(nq::SVC_UPDATENAME);
        b.byte(i as u8);
        b.string(&p.name);
        b.byte(nq::SVC_UPDATECOLORS);
        b.byte(i as u8);
        b.byte(((p.topcolor.clamp(0, 15) << 4) | p.bottomcolor.clamp(0, 15)) as u8);
    }

    /// CL_ProcessUserInfo.
    pub(super) fn process_userinfo(&mut self, i: usize, b: &mut Nq) {
        let p = &mut self.players[i];
        p.name = info_value(&p.userinfo, "name").to_string();
        p.topcolor = info_value(&p.userinfo, "topcolor").parse().unwrap_or(0);
        p.bottomcolor = info_value(&p.userinfo, "bottomcolor").parse().unwrap_or(0);
        p.spectator = !info_value(&p.userinfo, "*spectator").is_empty();
        self.emit_player(b, i);
    }

    /// Stuffed command lines: the signon commands are handled here, the
    /// rest go to the console as NQ stufftext.
    pub(super) fn stuff_text(&mut self, s: &str, body: &mut Nq, out: &mut Parsed) {
        self.stuffbuf.push_str(s);
        while let Some(nl) = self.stuffbuf.find('\n') {
            let line: String = self.stuffbuf.drain(..=nl).collect();
            let t = line.trim();
            let (word, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
            let rest = rest.trim();
            match word {
                "cmd" => {
                    if rest.starts_with("spawn") && self.nq_signon == 1 {
                        self.nq_signon = 2;
                        body.byte(nq::SVC_SIGNONNUM);
                        body.byte(2);
                    }
                    out.actions.push(QwAction::Cmd(rest.to_string()));
                }
                "skins" => {
                    if self.nq_signon == 2 {
                        self.nq_signon = 3;
                        body.byte(nq::SVC_SIGNONNUM);
                        body.byte(3);
                    }
                    out.actions.push(QwAction::Cmd(format!("begin {}", self.servercount)));
                }
                "reconnect" => out.actions.push(QwAction::Reconnect),
                "changing" | "nextul" | "stopul" => {}
                "fullserverinfo" => self.serverinfo = rest.trim_matches('"').to_string(),
                "packet" => {
                    let (addr, data) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                    let data = data.trim().trim_matches('"').replace("\\n", "\n");
                    out.actions.push(QwAction::Packet { addr: addr.to_string(), data });
                }
                _ => {
                    body.byte(nq::SVC_STUFFTEXT);
                    body.string(&line);
                }
            }
        }
    }
}
