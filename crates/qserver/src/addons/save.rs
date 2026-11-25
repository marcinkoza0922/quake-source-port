//! Add-on savegames. A game started with add-ons enabled saves as `.qsa`
//! (not `.sav`) and begins with a header no other port accepts, followed by
//! an ordinary version 5 body:
//!
//! ```text
//! QSPADDONS 1 <n>      magic, header version, number of lines that follow
//! toy portalgun        one line per enabled toy
//! portal 0 0 ox oy oz nx ny nz ux uy uz
//! 5                    the v5 savegame starts here
//! ```
//!
//! Other engines read a version number on the first line and refuse the
//! file ("Savegame is version QSPADDONS, not 5"); they never list `.qsa`.

use super::Toy;

pub const MAGIC: &str = "QSPADDONS";
const HEADER_VERSION: u32 = 1;

/// Player slots and rules of a multiplayer (split-screen, co-op) save.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Players {
    /// maxplayers
    pub slots: usize,
    /// Local split-screen players (`cl_splitscreen`).
    pub local: usize,
    pub coop: f32,
    pub deathmatch: f32,
    pub teamplay: f32,
}

impl Players {
    /// `players <slots> <local> <coop> <deathmatch> <teamplay>`
    pub fn line(&self) -> String {
        format!("players {} {} {} {} {}\n", self.slots, self.local, self.coop, self.deathmatch, self.teamplay)
    }
    fn parse(rest: &str) -> Option<Self> {
        let w: Vec<f32> = rest.split_whitespace().filter_map(|v| v.parse().ok()).collect();
        let [slots, local, coop, deathmatch, teamplay] = w[..] else { return None };
        let max = crate::server::MAX_SCOREBOARD as f32;
        if !w.iter().all(|v| v.is_finite()) || !(1.0..=max).contains(&slots) {
            return None;
        }
        let slots = slots as usize;
        Some(Players { slots, local: (local as usize).clamp(1, slots), coop, deathmatch, teamplay })
    }
}

/// State stored in the `.qsa` header.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    /// Not a customized game (a split-screen or co-op save of a vanilla
    /// game); headers without the `vanilla` line are customized.
    pub vanilla: bool,
    pub players: Option<Players>,
    pub toys: Vec<Toy>,
    /// `Portals::save_text` lines.
    pub portals: String,
    /// Other toys' state lines (`<key> ...`), kept verbatim.
    pub extra: String,
}

/// Prefixes a v5 savegame with the add-on header.
pub fn wrap(toys: &[Toy], portals: Option<&str>, body: &str) -> String {
    wrap_with(toys, portals, "", body)
}

/// [`wrap`] with further whole header lines (`extra`, e.g. `sticky ...`).
pub fn wrap_with(toys: &[Toy], portals: Option<&str>, extra: &str, body: &str) -> String {
    let mut lines: Vec<String> = toys.iter().map(|t| format!("toy {}", t.key())).collect();
    lines.extend(portals.unwrap_or("").lines().map(|l| format!("portal {l}")));
    lines.extend(extra.lines().filter(|l| !l.is_empty()).map(str::to_string));
    let mut s = format!("{MAGIC} {HEADER_VERSION} {}\n", lines.len());
    for l in lines {
        s += &l;
        s.push('\n');
    }
    s + body
}

/// Splits a savegame into its add-on header (None for a plain v5 save) and
/// the v5 body. Errors on a damaged or newer header.
pub fn split(text: &str) -> Result<(Option<Header>, &str), String> {
    let first = text.lines().next().unwrap_or("");
    let mut w = first.split_whitespace();
    if w.next() != Some(MAGIC) {
        return Ok((None, text));
    }
    let version: u32 = w.next().and_then(|v| v.parse().ok()).ok_or("bad add-on save header")?;
    if version != HEADER_VERSION {
        return Err(format!("add-on savegame header is version {version}, not {HEADER_VERSION}"));
    }
    let n: usize = w.next().and_then(|v| v.parse().ok()).ok_or("bad add-on save header")?;
    let mut h = Header::default();
    let mut rest = &text[first.len()..];
    for _ in 0..n {
        rest = rest.strip_prefix('\r').unwrap_or(rest).strip_prefix('\n').ok_or("truncated add-on save header")?;
        let line = rest.lines().next().unwrap_or("");
        rest = &rest[line.len()..];
        if line.trim() == "vanilla" {
            h.vanilla = true;
            continue;
        }
        match line.split_once(' ') {
            Some(("toy", name)) => h.toys.extend(Toy::from_key(name.trim())),
            Some(("players", p)) => h.players = Players::parse(p),
            Some(("portal", p)) => {
                h.portals += p;
                h.portals.push('\n');
            }
            Some(_) => {
                h.extra += line;
                h.extra.push('\n');
            }
            _ => {}
        }
    }
    let rest = rest.strip_prefix('\r').unwrap_or(rest).strip_prefix('\n').ok_or("truncated add-on save header")?;
    Ok((Some(h), rest))
}

/// Replaces the comment line of a save (the second line of the v5 body),
/// keeping any add-on header. Used to label autosaves.
pub fn set_comment(text: &str, comment: &str) -> String {
    let body_at = match split(text) {
        Ok((_, body)) => text.len() - body.len(),
        Err(_) => 0,
    };
    let (head, body) = text.split_at(body_at);
    let mut lines = body.splitn(3, '\n');
    let version = lines.next().unwrap_or("5");
    lines.next();
    format!("{head}{version}\n{comment}\n{}", lines.next().unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips_and_plain_saves_pass_through() {
        let body = "5\ncomment\n0\n";
        let w = wrap_with(&[Toy::PortalGun], Some("0 1 1 2 3 0 0 1 1 0 0\n"), "sticky 1 2\n", body);
        assert!(w.starts_with("QSPADDONS 1 3\ntoy portalgun\nportal 0 1 "));
        let (h, rest) = split(&w).unwrap();
        let h = h.unwrap();
        assert_eq!(h.toys, [Toy::PortalGun]);
        assert_eq!(h.portals, "0 1 1 2 3 0 0 1 1 0 0\n");
        assert_eq!(h.extra, "sticky 1 2\n");
        assert_eq!(rest, body);
        assert_eq!(split(body).unwrap(), (None, body));
        // no toys, no portals: still marked as an add-on save
        let w = wrap(&[], None, body);
        assert_eq!(split(&w).unwrap(), (Some(Header::default()), body));
    }

    #[test]
    fn bad_headers_are_errors() {
        assert!(split("QSPADDONS 9 0\n5\n").is_err());
        assert!(split("QSPADDONS 1 3\ntoy portalgun\n").is_err());
        assert!(split("QSPADDONS\n5\n").is_err());
    }

    #[test]
    fn bad_players_lines_are_not_used() {
        for p in ["0 1 0 0 0", "-3 1 0 0 0", "0.5 1 0 0 0", "NaN 1 0 0 0", "inf 1 0 0 0", "17 1 0 0 0", "1e30 1 0 0 0", "2 NaN 0 0 0", "2 1 0 0"] {
            assert_eq!(Players::parse(p), None, "{p}");
        }
        let p = Players::parse("2 9 1 0 0").unwrap();
        assert_eq!((p.slots, p.local), (2, 2));
        assert_eq!(Players::parse("4 -1 1 0 0").unwrap().local, 1);
    }

    #[test]
    fn vanilla_split_screen_header() {
        let p = Players { slots: 4, local: 3, coop: 1.0, deathmatch: 0.0, teamplay: 0.0 };
        let w = wrap_with(&[], None, &format!("vanilla\n{}", p.line()), "5\nc\n");
        let (h, rest) = split(&w).unwrap();
        let h = h.unwrap();
        assert!(h.vanilla);
        assert_eq!(h.players, Some(p));
        assert_eq!(rest, "5\nc\n");
        // customized headers have no `vanilla` line
        assert!(!split(&wrap(&[Toy::PortalGun], None, "5\n")).unwrap().0.unwrap().vanilla);
    }

    #[test]
    fn comment_is_replaced_under_the_header() {
        let w = wrap(&[Toy::PortalGun], None, "5\nold\nrest\n");
        assert_eq!(set_comment(&w, "AUTOSAVE_x"), wrap(&[Toy::PortalGun], None, "5\nAUTOSAVE_x\nrest\n"));
        assert_eq!(set_comment("5\nold\nrest\n", "n"), "5\nn\nrest\n");
    }
}
