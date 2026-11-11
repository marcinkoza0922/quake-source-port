//! Minimal JSON value, parser and writer for the engine's own small files
//! (`qsp_stats.json`). Not a general-purpose JSON library: numbers are f64,
//! object key order is preserved, and `\u` escapes outside the BMP are
//! replaced by U+FFFD.

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_obj(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Obj(o) => Some(o),
            _ => None,
        }
    }

    /// Pretty-printed with two-space indents.
    pub fn to_string_pretty(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, 0);
        s.push('\n');
        s
    }

    fn write(&self, s: &mut String, ind: usize) {
        let pad = |s: &mut String, n: usize| s.extend(std::iter::repeat_n(' ', n * 2));
        match self {
            Value::Null => s.push_str("null"),
            Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
            Value::Num(n) if n.is_finite() => {
                let _ = write!(s, "{n}");
            }
            Value::Num(_) => s.push_str("null"),
            Value::Str(t) => write_str(s, t),
            Value::Arr(a) => {
                s.push('[');
                for (i, v) in a.iter().enumerate() {
                    s.push_str(if i > 0 { ", " } else { "" });
                    v.write(s, ind);
                }
                s.push(']');
            }
            Value::Obj(o) if o.is_empty() => s.push_str("{}"),
            Value::Obj(o) => {
                s.push_str("{\n");
                for (i, (k, v)) in o.iter().enumerate() {
                    pad(s, ind + 1);
                    write_str(s, k);
                    s.push_str(": ");
                    v.write(s, ind + 1);
                    s.push_str(if i + 1 < o.len() { ",\n" } else { "\n" });
                }
                pad(s, ind);
                s.push('}');
            }
        }
    }
}

fn write_str(s: &mut String, t: &str) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(s, "\\u{:04x}", c as u32);
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

/// Parses one JSON document; `None` on any syntax error.
pub fn parse(text: &str) -> Option<Value> {
    let mut p = Parser { s: text.as_bytes(), i: 0, depth: 0 };
    let v = p.value()?;
    p.ws();
    (p.i == p.s.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: u32,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        let ok = self.s.get(self.i) == Some(&c);
        self.i += ok as usize;
        ok
    }
    fn lit(&mut self, w: &str, v: Value) -> Option<Value> {
        self.s[self.i..].starts_with(w.as_bytes()).then(|| {
            self.i += w.len();
            v
        })
    }
    fn value(&mut self) -> Option<Value> {
        self.ws();
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let v = match *self.s.get(self.i)? {
            b'n' => self.lit("null", Value::Null),
            b't' => self.lit("true", Value::Bool(true)),
            b'f' => self.lit("false", Value::Bool(false)),
            b'"' => self.string().map(Value::Str),
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                if !self.eat(b']') {
                    loop {
                        a.push(self.value()?);
                        if self.eat(b']') {
                            break;
                        }
                        self.eat(b',').then_some(())?;
                    }
                }
                Some(Value::Arr(a))
            }
            b'{' => {
                self.i += 1;
                let mut o = Vec::new();
                if !self.eat(b'}') {
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.eat(b':').then_some(())?;
                        o.push((k, self.value()?));
                        if self.eat(b'}') {
                            break;
                        }
                        self.eat(b',').then_some(())?;
                    }
                }
                Some(Value::Obj(o))
            }
            _ => {
                let st = self.i;
                while self.s.get(self.i).is_some_and(|c| matches!(c, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')) {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[st..self.i]).ok()?.parse().ok().map(Value::Num)
            }
        };
        self.depth -= 1;
        v
    }
    fn string(&mut self) -> Option<String> {
        if self.s.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let c = *self.s.get(self.i)?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = *self.s.get(self.i)?;
                    self.i += 1;
                    let ch = match e {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'u' => {
                            let h = std::str::from_utf8(self.s.get(self.i..self.i + 4)?).ok()?;
                            self.i += 4;
                            char::from_u32(u32::from_str_radix(h, 16).ok()?).unwrap_or('\u{fffd}')
                        }
                        c => c as char,
                    };
                    let mut b = [0; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                }
                c => out.push(c),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let v = Value::Obj(vec![
            ("a \"q\"".into(), Value::Num(1.5)),
            ("b".into(), Value::Arr(vec![Value::Bool(true), Value::Null, Value::Str("x\ny".into())])),
            ("c".into(), Value::Obj(vec![])),
        ]);
        assert_eq!(parse(&v.to_string_pretty()), Some(v));
        assert_eq!(parse("{\"k\": \"\\u0041\"}").unwrap().get("k"), Some(&Value::Str("A".into())));
        assert!(parse("{\"a\": 1,}").is_none());
        assert!(parse("[1 2]").is_none());
        assert!(parse(&"[".repeat(100)).is_none());
    }
}
