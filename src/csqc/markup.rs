//! DP/FTE `^0..^9` and `^xRGB` color-coded string parsing for CSQC `drawstring` / `stringwidth`.

/// Standard DP/FTE `^0..^9` palette colors (RGB in 0..1).
const DP_COLORS: [[f32; 3]; 10] = [
    [0.0, 0.0, 0.0], // ^0 black
    [1.0, 0.0, 0.0], // ^1 red
    [0.0, 1.0, 0.0], // ^2 green
    [1.0, 1.0, 0.0], // ^3 yellow
    [0.0, 0.0, 1.0], // ^4 blue
    [0.0, 1.0, 1.0], // ^5 cyan
    [1.0, 0.0, 1.0], // ^6 magenta
    [1.0, 1.0, 1.0], // ^7 white
    [0.5, 0.5, 0.5], // ^8 half-alpha / grey
    [0.5, 0.5, 0.5], // ^9 half-alpha / grey
];

fn hex_nibble(b: u8) -> Option<f32> {
    let v = match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => return None,
    };
    Some(v as f32 / 15.0)
}

/// Splits a DP/FTE color-coded byte string into `(bytes, rgba)` runs.
pub fn parse_colored_runs(bytes: &[u8], base_rgb: [f32; 3], base_alpha: f32) -> Vec<(Vec<u8>, [f32; 4])> {
    let mut out = Vec::new();
    let mut cur_rgb = base_rgb;
    let mut cur_a = base_alpha;
    let mut run = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'^' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'^' {
                run.push(b'^');
                i += 2;
                continue;
            }
            if next.is_ascii_digit() {
                if !run.is_empty() {
                    out.push((std::mem::take(&mut run), [cur_rgb[0], cur_rgb[1], cur_rgb[2], cur_a]));
                }
                let idx = (next - b'0') as usize;
                cur_rgb = if idx == 7 { base_rgb } else { DP_COLORS[idx] };
                cur_a = if idx == 8 || idx == 9 { base_alpha * 0.5 } else { base_alpha };
                i += 2;
                continue;
            }
            if (next == b'x' || next == b'X')
                && i + 4 < bytes.len()
                && let (Some(r), Some(g), Some(b)) = (
                    hex_nibble(bytes[i + 2]),
                    hex_nibble(bytes[i + 3]),
                    hex_nibble(bytes[i + 4]),
                )
            {
                if !run.is_empty() {
                    out.push((std::mem::take(&mut run), [cur_rgb[0], cur_rgb[1], cur_rgb[2], cur_a]));
                }
                cur_rgb = [r, g, b];
                cur_a = base_alpha;
                i += 5;
                continue;
            }
            if next == b'&' && i + 3 < bytes.len() {
                // FTE ^&FB background/foreground hex pair — skip markup cleanly
                i += 4;
                continue;
            }
        }
        run.push(bytes[i]);
        i += 1;
    }
    if !run.is_empty() {
        out.push((run, [cur_rgb[0], cur_rgb[1], cur_rgb[2], cur_a]));
    }
    out
}

/// Number of visible glyphs in `bytes`, optionally stripping DP/FTE color codes.
pub fn visible_len(bytes: &[u8], handle_colors: bool) -> usize {
    if !handle_colors {
        return bytes.len();
    }
    let mut n = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'^' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'^' {
                n += 1;
                i += 2;
                continue;
            }
            if next.is_ascii_digit() {
                i += 2;
                continue;
            }
            if (next == b'x' || next == b'X')
                && i + 4 < bytes.len()
                && hex_nibble(bytes[i + 2]).is_some()
                && hex_nibble(bytes[i + 3]).is_some()
                && hex_nibble(bytes[i + 4]).is_some()
            {
                i += 5;
                continue;
            }
            if next == b'&' && i + 3 < bytes.len() {
                i += 4;
                continue;
            }
        }
        n += 1;
        i += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dp_color_codes_and_hex() {
        let runs = parse_colored_runs(b"hi ^1red^x0f0grn^^!", [1.0; 3], 1.0);
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].0, b"hi ");
        assert_eq!(runs[1].0, b"red");
        assert_eq!(runs[1].1, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(runs[2].0, b"grn^!");
        assert_eq!(runs[2].1, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(visible_len(b"hi ^1red^x0f0grn^^!", true), 11);
        assert_eq!(visible_len(b"hi ^1red", false), 8);
    }
}
