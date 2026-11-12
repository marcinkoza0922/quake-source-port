use super::*;

impl NetServer {
    pub(super) fn control(&mut self, sv: &mut Server, from: SocketAddr, q: Ccreq, now: f64, hostname: &str) {
        let port = self.port() as i32;
        match q {
            Ccreq::Connect { game, version } => {
                if game != "QUAKE" {
                    return;
                }
                if version != NET_PROTOCOL_VERSION {
                    return self.reply(from, Ccrep::Reject { reason: "Incompatible version.\n".into() });
                }
                if !sv.active() {
                    return self.reply(from, Ccrep::Reject { reason: "Server is not running.\n".into() });
                }
                // A retransmitted request from a connected client: accept again
                // if nothing has been exchanged yet.
                if let Some(i) = self.remotes.iter().position(|r| r.addr == from) {
                    if now - self.remotes[i].connect_time < 2.0 && self.remotes[i].closing.is_none() {
                        return self.reply(from, Ccrep::Accept { port });
                    }
                    let slot = self.remotes[i].slot;
                    if self.remotes[i].closing.is_none() {
                        sv.drop_client(slot, true);
                    }
                    self.remotes.remove(i);
                }
                let Some(slot) = sv.core.clients.iter().position(|c| !c.active) else {
                    return self.reply(from, Ccrep::Reject { reason: "Server is full.\n".into() });
                };
                sv.connect_client(slot);
                if !sv.core.clients[slot].active {
                    return self.reply(from, Ccrep::Reject { reason: "Server is not running.\n".into() });
                }
                sv.core.clients[slot].max_datagram = qserver::msg::DATAGRAM_MTU;
                sv.core.print(&format!("Client {from} connected\n"));
                self.remotes.push(Remote { addr: from, slot, conn: Connection::new(), connect_time: now, last_send: now, closing: None });
                self.reply(from, Ccrep::Accept { port });
            }
            Ccreq::ServerInfo { game, version } => {
                if game != "QUAKE" || version != NET_PROTOCOL_VERSION {
                    return;
                }
                let c = &sv.core;
                self.reply(from, Ccrep::ServerInfo {
                    address: format!("{}", self.sock.local_addr()),
                    hostname: hostname.into(),
                    level: c.sv.name.clone(),
                    players: c.clients.iter().filter(|c| c.active).count() as u8,
                    max_players: c.maxclients() as u8,
                    version: NET_PROTOCOL_VERSION,
                });
            }
            Ccreq::PlayerInfo { player } => {
                let active: Vec<usize> = (0..sv.core.maxclients()).filter(|&i| sv.core.clients[i].active).collect();
                let Some(&slot) = active.get(player as usize) else { return };
                let cl = &sv.core.clients[slot];
                let frags = sv.level.as_ref().map_or(0, |l| l.vm.ent_f(slot as u32 + 1, sv.core.sv.fields.frags) as i32);
                let rem = self.remotes.iter().find(|r| r.slot == slot);
                let p = Ccrep::PlayerInfo {
                    player,
                    name: cl.name.clone(),
                    colors: cl.colors,
                    frags,
                    connect_time: rem.map_or(0, |r| (now - r.connect_time) as i32),
                    address: rem.map_or("local".into(), |r| r.addr.to_string()),
                };
                self.reply(from, p);
            }
            Ccreq::RuleInfo { prev } => {
                let names: Vec<String> = sv.core.cvars.iter().filter(|c| c.flags & qcore::cvar::CVAR_SERVERINFO != 0).map(|c| c.name.clone()).collect();
                let next = if prev.is_empty() {
                    names.first()
                } else {
                    names.iter().position(|n| n.eq_ignore_ascii_case(&prev)).and_then(|i| names.get(i + 1))
                };
                let p = match next {
                    Some(n) => Ccrep::RuleInfo { rule: n.clone(), value: sv.core.cvars.by_name(n).map_or(String::new(), |c| c.string.clone()) },
                    None => Ccrep::RuleInfo { rule: String::new(), value: String::new() },
                };
                self.reply(from, p);
            }
        }
    }
}
