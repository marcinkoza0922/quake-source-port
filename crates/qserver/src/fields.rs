//! Field and global offsets of the loaded progs (progdefs.h by name).

use qvm::{Vm, VmError};
use qworld::{E, F, V};

macro_rules! offsets {
    ($name:ident, $lookup:ident, req { $($r:ident = $rn:literal),* $(,)? } opt { $($o:ident = $on:literal),* $(,)? }) => {
        /// Offsets resolved by name; required ones error at load when missing.
        #[derive(Debug, Clone, Default)]
        pub struct $name {
            $(pub $r: usize,)*
            $(pub $o: Option<usize>,)*
        }
        impl $name {
            pub fn new(vm: &Vm) -> Result<Self, VmError> {
                let need = |n: &str| vm.$lookup(n).ok_or_else(|| VmError::new(format!("progs.dat: missing {n}")));
                Ok(Self { $($r: need($rn)?,)* $($o: vm.$lookup($on),)* })
            }
        }
    };
}

offsets!(Fields, field_ofs,
    req {
        modelindex = "modelindex", absmin = "absmin", absmax = "absmax", ltime = "ltime",
        movetype = "movetype", solid = "solid", origin = "origin", oldorigin = "oldorigin",
        velocity = "velocity", angles = "angles", avelocity = "avelocity", punchangle = "punchangle",
        classname = "classname", model = "model", frame = "frame", skin = "skin", effects = "effects",
        mins = "mins", maxs = "maxs", size = "size", touch = "touch", think = "think",
        blocked = "blocked", nextthink = "nextthink", groundentity = "groundentity",
        health = "health", frags = "frags", weapon = "weapon", weaponmodel = "weaponmodel",
        weaponframe = "weaponframe", currentammo = "currentammo", ammo_shells = "ammo_shells",
        ammo_nails = "ammo_nails", ammo_rockets = "ammo_rockets", ammo_cells = "ammo_cells",
        items = "items", takedamage = "takedamage", chain = "chain", view_ofs = "view_ofs",
        button0 = "button0", button2 = "button2", impulse = "impulse",
        fixangle = "fixangle", v_angle = "v_angle", idealpitch = "idealpitch", netname = "netname",
        enemy = "enemy", flags = "flags", colormap = "colormap", team = "team",
        teleport_time = "teleport_time", armortype = "armortype", armorvalue = "armorvalue",
        waterlevel = "waterlevel", watertype = "watertype", ideal_yaw = "ideal_yaw",
        yaw_speed = "yaw_speed", goalentity = "goalentity", dmg_take = "dmg_take",
        dmg_save = "dmg_save", dmg_inflictor = "dmg_inflictor", owner = "owner", movedir = "movedir",
    }
    opt { gravity = "gravity", items2 = "items2", alpha = "alpha", scale = "scale" }
);

offsets!(Globals, global_ofs,
    req {
        self_ = "self", other = "other", world = "world", time = "time", frametime = "frametime",
        force_retouch = "force_retouch", mapname = "mapname", deathmatch = "deathmatch",
        coop = "coop", teamplay = "teamplay", serverflags = "serverflags",
        total_secrets = "total_secrets", total_monsters = "total_monsters",
        found_secrets = "found_secrets", killed_monsters = "killed_monsters", parm1 = "parm1",
        v_forward = "v_forward", v_up = "v_up", v_right = "v_right",
        trace_allsolid = "trace_allsolid", trace_startsolid = "trace_startsolid",
        trace_fraction = "trace_fraction", trace_endpos = "trace_endpos",
        trace_plane_normal = "trace_plane_normal", trace_plane_dist = "trace_plane_dist",
        trace_ent = "trace_ent", trace_inopen = "trace_inopen", trace_inwater = "trace_inwater",
        msg_entity = "msg_entity",
        f_main = "main", f_start_frame = "StartFrame", f_player_pre_think = "PlayerPreThink",
        f_player_post_think = "PlayerPostThink", f_client_kill = "ClientKill",
        f_client_connect = "ClientConnect", f_put_client_in_server = "PutClientInServer",
        f_client_disconnect = "ClientDisconnect", f_set_new_parms = "SetNewParms",
        f_set_change_parms = "SetChangeParms",
    }
    opt {}
);

impl Fields {
    pub fn v(&self, f: V) -> usize {
        match f {
            V::Origin => self.origin,
            V::OldOrigin => self.oldorigin,
            V::Velocity => self.velocity,
            V::Angles => self.angles,
            V::AVelocity => self.avelocity,
            V::PunchAngle => self.punchangle,
            V::VAngle => self.v_angle,
            V::Mins => self.mins,
            V::Maxs => self.maxs,
            V::Size => self.size,
            V::AbsMin => self.absmin,
            V::AbsMax => self.absmax,
            V::ViewOfs => self.view_ofs,
            V::MoveDir => self.movedir,
        }
    }
    pub fn f(&self, f: F) -> Option<usize> {
        Some(match f {
            F::ModelIndex => self.modelindex,
            F::Ltime => self.ltime,
            F::MoveType => self.movetype,
            F::Solid => self.solid,
            F::Flags => self.flags,
            F::NextThink => self.nextthink,
            F::Health => self.health,
            F::WaterLevel => self.waterlevel,
            F::WaterType => self.watertype,
            F::IdealYaw => self.ideal_yaw,
            F::YawSpeed => self.yaw_speed,
            F::TeleportTime => self.teleport_time,
            F::FixAngle => self.fixangle,
            F::Gravity => return self.gravity,
            F::IdealPitch => self.idealpitch,
        })
    }
    pub fn e(&self, f: E) -> usize {
        match f {
            E::Owner => self.owner,
            E::GroundEntity => self.groundentity,
            E::Enemy => self.enemy,
            E::GoalEntity => self.goalentity,
        }
    }
}
