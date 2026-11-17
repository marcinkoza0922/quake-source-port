//! Line breaking for translated text. Words break at spaces; Chinese and
//! Japanese have none, so a line may also break before or after any Han
//! character or kana, except before closing punctuation and small kana or
//! after opening brackets (a light kinsoku). Korean keeps breaking at spaces.

/// Han, kana and CJK punctuation/full-width forms: break opportunities on
/// both sides.
fn breakable(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FFF | 0x3000..=0x303F | 0x3040..=0x30FF | 0x31F0..=0x31FF | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFF60 | 0x20000..=0x3FFFF)
}

/// Two columns wide in column-counted text (CJK, Hangul, full-width forms).
pub fn is_wide(c: char) -> bool {
    breakable(c) || matches!(c as u32, 0x1100..=0x115F | 0x3130..=0x318F | 0xAC00..=0xD7A3 | 0xFFE0..=0xFFE6)
}

/// Display columns of `s` ([`is_wide`] chars count two).
pub fn cols(s: &str) -> usize {
    s.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

/// May not start a line.
fn no_start(c: char) -> bool {
    "、。，．：；！？）」』】〉》〕］｝〙〗ー々〻ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ…‥・,.:;!?)]}%".contains(c)
}

/// May not end a line.
fn no_end(c: char) -> bool {
    "（「『【〈《〔［｛〘〖([{".contains(c)
}

/// `s` split into unbreakable pieces; `true` marks a piece that follows a
/// space (joined back with one space on the same line).
pub fn pieces(s: &str) -> Vec<(bool, &str)> {
    let mut out = Vec::new();
    for word in s.split_whitespace() {
        let (mut spaced, mut start, mut prev) = (true, 0, None::<char>);
        for (i, c) in word.char_indices() {
            if let Some(p) = prev
                && (breakable(p) || breakable(c))
                && !no_start(c)
                && !no_end(p)
            {
                out.push((spaced, &word[start..i]));
                spaced = false;
                start = i;
            }
            prev = Some(c);
        }
        out.push((spaced, &word[start..]));
    }
    out
}

/// Greedy wrap of `s` to `max` (in the units of `width`); a piece wider
/// than a line is split between characters.
pub fn wrap(s: &str, max: f32, width: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for (spaced, piece) in pieces(s) {
        let cur = lines.last_mut().unwrap();
        let cand = if cur.is_empty() {
            piece.to_string()
        } else if spaced {
            format!("{cur} {piece}")
        } else {
            format!("{cur}{piece}")
        };
        if cur.is_empty() || width(&cand) <= max {
            *cur = cand;
        } else {
            lines.push(piece.to_string());
        }
        // cut a piece that alone overflows
        while let Some(last) = lines.last()
            && width(last) > max
            && last.chars().count() > 1
        {
            let last = lines.pop().unwrap();
            let mut head = String::new();
            let mut cut = last.len();
            for (i, c) in last.char_indices() {
                head.push(c);
                if head.chars().count() > 1 && width(&head) > max {
                    cut = i;
                    break;
                }
            }
            lines.push(last[..cut].to_string());
            lines.push(last[cut..].to_string());
            if cut == last.len() {
                break;
            }
        }
    }
    if lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_cols(s: &str, n: usize) -> Vec<String> {
        wrap(s, n as f32, |t| cols(t) as f32)
    }

    #[test]
    fn latin_breaks_at_spaces() {
        assert_eq!(by_cols("one two three four", 9), ["one two", "three", "four"]);
        assert_eq!(by_cols("abcdefghijkl xy", 5), ["abcde", "fghij", "kl xy"]);
    }

    #[test]
    fn cjk_breaks_between_characters_with_kinsoku() {
        // 6 columns = 3 wide characters; "。" can't start a line
        assert_eq!(by_cols("設定を保存しました。", 6), ["設定を", "保存し", "まし", "た。"]);
        assert_eq!(pieces("「画面」FPS表示"), [(true, "「画"), (false, "面」"), (false, "FPS"), (false, "表"), (false, "示")]);
        // Hangul breaks at spaces only
        assert_eq!(by_cols("설정을 저장했습니다", 12), ["설정을", "저장했습니다"]);
    }
}
