//! File names from servers, mods, saves and console text.
//!
//! `Path::join` replaces the base when the name is absolute, so checking for
//! ".." (enough in the C engines, which concatenate) is not enough here.
use std::path::PathBuf;

/// `name` as a relative path that stays inside whatever it is joined to, on
/// every platform; None when it could leave it or name a device.
pub fn safe_rel(name: &str) -> Option<PathBuf> {
    name.split('/').map(|c| safe_component(c).then_some(c)).collect()
}

/// One file or directory name: no separators, `.`/`..`, drive or stream
/// marks, wildcards, control characters or Windows device names.
pub fn safe_component(c: &str) -> bool {
    let bad = |ch: char| ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
    // Windows drops trailing dots and spaces, so "x. " would be "x"
    if c.is_empty() || c.contains(bad) || c.ends_with(['.', ' ']) {
        return false;
    }
    let stem = c.split('.').next().unwrap_or(c).trim_end().to_ascii_lowercase();
    let numbered = |p: &str| stem.strip_prefix(p).is_some_and(|d| d.chars().count() == 1 && d.contains(['1', '2', '3', '4', '5', '6', '7', '8', '9', '¹', '²', '³']));
    !matches!(stem.as_str(), "con" | "prn" | "aux" | "nul" | "conin$" | "conout$") && !numbered("com") && !numbered("lpt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_inside() {
        for n in ["s0", "maps/e1m1.bsp", "my save", ".hidden", "a..b", "console.txt", "com10", "Żółw"] {
            assert_eq!(safe_rel(n), Some(PathBuf::from(n)), "{n}");
        }
        for n in [
            "", "/", "/etc/x", "..", "../x", "a/../b", "a/..", ".", "./a", "a//b", "a/", "a\\b", "\\\\host\\x", "C:x", "C:/x", "a:b", "a\nb", "a\0", "a\"b", "a|b",
            "x.", "x ", "nul", "NUL.txt", "maps/con.bsp", "com1", "lpt9.x", "aux .y",
        ] {
            assert_eq!(safe_rel(n), None, "{n:?}");
        }
    }
}
