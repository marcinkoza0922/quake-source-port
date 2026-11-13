//! qvm — QuakeC virtual machine (see docs/DESIGN.md).
//!
//! Port of ironwail/vkQuake `pr_exec.c`, `pr_edict.c` and the VM-internal
//! parts of `pr_cmds.c`.
//!
//! - [`Progs`]: progs.dat v6 loader with name lookup.
//! - [`Vm`]: globals, edicts (dense `u32` slots, `entityfields` per edict),
//!   strings, interpreter ([`Vm::execute`]).
//! - [`Host`]: world/entity builtins ([`Builtin`], id1 numbering) and
//!   name-resolved extension builtins, implemented by the server.

mod builtins;
pub mod disasm;
mod edict;
mod exec;
mod progs;
mod spawn;
mod strings;

use std::fmt;

pub use builtins::{Builtin, EXTENSIONS};
pub use exec::{MAX_STACK_DEPTH, LOCALSTACK_SIZE, RUNAWAY_LIMIT};
pub use progs::{Def, EType, Function, PROGHEADER_CRC, PROG_VERSION, Progs, Statement, crc_block};
pub use spawn::{ParsedEdict, SpawnFilter, SpawnStats};
pub use builtins::{angle_vectors, atof, ftos};
pub use strings::{STRINGTEMP_BUFFERS, STRINGTEMP_LENGTH, StrKind, Strings};

/// Global offsets fixed by the QC calling convention.
pub const OFS_NULL: usize = 0;
pub const OFS_RETURN: usize = 1;
pub const OFS_PARM0: usize = 4;
pub const fn ofs_parm(i: usize) -> usize {
    OFS_PARM0 + i * 3
}

/// PR_RunError / Host_Error from QC: message plus QC stack trace (innermost first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub msg: String,
    pub trace: Vec<String>,
}

impl VmError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into(), trace: Vec::new() }
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.msg)?;
        for l in &self.trace {
            write!(f, "\n{l}")?;
        }
        Ok(())
    }
}

impl std::error::Error for VmError {}

pub type Result<T> = std::result::Result<T, VmError>;

/// Engine side of the VM: builtins that touch the world, clients or network.
pub trait Host {
    /// An id1-numbered builtin not handled inside the VM. Args are in
    /// `vm.g_*(ofs_parm(i))`, `vm.argc` holds the argument count; write the
    /// result with `vm.ret_*`.
    fn builtin(&mut self, vm: &mut Vm, b: Builtin) -> Result<()>;
    /// Extension builtin resolved by name (`#0` or a known extension number).
    fn ext_builtin(&mut self, vm: &mut Vm, name: &str) -> Result<()> {
        Err(vm.run_error(format!("unimplemented builtin {name}")))
    }
    /// Particle effect index for an entity's `traileffect` / `emiteffect`
    /// spawn key (vkQuake ED_ParseEdict; DP_ENT_TRAILEFFECTNUM). `None`
    /// leaves the key unhandled.
    fn particle_effect_index(&mut self, _name: &str) -> Option<f32> {
        None
    }
    /// Extra extension names `checkextension` should report as supported.
    fn has_extension(&self, _name: &str) -> bool {
        false
    }
    /// `dprint` (developer-only console print).
    fn dprint(&mut self, _msg: &str) {}
    /// Console print (warnings, "No spawn function for", ...).
    fn print(&mut self, _msg: &str) {}
    /// Called just before QC calls `T_Damage(targ, inflictor, attacker,
    /// damage)`; the args are in the parm globals and may be changed.
    /// Returning `true` skips the call.
    fn before_damage(&mut self, _vm: &mut Vm) -> Result<bool> {
        Ok(false)
    }
}

pub use exec::Vm;
