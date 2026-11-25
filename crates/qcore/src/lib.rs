//! qcore — math, byte reader, filesystem, cvars and commands. See docs/DESIGN.md.
pub mod anorms;
pub mod cmd;
pub mod cvar;
pub mod fs;
pub mod lang;
pub mod math;
pub mod procstat;
pub mod reader;
pub mod safe_path;
pub mod sha256;
pub mod vertex;
pub mod text;
pub mod time;
pub mod vfs;

pub use math::{Plane, Vec3};
pub use reader::{ReadError, Reader};

/// `eprintln!` that doesn't mind when nobody reads stderr any more.
#[macro_export]
macro_rules! warnln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}
