//! qecs — fit-to-purpose ECS (see docs/DESIGN.md, "ECS").
//!
//! - [`Entity`] / [`EntityAllocator`]: generational handles + raw-index addressing for QC edicts.
//! - [`SparseSet`]: component storage keyed by entity index, with [`join2`] / [`join3`].
//! - [`Pool`]: typed free-list pool with dense iteration (beams, dlights, temp ents).
//! - [`ParticlePool`]: SoA particle storage with swap-remove and bulk update.
//!
//! No scheduler: systems are plain functions.

mod entity;
mod particles;
mod pool;
mod sparse;

pub use entity::{Entity, EntityAllocator};
pub use particles::{ParticleMut, ParticlePool, ParticleSpawn};
pub use pool::{Pool, PoolHandle};
pub use sparse::{SparseSet, join2, join3};
