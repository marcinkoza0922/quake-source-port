//! Fuzzy and context-aware autocomplete for the modern console.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Cmd,
    Cvar,
    Alias,
    Map,
    Cfg,
    Demo,
    Mod,
    Save,
}

impl CompletionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cmd => "cmd",
            Self::Cvar => "cvar",
            Self::Alias => "alias",
            Self::Map => "map",
            Self::Cfg => "cfg",
            Self::Demo => "demo",
            Self::Mod => "mod",
            Self::Save => "save",
        }
    }

    pub fn color(self) -> [f32; 4] {
        match self {
            Self::Cmd => [0.42, 0.76, 0.98, 1.0],
            Self::Cvar => [0.52, 0.88, 0.56, 1.0],
            Self::Alias => [0.86, 0.62, 0.96, 1.0],
            Self::Map => [0.96, 0.76, 0.38, 1.0],
            Self::Cfg | Self::Demo | Self::Mod | Self::Save => [0.46, 0.88, 0.86, 1.0],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub score: i32,
}

/// Cached completion data populated by the host so keystroke completion does zero I/O.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub cmds: Vec<String>,
    pub aliases: Vec<(String, String)>,
    /// (name, current, default)
    pub cvars: Vec<(String, String, String)>,
    pub maps: Vec<String>,
    pub cfgs: Vec<String>,
    pub demos: Vec<String>,
    pub mods: Vec<String>,
    pub saves: Vec<String>,
}

/// Range in `input` that a chosen candidate replaces, plus ranked candidates.
#[derive(Clone, Debug, Default)]
pub struct CompletionState {
    pub replace_start: usize,
    pub replace_end: usize,
    pub items: Vec<Candidate>,
    pub selected: usize,
    pub visible: bool,
}

impl CompletionState {
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected = 0;
        self.visible = false;
    }

    pub fn current(&self) -> Option<&Candidate> {
        self.visible.then(|| self.items.get(self.selected)).flatten()
    }

    pub fn step(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(n)) as usize;
    }
}

/// Recompute completions for `input[..cursor]`.
pub fn compute(input: &str, cursor: usize, cat: &Catalog) -> CompletionState {
    let cursor = cursor.min(input.len());
    let before = &input[..cursor];
    // After an unquoted pipe or `!`, don't show Quake command completions.
    if is_in_shell_segment(before) {
        return CompletionState::default();
    }
    let seg_start = before.rfind(';').map_or(0, |i| i + 1);
    let seg = &before[seg_start..];
    let trimmed = seg.trim_start();
    let lead_ws = seg.len() - trimmed.len();
    let tok_base = seg_start + lead_ws;

    if let Some(sp) = trimmed.find(char::is_whitespace) {
        let first_cmd = trimmed[..sp].to_ascii_lowercase();
        let after_cmd = &trimmed[sp..];
        let arg_trimmed = after_cmd.trim_start();
        // Only complete the first argument token.
        if arg_trimmed.contains(char::is_whitespace) {
            return CompletionState::default();
        }
        let arg_start = tok_base + sp + (after_cmd.len() - arg_trimmed.len());
        let items = complete_arg(&first_cmd, arg_trimmed, cat);
        let visible = !items.is_empty();
        return CompletionState {
            replace_start: arg_start,
            replace_end: cursor,
            items,
            selected: 0,
            visible,
        };
    }

    if trimmed.is_empty() {
        return CompletionState::default();
    }

    let mut items = Vec::new();
    for cmd in &cat.cmds {
        if let Some(score) = fuzzy_score(cmd, trimmed) {
            items.push(Candidate {
                text: cmd.clone(),
                kind: CompletionKind::Cmd,
                detail: cmd_hint(cmd).to_string(),
                score: score + 15,
            });
        }
    }
    for (name, val) in &cat.aliases {
        if let Some(score) = fuzzy_score(name, trimmed) {
            items.push(Candidate {
                text: name.clone(),
                kind: CompletionKind::Alias,
                detail: val.trim().to_string(),
                score: score + 10,
            });
        }
    }
    for (name, cur, def) in &cat.cvars {
        if let Some(score) = fuzzy_score(name, trimmed) {
            let detail = if cur == def {
                format!("= \"{cur}\"")
            } else {
                format!("= \"{cur}\" (default \"{def}\")")
            };
            items.push(Candidate {
                text: name.clone(),
                kind: CompletionKind::Cvar,
                detail,
                score,
            });
        }
    }

    items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.text.cmp(&b.text)));
    items.dedup_by(|a, b| a.text.eq_ignore_ascii_case(&b.text));
    items.truncate(12);
    let visible = !items.is_empty();
    CompletionState {
        replace_start: tok_base,
        replace_end: cursor,
        items,
        selected: 0,
        visible,
    }
}

fn complete_arg(cmd: &str, partial: &str, cat: &Catalog) -> Vec<Candidate> {
    let (list, kind, detail_str) = match cmd {
        "map" | "changelevel" => (&cat.maps, CompletionKind::Map, "bsp map"),
        "exec" => (&cat.cfgs, CompletionKind::Cfg, "script"),
        "playdemo" | "timedemo" => (&cat.demos, CompletionKind::Demo, "demo"),
        "game" | "menu_mods" => (&cat.mods, CompletionKind::Mod, "gamedir"),
        "load" => (&cat.saves, CompletionKind::Save, "savegame"),
        _ => {
            // If the first token is a cvar and the user hasn't typed a value yet,
            // show the default value as a quick-fill candidate.
            if partial.is_empty()
                && let Some((_, cur, def)) = cat.cvars.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(cmd))
            {
                let mut v = vec![Candidate {
                    text: cur.clone(),
                    kind: CompletionKind::Cvar,
                    detail: "current value".into(),
                    score: 100,
                }];
                if def != cur {
                    v.push(Candidate {
                        text: def.clone(),
                        kind: CompletionKind::Cvar,
                        detail: "default value".into(),
                        score: 90,
                    });
                }
                return v;
            }
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    for item in list {
        let score = if partial.is_empty() {
            Some(500 - item.len() as i32)
        } else {
            fuzzy_score(item, partial)
        };
        if let Some(score) = score {
            out.push(Candidate {
                text: item.clone(),
                kind,
                detail: detail_str.into(),
                score,
            });
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.text.cmp(&b.text)));
    out.truncate(12);
    out
}

pub fn is_in_shell_segment(before_cursor: &str) -> bool {
    let trimmed = before_cursor.trim_start();
    if trimmed.starts_with('!') || trimmed.starts_with("bash ") || trimmed.starts_with("sh ") {
        return true;
    }
    split_unquoted_pipe(before_cursor).is_some()
}

/// Split `line` at the first unquoted `|` into `(quake_cmd, bash_cmd)`.
pub fn split_unquoted_pipe(line: &str) -> Option<(&str, &str)> {
    let mut quotes = false;
    for (i, b) in line.bytes().enumerate() {
        match b {
            b'"' => quotes = !quotes,
            b'|' if !quotes => return Some((&line[..i], &line[i + 1..])),
            _ => {}
        }
    }
    None
}

/// Case-insensitive fuzzy/prefix scorer. Higher is better.
pub fn fuzzy_score(candidate: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return None;
    }
    let cand = candidate.to_ascii_lowercase();
    let q = query.to_ascii_lowercase();
    if cand == q {
        return Some(2000);
    }
    if cand.starts_with(&q) {
        return Some(1200 - (cand.len() as i32 - q.len() as i32) * 3);
    }
    if let Some(pos) = cand.find(&q) {
        let boundary = pos == 0 || matches!(cand.as_bytes()[pos - 1], b'_' | b'-' | b'/');
        let base = if boundary { 850 } else { 600 };
        return Some(base - pos as i32 * 4 - cand.len() as i32);
    }
    // Subsequence match (e.g. "rdvm" -> "r_drawviewmodel").
    let cb = cand.as_bytes();
    let qb = q.as_bytes();
    let mut ci = 0usize;
    let mut score = 250i32 - cand.len() as i32;
    let mut last_match = None;
    for &qc in qb {
        let found = cb[ci..].iter().position(|&c| c == qc)?;
        let idx = ci + found;
        if idx == 0 || matches!(cb[idx - 1], b'_' | b'-' | b'/') {
            score += 35;
        }
        if let Some(prev) = last_match {
            if idx == prev + 1 {
                score += 25;
            } else {
                score -= (idx - prev - 1) as i32 * 3;
            }
        }
        last_match = Some(idx);
        ci = idx + 1;
    }
    Some(score)
}

fn cmd_hint(name: &str) -> &'static str {
    match name {
        "bash" | "sh" => "open interactive bash shell in install dir",
        "map" => "load a level (map <bsp>)",
        "changelevel" => "transition to another level keeping inventory",
        "exec" => "execute a script file (.cfg)",
        "cvarlist" => "list cvars matching prefix",
        "cmdlist" => "list commands matching prefix",
        "god" => "toggle invulnerability",
        "noclip" => "toggle movement through walls",
        "notarget" => "monsters ignore player",
        "give" => "grant weapon/ammo/item",
        "setpos" => "teleport player to x y z [pitch yaw roll]",
        "screenshot" => "save PNG screenshot",
        "record" => "record a demo",
        "playdemo" => "play back a demo",
        "timedemo" => "benchmark a demo at uncapped framerate",
        "connect" => "connect to a NetQuake or QuakeWorld server",
        "game" => "switch active mod directory",
        "save" => "save current game",
        "load" => "load a saved game",
        "clear" => "clear console scrollback",
        "quit" => "exit to desktop",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_beats_substring_and_fuzzy_matches() {
        let p = fuzzy_score("r_drawviewmodel", "r_draw").unwrap();
        let s = fuzzy_score("scr_drawviewmodel", "r_draw").unwrap();
        let f = fuzzy_score("r_drawviewmodel", "rdvm").unwrap();
        assert!(p > s);
        assert!(s > f);
        assert!(fuzzy_score("fov", "xyz").is_none());
    }

    #[test]
    fn completes_commands_cvars_and_map_args() {
        let cat = Catalog {
            cmds: vec!["map".into(), "cvarlist".into(), "bash".into()],
            cvars: vec![("fov".into(), "90".into(), "90".into()), ("con_modern".into(), "1".into(), "1".into())],
            maps: vec!["e1m1".into(), "e1m2".into(), "start".into()],
            ..Default::default()
        };
        let st = compute("fo", 2, &cat);
        assert!(st.visible);
        assert_eq!(st.items[0].text, "fov");
        assert_eq!(st.items[0].kind, CompletionKind::Cvar);

        let st_map = compute("map e1", 6, &cat);
        assert!(st_map.visible);
        assert_eq!(st_map.replace_start, 4);
        assert_eq!(st_map.items[0].text, "e1m1");
        assert_eq!(st_map.items[0].kind, CompletionKind::Map);
    }

    #[test]
    fn unquoted_pipe_splits_cleanly() {
        assert_eq!(split_unquoted_pipe("cvarlist | grep r_"), Some(("cvarlist ", " grep r_")));
        assert_eq!(split_unquoted_pipe("echo \"a | b\" | wc -c"), Some(("echo \"a | b\" ", " wc -c")));
        assert_eq!(split_unquoted_pipe("echo \"a | b\""), None);
    }
}
