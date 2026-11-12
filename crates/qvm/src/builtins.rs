//! Builtin numbering/resolution and VM-internal builtins. Port of the
//! PF_BOTH math/string parts of pr_cmds.c and PR_FillBuiltinTable.

use std::rc::Rc;

use crate::edict::fmt_g;
use crate::progs::Progs;
use crate::{Host, Result, Vm, ofs_parm};
mod pf;
pub use pf::*;

macro_rules! builtin_enum {
    ($($v:ident = $n:literal $name:literal,)*) => {
        /// id1 builtin numbers (defs.qc). Those the VM handles itself (math,
        /// strings) never reach [`Host::builtin`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u16)]
        pub enum Builtin { $($v = $n,)* }
        impl Builtin {
            pub const ALL: &'static [Builtin] = &[$(Builtin::$v,)*];
            pub fn from_num(n: i32) -> Option<Self> {
                match n { $($n => Some(Self::$v),)* _ => None }
            }
            /// QC name.
            pub fn name(self) -> &'static str {
                match self { $(Self::$v => $name,)* }
            }
        }
    };
}

builtin_enum! {
    Makevectors = 1 "makevectors", Setorigin = 2 "setorigin", Setmodel = 3 "setmodel",
    Setsize = 4 "setsize", Break = 6 "break", Random = 7 "random", Sound = 8 "sound",
    Normalize = 9 "normalize", Error = 10 "error", Objerror = 11 "objerror", Vlen = 12 "vlen",
    Vectoyaw = 13 "vectoyaw", Spawn = 14 "spawn", Remove = 15 "remove", Traceline = 16 "traceline",
    Checkclient = 17 "checkclient", Find = 18 "find", PrecacheSound = 19 "precache_sound",
    PrecacheModel = 20 "precache_model", Stuffcmd = 21 "stuffcmd", Findradius = 22 "findradius",
    Bprint = 23 "bprint", Sprint = 24 "sprint", Dprint = 25 "dprint", Ftos = 26 "ftos",
    Vtos = 27 "vtos", Coredump = 28 "coredump", Traceon = 29 "traceon", Traceoff = 30 "traceoff",
    Eprint = 31 "eprint", Walkmove = 32 "walkmove", Droptofloor = 34 "droptofloor",
    Lightstyle = 35 "lightstyle", Rint = 36 "rint", Floor = 37 "floor", Ceil = 38 "ceil",
    Checkbottom = 40 "checkbottom", Pointcontents = 41 "pointcontents", Fabs = 43 "fabs",
    Aim = 44 "aim", Cvar = 45 "cvar", Localcmd = 46 "localcmd", Nextent = 47 "nextent",
    Particle = 48 "particle", ChangeYaw = 49 "ChangeYaw", Vectoangles = 51 "vectoangles",
    WriteByte = 52 "WriteByte", WriteChar = 53 "WriteChar", WriteShort = 54 "WriteShort",
    WriteLong = 55 "WriteLong", WriteCoord = 56 "WriteCoord", WriteAngle = 57 "WriteAngle",
    WriteString = 58 "WriteString", WriteEntity = 59 "WriteEntity", Movetogoal = 67 "movetogoal",
    PrecacheFile = 68 "precache_file", Makestatic = 69 "makestatic", Changelevel = 70 "changelevel",
    CvarSet = 72 "cvar_set", Centerprint = 73 "centerprint", Ambientsound = 74 "ambientsound",
    PrecacheModel2 = 75 "precache_model2", PrecacheSound2 = 76 "precache_sound2",
    PrecacheFile2 = 77 "precache_file2", Setspawnparms = 78 "setspawnparms",
    FinaleFinished = 79 "finaleFinished", Localsound = 80 "localsound",
}

impl Builtin {
    /// Name lookup incl. the 2021 re-release `ex_*` aliases. The aliases are
    /// matched case-insensitively: rogue's progs declare `ex_finaleFinished`.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "ex_centerprint" => Self::Centerprint,
            "ex_bprint" => Self::Bprint,
            "ex_sprint" => Self::Sprint,
            "ex_finalefinished" => Self::FinaleFinished,
            "ex_localsound" => Self::Localsound,
            _ => return Self::ALL.iter().copied().find(|b| b.name() == s),
        })
    }
}

pub(crate) type InternalFn = fn(&mut Vm, &mut dyn Host) -> Result<()>;

#[derive(Clone)]
pub(crate) enum Dispatch {
    Qc,
    Internal(InternalFn),
    Host(Builtin),
    HostExt(Rc<str>),
    Missing(i32),
}

/// Host-side extension builtins that have fixed numbers.
const HOST_EXT_NUMS: &[(&str, i32)] = &[
    ("clientstat", 232),
    ("checkcommand", 294),
    ("particleeffectnum", 335),
    ("trailparticles", 336),
    ("pointparticles", 337),
    ("te_particlerain", 409),
    ("te_particlesnow", 410),
    ("clientcommand", 440),
];

/// Extension names the VM-internal builtins implement (for documentation /
/// host `checkextension` policy). Like ironwail, `checkextension` only
/// advertises FTE_QC_CHECKCOMMAND plus whatever [`Host::has_extension`] allows.
pub const EXTENSIONS: &[&str] = &[
    "DP_QC_SINCOSSQRTPOW", "DP_QC_ETOS", "DP_QC_MINMAXBOUND", "FRIK_FILE", "FTE_STRINGS",
    "DP_QC_VECTORVECTORS", "KRIMZON_SV_PARSECLIENTCOMMAND", "DP_QC_ASINACOSATANATAN2TAN",
    "DP_QC_TOKENIZE_CONSOLE", "DP_QC_SPRINTF",
];

const INTERNAL: &[(&str, i32, InternalFn)] = &[
    ("makevectors", 1, pf_makevectors),
    ("break", 6, pf_break),
    ("random", 7, pf_random),
    ("normalize", 9, pf_normalize),
    ("vlen", 12, pf_vlen),
    ("vectoyaw", 13, pf_vectoyaw),
    ("dprint", 25, pf_dprint),
    ("ftos", 26, pf_ftos),
    ("vtos", 27, pf_vtos),
    ("traceon", 29, |vm, _| {
        vm.trace = true;
        Ok(())
    }),
    ("traceoff", 30, |vm, _| {
        vm.trace = false;
        Ok(())
    }),
    ("rint", 36, |vm, _| {
        let f = vm.g_f(ofs_parm(0));
        vm.ret_f(if f > 0.0 { (f + 0.5) as i32 } else { (f - 0.5) as i32 } as f32);
        Ok(())
    }),
    ("floor", 37, |vm, _| f1(vm, f32::floor)),
    ("ceil", 38, |vm, _| f1(vm, f32::ceil)),
    ("fabs", 43, |vm, _| f1(vm, f32::abs)),
    ("vectoangles", 51, pf_vectoangles),
    ("sin", 60, |vm, _| f1(vm, f32::sin)),
    ("cos", 61, |vm, _| f1(vm, f32::cos)),
    ("sqrt", 62, |vm, _| f1(vm, f32::sqrt)),
    ("etos", 65, |vm, _| {
        let s = format!("entity {}", vm.g_i(ofs_parm(0)));
        vm.ret_temp(s.as_bytes());
        Ok(())
    }),
    ("stof", 81, |vm, _| {
        let v = atof(vm.g_str(ofs_parm(0))?);
        vm.ret_f(v);
        Ok(())
    }),
    ("min", 94, |vm, _| fold(vm, |r, x| if r > x { x } else { r })),
    ("max", 95, |vm, _| fold(vm, |r, x| if r < x { x } else { r })),
    ("bound", 96, |vm, _| {
        let (lo, mut v, hi) = (vm.g_f(ofs_parm(0)), vm.g_f(ofs_parm(1)), vm.g_f(ofs_parm(2)));
        if v > hi {
            v = hi;
        }
        if v < lo {
            v = lo;
        }
        vm.ret_f(v);
        Ok(())
    }),
    ("pow", 97, |vm, _| {
        let v = vm.g_f(ofs_parm(0)).powf(vm.g_f(ofs_parm(1)));
        vm.ret_f(v);
        Ok(())
    }),
    ("checkextension", 99, |vm, host| {
        let name = vm.g_string(ofs_parm(0))?;
        let r = name == "FTE_QC_CHECKCOMMAND" || host.has_extension(&name);
        vm.ret_f(r as i32 as f32);
        Ok(())
    }),
    ("strlen", 114, |vm, _| {
        let n = vm.g_str(ofs_parm(0))?.len();
        vm.ret_f(n as f32);
        Ok(())
    }),
    ("strcat", 115, |vm, _| {
        let mut s = vm.var_string(0)?;
        s.truncate(crate::STRINGTEMP_LENGTH - 1);
        vm.ret_temp(&s);
        Ok(())
    }),
    ("substring", 116, pf_substring),
    ("stov", 117, |vm, _| {
        let s = String::from_utf8_lossy(vm.g_str(ofs_parm(0))?).into_owned();
        let mut rest = s.as_str();
        let mut v = [0.0; 3];
        for x in &mut v {
            let (tok, r) = qcore::cmd::com_parse(rest).unwrap_or(("", ""));
            *x = atof(tok.as_bytes());
            rest = r;
        }
        vm.ret_v(v);
        Ok(())
    }),
    ("strzone", 118, |vm, _| {
        let s = vm.var_string(0)?;
        let h = vm.strings.zone(&s);
        vm.ret_i(h);
        Ok(())
    }),
    ("strunzone", 119, |vm, host| {
        let h = vm.g_i(ofs_parm(0));
        if !vm.strings.unzone(h) {
            host.print("PF_strunzone: string wasn't strzoned\n");
        }
        Ok(())
    }),
    ("str2chr", 222, |vm, _| {
        let s = vm.g_str(ofs_parm(0))?;
        let mut ofs = if vm.argc > 1 { vm.g_f(ofs_parm(1)) as i32 } else { 0 };
        if ofs < 0 {
            ofs += s.len() as i32;
        }
        let r = if ofs != 0 && (ofs < 0 || ofs > s.len() as i32) { 0 } else { s.get(ofs as usize).copied().unwrap_or(0) };
        vm.ret_f(r as f32);
        Ok(())
    }),
    ("chr2str", 223, |vm, _| {
        let mut out = Vec::new();
        for i in 0..vm.argc {
            let u = vm.g_f(ofs_parm(i)) as u32;
            out.push(if (0xe000..0xe100).contains(&u) || u < 256 { u as u8 } else { b'?' });
        }
        vm.ret_temp(&out);
        Ok(())
    }),
    ("strconv", 224, pf_strconv),
    ("mod", 245, |vm, host| {
        let (a, n) = (vm.g_f(ofs_parm(0)), vm.g_f(ofs_parm(1)));
        if n == 0.0 {
            host.dprint("PF_mod: mod by zero\n");
            vm.ret_f(0.0);
        } else {
            vm.ret_f(a - n * ((a / n) as i32) as f32);
        }
        Ok(())
    }),
    ("ftoi", 0, |vm, _| {
        let v = vm.g_f(ofs_parm(0)) as i32;
        vm.ret_i(v);
        Ok(())
    }),
    ("itof", 0, |vm, _| {
        let v = vm.g_i(ofs_parm(0)) as f32;
        vm.ret_f(v);
        Ok(())
    }),
    ("vectorvectors", 432, pf_vectorvectors),
    ("tokenize", 441, |vm, _| {
        let s = String::from_utf8_lossy(vm.g_str(ofs_parm(0))?).into_owned();
        vm.tokens.clear();
        let mut rest = s.as_str();
        while vm.tokens.len() < 64 {
            let Some((t, r)) = qcore::cmd::com_parse(rest) else { break };
            vm.tokens.push(t.as_bytes().to_vec());
            rest = r;
        }
        vm.ret_f(vm.tokens.len() as f32);
        Ok(())
    }),
    ("argv", 442, |vm, _| {
        let mut i = vm.g_f(ofs_parm(0)) as i32;
        if i < 0 {
            i += vm.tokens.len() as i32;
        }
        match vm.tokens.get(i as usize).cloned() {
            Some(t) if i >= 0 => vm.ret_temp(&t),
            _ => vm.ret_i(0),
        }
        Ok(())
    }),
    ("argc", 0, |vm, _| {
        vm.ret_f(vm.tokens.len() as f32);
        Ok(())
    }),
    ("asin", 471, |vm, _| f1(vm, f32::asin)),
    ("acos", 472, |vm, _| f1(vm, f32::acos)),
    ("atan", 473, |vm, _| f1(vm, f32::atan)),
    ("atan2", 474, |vm, _| {
        let v = vm.g_f(ofs_parm(0)).atan2(vm.g_f(ofs_parm(1)));
        vm.ret_f(v);
        Ok(())
    }),
    ("tan", 475, |vm, _| f1(vm, f32::tan)),
    ("tokenize_console", 514, |vm, _| {
        let s = String::from_utf8_lossy(vm.g_str(ofs_parm(0))?).into_owned();
        let a = qcore::cmd::Args::tokenize(&s);
        vm.tokens = a.all().iter().map(|t| t.as_bytes().to_vec()).collect();
        vm.ret_f(vm.tokens.len() as f32);
        Ok(())
    }),
    ("sprintf", 627, pf_sprintf),
];

/// PR_FillBuiltinTable + PR_PatchRereleaseBuiltins: decide per function how calls dispatch.
pub(crate) fn resolve(p: &Progs) -> Vec<Dispatch> {
    let by_num = |n: i32| -> Dispatch {
        if let Some(&(_, _, f)) = INTERNAL.iter().find(|i| i.1 == n) {
            return Dispatch::Internal(f);
        }
        if let Some(b) = Builtin::from_num(n) {
            return Dispatch::Host(b);
        }
        if let Some(&(name, _)) = HOST_EXT_NUMS.iter().find(|i| i.1 == n) {
            return Dispatch::HostExt(name.into());
        }
        Dispatch::Missing(n)
    };
    p.functions
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let name = String::from_utf8_lossy(p.string(f.name)).into_owned();
            if f.first_statement > 0 {
                return Dispatch::Qc;
            }
            if f.first_statement < 0 {
                let n = match (name.as_str(), -f.first_statement) {
                    ("centerprint", 90) => 73,
                    ("bprint", 91) => 23,
                    ("sprint", 92) => 24,
                    (_, n) => n,
                };
                return by_num(n);
            }
            if i == 0 || f.parm_start != 0 || f.locals != 0 {
                return Dispatch::Missing(0);
            }
            if let Some(&(_, _, f)) = INTERNAL.iter().find(|x| x.0 == name) {
                return Dispatch::Internal(f);
            }
            if let Some(b) = Builtin::from_name(&name) {
                return Dispatch::Host(b);
            }
            Dispatch::HostExt(name.into())
        })
        .collect()
}

impl Vm {
    /// PF_VarString: concatenation of string args `first..argc`.
    pub fn var_string(&self, first: usize) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        for i in first..self.argc {
            out.extend_from_slice(self.g_str(ofs_parm(i))?);
        }
        Ok(out)
    }

    /// Seed for `random()`.
    pub fn seed_random(&mut self, seed: u32) {
        self.rng = seed;
    }

    /// C `rand() & 0x7fff` equivalent.
    pub fn rand15(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (self.rng >> 16) & 0x7fff
    }
}

fn f1(vm: &mut Vm, f: fn(f32) -> f32) -> Result<()> {
    let v = f(vm.g_f(ofs_parm(0)));
    vm.ret_f(v);
    Ok(())
}

fn fold(vm: &mut Vm, f: fn(f32, f32) -> f32) -> Result<()> {
    let mut r = vm.g_f(ofs_parm(0));
    for i in 1..vm.argc {
        r = f(r, vm.g_f(ofs_parm(i)));
    }
    vm.ret_f(r);
    Ok(())
}

/// C `atof`: leading whitespace, optional sign, digits, fraction, exponent; ignores the rest.
pub fn atof(s: &[u8]) -> f32 {
    let s = std::str::from_utf8(s).unwrap_or("");
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits = |i: &mut usize| {
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
    };
    digits(&mut i);
    if i < b.len() && b[i] == b'.' {
        i += 1;
        digits(&mut i);
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            i = j;
            digits(&mut i);
        }
    }
    s[..i].parse::<f64>().unwrap_or(0.0) as f32
}

/// AngleVectors (mathlib.c).
pub fn angle_vectors(a: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let r = std::f32::consts::PI * 2.0 / 360.0;
    let (sy, cy) = (a[1] * r).sin_cos();
    let (sp, cp) = (a[0] * r).sin_cos();
    let (sr, cr) = (a[2] * r).sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}


#[cfg(test)]
mod name_tests {
    use super::Builtin;

    #[test]
    fn rerelease_aliases_ignore_case() {
        assert_eq!(Builtin::from_name("ex_finaleFinished"), Some(Builtin::FinaleFinished));
        assert_eq!(Builtin::from_name("ex_finalefinished"), Some(Builtin::FinaleFinished));
        assert_eq!(Builtin::from_name("finaleFinished"), Some(Builtin::FinaleFinished));
    }
}
