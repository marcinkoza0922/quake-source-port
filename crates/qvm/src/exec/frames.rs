use super::*;

impl Vm {
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

    pub(super) fn fail(&mut self, e: VmError) -> VmError {
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

    pub(super) fn enter_function(&mut self, fnum: usize) -> Result<usize> {
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

    pub(super) fn leave_function(&mut self) -> Result<usize> {
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

    /// Name of the QC function currently running (the caller, inside a
    /// [`Host::before_damage`] hook).
    pub fn current_function_name(&self) -> String {
        self.progs.function_name(self.xfunction)
    }
}
