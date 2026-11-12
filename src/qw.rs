//! QuakeWorld client connection (QW/client cl_main.c connect/resend,
//! cl_parse.c downloads, net_chan.c over UDP).
//!
//! [`QwClient`] owns the socket, the out-of-band challenge/connect
//! handshake, the [`Netchan`] and the [`QwState`] translator. The host
//! polls it every frame for NetQuake-format messages, sends one move
//! command per server tic, and lets it predict the local player.

use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Arc;

use qclient::qw::{QwAction, QwState, Stage};
use qcore::fs::Fs;
use qnet::qw::{self as proto, Netchan, UserCmd};
use qnet::{Transport, UdpTransport};
use qworld::CollisionModel;

/// cl_timeout default.
pub const TIMEOUT: f64 = 60.0;
const RESEND: f64 = 2.5;
const TRIES: u32 = 4;

/// Resolve `host[:port]` with the QW default port (27500).
pub fn resolve(s: &str) -> Option<SocketAddr> {
    let s = s.trim();
    let s = s.strip_prefix("qw://").unwrap_or(s).trim_end_matches('/');
    let with_port = if s.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        s.to_string()
    } else {
        format!("{s}:{}", proto::PORT_SERVER)
    };
    let with_port = with_port.replace("localhost", "127.0.0.1");
    with_port.to_socket_addrs().ok()?.find(SocketAddr::is_ipv4)
}

enum Conn {
    /// Sent getchallenge (tries, last send time).
    Challenging(u32, f64),
    /// Sent connect with this challenge.
    Connecting(i32, u32, f64),
    Connected(Netchan),
}

struct Download {
    name: String,
    data: Vec<u8>,
}

/// What [`QwClient::poll`] produced.
pub enum QwPoll {
    /// NQ messages for the client, console lines.
    Messages(Vec<Vec<u8>>, Vec<String>),
    Failed(String),
}

pub struct QwClient {
    sock: UdpTransport,
    pub server: SocketAddr,
    conn: Conn,
    pub qw: QwState,
    qport: u16,
    pub userinfo: String,
    /// Collision model of the current map (prediction).
    pub world: Option<Arc<CollisionModel>>,
    /// Files still to check during precache.
    precache: Option<Vec<String>>,
    download: Option<Download>,
    /// Where downloads go (mounted on the fs search path).
    dl_dir: Option<PathBuf>,
    /// Overrides the user data root for downloads (tests).
    pub download_root: Option<PathBuf>,
    last_cmd: f64,
    buf: Vec<u8>,
}

impl QwClient {
    pub fn connect(server: SocketAddr, userinfo: String, now: f64) -> std::io::Result<Self> {
        let sock = UdpTransport::bind((Ipv4Addr::UNSPECIFIED, 0).into())?;
        // Netchan_Init: "a value that should be nice and random".
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
        let qport = ((t ^ std::process::id().wrapping_mul(2_654_435_761)) & 0xffff) as u16;
        Ok(Self {
            sock,
            server,
            conn: Conn::Challenging(0, now - RESEND * 2.0),
            qw: QwState::new(),
            qport,
            userinfo,
            world: None,
            precache: None,
            download: None,
            dl_dir: None,
            download_root: None,
            last_cmd: now,
            buf: vec![0; 8192],
        })
    }

    pub fn connected(&self) -> bool {
        matches!(self.conn, Conn::Connected(_))
    }

    fn oob(&mut self, text: &str) {
        let _ = self.sock.send_to(&proto::oob(text.as_bytes()), self.server);
    }

    /// Queue a reliable clc_stringcmd.
    pub fn stringcmd(&mut self, s: &str) {
        if let Conn::Connected(c) = &mut self.conn {
            c.message.byte(proto::CLC_STRINGCMD as i32);
            c.message.string(s.as_bytes());
        }
    }

    /// Update one userinfo key locally and on the server.
    pub fn setinfo(&mut self, key: &str, value: &str) {
        proto::info_set(&mut self.userinfo, key, value);
        self.stringcmd(&format!("setinfo \"{key}\" \"{value}\""));
    }

    /// CL_Disconnect: "drop" sent unreliably three times.
    pub fn disconnect(&mut self) {
        if let Conn::Connected(c) = &mut self.conn {
            let mut b = vec![proto::CLC_STRINGCMD];
            b.extend_from_slice(b"drop\0");
            for _ in 0..3 {
                let p = c.transmit(&b);
                let _ = self.sock.send_to(&p, self.server);
            }
        }
    }

    /// Resend handshake packets, read everything pending, run the
    /// translator and its actions.
    pub fn poll(&mut self, fs: &mut Fs, now: f64) -> QwPoll {
        let mut prints = Vec::new();
        match self.conn {
            Conn::Challenging(tries, last) if now - last >= RESEND => {
                if tries >= TRIES {
                    return QwPoll::Failed("No response from server\n".into());
                }
                self.conn = Conn::Challenging(tries + 1, now);
                self.oob("getchallenge\n");
            }
            Conn::Connecting(ch, tries, last) if now - last >= RESEND => {
                if tries >= TRIES {
                    return QwPoll::Failed("No response from server\n".into());
                }
                self.conn = Conn::Connecting(ch, tries + 1, now);
                self.send_connect(ch);
            }
            _ => {}
        }
        let mut nq = Vec::new();
        loop {
            let (n, from) = match self.sock.recv_from(&mut self.buf) {
                Ok(Some(x)) => x,
                Ok(None) => break,
                Err(e) => return QwPoll::Failed(format!("{e}\n")),
            };
            if from != self.server {
                continue;
            }
            let pkt = self.buf[..n].to_vec();
            if let Some(p) = proto::oob_payload(&pkt) {
                if let Some(e) = self.connectionless(p, now, &mut prints) {
                    return QwPoll::Failed(e);
                }
                continue;
            }
            let Conn::Connected(c) = &mut self.conn else { continue };
            let Some(payload) = c.process(&pkt, now) else { continue };
            let (ack, inseq) = (c.incoming_acknowledged, c.incoming_sequence);
            let parsed = match self.qw.parse(payload, ack, inseq, now) {
                Ok(p) => p,
                Err(e) => return QwPoll::Failed(format!("{e}\n")),
            };
            nq.extend(parsed.nq);
            for a in parsed.actions {
                if let Err(e) = self.action(a, fs, &mut nq, &mut prints) {
                    return QwPoll::Failed(e);
                }
            }
        }
        if let Conn::Connected(c) = &self.conn
            && now - c.last_received > TIMEOUT
        {
            return QwPoll::Failed("Server connection timed out\n".into());
        }
        QwPoll::Messages(nq, prints)
    }

    fn send_connect(&mut self, challenge: i32) {
        let ui = self.userinfo.replace('"', "");
        let s = format!("connect {} {} {challenge} \"{ui}\"\n", proto::PROTOCOL_VERSION, self.qport);
        self.oob(&s);
    }

    /// Connectionless packets: challenge, accept, prints. Some(error) ends the connection.
    fn connectionless(&mut self, p: &[u8], now: f64, prints: &mut Vec<String>) -> Option<String> {
        let (&c, rest) = p.split_first()?;
        let text = String::from_utf8_lossy(rest.split(|&b| b == 0).next().unwrap_or(rest)).into_owned();
        match c {
            proto::S2C_CHALLENGE if matches!(self.conn, Conn::Challenging(..)) => {
                let ch = text.trim().parse::<i32>().unwrap_or(0);
                self.conn = Conn::Connecting(ch, 1, now);
                self.send_connect(ch);
            }
            proto::S2C_CONNECTION if matches!(self.conn, Conn::Connecting(..)) => {
                let mut nc = Netchan::new(self.qport, now);
                nc.message.byte(proto::CLC_STRINGCMD as i32);
                nc.message.string(b"new");
                self.conn = Conn::Connected(nc);
                self.qw = QwState::new();
                prints.push(format!("Connected to {}\n", self.server));
            }
            proto::A2C_PRINT => {
                // A print before we are connected is a rejection.
                if !self.connected() {
                    return Some(text);
                }
                prints.push(text);
            }
            proto::A2A_PING => self.oob(&(proto::A2A_ACK as char).to_string()),
            _ => {}
        }
        None
    }

    fn action(&mut self, a: QwAction, fs: &mut Fs, nq: &mut Vec<Vec<u8>>, prints: &mut Vec<String>) -> Result<(), String> {
        match a {
            QwAction::Cmd(c) => self.stringcmd(&c),
            QwAction::Precache => {
                self.mount_download_dir(fs);
                self.precache = Some(self.qw.needed_files().into_iter().rev().collect());
                self.next_precache(fs, nq, prints)?;
            }
            QwAction::Download { size, percent, data } => {
                let Some(dl) = &mut self.download else { return Ok(()) };
                if size < 0 {
                    prints.push(format!("File not found: {}\n", dl.name));
                    self.download = None;
                    return self.next_precache(fs, nq, prints);
                }
                dl.data.extend_from_slice(&data);
                if percent != 100 {
                    self.stringcmd("nextdl");
                    return Ok(());
                }
                let dl = self.download.take().unwrap();
                if let Some(dir) = &self.dl_dir {
                    let path = dir.join(&dl.name);
                    let tmp = path.with_extension("tmp");
                    let ok = path.parent().is_some_and(|p| std::fs::create_dir_all(p).is_ok())
                        && std::fs::write(&tmp, &dl.data).is_ok()
                        && std::fs::rename(&tmp, &path).is_ok();
                    prints.push(if ok { format!("Downloaded {}\n", dl.name) } else { format!("Couldn't write {}\n", path.display()) });
                }
                self.next_precache(fs, nq, prints)?;
            }
            QwAction::Packet { addr, data } => {
                if let Some(a) = resolve(&addr) {
                    let _ = self.sock.send_to(&proto::oob(data.as_bytes()), a);
                }
            }
            QwAction::Reconnect => {
                self.qw.reconnect();
                self.world = None;
                self.precache = None;
                self.download = None;
                self.stringcmd("new");
            }
            QwAction::Disconnect(why) => return Err(why),
        }
        Ok(())
    }

    /// Downloads go to the user data dir of the server's gamedir.
    fn mount_download_dir(&mut self, fs: &mut Fs) {
        let game = if self.qw.gamedir.is_empty() { "qw" } else { self.qw.gamedir.as_str() };
        let dir = match &self.download_root {
            Some(r) => r.join(game.to_ascii_lowercase()),
            None => crate::user_dir(game),
        };
        if self.dl_dir.as_ref() != Some(&dir) {
            fs.add_user_dir(&dir);
            self.dl_dir = Some(dir);
        }
    }

    /// Model_NextDownload / Sound_NextDownload: request the next missing
    /// file, or finish the precache once all are present.
    fn next_precache(&mut self, fs: &mut Fs, nq: &mut Vec<Vec<u8>>, prints: &mut Vec<String>) -> Result<(), String> {
        let Some(queue) = &mut self.precache else { return Ok(()) };
        while let Some(f) = queue.pop() {
            if f.contains("..") || fs.exists(&f) {
                continue;
            }
            prints.push(format!("Downloading {f}...\n"));
            self.download = Some(Download { name: f.clone(), data: Vec::new() });
            self.stringcmd(&format!("download {f}"));
            return Ok(());
        }
        self.precache = None;
        let map = self.qw.model_names.get(1).cloned().unwrap_or_default();
        let Some(bsp) = fs.open(&map) else {
            return Err(format!("The required map {map} could not be found or downloaded.\n"));
        };
        let sum = proto::map_checksum2(&bsp).ok_or_else(|| format!("{map} is not a valid map\n"))?;
        self.world = qformats::Bsp::parse(&bsp).ok().map(|b| Arc::new(CollisionModel::from_bsp(&b)));
        // The server checks the player and eyes models (cl_parse.c, CRC of the file).
        for (key, file) in [("pmodel", "progs/player.mdl"), ("emodel", "progs/eyes.mdl")] {
            if let Some(d) = fs.open(file) {
                self.setinfo(key, &proto::crc_block(&d).to_string());
            }
        }
        let (msg, cmd) = self.qw.finish_precache(sum);
        nq.push(msg);
        self.stringcmd(&cmd);
        Ok(())
    }

    /// CL_SendCmd: one move per call (the host calls it per server tic).
    /// Before the game is active the packet still carries reliable data.
    pub fn send_cmd(&mut self, mut cmd: UserCmd, now: f64) {
        let Conn::Connected(c) = &mut self.conn else { return };
        cmd.msec = ((now - self.last_cmd) * 1000.0).round().clamp(0.0, 250.0) as u8;
        self.last_cmd = now;
        if self.qw.stage != Stage::Active {
            cmd = UserCmd { msec: cmd.msec, angles: cmd.angles, ..Default::default() };
        }
        self.qw.outgoing_sequence = c.outgoing_sequence;
        let lost = (c.drop.min(100)) as u8;
        let m = self.qw.build_cmd(cmd, lost, now);
        let p = c.transmit(&m);
        let _ = self.sock.send_to(&p, self.server);
    }

    /// CL_PredictMove for the local player.
    pub fn predict(&mut self, now: f64, nopred: bool) {
        if let Some(w) = &self.world {
            self.qw.predict(w, now, nopred);
        }
    }
}

#[cfg(test)]
mod tests;
