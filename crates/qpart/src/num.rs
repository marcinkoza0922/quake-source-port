//! C-style lenient number parsing (atof / atoi / strtoul), as the particle
//! scripts rely on them: garbage parses as 0, trailing junk is ignored.

/// Length of the longest float-looking prefix (after leading whitespace).
fn float_prefix(s: &str) -> &str {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut ndigits = i - digits_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let f = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        ndigits += i - f;
    }
    if ndigits == 0 {
        return "";
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let e = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > e {
            i = j;
        }
    }
    &s[..i]
}

pub fn atof(s: &str) -> f32 {
    let s = s.trim_start();
    float_prefix(s).parse::<f64>().map_or(0.0, |v| v as f32)
}

pub fn atoi(s: &str) -> i32 {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut i = 0;
    let neg = match b.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = (v * 10 + i64::from(b[i] - b'0')).min(i64::from(u32::MAX));
        i += 1;
    }
    (if neg { -v } else { v }) as i32
}

/// strtoul with the given base (0 = auto: 0x hex, leading 0 octal).
/// Returns the value and the unparsed remainder.
pub fn strtoul(s: &str, base: u32) -> (u32, &str) {
    let t = s.trim_start();
    let (neg, t) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let (radix, body) = if (base == 0 || base == 16)
        && (t.starts_with("0x") || t.starts_with("0X"))
        && t[2..].starts_with(|c: char| c.is_ascii_hexdigit())
    {
        (16, &t[2..])
    } else if base == 0 && t.starts_with('0') {
        (8, t)
    } else if base == 0 {
        (10, t)
    } else {
        (base, t)
    };
    let end = body.find(|c: char| !c.is_digit(radix)).unwrap_or(body.len());
    if end == 0 {
        return (0, s);
    }
    let mut v: u64 = 0;
    for c in body[..end].chars() {
        v = (v * u64::from(radix) + u64::from(c.to_digit(radix).unwrap_or(0))).min(u64::from(u32::MAX));
    }
    let v = v as u32;
    (if neg { v.wrapping_neg() } else { v }, &body[end..])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_parsing() {
        assert_eq!(atof("1.5abc"), 1.5);
        assert_eq!(atof("  -.25"), -0.25);
        assert_eq!(atof("x"), 0.0);
        assert_eq!(atof("1e2"), 100.0);
        assert_eq!(atof("3e"), 3.0);
        assert_eq!(atoi("42x"), 42);
        assert_eq!(atoi("-7"), -7);
        assert_eq!(strtoul("0x1f", 0).0, 31);
        assert_eq!(strtoul("017", 0).0, 15);
        assert_eq!(strtoul("12_34", 10), (12, "_34"));
        assert_eq!(strtoul("ff", 16).0, 255);
    }
}
