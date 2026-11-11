//! progs.dat (v6) loader. Port of PR_LoadProgs (pr_edict.c).

use std::collections::HashMap;

use crate::VmError;

pub const PROG_VERSION: i32 = 6;
pub const PROGHEADER_CRC: i32 = 5927;
pub const DEF_SAVEGLOBAL: u16 = 1 << 15;
pub const MAX_PARMS: usize = 8;

/// QC value types (`etype_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EType {
    Void,
    String,
    Float,
    Vector,
    Entity,
    Field,
    Function,
    Pointer,
    Other(u16),
}

impl EType {
    pub fn from_raw(t: u16) -> Self {
        match t & !DEF_SAVEGLOBAL {
            0 => Self::Void,
            1 => Self::String,
            2 => Self::Float,
            3 => Self::Vector,
            4 => Self::Entity,
            5 => Self::Field,
            6 => Self::Function,
            7 => Self::Pointer,
            o => Self::Other(o),
        }
    }
    /// Size in 32-bit slots.
    pub fn size(self) -> usize {
        if self == Self::Vector { 3 } else { 1 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Statement {
    pub op: u16,
    pub a: u16,
    pub b: u16,
    pub c: u16,
}

/// `ddef_t`: global or field definition.
#[derive(Debug, Clone, Copy)]
pub struct Def {
    pub ty: u16,
    pub ofs: u16,
    pub name: i32,
}

impl Def {
    pub fn etype(&self) -> EType {
        EType::from_raw(self.ty)
    }
    pub fn save_global(&self) -> bool {
        self.ty & DEF_SAVEGLOBAL != 0
    }
}

/// `dfunction_t`.
#[derive(Debug, Clone, Copy)]
pub struct Function {
    pub first_statement: i32,
    pub parm_start: i32,
    pub locals: i32,
    pub name: i32,
    pub file: i32,
    pub numparms: i32,
    pub parm_size: [u8; MAX_PARMS],
}

/// Parsed, validated progs.dat.
#[derive(Debug, Clone)]
pub struct Progs {
    pub version: i32,
    /// Header CRC field (must be [`PROGHEADER_CRC`]).
    pub header_crc: i32,
    /// CRC_Block of the whole file (sent to clients / used by savegames).
    pub crc: u16,
    pub entityfields: usize,
    pub statements: Vec<Statement>,
    pub globaldefs: Vec<Def>,
    pub fielddefs: Vec<Def>,
    pub functions: Vec<Function>,
    pub strings: Vec<u8>,
    pub globals: Vec<u32>,
    field_map: HashMap<Vec<u8>, usize>,
    global_map: HashMap<Vec<u8>, usize>,
    func_map: HashMap<Vec<u8>, usize>,
}

/// CRC-CCITT as in crc.c (init 0xffff, poly 0x1021, no final xor).
pub fn crc_block(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

fn cstr(strings: &[u8], ofs: i32) -> &[u8] {
    if ofs < 0 || ofs as usize >= strings.len() {
        return b"";
    }
    let s = &strings[ofs as usize..];
    &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]
}

impl Progs {
    /// Parse progs.dat. Rejects wrong version or header CRC (like `fatal` PR_LoadProgs).
    pub fn parse(data: &[u8]) -> Result<Self, VmError> {
        let err = |m: String| VmError::new(format!("progs.dat: {m}"));
        let rd = |i: usize| -> Result<i32, VmError> {
            data.get(i * 4..i * 4 + 4)
                .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| err("truncated header".into()))
        };
        let h: Vec<i32> = (0..15).map(rd).collect::<Result<_, _>>()?;
        let (version, header_crc) = (h[0], h[1]);
        if version != PROG_VERSION {
            return Err(err(format!("wrong version number ({version} should be {PROG_VERSION})")));
        }
        if header_crc != PROGHEADER_CRC {
            return Err(err(format!("system vars have been modified (crc {header_crc})")));
        }
        let lump = |idx: usize, elem: usize| -> Result<&[u8], VmError> {
            let (ofs, n) = (h[idx], h[idx + 1]);
            if ofs < 0 || n < 0 {
                return Err(err(format!("bad lump {idx}")));
            }
            let (ofs, len) = (ofs as usize, n as usize * elem);
            data.get(ofs..ofs + len).ok_or_else(|| err(format!("lump {idx} past end of file")))
        };
        let words = |b: &[u8]| -> Vec<u32> {
            b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()
        };
        let u16s = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);

        let statements: Vec<Statement> = lump(2, 8)?
            .as_chunks::<8>().0.iter()
            .map(|c| Statement { op: u16s(c, 0), a: u16s(c, 2), b: u16s(c, 4), c: u16s(c, 6) })
            .collect();
        let defs = |idx| -> Result<Vec<Def>, VmError> {
            Ok(lump(idx, 8)?
                .as_chunks::<8>().0.iter()
                .map(|c| Def { ty: u16s(c, 0), ofs: u16s(c, 2), name: i32::from_le_bytes(c[4..8].try_into().unwrap()) })
                .collect())
        };
        let globaldefs = defs(4)?;
        let fielddefs = defs(6)?;
        let functions: Vec<Function> = lump(8, 36)?
            .as_chunks::<36>().0.iter()
            .map(|c| {
                let w = words(&c[..28]);
                Function {
                    first_statement: w[0] as i32,
                    parm_start: w[1] as i32,
                    locals: w[2] as i32,
                    name: w[4] as i32,
                    file: w[5] as i32,
                    numparms: w[6] as i32,
                    parm_size: c[28..36].try_into().unwrap(),
                }
            })
            .collect();
        let mut strings = lump(10, 1)?.to_vec();
        if h[10] as usize + strings.len() >= data.len() {
            return Err(err("strings go past end of file".into()));
        }
        strings.push(0);
        let globals = words(lump(12, 4)?);
        let entityfields = h[14].max(0) as usize;

        let mut p = Progs {
            version,
            header_crc,
            crc: crc_block(data),
            entityfields,
            statements,
            globaldefs,
            fielddefs,
            functions,
            strings,
            globals,
            field_map: HashMap::new(),
            global_map: HashMap::new(),
            func_map: HashMap::new(),
        };
        p.validate()?;
        for (i, d) in p.fielddefs.iter().enumerate() {
            p.field_map.entry(cstr(&p.strings, d.name).to_vec()).or_insert(i);
        }
        for (i, d) in p.globaldefs.iter().enumerate() {
            p.global_map.entry(cstr(&p.strings, d.name).to_vec()).or_insert(i);
        }
        for (i, f) in p.functions.iter().enumerate() {
            p.func_map.entry(cstr(&p.strings, f.name).to_vec()).or_insert(i);
        }
        Ok(p)
    }

    /// Bounds-check everything the interpreter indexes so execution never panics.
    fn validate(&self) -> Result<(), VmError> {
        let ng = self.globals.len();
        let ns = self.statements.len();
        let bad = |m: String| Err(VmError::new(format!("progs.dat: {m}")));
        for d in &self.fielddefs {
            if d.ofs as usize + d.etype().size() > self.entityfields {
                return bad(format!("field ofs {} out of range", d.ofs));
            }
        }
        for d in &self.globaldefs {
            if d.ofs as usize + d.etype().size() > ng {
                return bad(format!("global ofs {} out of range", d.ofs));
            }
        }
        for (i, f) in self.functions.iter().enumerate() {
            if f.first_statement > 0 && f.first_statement as usize >= ns {
                return bad(format!("function {i} first_statement out of range"));
            }
            if f.first_statement > 0
                && (f.parm_start < 0 || f.locals < 0 || (f.parm_start + f.locals) as usize > ng)
            {
                return bad(format!("function {i} locals out of range"));
            }
            // Builtins may carry negative numparms (qcc varargs marker);
            // ironwail never reads it for them, so only validate QC functions.
            if f.first_statement <= 0 {
                continue;
            }
            if !(0..=MAX_PARMS as i32).contains(&f.numparms) {
                return bad(format!("function {i} numparms"));
            }
            let psum: usize = f.parm_size[..f.numparms as usize].iter().map(|&s| s as usize).sum();
            if f.parm_size.iter().any(|&s| s > 3) || f.parm_start as usize + psum > ng {
                return bad(format!("function {i} parm sizes"));
            }
        }
        for (i, s) in self.statements.iter().enumerate() {
            let jump = |o: u16| {
                let t = i as i64 + o as i16 as i64;
                t >= 0 && (t as usize) < ns
            };
            let g = |o: u16| (o as usize) < ng;
            let ok = match s.op {
                crate::exec::OP_IF | crate::exec::OP_IFNOT => g(s.a) && jump(s.b),
                crate::exec::OP_GOTO => jump(s.a),
                _ => g(s.a) && g(s.b) && g(s.c),
            };
            if !ok {
                return bad(format!("statement {i} operand out of range"));
            }
        }
        Ok(())
    }

    /// NUL-terminated progs string at `ofs` (empty if out of range).
    pub fn string(&self, ofs: i32) -> &[u8] {
        cstr(&self.strings, ofs)
    }
    pub fn find_field(&self, name: &str) -> Option<&Def> {
        self.field_map.get(name.as_bytes()).map(|&i| &self.fielddefs[i])
    }
    pub fn find_global(&self, name: &str) -> Option<&Def> {
        self.global_map.get(name.as_bytes()).map(|&i| &self.globaldefs[i])
    }
    /// Function index by name (ED_FindFunction).
    pub fn find_function(&self, name: &str) -> Option<usize> {
        self.func_map.get(name.as_bytes()).copied()
    }
    pub fn function_name(&self, f: usize) -> String {
        self.functions.get(f).map_or_else(String::new, |f| String::from_utf8_lossy(self.string(f.name)).into_owned())
    }
    /// Global def at a given offset (ED_GlobalAtOfs).
    pub fn global_at_ofs(&self, ofs: usize) -> Option<&Def> {
        self.globaldefs.iter().find(|d| d.ofs as usize == ofs)
    }
    /// Field def at a given offset (ED_FieldAtOfs).
    pub fn field_at_ofs(&self, ofs: usize) -> Option<&Def> {
        self.fielddefs.iter().find(|d| d.ofs as usize == ofs)
    }
}
