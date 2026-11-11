//! Console variables.
use std::collections::HashMap;

pub const CVAR_ARCHIVE: u32 = 1;
pub const CVAR_NOTIFY: u32 = 2;
pub const CVAR_SERVERINFO: u32 = 4;
pub const CVAR_USERINFO: u32 = 8;
/// Read-only from the console.
pub const CVAR_ROM: u32 = 16;
/// Created by `set`/config before (or without) engine registration.
pub const CVAR_USERDEFINED: u32 = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct Cvar {
    pub name: String,
    pub string: String,
    pub value: f32,
    pub default: String,
    pub flags: u32,
}

impl Cvar {
    pub fn int(&self) -> i32 {
        self.value as i32
    }
    pub fn bool(&self) -> bool {
        self.value != 0.0
    }
}

/// Stable handle for fast access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CvarId(pub u32);

/// Result of a successful set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetResult {
    pub id: CvarId,
    pub changed: bool,
    pub flags: u32,
}

#[derive(Default, Debug)]
pub struct Cvars {
    vars: Vec<Cvar>,
    by_name: HashMap<String, CvarId>,
}

/// Q_atof: decimal or 0x hex, optional sign, ignores trailing junk.
pub fn q_atof(s: &str) -> f32 {
    let s = s.trim_start();
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let end = h.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(h.len());
        u64::from_str_radix(&h[..end], 16).map_or(0.0, |v| v as f64)
    } else {
        let b = s.as_bytes();
        let mut end = 0;
        let mut dot = false;
        while end < b.len() && (b[end].is_ascii_digit() || (b[end] == b'.' && !dot)) {
            dot |= b[end] == b'.';
            end += 1;
        }
        // optional exponent
        if end < b.len() && (b[end] | 0x20) == b'e' {
            let mut e = end + 1;
            if e < b.len() && (b[e] == b'-' || b[e] == b'+') {
                e += 1;
            }
            if e < b.len() && b[e].is_ascii_digit() {
                while e < b.len() && b[e].is_ascii_digit() {
                    e += 1;
                }
                end = e;
            }
        }
        s[..end].parse::<f64>().unwrap_or(0.0)
    };
    (if neg { -v } else { v }) as f32
}

impl Cvars {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cvar_RegisterVariable. If already present (e.g. from `set` in a config),
    /// keeps the current string, sets the default and merges flags.
    pub fn register(&mut self, name: &str, default: &str, flags: u32) -> CvarId {
        if let Some(id) = self.find(name) {
            let v = &mut self.vars[id.0 as usize];
            v.default = default.to_string();
            v.flags = (v.flags & !CVAR_USERDEFINED) | flags;
            return id;
        }
        let id = CvarId(self.vars.len() as u32);
        self.vars.push(Cvar {
            name: name.to_string(),
            string: default.to_string(),
            value: q_atof(default),
            default: default.to_string(),
            flags,
        });
        self.by_name.insert(name.to_ascii_lowercase(), id);
        id
    }

    pub fn find(&self, name: &str) -> Option<CvarId> {
        self.by_name.get(&name.to_ascii_lowercase()).copied()
    }
    pub fn get(&self, id: CvarId) -> &Cvar {
        &self.vars[id.0 as usize]
    }
    pub fn by_name(&self, name: &str) -> Option<&Cvar> {
        self.find(name).map(|id| self.get(id))
    }
    pub fn value(&self, id: CvarId) -> f32 {
        self.vars[id.0 as usize].value
    }
    pub fn string(&self, id: CvarId) -> &str {
        &self.vars[id.0 as usize].string
    }
    /// Value by name, 0 if missing (Cvar_VariableValue).
    pub fn value_of(&self, name: &str) -> f32 {
        self.by_name(name).map_or(0.0, |c| c.value)
    }

    pub fn set_id(&mut self, id: CvarId, value: &str) -> SetResult {
        let v = &mut self.vars[id.0 as usize];
        let changed = v.string != value;
        if changed {
            v.string = value.to_string();
            v.value = q_atof(value);
        }
        SetResult { id, changed, flags: v.flags }
    }

    /// Cvar_Set on an existing cvar; None if unknown.
    pub fn set(&mut self, name: &str, value: &str) -> Option<SetResult> {
        let id = self.find(name)?;
        Some(self.set_id(id, value))
    }

    pub fn set_value(&mut self, name: &str, value: f32) -> Option<SetResult> {
        self.set(name, &format_float(value))
    }

    /// `set`/`seta`: create a user-defined cvar if missing.
    pub fn set_or_create(&mut self, name: &str, value: &str, flags: u32) -> SetResult {
        match self.find(name) {
            Some(id) => {
                self.vars[id.0 as usize].flags |= flags;
                self.set_id(id, value)
            }
            None => {
                let id = self.register(name, value, flags | CVAR_USERDEFINED);
                SetResult { id, changed: true, flags: self.get(id).flags }
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Cvar> {
        self.vars.iter()
    }

    /// Names starting with `partial` (case-insensitive), for completion.
    pub fn complete(&self, partial: &str) -> Vec<&str> {
        let p = partial.to_ascii_lowercase();
        let mut v: Vec<&str> =
            self.vars.iter().filter(|c| c.name.to_ascii_lowercase().starts_with(&p)).map(|c| c.name.as_str()).collect();
        v.sort_unstable();
        v
    }

    /// Lines for config.cfg (Cvar_WriteVariables).
    pub fn archive_lines(&self) -> Vec<String> {
        self.vars
            .iter()
            .filter(|c| c.flags & CVAR_ARCHIVE != 0)
            .map(|c| format!("{} \"{}\"", c.name, c.string))
            .collect()
    }
}

/// Format like Cvar_SetValue (integers without decimals, else trimmed).
pub fn format_float(v: f32) -> String {
    if v == v.trunc() && v.abs() < 1e9 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cvars() {
        let mut c = Cvars::new();
        assert_eq!(c.set_or_create("Foo", "3", CVAR_ARCHIVE).flags, CVAR_ARCHIVE | CVAR_USERDEFINED);
        let id = c.register("foo", "1", CVAR_SERVERINFO);
        assert_eq!(c.value(id), 3.0);
        assert_eq!(c.get(id).flags, CVAR_ARCHIVE | CVAR_SERVERINFO);
        assert!(!c.set("FOO", "3").unwrap().changed);
        c.set_value("foo", 0.5).unwrap();
        assert_eq!(c.string(id), "0.5");
        assert!(c.set("nope", "1").is_none());
        assert_eq!(c.archive_lines(), vec!["Foo \"0.5\""]);
        assert_eq!(q_atof("0x10"), 16.0);
        assert_eq!(q_atof("-2.5abc"), -2.5);
        assert_eq!(q_atof("x"), 0.0);
        assert_eq!(format_float(2.0), "2");
    }
}
