//! The server console log with `developer 1`: QC `dprint` and engine
//! warnings. Lines a normal level prints are dropped; anything else is kept
//! (deduplicated, with a count and the first sim time) as `unusual` in the
//! report, since a level printing something new is worth a look.

/// Substrings of lines that every level (or the bot itself) prints.
const EXPECTED: &[&str] = &["player entered the game", "godmode ON", "entities inhibited"];

#[derive(Default)]
pub struct DevLog {
    partial: String,
    /// (line, first sim time, count), in order of first appearance.
    lines: Vec<(String, f64, u32)>,
}

impl DevLog {
    /// Feed console text (fragments; QC builds lines from several prints).
    pub fn feed(&mut self, text: &str, t: f64) {
        self.partial.push_str(text);
        while let Some(i) = self.partial.find('\n') {
            let line: String = self.partial[..i].trim_end().to_string();
            self.partial.drain(..=i);
            self.add(line, t);
        }
    }

    fn add(&mut self, line: String, t: f64) {
        if line.trim().is_empty() || is_expected(&line) {
            return;
        }
        match self.lines.iter_mut().find(|l| l.0 == line) {
            Some(l) => l.2 += 1,
            None => self.lines.push((line, t, 1)),
        }
    }

    /// Report lines: `t=12.3 (x4): text`.
    pub fn finish(mut self, t: f64) -> Vec<String> {
        let rest = std::mem::take(&mut self.partial);
        self.add(rest.trim_end().to_string(), t);
        self.lines
            .into_iter()
            .map(|(l, t, n)| if n > 1 { format!("t={t:.1} (x{n}): {l}") } else { format!("t={t:.1}: {l}") })
            .collect()
    }
}

fn is_expected(line: &str) -> bool {
    EXPECTED.iter().any(|p| line.contains(p))
}

/// The text of a report line, without its `t=… (xN): ` prefix: what the
/// baseline stores and compares.
pub fn text(line: &str) -> &str {
    line.split_once(": ").map_or(line, |(_, t)| t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_fragments_drops_expected_and_counts_repeats() {
        let mut d = DevLog::default();
        d.feed("player entered the game\nmonster_ogre", 1.0);
        d.feed(" at '1 2 3'\n", 1.5);
        d.feed("monster_ogre at '1 2 3'\nno newline", 2.0);
        let r = d.finish(3.0);
        assert_eq!(r, ["t=1.5 (x2): monster_ogre at '1 2 3'", "t=3.0: no newline"]);
        assert_eq!(text(&r[0]), "monster_ogre at '1 2 3'");
    }
}
