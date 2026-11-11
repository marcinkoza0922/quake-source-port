//! Entity access interface implemented by the server (QC edicts).

use crate::{Vec3, World};

/// Edict number; 0 is the world.
pub type Ent = u32;

/// Vector fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V {
    Origin,
    OldOrigin,
    Velocity,
    Angles,
    AVelocity,
    PunchAngle,
    VAngle,
    Mins,
    Maxs,
    Size,
    AbsMin,
    AbsMax,
    ViewOfs,
    MoveDir,
}

/// Float fields (flags/solid/movetype are floats in QC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum F {
    ModelIndex,
    Ltime,
    MoveType,
    Solid,
    Flags,
    NextThink,
    Health,
    WaterLevel,
    WaterType,
    IdealYaw,
    YawSpeed,
    TeleportTime,
    FixAngle,
    /// Optional "gravity" field; return 0 when absent.
    Gravity,
    IdealPitch,
}

/// Entity reference fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum E {
    Owner,
    GroundEntity,
    Enemy,
    GoalEntity,
}

/// Server-side entity storage + QC callbacks. Callbacks get the `World` so
/// QC builtins (traceline, setorigin, ...) can use it re-entrantly.
pub trait Edicts {
    fn num_edicts(&self) -> u32;
    fn is_free(&self, e: Ent) -> bool;
    fn v(&self, e: Ent, f: V) -> Vec3;
    fn set_v(&mut self, e: Ent, f: V, v: Vec3);
    fn f(&self, e: Ent, f: F) -> f32;
    fn set_f(&mut self, e: Ent, f: F, v: f32);
    fn e(&self, e: Ent, f: E) -> Ent;
    fn set_e(&mut self, e: Ent, f: E, v: Ent);
    /// `ent.touch` is a non-null function.
    fn has_touch(&self, e: Ent) -> bool;
    /// Run `ent.touch` with self=ent, other=other, time=current time.
    fn touch(&mut self, w: &mut World, ent: Ent, other: Ent);
    /// Run `pusher.blocked` (if set) with self=pusher, other=other.
    fn blocked(&mut self, w: &mut World, pusher: Ent, other: Ent);
    /// Run `ent.think` with self=ent, other=world, time=`time`.
    fn think(&mut self, w: &mut World, ent: Ent, time: f64);
    /// Leafs touched by a linked entity (SV_FindTouchedLeafs), for PVS checks.
    fn set_leafs(&mut self, _e: Ent, _leafs: &[u32]) {}
    fn start_sound(&mut self, _e: Ent, _sample: &str) {}
    /// C `rand()`.
    fn rand(&mut self) -> i32 {
        0
    }

    fn fi(&self, e: Ent, f: F) -> i32 {
        self.f(e, f) as i32
    }
    fn flags(&self, e: Ent) -> i32 {
        self.fi(e, F::Flags)
    }
    fn set_flags(&mut self, e: Ent, fl: i32) {
        self.set_f(e, F::Flags, fl as f32)
    }
}
