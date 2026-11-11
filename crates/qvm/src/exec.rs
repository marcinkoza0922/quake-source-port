//! Interpreter. Port of pr_exec.c.

use qecs::EntityAllocator;

use crate::builtins::{self, Dispatch};
use crate::progs::Progs;
use crate::strings::Strings;
use crate::{Host, OFS_PARM0, OFS_RETURN, Result, VmError};

pub const MAX_STACK_DEPTH: usize = 1024;
pub const LOCALSTACK_SIZE: usize = 16384;
/// Statements per [`Vm::execute`] before "runaway loop error".
pub const RUNAWAY_LIMIT: u32 = 0x100_0000;

pub(crate) const OP_DONE: u16 = 0;
pub(crate) const OP_IF: u16 = 49;
pub(crate) const OP_IFNOT: u16 = 50;
pub(crate) const OP_CALL0: u16 = 51;
pub(crate) const OP_CALL8: u16 = 59;
pub(crate) const OP_GOTO: u16 = 61;

#[derive(Debug, Clone, Copy, Default)]
struct Frame {
    s: usize,
    f: usize,
}

/// Offsets of system globals/fields the VM itself needs (progdefs.h).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sys {
    pub g_self: usize,
    pub g_time: usize,
    pub f_nextthink: usize,
    pub f_frame: usize,
    pub f_think: usize,
    pub f_classname: usize,
}

/// A loaded QuakeC program and its state.
pub struct Vm {
    pub progs: Progs,
    /// Global slots (`numglobals` + 3 padding for vector operands).
    pub globals: Vec<u32>,
    pub strings: Strings,
    /// Server time (`qcvm->time`), used by ED_Alloc/ED_Free.
    pub time: f64,
    /// Argument count of the builtin being called.
    pub argc: usize,
    /// `traceon`/`traceoff` flag (statements are not printed; kept for builtins).
    pub trace: bool,
    /// Error on `OP_ADDRESS` of world (the reference checks `sv.state == ss_active`).
    pub protect_world: bool,
    pub(crate) sys: Sys,
    pub(crate) dispatch: Vec<Dispatch>,
    pub(crate) fields: Vec<u32>,
    pub(crate) alloc: EntityAllocator,
    pub(crate) max_edicts: usize,
    pub(crate) reserved: usize,
    pub(crate) rng: u32,
    pub(crate) tokens: Vec<Vec<u8>>,
    stack: Vec<Frame>,
    localstack: Vec<u32>,
    xfunction: usize,
    xstatement: usize,
}

impl Vm {
    /// Create a VM for `progs` with room for `max_edicts`; world + `maxclients` edicts are reserved.
    pub fn new(progs: Progs, max_edicts: usize, maxclients: usize) -> Result<Self> {
        let need = |n: &str, f: bool| -> Result<usize> {
            let d = if f { progs.find_field(n) } else { progs.find_global(n) };
            d.map(|d| d.ofs as usize).ok_or_else(|| VmError::new(format!("progs.dat: missing system def {n}")))
        };
        let sys = Sys {
            g_self: need("self", false)?,
            g_time: need("time", false)?,
            f_nextthink: need("nextthink", true)?,
            f_frame: need("frame", true)?,
            f_think: need("think", true)?,
            f_classname: need("classname", true)?,
        };
        let mut globals = progs.globals.clone();
        globals.extend([0; 3]);
        let mut vm = Vm {
            strings: Strings::new(progs.strings.clone()),
            dispatch: Vec::new(),
            globals,
            time: 0.0,
            argc: 0,
            trace: false,
            protect_world: false,
            sys,
            fields: Vec::new(),
            alloc: EntityAllocator::new(),
            max_edicts: max_edicts.max(maxclients + 1),
            reserved: maxclients + 1,
            rng: 1,
            tokens: Vec::new(),
            stack: Vec::with_capacity(64),
            localstack: Vec::with_capacity(LOCALSTACK_SIZE),
            xfunction: 0,
            xstatement: 0,
            progs,
        };
        vm.dispatch = builtins::resolve(&vm.progs);
        vm.clear_edicts();
        Ok(vm)
    }

    /// Load and create in one step.
    pub fn load(data: &[u8], max_edicts: usize, maxclients: usize) -> Result<Self> {
        Self::new(Progs::parse(data)?, max_edicts, maxclients)
    }

    // ---- globals ----
    #[inline]
    pub fn g_i(&self, ofs: usize) -> i32 {
        self.globals[ofs] as i32
    }
    #[inline]
    pub fn g_f(&self, ofs: usize) -> f32 {
        f32::from_bits(self.globals[ofs])
    }
    #[inline]
    pub fn g_v(&self, ofs: usize) -> [f32; 3] {
        [self.g_f(ofs), self.g_f(ofs + 1), self.g_f(ofs + 2)]
    }
    #[inline]
    pub fn set_g_i(&mut self, ofs: usize, v: i32) {
        self.globals[ofs] = v as u32;
    }
    #[inline]
    pub fn set_g_f(&mut self, ofs: usize, v: f32) {
        self.globals[ofs] = v.to_bits();
    }
    #[inline]
    pub fn set_g_v(&mut self, ofs: usize, v: [f32; 3]) {
        for (i, x) in v.into_iter().enumerate() {
            self.globals[ofs + i] = x.to_bits();
        }
    }
    /// String global (G_STRING).
    pub fn g_str(&self, ofs: usize) -> Result<&[u8]> {
        self.strings.get(self.g_i(ofs))
    }
    /// Lossy UTF-8 of a string global.
    pub fn g_string(&self, ofs: usize) -> Result<String> {
        Ok(String::from_utf8_lossy(self.g_str(ofs)?).into_owned())
    }
    pub fn ret_f(&mut self, v: f32) {
        self.set_g_f(OFS_RETURN, v)
    }
    pub fn ret_i(&mut self, v: i32) {
        self.set_g_i(OFS_RETURN, v)
    }
    pub fn ret_v(&mut self, v: [f32; 3]) {
        self.set_g_v(OFS_RETURN, v)
    }
    /// Return an entity (RETURN_EDICT).
    pub fn ret_e(&mut self, e: u32) {
        self.set_g_i(OFS_RETURN, e as i32)
    }
    /// Return a copy of `s` in a tempstring.
    pub fn ret_temp(&mut self, s: &[u8]) {
        let h = self.strings.temp(s);
        self.ret_i(h);
    }
    /// Global offset by name.
    pub fn global_ofs(&self, name: &str) -> Option<usize> {
        self.progs.find_global(name).map(|d| d.ofs as usize)
    }
    /// Field offset by name.
    pub fn field_ofs(&self, name: &str) -> Option<usize> {
        self.progs.find_field(name).map(|d| d.ofs as usize)
    }
    /// `self` global as edict number.
    pub fn self_ent(&self) -> u32 {
        self.g_i(self.sys.g_self) as u32
    }
    pub fn set_self(&mut self, e: u32) {
        self.set_g_i(self.sys.g_self, e as i32)
    }

    /// Current call depth.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// PR_StackTrace lines, innermost first.
    pub fn stack_trace(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.stack.is_empty() && self.xfunction == 0 {
            return out;
        }
        let cur = Frame { s: self.xstatement, f: self.xfunction };
        for fr in std::iter::once(&cur).chain(self.stack.iter().rev()) {
            if fr.f == 0 {
                out.push("<NO FUNCTION>".into());
                continue;
            }
            let f = &self.progs.functions[fr.f];
            out.push(format!(
                "{:>12} : {} : statement {}",
                String::from_utf8_lossy(self.progs.string(f.file)),
                String::from_utf8_lossy(self.progs.string(f.name)),
                fr.s as i64 - f.first_statement as i64
            ));
        }
        out
    }

    /// PR_RunError: error with QC stack trace. The interpreter dumps the stack when it propagates.
    pub fn run_error(&self, msg: impl Into<String>) -> VmError {
        VmError { msg: msg.into(), trace: self.stack_trace() }
    }

    fn fail(&mut self, e: VmError) -> VmError {
        let mut e = e;
        if e.trace.is_empty() {
            e.trace = self.stack_trace();
        }
        // dump the stack so the host can shut down
        self.stack.clear();
        self.localstack.clear();
        self.xfunction = 0;
        e
    }

    fn enter_function(&mut self, fnum: usize) -> Result<usize> {
        self.stack.push(Frame { s: self.xstatement, f: self.xfunction });
        if self.stack.len() >= MAX_STACK_DEPTH {
            return Err(self.run_error("stack overflow"));
        }
        let f = self.progs.functions[fnum];
        let (ps, c) = (f.parm_start as usize, f.locals as usize);
        if self.localstack.len() + c > LOCALSTACK_SIZE {
            return Err(self.run_error("PR_ExecuteProgram: locals stack overflow"));
        }
        self.localstack.extend_from_slice(&self.globals[ps..ps + c]);
        let mut o = ps;
        for i in 0..f.numparms as usize {
            for j in 0..f.parm_size[i] as usize {
                self.globals[o] = self.globals[OFS_PARM0 + i * 3 + j];
                o += 1;
            }
        }
        self.xfunction = fnum;
        Ok(f.first_statement as usize)
    }

    fn leave_function(&mut self) -> Result<usize> {
        let Some(fr) = self.stack.last().copied() else {
            return Err(VmError::new("prog stack underflow"));
        };
        let f = self.progs.functions[self.xfunction];
        let (ps, c) = (f.parm_start as usize, f.locals as usize);
        if c > self.localstack.len() {
            return Err(self.run_error("PR_ExecuteProgram: locals stack underflow"));
        }
        let base = self.localstack.len() - c;
        self.globals[ps..ps + c].copy_from_slice(&self.localstack[base..]);
        self.localstack.truncate(base);
        self.stack.pop();
        self.xfunction = fr.f;
        Ok(fr.s)
    }

    /// Call a function by name.
    pub fn execute_by_name(&mut self, host: &mut dyn Host, name: &str) -> Result<()> {
        let f = self.progs.find_function(name).ok_or_else(|| VmError::new(format!("no function {name}")))?;
        self.execute(host, f)
    }

    /// PR_ExecuteProgram. Reentrant (builtins may call back into QC).
    pub fn execute(&mut self, host: &mut dyn Host, fnum: usize) -> Result<()> {
        if fnum == 0 || fnum >= self.progs.functions.len() {
            return Err(VmError::new("PR_ExecuteProgram: NULL function"));
        }
        if self.progs.functions[fnum].first_statement <= 0 {
            // builtin called directly from the engine
            self.argc = self.progs.functions[fnum].numparms.clamp(0, 8) as usize;
            return self.call_builtin(host, fnum).map_err(|e| self.fail(e));
        }
        self.trace = false;
        let exitdepth = self.stack.len();
        let r = self.run(host, fnum, exitdepth);
        r.map_err(|e| self.fail(e))
    }

    fn call_builtin(&mut self, host: &mut dyn Host, fnum: usize) -> Result<()> {
        match self.dispatch[fnum].clone() {
            Dispatch::Qc => unreachable!(),
            Dispatch::Internal(f) => f(self, host),
            Dispatch::Host(b) => host.builtin(self, b),
            Dispatch::HostExt(name) => host.ext_builtin(self, &name),
            Dispatch::Missing(n) => Err(self.run_error(format!("unimplemented builtin #{n} ({})", self.progs.function_name(fnum)))),
        }
    }

    #[inline]
    fn ent_ptr(&self, e: i32, fld: i32, size: usize) -> Result<usize> {
        let stride = self.progs.entityfields;
        if e < 0 || e as usize >= self.alloc.len() {
            return Err(self.run_error(format!("bad entity number {e}")));
        }
        if fld < 0 || fld as usize + size > stride {
            return Err(self.run_error(format!("bad field offset {fld}")));
        }
        Ok(e as usize * stride + fld as usize)
    }

    #[inline]
    fn store_ptr(&self, p: i32, size: usize) -> Result<usize> {
        // pointers are byte offsets into the edict field area (as in the references)
        if p < 0 || p & 3 != 0 || (p as usize >> 2) + size > self.fields.len() {
            return Err(self.run_error(format!("bad pointer {p}")));
        }
        Ok(p as usize >> 2)
    }

    fn run(&mut self, host: &mut dyn Host, fnum: usize, exitdepth: usize) -> Result<()> {
        let mut pc = self.enter_function(fnum)?;
        let mut profile: u32 = 0;
        loop {
            let Some(&st) = self.progs.statements.get(pc) else {
                return Err(self.run_error("statement out of range"));
            };
            self.xstatement = pc;
            profile += 1;
            if profile > RUNAWAY_LIMIT {
                return Err(self.run_error("runaway loop error"));
            }
            let (a, b, c) = (st.a as usize, st.b as usize, st.c as usize);
            let g = &mut self.globals;
            macro_rules! f {
                ($i:expr) => {
                    f32::from_bits(g[$i])
                };
            }
            macro_rules! setf {
                ($i:expr, $v:expr) => {{
                    let v: f32 = $v;
                    g[$i] = v.to_bits()
                }};
            }
            macro_rules! b2f {
                ($e:expr) => {
                    if $e { 1.0 } else { 0.0 }
                };
            }
            let mut next = pc + 1;
            match st.op {
                1 => setf!(c, f!(a) * f!(b)),
                2 => setf!(c, f!(a) * f!(b) + f!(a + 1) * f!(b + 1) + f!(a + 2) * f!(b + 2)),
                3 => {
                    let s = f!(a);
                    for i in 0..3 {
                        setf!(c + i, s * f!(b + i));
                    }
                }
                4 => {
                    let s = f!(b);
                    for i in 0..3 {
                        setf!(c + i, s * f!(a + i));
                    }
                }
                5 => setf!(c, f!(a) / f!(b)),
                6 => setf!(c, f!(a) + f!(b)),
                7 => {
                    for i in 0..3 {
                        setf!(c + i, f!(a + i) + f!(b + i));
                    }
                }
                8 => setf!(c, f!(a) - f!(b)),
                9 => {
                    for i in 0..3 {
                        setf!(c + i, f!(a + i) - f!(b + i));
                    }
                }
                10 => setf!(c, b2f!(f!(a) == f!(b))),
                11 => setf!(c, b2f!(f!(a) == f!(b) && f!(a + 1) == f!(b + 1) && f!(a + 2) == f!(b + 2))),
                12 | 17 => {
                    let (sa, sb) = (g[a] as i32, g[b] as i32);
                    let eq = self.strings.get(sa)? == self.strings.get(sb)?;
                    let r = if st.op == 12 { eq } else { !eq };
                    setf!(c, b2f!(r));
                }
                13 | 14 => setf!(c, b2f!(g[a] == g[b])),
                15 => setf!(c, b2f!(f!(a) != f!(b))),
                16 => setf!(c, b2f!(f!(a) != f!(b) || f!(a + 1) != f!(b + 1) || f!(a + 2) != f!(b + 2))),
                18 | 19 => setf!(c, b2f!(g[a] != g[b])),
                20 => setf!(c, b2f!(f!(a) <= f!(b))),
                21 => setf!(c, b2f!(f!(a) >= f!(b))),
                22 => setf!(c, b2f!(f!(a) < f!(b))),
                23 => setf!(c, b2f!(f!(a) > f!(b))),
                // OP_LOAD_*
                24 | 26..=29 => {
                    let (e, fld) = (g[a] as i32, g[b] as i32);
                    let p = self.ent_ptr(e, fld, 1)?;
                    self.globals[c] = self.fields[p];
                }
                25 => {
                    let (e, fld) = (g[a] as i32, g[b] as i32);
                    let p = self.ent_ptr(e, fld, 3)?;
                    self.globals[c..c + 3].copy_from_slice(&self.fields[p..p + 3]);
                }
                // OP_ADDRESS
                30 => {
                    let (e, fld) = (g[a] as i32, g[b] as i32);
                    if e == 0 && self.protect_world {
                        return Err(self.run_error("assignment to world entity"));
                    }
                    let p = self.ent_ptr(e, fld, 1)?;
                    self.globals[c] = (p * 4) as u32;
                }
                // OP_STORE_*
                31 | 33..=36 => g[b] = g[a],
                32 => {
                    g[b] = g[a];
                    g[b + 1] = g[a + 1];
                    g[b + 2] = g[a + 2];
                }
                // OP_STOREP_*
                37 | 39..=42 => {
                    let gb = g[b] as i32;
                    let p = self.store_ptr(gb, 1)?;
                    self.fields[p] = self.globals[a];
                }
                38 => {
                    let gb = g[b] as i32;
                    let p = self.store_ptr(gb, 3)?;
                    self.fields[p..p + 3].copy_from_slice(&self.globals[a..a + 3]);
                }
                // OP_RETURN / OP_DONE
                OP_DONE | 43 => {
                    self.globals.copy_within(a..a + 3, OFS_RETURN);
                    next = self.leave_function()? + 1;
                    if self.stack.len() == exitdepth {
                        return Ok(());
                    }
                }
                44 => setf!(c, b2f!(f!(a) == 0.0)),
                45 => setf!(c, b2f!(f!(a) == 0.0 && f!(a + 1) == 0.0 && f!(a + 2) == 0.0)),
                46 => {
                    let s = g[a] as i32;
                    let r = s == 0 || self.strings.get(s)?.is_empty();
                    setf!(c, b2f!(r));
                }
                47 | 48 => setf!(c, b2f!(g[a] == 0)),
                OP_IF => {
                    if g[a] != 0 {
                        next = (pc as i64 + st.b as i16 as i64) as usize;
                    }
                }
                OP_IFNOT => {
                    if g[a] == 0 {
                        next = (pc as i64 + st.b as i16 as i64) as usize;
                    }
                }
                OP_CALL0..=OP_CALL8 => {
                    self.argc = (st.op - OP_CALL0) as usize;
                    let fnum = g[a] as usize;
                    if fnum == 0 {
                        return Err(self.run_error("NULL function"));
                    }
                    if fnum >= self.progs.functions.len() {
                        return Err(self.run_error(format!("bad function {fnum}")));
                    }
                    if matches!(self.dispatch[fnum], Dispatch::Qc) {
                        next = self.enter_function(fnum)?;
                    } else {
                        self.call_builtin(host, fnum)?;
                    }
                }
                // OP_STATE
                60 => {
                    let ed = self.g_i(self.sys.g_self);
                    let t = self.g_f(self.sys.g_time) + 0.1;
                    let (fr, th) = (self.globals[a], self.globals[b]);
                    let p = self.ent_ptr(ed, 0, 0)?;
                    let s = self.sys;
                    self.fields[p + s.f_nextthink] = t.to_bits();
                    self.fields[p + s.f_frame] = fr;
                    self.fields[p + s.f_think] = th;
                }
                OP_GOTO => next = (pc as i64 + st.a as i16 as i64) as usize,
                62 => setf!(c, b2f!(f!(a) != 0.0 && f!(b) != 0.0)),
                63 => setf!(c, b2f!(f!(a) != 0.0 || f!(b) != 0.0)),
                64 => setf!(c, (f!(a) as i32 & f!(b) as i32) as f32),
                65 => setf!(c, (f!(a) as i32 | f!(b) as i32) as f32),
                op => {
                    return Err(self.run_error(format!("Bad opcode {op}")));
                }
            }
            pc = next;
        }
    }
}
