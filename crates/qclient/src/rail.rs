//! Railgun shots from `//qsp_rail` comment stufftext (see
//! `qserver::addons::railgun`).

/// One slug: kind 1 fired, 0 dry click (no ammo).
#[derive(Clone, Debug, PartialEq)]
pub struct RailEffect {
    pub owner: u8,
    pub kind: u8,
    pub start: [f32; 3],
    pub end: [f32; 3],
    /// Wall normal at `end`, zero when the slug hit nothing solid.
    pub normal: [f32; 3],
    /// Where the slug pierced entities, in order.
    pub hits: Vec<[f32; 3]>,
}

impl RailEffect {
    pub(crate) fn parse(args: &str) -> Option<Self> {
        let words: Vec<_> = args.split_whitespace().collect();
        if words.len() < 11 || (words.len() - 11) % 3 != 0 || words.len() > 11 + 3 * 16 {
            return None;
        }
        let owner = words[0].parse().ok()?;
        let kind = words[1].parse::<u8>().ok()?;
        if kind > 1 {
            return None;
        }
        let mut v = Vec::with_capacity(words.len() - 2);
        for w in &words[2..] {
            let x: f32 = w.parse().ok()?;
            if !x.is_finite() || x.abs() > 1_000_000.0 {
                return None;
            }
            v.push(x);
        }
        let p = |i: usize| [v[i], v[i + 1], v[i + 2]];
        Some(Self { owner, kind, start: p(0), end: p(3), normal: p(6), hits: (9..v.len()).step_by(3).map(p).collect() })
    }
}

#[cfg(test)]
mod tests {
    use super::RailEffect;
    #[test]
    fn parses_hits_and_rejects_malformed() {
        let r = RailEffect::parse("0 1 0 0 0 4 5 6 0 0 1 1 2 3").unwrap();
        assert_eq!(r.hits, vec![[1.0, 2.0, 3.0]]);
        assert_eq!(r.normal, [0.0, 0.0, 1.0]);
        for bad in ["0 2 0 0 0 4 5 6 0 0 1", "0 1 0 0 0 4 5 6 0 0", "0 1 0 0 0 4 5 6 0 0 1 1 2", "0 1 NaN 0 0 4 5 6 0 0 1"] {
            assert!(RailEffect::parse(bad).is_none(), "{bad}");
        }
    }
}
