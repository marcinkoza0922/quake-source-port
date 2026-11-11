//! Command tokenizer (COM_Parse / Cmd_TokenizeString), command buffer
//! (Cbuf_*), and command/alias registry with cvar fallback (Cmd_ExecuteString).
use crate::cvar::{CVAR_ROM, Cvars};
use std::collections::HashMap;

pub const MAX_ARGS: usize = 80;

/// COM_Parse: returns (token, rest) or None at end of data.
/// Handles `//` and `/* */` comments, quoted strings, and `{ } ( ) ' :` as
/// single-character tokens.
pub fn com_parse(data: &str) -> Option<(&str, &str)> {
    let mut s = data;
    loop {
        s = s.trim_start_matches(|c: char| c <= ' ');
        if s.is_empty() {
            return None;
        }
        if let Some(r) = s.strip_prefix("//") {
            s = r.find('\n').map_or("", |i| &r[i..]);
        } else if let Some(r) = s.strip_prefix("/*") {
            s = r.find("*/").map_or("", |i| &r[i + 2..]);
        } else {
            break;
        }
    }
    if let Some(r) = s.strip_prefix('"') {
        return Some(match r.find('"') {
            Some(i) => (&r[..i], &r[i + 1..]),
            None => (r, ""),
        });
    }
    let single = |c: char| matches!(c, '{' | '}' | '(' | ')' | '\'' | ':');
    let first = s.chars().next()?;
    if single(first) {
        return Some((&s[..1], &s[1..]));
    }
    let end = s.find(|c: char| c <= ' ' || single(c)).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

/// Tokenized command line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Args {
    argv: Vec<String>,
    /// Everything after argv[0] (Cmd_Args), untrimmed tail.
    args: String,
}

impl Args {
    /// Cmd_TokenizeString: stops at the first newline.
    pub fn tokenize(text: &str) -> Args {
        let mut a = Args::default();
        let mut s = text;
        loop {
            s = s.trim_start_matches(|c: char| c <= ' ' && c != '\n');
            if s.is_empty() || s.starts_with('\n') {
                break;
            }
            if a.argv.len() == 1 {
                a.args = s.to_string();
            }
            let Some((tok, rest)) = com_parse(s) else { break };
            if a.argv.len() < MAX_ARGS {
                a.argv.push(tok.to_string());
            }
            s = rest;
        }
        a
    }
    pub fn argc(&self) -> usize {
        self.argv.len()
    }
    /// Cmd_Argv: "" when out of range.
    pub fn argv(&self, i: usize) -> &str {
        self.argv.get(i).map_or("", String::as_str)
    }
    pub fn args(&self) -> &str {
        &self.args
    }
    pub fn all(&self) -> &[String] {
        &self.argv
    }
}

/// Command buffer.
#[derive(Debug, Default)]
pub struct Cbuf {
    text: String,
    wait: bool,
}

impl Cbuf {
    pub fn new() -> Self {
        Self::default()
    }
    /// Cbuf_AddText (append).
    pub fn add_text(&mut self, text: &str) {
        self.text.push_str(text);
    }
    /// Cbuf_InsertText (prepend, followed by newline).
    pub fn insert_text(&mut self, text: &str) {
        self.text.insert(0, '\n');
        self.text.insert_str(0, text);
    }
    /// Pause execution until the next `execute` call (the `wait` command).
    pub fn set_wait(&mut self) {
        self.wait = true;
    }
    /// Consume a pending `wait` (true if one was set).
    pub fn take_wait(&mut self) -> bool {
        std::mem::take(&mut self.wait)
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
    /// Pop one command: split at newline, or `;` outside quotes and comments.
    pub fn next_line(&mut self) -> Option<String> {
        if self.text.is_empty() {
            return None;
        }
        let b = self.text.as_bytes();
        let (mut quotes, mut comment) = (false, false);
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\n' => break,
                b'"' if !comment => quotes = !quotes,
                b'/' if !quotes && b.get(i + 1) == Some(&b'/') => comment = true,
                b';' if !quotes && !comment => break,
                _ => {}
            }
            i += 1;
        }
        let line = self.text[..i].to_string();
        self.text.drain(..(i + 1).min(self.text.len()));
        Some(line)
    }
}

/// Command handler. Plain fn pointers so dispatch can borrow the context mutably.
pub type CmdFn<C> = fn(&mut C, &Args);

/// Engine state reachable from commands.
pub trait CmdContext: Sized {
    fn cbuf(&mut self) -> &mut Cbuf;
    fn cvars(&mut self) -> &mut Cvars;
    fn commands(&mut self) -> &mut Commands<Self>;
    fn print(&mut self, text: &str);
    /// Called after a console cvar set (e.g. serverinfo broadcast). Default no-op.
    fn cvar_changed(&mut self, _r: crate::cvar::SetResult) {}
}

pub struct Commands<C> {
    cmds: HashMap<String, (String, CmdFn<C>)>,
    aliases: HashMap<String, (String, String)>,
}

impl<C> Default for Commands<C> {
    fn default() -> Self {
        Self { cmds: HashMap::new(), aliases: HashMap::new() }
    }
}

impl<C> std::fmt::Debug for Commands<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commands").field("cmds", &self.cmds.len()).field("aliases", &self.aliases).finish()
    }
}

/// Outcome of executing one line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exec {
    Empty,
    Command,
    Alias,
    Cvar,
    Unknown,
}

impl<C: CmdContext> Commands<C> {
    pub fn new() -> Self {
        Self::default()
    }
    /// Commands with builtins: wait, alias, unalias, echo, set, seta, toggle.
    pub fn with_builtins() -> Self {
        let mut c = Self::new();
        c.add("wait", |ctx, _| ctx.cbuf().set_wait());
        c.add("echo", |ctx, a| {
            let s = format!("{}\n", a.all()[1..].join(" "));
            ctx.print(&s)
        });
        c.add("alias", cmd_alias);
        c.add("unalias", |ctx, a| {
            ctx.commands().aliases.remove(&a.argv(1).to_ascii_lowercase());
        });
        c.add("set", |ctx, a| cmd_set(ctx, a, 0));
        c.add("seta", |ctx, a| cmd_set(ctx, a, crate::cvar::CVAR_ARCHIVE));
        c.add("toggle", |ctx, a| {
            let v = ctx.cvars().value_of(a.argv(1));
            if let Some(r) = ctx.cvars().set(a.argv(1), if v != 0.0 { "0" } else { "1" }) {
                ctx.cvar_changed(r);
            }
        });
        c
    }
    /// Cmd_AddCommand; replaces an existing one.
    pub fn add(&mut self, name: &str, f: CmdFn<C>) {
        self.cmds.insert(name.to_ascii_lowercase(), (name.to_string(), f));
    }
    pub fn remove(&mut self, name: &str) {
        self.cmds.remove(&name.to_ascii_lowercase());
    }
    pub fn exists(&self, name: &str) -> bool {
        self.cmds.contains_key(&name.to_ascii_lowercase())
    }
    pub fn alias(&self, name: &str) -> Option<&str> {
        self.aliases.get(&name.to_ascii_lowercase()).map(|(_, v)| v.as_str())
    }
    pub fn set_alias(&mut self, name: &str, value: &str) {
        self.aliases.insert(name.to_ascii_lowercase(), (name.to_string(), value.to_string()));
    }
    /// Command and alias names starting with `partial`.
    pub fn complete(&self, partial: &str) -> Vec<&str> {
        let p = partial.to_ascii_lowercase();
        let mut v: Vec<&str> = self
            .cmds
            .iter()
            .map(|(k, (n, _))| (k, n))
            .chain(self.aliases.iter().map(|(k, (n, _))| (k, n)))
            .filter(|(k, _)| k.starts_with(&p))
            .map(|(_, n)| n.as_str())
            .collect();
        v.sort_unstable();
        v
    }
}

fn cmd_alias<C: CmdContext>(ctx: &mut C, a: &Args) {
    match a.argc() {
        1 => {
            let mut l: Vec<String> =
                ctx.commands().aliases.values().map(|(n, v)| format!("{n} : {v}")).collect();
            l.sort();
            for s in l {
                ctx.print(&s);
            }
        }
        2 => {
            if let Some(v) = ctx.commands().alias(a.argv(1)).map(str::to_string) {
                ctx.print(&format!("{} : {v}", a.argv(1)));
            }
        }
        _ => {
            let value = a.all()[2..].join(" ") + "\n";
            ctx.commands().set_alias(a.argv(1), &value);
        }
    }
}

fn cmd_set<C: CmdContext>(ctx: &mut C, a: &Args, flags: u32) {
    if a.argc() < 3 {
        ctx.print("set <cvar> <value>\n");
        return;
    }
    let r = ctx.cvars().set_or_create(a.argv(1), a.argv(2), flags);
    ctx.cvar_changed(r);
}

/// Cmd_ExecuteString: command, then alias, then cvar.
pub fn execute_string<C: CmdContext>(ctx: &mut C, text: &str) -> Exec {
    let args = Args::tokenize(text);
    if args.argc() == 0 {
        return Exec::Empty;
    }
    let key = args.argv(0).to_ascii_lowercase();
    if let Some(&(_, f)) = ctx.commands().cmds.get(&key) {
        f(ctx, &args);
        return Exec::Command;
    }
    if let Some((_, v)) = ctx.commands().aliases.get(&key) {
        let v = v.clone();
        ctx.cbuf().insert_text(&v);
        return Exec::Alias;
    }
    if let Some(id) = ctx.cvars().find(&key) {
        if args.argc() == 1 {
            let c = ctx.cvars().get(id);
            let s = format!("\"{}\" is \"{}\"\n", c.name, c.string);
            ctx.print(&s);
        } else if ctx.cvars().get(id).flags & CVAR_ROM != 0 {
            let s = format!("{} is read-only\n", args.argv(0));
            ctx.print(&s);
        } else {
            let r = ctx.cvars().set_id(id, args.argv(1));
            ctx.cvar_changed(r);
        }
        return Exec::Cvar;
    }
    let s = format!("Unknown command \"{}\"\n", args.argv(0));
    ctx.print(&s);
    Exec::Unknown
}

/// Cbuf_Execute: run buffered commands until empty or `wait`.
pub fn execute<C: CmdContext>(ctx: &mut C) {
    loop {
        if std::mem::take(&mut ctx.cbuf().wait) {
            break;
        }
        let Some(line) = ctx.cbuf().next_line() else { break };
        execute_string(ctx, &line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Ctx {
        cbuf: Cbuf,
        cvars: Cvars,
        cmds: Commands<Ctx>,
        out: String,
        hits: Vec<String>,
    }
    impl CmdContext for Ctx {
        fn cbuf(&mut self) -> &mut Cbuf {
            &mut self.cbuf
        }
        fn cvars(&mut self) -> &mut Cvars {
            &mut self.cvars
        }
        fn commands(&mut self) -> &mut Commands<Self> {
            &mut self.cmds
        }
        fn print(&mut self, t: &str) {
            self.out.push_str(t);
        }
    }

    #[test]
    fn tokenize() {
        let a = Args::tokenize("  bind \"MOUSE1\" +attack // c");
        assert_eq!(a.all(), ["bind", "MOUSE1", "+attack"]);
        assert_eq!(a.args(), "\"MOUSE1\" +attack // c");
        assert_eq!(Args::tokenize("a b\nc").all(), ["a", "b"]);
        let a = Args::tokenize("say {hi}:x");
        assert_eq!(a.all(), ["say", "{", "hi", "}", ":", "x"]);
        assert_eq!(Args::tokenize("/* a */ map e1m1").all(), ["map", "e1m1"]);
        assert_eq!(Args::tokenize("").argc(), 0);
        assert_eq!(Args::tokenize("x \"unterminated").argv(1), "unterminated");
    }

    #[test]
    fn cbuf_split() {
        let mut c = Cbuf::new();
        c.add_text("a 1; b \"x;y\"\nc // d;e\n");
        assert_eq!(c.next_line().unwrap(), "a 1");
        assert_eq!(c.next_line().unwrap(), " b \"x;y\"");
        assert_eq!(c.next_line().unwrap(), "c // d;e");
        assert_eq!(c.next_line(), None);
    }

    #[test]
    fn exec() {
        let mut ctx = Ctx { cmds: Commands::with_builtins(), ..Default::default() };
        ctx.cmds.add("hit", |c, a| c.hits.push(a.argv(1).to_string()));
        ctx.cvars.register("volume", "0.7", 0);
        ctx.cbuf.add_text("hit 1; wait; hit 2\nalias go \"hit a; hit b\"\ngo\nvolume 0.3\nVOLUME\nbogus\nseta newvar 5\n");
        execute(&mut ctx);
        assert_eq!(ctx.hits, ["1"]);
        execute(&mut ctx);
        assert_eq!(ctx.hits, ["1", "2", "a", "b"]);
        assert_eq!(ctx.cvars.value_of("volume"), 0.3);
        assert!(ctx.out.contains("\"volume\" is \"0.3\""));
        assert!(ctx.out.contains("Unknown command \"bogus\""));
        assert_eq!(ctx.cvars.value_of("newvar"), 5.0);
        assert_eq!(ctx.cmds.complete("al"), ["alias"]);
    }
}
