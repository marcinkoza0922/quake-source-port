//! Builtin numbering/resolution and VM-internal builtins. Port of the
//! PF_BOTH math/string parts of pr_cmds.c and PR_FillBuiltinTable.

use std::rc::Rc;

use crate::edict::fmt_g;
use crate::progs::Progs;
use crate::{Host, Result, Vm, ofs_parm};

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

fn sys_vecs(vm: &Vm) -> Result<[usize; 3]> {
    let g = |n| vm.global_ofs(n).ok_or_else(|| vm.run_error(format!("missing global {n}")));
    Ok([g("v_forward")?, g("v_right")?, g("v_up")?])
}

fn pf_makevectors(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let [f, r, u] = sys_vecs(vm)?;
    let (a, b, c) = angle_vectors(vm.g_v(ofs_parm(0)));
    vm.set_g_v(f, a);
    vm.set_g_v(r, b);
    vm.set_g_v(u, c);
    Ok(())
}

fn normalize_f(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { v } else { v.map(|x| x * (1.0 / l)) }
}

fn pf_vectorvectors(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let [fo, ro, uo] = sys_vecs(vm)?;
    let f = normalize_f(vm.g_v(ofs_parm(0)));
    let r = if f[0] == 0.0 && f[1] == 0.0 {
        [0.0, if f[2] != 0.0 { -1.0 } else { 0.0 }, 0.0]
    } else {
        normalize_f([f[1], -f[0], 0.0])
    };
    let u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
    vm.set_g_v(fo, f);
    vm.set_g_v(ro, r);
    vm.set_g_v(uo, u);
    Ok(())
}

fn pf_break(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    Err(vm.run_error("break statement"))
}

fn pf_random(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    // sv_gameplayfix_random 1
    let n = (vm.rand15() as f32 + 0.5) * (1.0 / 32768.0);
    vm.ret_f(n);
    Ok(())
}

fn pf_normalize(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    let l = (v[0] as f64 * v[0] as f64 + v[1] as f64 * v[1] as f64 + v[2] as f64 * v[2] as f64).sqrt();
    let r = if l == 0.0 { [0.0; 3] } else { v.map(|x| (x as f64 * (1.0 / l)) as f32) };
    vm.ret_v(r);
    Ok(())
}

fn pf_vlen(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    let l = (v[0] as f64 * v[0] as f64 + v[1] as f64 * v[1] as f64 + v[2] as f64 * v[2] as f64).sqrt();
    vm.ret_f(l as f32);
    Ok(())
}

fn yaw_of(v: [f32; 3]) -> f32 {
    if v[1] == 0.0 && v[0] == 0.0 {
        return 0.0;
    }
    let mut yaw = ((v[1] as f64).atan2(v[0] as f64) * 180.0 / std::f64::consts::PI) as i32 as f32;
    if yaw < 0.0 {
        yaw += 360.0;
    }
    yaw
}

fn pf_vectoyaw(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let y = yaw_of(vm.g_v(ofs_parm(0)));
    vm.ret_f(y);
    Ok(())
}

fn pf_vectoangles(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    let (pitch, yaw);
    if v[1] == 0.0 && v[0] == 0.0 {
        yaw = 0.0;
        pitch = if v[2] > 0.0 { 90.0 } else { 270.0 };
    } else {
        yaw = yaw_of(v);
        let fwd = (v[0] * v[0] + v[1] * v[1]).sqrt();
        let mut p = ((v[2] as f64).atan2(fwd as f64) * 180.0 / std::f64::consts::PI) as i32 as f32;
        if p < 0.0 {
            p += 360.0;
        }
        pitch = p;
    }
    vm.ret_v([pitch, yaw, 0.0]);
    Ok(())
}

fn pf_dprint(vm: &mut Vm, host: &mut dyn Host) -> Result<()> {
    let s = vm.var_string(0)?;
    host.dprint(&String::from_utf8_lossy(&s));
    Ok(())
}

/// PF_ftos formatting.
pub fn ftos(v: f32) -> String {
    if v == (v as i32) as f32 { format!("{}", v as i32) } else { format!("{v:5.1}") }
}

fn pf_ftos(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let s = ftos(vm.g_f(ofs_parm(0)));
    vm.ret_temp(s.as_bytes());
    Ok(())
}

fn pf_vtos(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let v = vm.g_v(ofs_parm(0));
    let s = format!("'{:5.1} {:5.1} {:5.1}'", v[0], v[1], v[2]);
    vm.ret_temp(s.as_bytes());
    Ok(())
}

fn pf_substring(vm: &mut Vm, host: &mut dyn Host) -> Result<()> {
    let s = vm.g_str(ofs_parm(0))?.to_vec();
    let (mut start, mut length) = (vm.g_f(ofs_parm(1)) as i32, vm.g_f(ofs_parm(2)) as i32);
    let mut slen = s.len() as i32;
    if start < 0 {
        start += slen;
    }
    if length < 0 {
        length = slen - start + (length + 1);
    }
    if start < 0 {
        start = 0;
    }
    if start >= slen || length <= 0 {
        let h = vm.strings.engine(b"");
        vm.ret_i(h);
        return Ok(());
    }
    slen -= start;
    length = length.min(slen);
    if length as usize >= crate::STRINGTEMP_LENGTH {
        length = crate::STRINGTEMP_LENGTH as i32 - 1;
        host.print("PF_substring: truncation\n");
    }
    vm.ret_temp(&s[start as usize..(start + length) as usize]);
    Ok(())
}

fn pf_strconv(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let ccase = vm.g_f(ofs_parm(0)) as i32;
    let redalpha = vm.g_f(ofs_parm(1)) as i32;
    let redchars = vm.g_f(ofs_parm(2)) as i32;
    let mut s = vm.var_string(3)?;
    for c in &mut s {
        let base = *c & 0x7f;
        let alpha = base.is_ascii_alphabetic();
        let mode = if alpha { redalpha } else { redchars };
        let mut ch = *c;
        match ccase {
            1 => ch = (ch & 0x80) | (ch & 0x7f).to_ascii_lowercase(),
            2 => ch = (ch & 0x80) | (ch & 0x7f).to_ascii_uppercase(),
            _ => {}
        }
        match mode {
            1 => ch |= 0x80,
            2 => ch &= 0x7f,
            _ => {}
        }
        *c = ch;
    }
    s.truncate(crate::STRINGTEMP_LENGTH - 1);
    vm.ret_temp(&s);
    Ok(())
}

/// Minimal C printf for one spec.
fn pad(s: String, width: usize, left: bool, zero: bool) -> String {
    if s.len() >= width {
        return s;
    }
    let n = width - s.len();
    if left {
        s + &" ".repeat(n)
    } else if zero {
        let (sign, rest) = if s.starts_with('-') || s.starts_with('+') { s.split_at(1) } else { ("", s.as_str()) };
        format!("{sign}{}{rest}", "0".repeat(n))
    } else {
        " ".repeat(n) + &s
    }
}

fn pf_sprintf(vm: &mut Vm, _: &mut dyn Host) -> Result<()> {
    let fmt = vm.g_str(ofs_parm(0))?.to_vec();
    let mut out: Vec<u8> = Vec::new();
    let mut arg = 1;
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        if i < fmt.len() && fmt[i] == b'%' {
            out.push(b'%');
            i += 1;
            continue;
        }
        let (mut left, mut zero, mut plus) = (false, false, false);
        while i < fmt.len() && b"#0- +".contains(&fmt[i]) {
            match fmt[i] {
                b'-' => left = true,
                b'0' => zero = true,
                b'+' => plus = true,
                _ => {}
            }
            i += 1;
        }
        let num = |i: &mut usize, arg: &mut usize| -> Option<usize> {
            if *i < fmt.len() && fmt[*i] == b'*' {
                *i += 1;
                let v = if *arg < vm.argc { vm.g_f(ofs_parm(*arg)) as i32 } else { 0 };
                *arg += 1;
                return Some(v.max(0) as usize);
            }
            let st = *i;
            while *i < fmt.len() && fmt[*i].is_ascii_digit() {
                *i += 1;
            }
            std::str::from_utf8(&fmt[st..*i]).ok()?.parse().ok()
        };
        let width = num(&mut i, &mut arg).unwrap_or(0);
        let prec = if i < fmt.len() && fmt[i] == b'.' {
            i += 1;
            Some(num(&mut i, &mut arg).unwrap_or(0))
        } else {
            None
        };
        while i < fmt.len() && b"hlLjzt".contains(&fmt[i]) {
            i += 1;
        }
        let Some(&conv) = fmt.get(i) else { break };
        i += 1;
        let p = ofs_parm(arg);
        let have = arg < vm.argc && arg < 8;
        arg += 1;
        let fa = if have { vm.g_f(p) } else { 0.0 };
        let sign = |s: String, v: f32| if plus && v >= 0.0 { format!("+{s}") } else { s };
        let s: Vec<u8> = match conv {
            b'd' | b'i' => sign(format!("{}", fa as i32), fa).into_bytes(),
            b'u' => format!("{}", fa as i32 as u32).into_bytes(),
            b'x' => format!("{:x}", fa as i32).into_bytes(),
            b'X' => format!("{:X}", fa as i32).into_bytes(),
            b'c' => vec![fa as i32 as u8],
            b'f' | b'F' => sign(format!("{:.*}", prec.unwrap_or(6), fa), fa).into_bytes(),
            b'e' | b'E' => sign(format!("{:.*e}", prec.unwrap_or(6), fa), fa).into_bytes(),
            b'g' | b'G' => sign(fmt_g(fa), fa).into_bytes(),
            b'v' | b'V' => {
                let v = if have { vm.g_v(p) } else { [0.0; 3] };
                let f = |x: f32| match prec {
                    Some(pr) => format!("{x:.pr$}"),
                    None => fmt_g(x),
                };
                format!("{} {} {}", f(v[0]), f(v[1]), f(v[2])).into_bytes()
            }
            b's' | b'S' => {
                let mut s = if have { vm.strings.get(vm.g_i(p))?.to_vec() } else { Vec::new() };
                if let Some(pr) = prec {
                    s.truncate(pr);
                }
                s
            }
            other => vec![b'%', other],
        };
        let s = pad(String::from_utf8_lossy(&s).into_owned(), width, left, zero && !matches!(conv, b's' | b'S'));
        out.extend_from_slice(s.as_bytes());
    }
    out.truncate(crate::STRINGTEMP_LENGTH - 1);
    vm.ret_temp(&out);
    Ok(())
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
