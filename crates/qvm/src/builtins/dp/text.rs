//! `^` markup and dates for the DP string builtins.

/// PR_Markup_Parse: the text without FTE/DP `^` codes.
pub(super) fn strip_markup(s: &[u8]) -> Vec<u8> {
    let (mut mask, mut i) = (0u8, 0);
    if matches!(s.first(), Some(1 | 2)) {
        (mask, i) = (128, 1);
    }
    let mut out = Vec::with_capacity(s.len());
    while i < s.len() {
        let hex = |k: usize| s.get(i + k).is_some_and(u8::is_ascii_hexdigit);
        let ansi = |k: usize| hex(k) || s.get(i + k) == Some(&b'-');
        if s[i] == b'^' {
            match s.get(i + 1) {
                Some(b'^') => i += 1,
                Some(b'0'..=b'9' | b'h' | b'b' | b's' | b'r') => {
                    i += 2;
                    continue;
                }
                Some(b'd') => {
                    mask = 0;
                    i += 2;
                    continue;
                }
                Some(b'a' | b'm') => {
                    mask ^= 128;
                    i += 2;
                    continue;
                }
                Some(b'x') if hex(2) && hex(3) && hex(4) => {
                    i += 5;
                    continue;
                }
                Some(b'&') if ansi(2) && ansi(3) => {
                    i += 4;
                    continue;
                }
                Some(b'U') if (2..6).all(hex) => {
                    let c = (2..6).fold(0, |v, k| v * 16 + (s[i + k] as char).to_digit(16).unwrap_or(0));
                    out.push(match c {
                        0xe000..=0xe0ff => (c & 0xff) as u8,
                        0x20..=0x7f => c as u8,
                        _ => b'?',
                    });
                    i += 6;
                    continue;
                }
                _ => {}
            }
        }
        // white text masks everything above the space
        out.push(if s[i] > 32 { s[i] | mask } else { s[i] });
        i += 1;
    }
    out
}

/// strftime in the C locale for `t` seconds since 1970 (UTC).
pub(super) fn strftime(fmt: &[u8], t: i64) -> Vec<u8> {
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
    ];
    let (days, secs) = (t.div_euclid(86400), t.rem_euclid(86400));
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let yday = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(month - 1) as usize] + day - 1 + i64::from(leap && month > 2);
    let wday = (days + 4).rem_euclid(7);
    let (dn, mn) = (DAYS[wday as usize], MONTHS[(month - 1) as usize]);
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    let mut out = String::new();
    let fmt = String::from_utf8_lossy(fmt);
    let mut it = fmt.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        out += &match it.next() {
            Some('Y') => format!("{year}"),
            Some('C') => format!("{:02}", year / 100),
            Some('y') => format!("{:02}", year % 100),
            Some('m') => format!("{month:02}"),
            Some('d') => format!("{day:02}"),
            Some('e') => format!("{day:2}"),
            Some('j') => format!("{:03}", yday + 1),
            Some('H') => format!("{h:02}"),
            Some('I') => format!("{h12:02}"),
            Some('M') => format!("{m:02}"),
            Some('S') => format!("{s:02}"),
            Some('p') => if h < 12 { "AM" } else { "PM" }.into(),
            Some('A') => dn.into(),
            Some('a') => dn[..3].into(),
            Some('B') => mn.into(),
            Some('b' | 'h') => mn[..3].into(),
            Some('u') => format!("{}", if wday == 0 { 7 } else { wday }),
            Some('w') => format!("{wday}"),
            Some('F') => format!("{year}-{month:02}-{day:02}"),
            Some('T' | 'X') => format!("{h:02}:{m:02}:{s:02}"),
            Some('R') => format!("{h:02}:{m:02}"),
            Some('D' | 'x') => format!("{month:02}/{day:02}/{:02}", year % 100),
            Some('r') => format!("{h12:02}:{m:02}:{s:02} {}", if h < 12 { "AM" } else { "PM" }),
            Some('c') => format!("{} {} {day:2} {h:02}:{m:02}:{s:02} {year}", &dn[..3], &mn[..3]),
            Some('s') => format!("{t}"),
            Some('Z') => "UTC".into(),
            Some('z') => "+0000".into(),
            Some('n') => "\n".into(),
            Some('t') => "\t".into(),
            Some('%') => "%".into(),
            Some(o) => format!("%{o}"),
            None => "%".into(),
        };
    }
    out.into_bytes()
}
