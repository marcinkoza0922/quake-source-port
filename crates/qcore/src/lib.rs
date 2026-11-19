//! qcore — math, byte reader, filesystem, cvars and commands. See docs/DESIGN.md.
pub mod anorms;
pub mod cmd;
pub mod cvar;
pub mod fs;
pub mod lang;
pub mod math;
pub mod procstat;
pub mod reader;
pub mod vertex;
pub mod text;
pub mod time;
pub mod vfs;

pub use math::{Plane, Vec3};
pub use reader::{ReadError, Reader};
