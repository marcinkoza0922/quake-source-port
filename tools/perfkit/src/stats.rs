//! Medians, spreads and frame-time percentiles.

/// Median of `v` (mean of the two middle values for even lengths).
pub fn median(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    Some(if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 })
}

/// (min, max) of `v`.
pub fn range(v: &[f64]) -> Option<(f64, f64)> {
    let lo = v.iter().copied().min_by(f64::total_cmp)?;
    let hi = v.iter().copied().max_by(f64::total_cmp)?;
    Some((lo, hi))
}

/// Nearest-rank percentile (`p` in 0..=100) of frame times.
pub fn percentile(v: &[f32], p: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    let rank = ((p / 100.0) * s.len() as f64).ceil() as usize;
    Some(s[rank.clamp(1, s.len()) - 1] as f64)
}

/// Frame-time summary of one run (milliseconds).
#[derive(Clone, Copy, Debug, Default)]
pub struct Frames {
    pub count: usize,
    pub mean: f64,
    pub p50: f64,
    pub p99: f64,
    pub max: f64,
}

impl Frames {
    pub fn of(ms: &[f32]) -> Option<Frames> {
        if ms.is_empty() {
            return None;
        }
        let sum: f64 = ms.iter().map(|&x| x as f64).sum();
        Some(Frames {
            count: ms.len(),
            mean: sum / ms.len() as f64,
            p50: percentile(ms, 50.0)?,
            p99: percentile(ms, 99.0)?,
            max: ms.iter().copied().fold(0.0, f32::max) as f64,
        })
    }
}

/// True when `b` differs from `a` by more than `pct` percent of `a` and by
/// more than the larger of the two run spreads (the plan's regression rule).
pub fn significant(a: &[f64], b: &[f64], pct: f64) -> bool {
    let (Some(ma), Some(mb), Some(ra), Some(rb)) = (median(a), median(b), range(a), range(b)) else {
        return false;
    };
    let diff = (mb - ma).abs();
    diff > ma.abs() * pct / 100.0 && diff > (ra.1 - ra.0).max(rb.1 - rb.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians_and_percentiles() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
        let ms: Vec<f32> = (1..=100).map(|x| x as f32).collect();
        assert_eq!(percentile(&ms, 99.0), Some(99.0));
        assert_eq!(percentile(&ms, 50.0), Some(50.0));
        assert_eq!(percentile(&ms, 100.0), Some(100.0));
        let f = Frames::of(&ms).unwrap();
        assert_eq!((f.count, f.mean, f.max), (100, 50.5, 100.0));
    }

    #[test]
    fn regression_rule() {
        // 10% apart, spreads small: significant.
        assert!(significant(&[100.0, 101.0, 99.0], &[110.0, 111.0, 109.0], 3.0));
        // 2% apart: below the 3% threshold.
        assert!(!significant(&[100.0, 101.0, 99.0], &[102.0, 103.0, 101.0], 3.0));
        // 10% apart but the runs spread 30: noise.
        assert!(!significant(&[100.0, 85.0, 115.0], &[110.0, 111.0, 109.0], 3.0));
    }
}
