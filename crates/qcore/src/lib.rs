//! qcore — math, byte reader, filesystem, cvars and commands. See docs/DESIGN.md.
pub mod cmd;
pub mod cvar;
pub mod fs;
pub mod math;
pub mod reader;

pub use math::{Plane, Vec3};
pub use reader::{ReadError, Reader};
