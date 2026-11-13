//! QuakeC bytecode disassembler (progs.dat v6 statements to text).
//!
//! Operands are named from the global defs: named globals and locals by
//! name, `IMMEDIATE` constants by value, field references as `ent.field`,
//! jumps as absolute statement numbers, calls by function name.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::progs::{Def, EType, Progs, Statement};

/// Opcode mnemonics, indexed by opcode (vanilla `opcode_t`, 0..=65).
pub const OP_NAMES: [&str; 66] = [
    "DONE", "MUL_F", "MUL_V", "MUL_FV", "MUL_VF", "DIV_F", "ADD_F", "ADD_V", "SUB_F", "SUB_V",
    "EQ_F", "EQ_V", "EQ_S", "EQ_E", "EQ_FNC", "NE_F", "NE_V", "NE_S", "NE_E", "NE_FNC",
    "LE", "GE", "LT", "GT",
    "LOAD_F", "LOAD_V", "LOAD_S", "LOAD_ENT", "LOAD_FLD", "LOAD_FNC", "ADDRESS",
    "STORE_F", "STORE_V", "STORE_S", "STORE_ENT", "STORE_FLD", "STORE_FNC",
    "STOREP_F", "STOREP_V", "STOREP_S", "STOREP_ENT", "STOREP_FLD", "STOREP_FNC",
    "RETURN", "NOT_F", "NOT_V", "NOT_S", "NOT_ENT", "NOT_FNC", "IF", "IFNOT",
    "CALL0", "CALL1", "CALL2", "CALL3", "CALL4", "CALL5", "CALL6", "CALL7", "CALL8",
    "STATE", "GOTO", "AND", "OR", "BITAND", "BITOR",
];

/// Mnemonic for an opcode (`OP_<n>` if unknown).
pub fn op_name(op: u16) -> String {
    OP_NAMES.get(op as usize).map_or_else(|| format!("OP_{op}"), |s| (*s).to_string())
}

/// Disassembler over a parsed progs with an offset-to-def index.
pub struct Disasm<'a> {
    p: &'a Progs,
    by_ofs: HashMap<u16, &'a Def>,
    /// Globals some statement writes, or that are system globals / locals:
    /// these are variables. Everything else is a constant and printed by value
    /// (fteqcc merges equal constants, so their names would mislead).
    vars: HashSet<u16>,
}

fn escape(s: &[u8]) -> String {
    let mut o = String::from("\"");
    for &c in s {
        match c {
            b'\n' => o.push_str("\\n"),
            b'"' => o.push_str("\\\""),
            b'\\' => o.push_str("\\\\"),
            0x20..=0x7e => o.push(c as char),
            _ => {
                let _ = write!(o, "\\x{c:02x}");
            }
        }
    }
    o.push('"');
    o
}

fn fmt_f(v: f32) -> String {
    if v == v.trunc() && v.abs() < 1e9 { format!("{v:.0}") } else { format!("{v}") }
}

impl<'a> Disasm<'a> {
    pub fn new(p: &'a Progs) -> Self {
        let mut by_ofs = HashMap::new();
        for d in &p.globaldefs {
            by_ofs.entry(d.ofs).or_insert(d);
        }
        let mut vars = HashSet::new();
        for s in &p.statements {
            match s.op {
                1..=30 | 44..=48 | 62..=65 => vars.insert(s.c),
                31..=36 => vars.insert(s.b),
                _ => false,
            };
        }
        let sys_end = p.find_global("end_sys_globals").map_or(28, |d| d.ofs);
        vars.extend(0..sys_end);
        for f in p.functions.iter().filter(|f| f.first_statement > 0) {
            vars.extend(f.parm_start as u16..(f.parm_start + f.locals) as u16);
        }
        Self { p, by_ofs, vars }
    }

    fn f(&self, ofs: usize) -> f32 {
        f32::from_bits(self.p.globals.get(ofs).copied().unwrap_or(0))
    }

    fn i(&self, ofs: usize) -> i32 {
        self.p.globals.get(ofs).copied().unwrap_or(0) as i32
    }

    /// Value of a constant global of type `t` at `ofs`.
    fn value(&self, t: EType, ofs: usize) -> String {
        match t {
            EType::String => escape(self.p.string(self.i(ofs))),
            EType::Float => fmt_f(self.f(ofs)),
            EType::Vector => format!("'{} {} {}'", fmt_f(self.f(ofs)), fmt_f(self.f(ofs + 1)), fmt_f(self.f(ofs + 2))),
            EType::Function => format!("{}()", self.p.function_name(self.i(ofs).max(0) as usize)),
            EType::Field => {
                let fo = self.i(ofs).max(0) as usize;
                self.p.field_at_ofs(fo).map_or_else(|| format!(".{fo}"), |d| format!(".{}", self.name(d)))
            }
            _ => format!("{}", self.i(ofs)),
        }
    }

    fn name(&self, d: &Def) -> String {
        String::from_utf8_lossy(self.p.string(d.name)).into_owned()
    }

    /// Readable operand for global offset `ofs`.
    pub fn global(&self, ofs: u16) -> String {
        match ofs {
            0 => return "NULL".into(),
            1 => return "RETURN".into(),
            4..=27 if !self.by_ofs.contains_key(&ofs) => {
                let (n, k) = ((ofs - 4) / 3, (ofs - 4) % 3);
                return if k == 0 { format!("PARM{n}") } else { format!("PARM{n}+{k}") };
            }
            _ => {}
        }
        let Some(d) = self.by_ofs.get(&ofs) else {
            // Vector component of a named vector, or an unnamed temp.
            for k in 1..=2 {
                if let Some(d) = ofs.checked_sub(k).and_then(|o| self.by_ofs.get(&o))
                    && d.etype() == EType::Vector
                {
                    let axis = ["", "_y", "_z"][k as usize];
                    return format!("{}{axis}", self.name(d));
                }
            }
            return format!("t{ofs}");
        };
        let n = self.name(d);
        let constant = matches!(d.etype(), EType::Float | EType::Vector | EType::String) && !self.vars.contains(&ofs);
        if n == "IMMEDIATE" || n.is_empty() || constant {
            self.value(d.etype(), ofs as usize)
        } else {
            n
        }
    }

    /// Field name for a global holding a field offset (`LOAD`/`ADDRESS` b).
    fn field(&self, ofs: u16) -> String {
        match self.by_ofs.get(&ofs) {
            Some(d) if d.etype() == EType::Field && self.name(d) != "IMMEDIATE" => self.name(d),
            _ => self.value(EType::Field, ofs as usize).trim_start_matches('.').to_string(),
        }
    }

    /// One statement's operands (without the mnemonic).
    pub fn operands(&self, pc: usize, s: &Statement) -> String {
        let g = |o| self.global(o);
        let jump = |o: u16| format!("{}", pc as i64 + o as i16 as i64);
        match s.op {
            0 | 43 => if s.a == 0 { String::new() } else { g(s.a) },
            24..=29 => format!("{}.{} -> {}", g(s.a), self.field(s.b), g(s.c)),
            30 => format!("&{}.{} -> {}", g(s.a), self.field(s.b), g(s.c)),
            31..=36 => format!("{} -> {}", g(s.a), g(s.b)),
            37..=42 => format!("{} -> *{}", g(s.a), g(s.b)),
            44..=48 => format!("{} -> {}", g(s.a), g(s.c)),
            49 | 50 => format!("{}, {}", g(s.a), jump(s.b)),
            51..=59 => {
                let fnum = self.i(s.a as usize);
                let f = self.p.functions.get(fnum.max(0) as usize);
                match f {
                    Some(f) if f.first_statement < 0 => format!("{} #{}", g(s.a), -f.first_statement),
                    _ => g(s.a),
                }
            }
            60 => format!("{}, {}", g(s.a), g(s.b)),
            61 => jump(s.a),
            _ => format!("{}, {} -> {}", g(s.a), g(s.b), g(s.c)),
        }
    }

    /// Header line for function `fi`.
    pub fn header(&self, fi: usize) -> String {
        let f = &self.p.functions[fi];
        let file = String::from_utf8_lossy(self.p.string(f.file));
        let parms: Vec<String> = (0..f.numparms.clamp(0, 8) as usize)
            .scan(f.parm_start as u16, |o, i| {
                let r = self.global(*o);
                *o += f.parm_size[i] as u16;
                Some(r)
            })
            .collect();
        if f.first_statement < 0 {
            format!("{}({} args) = #{}", self.p.function_name(fi), f.numparms, -f.first_statement)
        } else {
            format!(
                "{}({})  [fn {fi}, stmt {}, locals {}+{}, {file}]",
                self.p.function_name(fi), parms.join(", "), f.first_statement, f.parm_start, f.locals
            )
        }
    }

    /// Full listing of function `fi` (header plus statements up to `DONE`).
    pub fn function(&self, fi: usize) -> String {
        let f = &self.p.functions[fi];
        let mut out = self.header(fi);
        out.push('\n');
        if f.first_statement <= 0 {
            return out;
        }
        let start = f.first_statement as usize;
        // Statements end at the next function's first statement, or at DONE.
        let end = self
            .p
            .functions
            .iter()
            .filter(|o| o.first_statement > f.first_statement)
            .map(|o| o.first_statement as usize)
            .min()
            .unwrap_or(self.p.statements.len());
        for pc in start..end {
            let s = &self.p.statements[pc];
            let _ = writeln!(out, "  {pc:6}  {:<10} {}", op_name(s.op), self.operands(pc, s));
            if s.op == 0 {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_names() {
        assert_eq!(op_name(0), "DONE");
        assert_eq!(op_name(crate::exec::OP_IFNOT), "IFNOT");
        assert_eq!(op_name(crate::exec::OP_CALL8), "CALL8");
        assert_eq!(op_name(crate::exec::OP_GOTO), "GOTO");
        assert_eq!(op_name(65), "BITOR");
        assert_eq!(op_name(66), "OP_66");
        assert_eq!(fmt_f(1.0), "1");
        assert_eq!(fmt_f(0.5), "0.5");
        assert_eq!(escape(b"a\"\n\x01"), "\"a\\\"\\n\\x01\"");
    }
}
