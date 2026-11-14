//! qpool — small fixed-purpose containers (see docs/DESIGN.md, "Entity and effect storage").
//!
//! - [`Entity`] / [`EntityAllocator`]: edict slot allocation with generation-checked
//!   handles and Quake's `ED_Alloc` reuse policy supplied by the caller.
//! - [`Pool`]: capacity-capped dense list with swap-remove (beams, dlights).
//! - [`ParticlePool`]: SoA classic-particle storage with swap-remove and bulk integrate.

mod entity;
mod particles;
mod pool;

pub use entity::{Entity, EntityAllocator};
pub use particles::{ParticlePool, ParticleSpawn};
pub use pool::Pool;
