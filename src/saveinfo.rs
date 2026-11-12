//! Read previews from ordinary v5 saves without starting a server or loading game assets.
use qcore::cmd::com_parse;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct SaveInfo {
    pub level: String,
    pub timestamp: String,
    pub health: i32,
    pub armor: i32,
    pub items: u32,
    pub kills: (i32, i32),
    pub secrets: (i32, i32),
    pub reason: Option<String>,
    pub player: std::collections::HashMap<String, String>,
}

fn block<'a>(data: &mut &'a str) -> Option<Vec<(&'a str, &'a str)>> {
    let (token, rest) = com_parse(data)?;
    if token != "{" { return None; }
    *data = rest;
    let mut pairs = Vec::new();
    loop {
        let (key, rest) = com_parse(data)?;
        *data = rest;
        if key == "}" { return Some(pairs); }
        let (value, rest) = com_parse(data)?;
        *data = rest;
        pairs.push((key, value));
    }
}

impl SaveInfo {
    pub fn parse(text: &str, modified: Option<SystemTime>) -> Option<Self> {
        let mut lines = text.lines();
        if lines.next()?.trim() != "5" { return None; }
        let comment = lines.next()?;
        for _ in 0..17 { lines.next()?; } // spawn parms and skill
        let map = lines.next()?;
        lines.next()?; // server time
        for _ in 0..64 { lines.next()?; }
        let offset: usize = text.split_inclusive('\n').take(85).map(str::len).sum();
        let mut data = text.get(offset..)?;
        let globals = block(&mut data)?;
        let world = block(&mut data)?;
        let player = block(&mut data)?;
        let value = |pairs: &[(&str, &str)], name: &str| pairs.iter().find(|p| p.0 == name).map_or("0", |p| p.1).parse::<f64>().unwrap_or(0.0) as i32;
        let level = world.iter().find(|p| p.0 == "message").map_or(map, |p| p.1).replace("\\n", " ");
        Some(Self {
            level,
            timestamp: modified.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or_else(|| "Date unavailable".into(), |d| timestamp(d.as_secs())),
            health: value(&player, "health"), armor: value(&player, "armorvalue"),
            items: player.iter().find(|p| p.0 == "items").and_then(|p| p.1.parse::<f64>().ok()).unwrap_or(0.0) as u32,
            kills: (value(&globals, "killed_monsters"), value(&globals, "total_monsters")),
            secrets: (value(&globals, "found_secrets"), value(&globals, "total_secrets")),
            player: player.iter().map(|(k,v)| (k.to_string(),v.to_string())).collect(),
            reason: comment.strip_prefix("AUTOSAVE_").map(|s| s.replace('_', " ")),
        })
    }

    pub fn bits(&self, name: &str) -> u32 {
        self.player.get(name).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as u32
    }

}

/// Gregorian civil date from Unix days; show UTC explicitly so previews are portable.
fn timestamp(seconds: u64) -> String {
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC", seconds / 3600 % 24, seconds / 60 % 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_server_save_preview_and_autosave_restore() {
        let Some(base) = std::env::var_os("QUAKE_BASEDIR") else { return };
        let mut fs = qcore::fs::Fs::new(base);
        fs.add_game_dir("id1");
        if fs.open("maps/e1m1.bsp").is_none() { return; }
        let mut server = qserver::Server::new(fs, qcore::cvar::Cvars::new(), 1);
        server.spawn_server("e1m1").unwrap();
        server.connect_local().unwrap();
        for _ in 0..20 { server.frame(1.0 / 72.0); }
        let text = server.save_game();
        let preview = SaveInfo::parse(&text, None).unwrap();
        let stats = server.stats();
        assert_eq!(preview.kills, (stats.killed_monsters, stats.total_monsters));
        assert_eq!(preview.secrets, (stats.found_secrets, stats.total_secrets));
        assert!(preview.health > 0);
        let mut lines = text.splitn(3, '\n');
        let version = lines.next().unwrap(); lines.next();
        let autosave = format!("{version}\nAUTOSAVE_Level_entry\n{}", lines.next().unwrap());
        let preview = SaveInfo::parse(&autosave, None).unwrap();
        assert_eq!(preview.reason.as_deref(), Some("Level entry"));
        server.load_game(&autosave).unwrap();
        assert_eq!(server.stats(), stats);
        assert!(server.core.sv.loadgame);
    }

    #[test]
    fn legacy_save_preview_and_autosave_reason() {
        let header = format!("5\nlegacy_comment\n{}1\ne1m1\n12\n{}", "0\n".repeat(16), "m\n".repeat(64));
        let body = "{\n\"killed_monsters\" \"3\"\n\"total_monsters\" \"7\"\n\"found_secrets\" \"1\"\n\"total_secrets\" \"2\"\n}\n{\n\"message\" \"The Slipgate Complex\"\n}\n{\n\"health\" \"87\"\n\"armorvalue\" \"50\"\n\"items\" \"131075\"\n}\n";
        let text = header + body;
        let s = SaveInfo::parse(&text, Some(UNIX_EPOCH)).unwrap();
        assert_eq!(s.level, "The Slipgate Complex");
        assert_eq!((s.health, s.armor), (87, 50));
        assert_eq!(s.kills, (3, 7)); assert_eq!(s.secrets, (1, 2));
        assert_eq!(s.bits("items"), 131075);
        assert_eq!(s.timestamp, "1970-01-01 00:00:00 UTC");
        assert!(s.reason.is_none());
        assert_eq!(SaveInfo::parse(&text.replace("legacy_comment", "AUTOSAVE_Secret_found"), None).unwrap().reason.as_deref(), Some("Secret found"));
        assert!(SaveInfo::parse("5\ntruncated", None).is_none());
    }
}
