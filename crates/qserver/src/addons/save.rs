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

/// Add-on state stored in the header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
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
        match line.split_once(' ') {
            Some(("toy", name)) => h.toys.extend(Toy::from_key(name.trim())),
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
    fn comment_is_replaced_under_the_header() {
        let w = wrap(&[Toy::PortalGun], None, "5\nold\nrest\n");
        assert_eq!(set_comment(&w, "AUTOSAVE_x"), wrap(&[Toy::PortalGun], None, "5\nAUTOSAVE_x\nrest\n"));
        assert_eq!(set_comment("5\nold\nrest\n", "n"), "5\nn\nrest\n");
    }
}
