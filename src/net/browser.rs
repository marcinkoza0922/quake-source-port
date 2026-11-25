//! Internet & LAN server browser data model, filtering, sorting and
//! Favorites / Recent server persistence.

use std::path::Path;

#[path = "browser_query.rs"]
mod query;
pub use query::ServerSearch;
#[cfg(test)]
use query::{parse_master_reply, parse_server_reply};

/// Default NetQuake / FTE / DarkPlaces master servers (UDP port 27950).
pub const DEFAULT_NQ_MASTERS: &str = "master.frag-net.com:27950 dpmaster.deathmask.net:27950";
/// Default QuakeWorld master servers (UDP port 27000).
pub const DEFAULT_QW_MASTERS: &str = "master.quakeworld.nu:27000 qwmaster.ocrana.de:27000 qwmaster.fodquake.net:27000";

const MAX_RECENT: usize = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServerProto {
    #[default]
    NetQuake,
    QuakeWorld,
    FteDp,
}

impl ServerProto {
    pub fn tag(self) -> &'static str {
        match self {
            Self::NetQuake => "NQ",
            Self::QuakeWorld => "QW",
            Self::FteDp => "FTE",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "qw" | "QW" => Self::QuakeWorld,
            "fte" | "FTE" => Self::FteDp,
            _ => Self::NetQuake,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServerMode {
    #[default]
    Deathmatch,
    Coop,
    Teamplay,
}

impl ServerMode {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Deathmatch => "DM",
            Self::Coop => "COOP",
            Self::Teamplay => "TEAM",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "coop" | "COOP" => Self::Coop,
            "team" | "TEAM" => Self::Teamplay,
            _ => Self::Deathmatch,
        }
    }
}

/// A discovered or saved server shown in the server browser.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerEntry {
    pub address: String,
    pub hostname: String,
    pub map: String,
    pub players: u8,
    pub max_players: u8,
    pub ping_ms: Option<u32>,
    pub gamedir: String,
    pub mode: ServerMode,
    pub protocol: ServerProto,
    pub favorite: bool,
    pub recent: bool,
    pub lan: bool,
}

impl ServerEntry {
    /// Address argument for the `connect` console command (`qw://...` for QuakeWorld).
    pub fn connect_address(&self) -> String {
        let bare = self.address.strip_prefix("qw://").unwrap_or(&self.address);
        if self.protocol == ServerProto::QuakeWorld {
            format!("qw://{bare}")
        } else {
            bare.to_string()
        }
    }

    pub fn bare_address(&self) -> &str {
        self.address.strip_prefix("qw://").unwrap_or(&self.address)
    }

    pub fn effective_gamedir(&self) -> &str {
        if !self.gamedir.is_empty() {
            &self.gamedir
        } else if self.protocol == ServerProto::QuakeWorld {
            "qw"
        } else {
            "id1"
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServerTab {
    #[default]
    Lan,
    Internet,
}

impl ServerTab {
    pub const ALL: [Self; 2] = [Self::Lan, Self::Internet];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Lan => "slist",
            Self::Internet => "slist_internet",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PingFilter {
    #[default]
    Any,
    Below(u32),
    Unknown,
}

/// Active filters and sort order for one server list.
#[derive(Clone, Debug, Default)]
pub struct BrowserFilter {
    /// 0 = All, 1 = Favorites, 2 = Recent.
    pub saved: usize,
    /// 0 = All, 1 = NetQuake, 2 = QuakeWorld, 3 = FTE/DP.
    pub protocol: usize,
    /// 0 = All, 1 = Deathmatch, 2 = Cooperative, 3 = Teamplay.
    pub mode: usize,
    /// Empty = All mods, otherwise case-insensitive gamedir match.
    pub mod_filter: String,
    /// 0 = All, 1 = Empty, 2 = Populated, 3 = Open slots, 4 = Full.
    pub population: usize,
    pub ping: PingFilter,
    /// 0 = Ping, 1 = Players, 2 = Name, 3 = Map.
    pub sort: usize,
    /// Matches a substring of the server's name, address, map or gamedir.
    pub query: String,
}

impl BrowserFilter {
    pub fn has_constraints(&self) -> bool {
        self.saved != 0 || self.protocol != 0 || self.mode != 0 || !self.mod_filter.is_empty()
            || self.population != 0 || self.ping != PingFilter::Any
    }

    pub fn matches(&self, s: &ServerEntry) -> bool {
        if (self.saved == 1 && !s.favorite) || (self.saved == 2 && !s.recent) {
            return false;
        }
        let proto_ok = match self.protocol {
            1 => s.protocol == ServerProto::NetQuake,
            2 => s.protocol == ServerProto::QuakeWorld,
            3 => s.protocol == ServerProto::FteDp,
            _ => true,
        };
        if !proto_ok {
            return false;
        }
        let mode_ok = match self.mode {
            1 => s.mode == ServerMode::Deathmatch,
            2 => s.mode == ServerMode::Coop,
            3 => s.mode == ServerMode::Teamplay,
            _ => true,
        };
        if !mode_ok {
            return false;
        }
        if !self.mod_filter.is_empty() && !s.effective_gamedir().eq_ignore_ascii_case(&self.mod_filter) {
            return false;
        }
        let ping_ok = match self.ping {
            PingFilter::Any => true,
            PingFilter::Below(limit) => s.ping_ms.is_some_and(|ms| ms < limit),
            PingFilter::Unknown => s.ping_ms.is_none(),
        };
        if !ping_ok {
            return false;
        }
        let population_ok = match self.population {
            1 => s.players == 0,
            2 => s.players > 0,
            3 => s.players < s.max_players,
            4 => s.max_players > 0 && s.players >= s.max_players,
            _ => true,
        };
        if self.population != 0 && (s.ping_ms.is_none() || !population_ok) {
            return false;
        }
        self.query.is_empty()
            || [&s.hostname, &s.address, &s.map, s.effective_gamedir()]
                .iter()
                .any(|field| field.to_ascii_lowercase().contains(&self.query))
    }

    /// Returns indices into `servers` matching this filter, ordered by `self.sort`.
    pub fn filter_and_sort(&self, servers: &[ServerEntry]) -> Vec<usize> {
        let mut idxs: Vec<usize> = (0..servers.len()).filter(|&i| self.matches(&servers[i])).collect();
        idxs.sort_by(|&a, &b| {
            let (sa, sb) = (&servers[a], &servers[b]);
            match self.sort {
                1 => sb
                    .players
                    .cmp(&sa.players)
                    .then_with(|| sa.ping_ms.unwrap_or(u32::MAX).cmp(&sb.ping_ms.unwrap_or(u32::MAX))),
                2 => sa
                    .hostname
                    .to_ascii_lowercase()
                    .cmp(&sb.hostname.to_ascii_lowercase())
                    .then_with(|| a.cmp(&b)),
                3 => sa
                    .map
                    .to_ascii_lowercase()
                    .cmp(&sb.map.to_ascii_lowercase())
                    .then_with(|| sa.ping_ms.unwrap_or(u32::MAX).cmp(&sb.ping_ms.unwrap_or(u32::MAX))),
                _ => sa
                    .ping_ms
                    .unwrap_or(u32::MAX)
                    .cmp(&sb.ping_ms.unwrap_or(u32::MAX))
                    .then_with(|| sb.players.cmp(&sa.players))
                    .then_with(|| a.cmp(&b)),
            }
        });
        idxs
    }
}

/// Persistent Favorites and Recent server lists (`<userdata>/servers.txt`).
#[derive(Clone, Debug, Default)]
pub struct SavedServers {
    pub favorites: Vec<ServerEntry>,
    pub recent: Vec<ServerEntry>,
}

impl SavedServers {
    pub fn load(path: &Path) -> Self {
        let Ok(bytes) = qcore::vfs::read(path) else {
            return Self::default();
        };
        Self::parse(&String::from_utf8_lossy(&bytes))
    }

    pub fn save(&self, path: &Path) {
        let _ = qcore::vfs::write_atomic(path, self.serialize().as_bytes());
    }

    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }
            let kind = parts[0];
            let address = parts[1].trim().strip_prefix("qw://").unwrap_or(parts[1].trim()).to_string();
            if address.is_empty() {
                continue;
            }
            let protocol = parts.get(2).copied().map(ServerProto::parse).unwrap_or_default();
            let gamedir = parts.get(3).copied().unwrap_or("").to_string();
            let mode = parts.get(4).copied().map(ServerMode::parse).unwrap_or_default();
            let hostname = parts.get(5).copied().unwrap_or(&address).to_string();
            let map = parts.get(6).copied().unwrap_or("").to_string();
            let entry = ServerEntry {
                address,
                hostname,
                map,
                gamedir,
                mode,
                protocol,
                favorite: kind == "fav",
                recent: kind == "rec",
                ..Default::default()
            };
            if kind == "fav" {
                if !out.favorites.iter().any(|e| e.bare_address() == entry.bare_address()) {
                    out.favorites.push(entry);
                }
            } else if kind == "rec" && !out.recent.iter().any(|e| e.bare_address() == entry.bare_address()) {
                out.recent.push(entry);
            }
        }
        out
    }

    pub fn serialize(&self) -> String {
        let mut s = String::new();
        for (tag, list) in [("fav", &self.favorites), ("rec", &self.recent)] {
            for e in list {
                let clean = |v: &str| v.replace(['\t', '\n', '\r'], " ");
                s.push_str(&format!(
                    "{tag}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    clean(e.bare_address()),
                    e.protocol.tag(),
                    clean(e.effective_gamedir()),
                    e.mode.tag(),
                    clean(&e.hostname),
                    clean(&e.map),
                ));
            }
        }
        s
    }

    pub fn is_favorite(&self, addr: &str) -> bool {
        let bare = addr.trim().strip_prefix("qw://").unwrap_or(addr.trim());
        self.favorites.iter().any(|e| e.bare_address().eq_ignore_ascii_case(bare))
    }

    pub fn is_recent(&self, addr: &str) -> bool {
        let bare = addr.trim().strip_prefix("qw://").unwrap_or(addr.trim());
        self.recent.iter().any(|e| e.bare_address().eq_ignore_ascii_case(bare))
    }

    /// Toggles `entry` in favorites; returns `true` if now a favorite.
    pub fn toggle_favorite(&mut self, entry: &ServerEntry) -> bool {
        let bare = entry.bare_address();
        if let Some(pos) = self.favorites.iter().position(|e| e.bare_address().eq_ignore_ascii_case(bare)) {
            self.favorites.remove(pos);
            false
        } else {
            let mut e = entry.clone();
            e.address = bare.to_string();
            e.favorite = true;
            if e.hostname.is_empty() {
                e.hostname = bare.to_string();
            }
            self.favorites.push(e);
            true
        }
    }

    /// Records a connection in the recent list (most recent first, capped at `MAX_RECENT`).
    pub fn record_recent(&mut self, addr: &str, qw: bool, existing: Option<&ServerEntry>) {
        let bare = addr.trim().strip_prefix("qw://").unwrap_or(addr.trim());
        if bare.is_empty() {
            return;
        }
        let mut entry = existing.cloned().unwrap_or_else(|| ServerEntry {
            address: bare.to_string(),
            hostname: bare.to_string(),
            protocol: if qw { ServerProto::QuakeWorld } else { ServerProto::NetQuake },
            gamedir: if qw { "qw".into() } else { "id1".into() },
            ..Default::default()
        });
        entry.address = bare.to_string();
        entry.recent = true;
        entry.favorite = self.is_favorite(bare);
        self.recent.retain(|e| !e.bare_address().eq_ignore_ascii_case(bare));
        self.recent.insert(0, entry);
        self.recent.truncate(MAX_RECENT);
    }

    /// Updates flags on live entries and adds saved entries without a live reply.
    pub fn sync_into(&self, servers: &mut Vec<ServerEntry>) {
        servers.retain(|s| s.ping_ms.is_some());
        self.mark_entries(servers);
        for saved in self.favorites.iter().chain(self.recent.iter()) {
            let bare = saved.bare_address();
            if !servers.iter().any(|s| s.bare_address().eq_ignore_ascii_case(bare)) {
                let mut e = saved.clone();
                e.favorite = self.is_favorite(bare);
                e.recent = self.is_recent(bare);
                e.ping_ms = None;
                e.lan = false;
                servers.push(e);
            }
        }
    }

    pub fn mark_entries(&self, servers: &mut [ServerEntry]) {
        for s in servers {
            s.favorite = self.is_favorite(&s.address);
            s.recent = self.is_recent(&s.address);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn parses_qw_and_dp_master_replies() {
        let mut qw_pkt = b"\xff\xff\xff\xffd\n".to_vec();
        qw_pkt.extend([192, 168, 1, 10, 0x6b, 0x6c]); // 192.168.1.10:27500
        qw_pkt.extend([0, 0, 0, 0, 0, 0]);
        let (addrs, proto) = parse_master_reply(&qw_pkt).unwrap();
        assert_eq!(proto, ServerProto::QuakeWorld);
        assert_eq!(addrs, vec!["192.168.1.10:27500".parse::<SocketAddr>().unwrap()]);

        let mut dp_pkt = b"\xff\xff\xff\xffgetserversResponse\\".to_vec();
        dp_pkt.extend([10, 0, 0, 5, 0x65, 0x90]); // 10.0.0.5:26000
        dp_pkt.extend_from_slice(b"\\EOT\0\0\0");
        let (addrs, proto) = parse_master_reply(&dp_pkt).unwrap();
        assert_eq!(proto, ServerProto::FteDp);
        assert_eq!(addrs, vec!["10.0.0.5:26000".parse::<SocketAddr>().unwrap()]);
    }

    #[test]
    fn parses_qw_and_fte_server_status_and_sorts_filters() {
        let addr: SocketAddr = "203.0.113.5:27500".parse().unwrap();
        let qw_status = b"\xff\xff\xff\xffn\\hostname\\QuakeWorld DM\\map\\dm4\\*gamedir\\ktx\\maxclients\\8\\teamplay\\2\n1 10 5 24 \"Milton\" \"\" 4 4\n2 8 5 30 \"bps\" \"\" 13 13\n";
        let sv1 = parse_server_reply(qw_status, addr, Some(28), false).unwrap();
        assert_eq!(sv1.hostname, "QuakeWorld DM");
        assert_eq!(sv1.map, "dm4");
        assert_eq!(sv1.gamedir, "ktx");
        assert_eq!(sv1.players, 2);
        assert_eq!(sv1.max_players, 8);
        assert_eq!(sv1.mode, ServerMode::Teamplay);
        assert_eq!(sv1.protocol, ServerProto::QuakeWorld);
        assert_eq!(sv1.connect_address(), "qw://203.0.113.5:27500");

        let addr2: SocketAddr = "198.51.100.9:26000".parse().unwrap();
        let fte_info = b"\xff\xff\xff\xffinfoResponse\n\\hostname\\Empty NQ\\mapname\\e1m1\\modname\\id1\\clients\\0\\sv_maxclients\\16\n";
        let sv2 = parse_server_reply(fte_info, addr2, Some(12), false).unwrap();
        assert_eq!(sv2.players, 0);
        assert_eq!(sv2.ping_ms, Some(12));

        let list = vec![sv1.clone(), sv2.clone()];
        let mut f = BrowserFilter::default();
        // Default sort is Ping ascending: sv2 (12 ms) before sv1 (28 ms).
        assert_eq!(f.filter_and_sort(&list), vec![1, 0]);
        // Hide Empty filters out sv2.
        f.population = 2;
        assert_eq!(f.filter_and_sort(&list), vec![0]);
        // Mod filter "ktx" matches sv1.
        f.mod_filter = "ktx".into();
        assert_eq!(f.filter_and_sort(&list), vec![0]);
    }

    #[test]
    fn favorites_and_recent_persistence_round_trip() {
        let mut saved = SavedServers::default();
        let entry = ServerEntry {
            address: "10.1.2.3:27500".into(),
            hostname: "FavQW".into(),
            map: "aerowalk".into(),
            gamedir: "ktx".into(),
            mode: ServerMode::Deathmatch,
            protocol: ServerProto::QuakeWorld,
            ..Default::default()
        };
        assert!(saved.toggle_favorite(&entry));
        assert!(saved.is_favorite("qw://10.1.2.3:27500"));
        saved.record_recent("192.168.1.2:26000", false, None);
        assert!(saved.is_recent("192.168.1.2:26000"));

        let text = saved.serialize();
        let loaded = SavedServers::parse(&text);
        assert!(loaded.is_favorite("10.1.2.3:27500"));
        assert!(loaded.is_recent("192.168.1.2:26000"));
    }
}
