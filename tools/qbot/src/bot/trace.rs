//! The per-map trace: every console line (unfiltered, unlike `devlog`)
//! interleaved with what the bot did, each stamped with sim time. Written
//! to `<out>/trace/<map>.log`; the place to start when a goal fails or a
//! NEW-LOG line shows up.

use super::*;

#[derive(Default)]
pub struct Trace {
    partial: String,
    lines: Vec<String>,
}

impl Trace {
    /// Console text (fragments; QC builds lines from several prints).
    pub fn con(&mut self, text: &str, t: f64) {
        self.partial.push_str(text);
        while let Some(i) = self.partial.find('\n') {
            let line = self.partial[..i].trim_end().to_string();
            self.partial.drain(..=i);
            if !line.trim().is_empty() {
                self.lines.push(format!("{t:8.2} con  {line}"));
            }
        }
    }

    pub fn bot(&mut self, t: f64, msg: &str) {
        self.lines.push(format!("{t:8.2} bot  {msg}"));
    }

    pub fn finish(mut self, t: f64) -> Vec<String> {
        let rest = std::mem::take(&mut self.partial);
        if !rest.trim().is_empty() {
            self.lines.push(format!("{t:8.2} con  {}", rest.trim_end()));
        }
        self.lines
    }
}

impl Bot<'_> {
    /// Record a bot event in the trace (and on stderr with `-v`).
    pub(super) fn note(&mut self, msg: &str) {
        let t = self.sv.time();
        if self.opt.verbose {
            eprintln!("  t={t:.1} {msg}");
        }
        self.trace.bot(t, msg);
    }

    /// "Kind classname #ent at (x y z)", as in the report.
    pub(super) fn describe(&self, i: usize) -> String {
        let g = &self.goals[i];
        let c = self.center(g.ent);
        format!("{:?} {} #{} at ({:.0} {:.0} {:.0})", g.kind, g.class, g.ent, c.x, c.y, c.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_fragments_and_stamps_lines() {
        let mut t = Trace::default();
        t.con("monster_ogre", 1.0);
        t.con(" in wall\n\n", 1.5);
        t.bot(2.0, "goal X");
        t.con("tail", 2.5);
        assert_eq!(t.finish(3.0), ["    1.50 con  monster_ogre in wall", "    2.00 bot  goal X", "    3.00 con  tail"]);
    }
}
