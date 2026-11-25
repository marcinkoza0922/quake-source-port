//! Command buffer (Cbuf_*), tagged with where its text came from.
use std::collections::VecDeque;

/// Who wrote a command. Servers, demos, QuakeC, configs, binds and aliases are
/// all `Script`, the default: it may not reach the system shell, name files
/// outside the user dir, delete files or choose programs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    #[default]
    Script,
    /// The user's own key, click or touch, and what a menu does because of it.
    Menu,
    /// Typed at the console, or on the program's command line.
    User,
}

#[derive(Debug)]
struct Chunk {
    text: String,
    origin: Origin,
}

/// Command buffer. Text of different origins never joins into one command.
#[derive(Debug, Default)]
pub struct Cbuf {
    chunks: VecDeque<Chunk>,
    wait: bool,
}

impl Cbuf {
    pub fn new() -> Self {
        Self::default()
    }
    /// Cbuf_AddText (append).
    pub fn add_text(&mut self, text: &str) {
        self.add_text_from(text, Origin::Script);
    }
    pub fn add_text_from(&mut self, text: &str, origin: Origin) {
        if text.is_empty() {
            return;
        }
        match self.chunks.back_mut() {
            Some(c) if c.origin == origin => c.text.push_str(text),
            _ => self.chunks.push_back(Chunk { text: text.to_string(), origin }),
        }
    }
    /// Cbuf_InsertText (prepend, followed by newline).
    pub fn insert_text(&mut self, text: &str) {
        self.insert_text_from(text, Origin::Script);
    }
    pub fn insert_text_from(&mut self, text: &str, origin: Origin) {
        match self.chunks.front_mut() {
            Some(c) if c.origin == origin => {
                c.text.insert(0, '\n');
                c.text.insert_str(0, text);
            }
            _ => self.chunks.push_front(Chunk { text: format!("{text}\n"), origin }),
        }
    }
    /// Pause execution until the next `execute` call (the `wait` command).
    pub fn set_wait(&mut self) {
        self.wait = true;
    }
    /// Consume a pending `wait` (true if one was set).
    pub fn take_wait(&mut self) -> bool {
        std::mem::take(&mut self.wait)
    }
    /// Remove and return all queued text.
    pub fn take_text(&mut self) -> String {
        self.chunks.drain(..).map(|c| c.text).collect()
    }
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
    /// Pop one command, whoever wrote it.
    pub fn next_line(&mut self) -> Option<String> {
        self.next_line_from().map(|(line, _)| line)
    }
    /// Pop one command: split at newline, or `;` outside quotes and comments.
    /// In the user's text `;` also belongs to a shell command or pipe.
    pub fn next_line_from(&mut self) -> Option<(String, Origin)> {
        let Chunk { text, origin } = self.chunks.front_mut()?;
        let origin = *origin;
        let user = origin == Origin::User;
        let b = text.as_bytes();
        let shell_line = user && text.trim_start_matches([' ', '\t']).starts_with('!');
        let (mut quotes, mut comment, mut piped) = (false, false, shell_line);
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\n' => break,
                b'"' if !comment => quotes = !quotes,
                b'/' if !quotes && b.get(i + 1) == Some(&b'/') => comment = true,
                b'|' | b'!' if user && !quotes && !comment => piped = true,
                b';' if !quotes && !comment && !piped => break,
                _ => {}
            }
            i += 1;
        }
        let line = text[..i].to_string();
        text.drain(..(i + 1).min(text.len()));
        if text.is_empty() {
            self.chunks.pop_front();
        }
        Some((line, origin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_stay_apart() {
        let mut c = Cbuf::new();
        // an unterminated server line must not absorb what the user types next
        c.add_text("cvarlist | evil #");
        c.add_text("");
        c.add_text_from("echo hi\n", Origin::User);
        c.add_text("foo\n");
        c.insert_text("alias body");
        let mut got = Vec::new();
        while let Some(l) = c.next_line_from() {
            got.push(l);
        }
        let want = [
            ("alias body", Origin::Script),
            ("cvarlist | evil #", Origin::Script),
            ("echo hi", Origin::User),
            ("foo", Origin::Script),
        ];
        assert_eq!(got, want.map(|(l, o)| (l.to_string(), o)));
        assert!(c.is_empty());
    }

    #[test]
    fn only_user_text_gets_shell_splitting() {
        let mut c = Cbuf::new();
        c.add_text_from("!a; b\n", Origin::User);
        assert_eq!(c.next_line().as_deref(), Some("!a; b"));
        c.add_text("say hi!; b\n");
        assert_eq!(c.next_line().as_deref(), Some("say hi!"));
        assert_eq!(c.next_line().as_deref(), Some(" b"));
    }
}
