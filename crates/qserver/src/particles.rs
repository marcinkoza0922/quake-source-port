//! FTE/DP particle builtins (vkQuake pr_ext.c): particleeffectnum (#335),
//! trailparticles (#336), pointparticles (#337), te_particlerain (#409),
//! te_particlesnow (#410), and their svcdp_* messages.
use qvm::{Result, Vm, ofs_parm};

use crate::msg::SizeBuf;
use crate::server::Core;

/// svc_temp_entity.
const SVC_TEMP_ENTITY: i32 = 23;
pub const SVCDP_PRECACHE: i32 = 54;
pub const SVCDP_TRAILPARTICLES: i32 = 60;
pub const SVCDP_POINTPARTICLES: i32 = 61;
pub const SVCDP_POINTPARTICLES1: i32 = 62;
pub const TEDP_PARTICLERAIN: i32 = 55;
pub const TEDP_PARTICLESNOW: i32 = 56;
pub const MAX_PARTICLETYPES: usize = 2048;

/// Extensions advertised while [`Core::fte_particles`] is on.
pub const EXTENSIONS: &[&str] = &[
    "DP_SV_POINTPARTICLES",
    "FTE_SV_POINTPARTICLES",
    "DP_TE_PARTICLERAIN",
    "DP_TE_PARTICLESNOW",
    "FTE_PART_SCRIPT",
    "FTE_PART_NAMESPACES",
    "FTE_PART_NAMESPACE_EFFECTINFO",
    // .traileffectnum / .emiteffectnum, sent with replacement deltas
    "DP_ENT_TRAILEFFECTNUM",
];

/// DP's built-in effect names, precached first (COM_Effectinfo_Enumerate).
const DP_NAMES: &[&str] = &[
    "TE_GUNSHOT", "TE_GUNSHOTQUAD", "TE_SPIKE", "TE_SPIKEQUAD", "TE_SUPERSPIKE", "TE_SUPERSPIKEQUAD",
    "TE_WIZSPIKE", "TE_KNIGHTSPIKE", "TE_EXPLOSION", "TE_EXPLOSIONQUAD", "TE_TAREXPLOSION", "TE_TELEPORT",
    "TE_LAVASPLASH", "TE_SMALLFLASH", "TE_FLAMEJET", "EF_FLAME", "TE_BLOOD", "TE_SPARK", "TE_PLASMABURN",
    "TE_TEI_G3", "TE_TEI_SMOKE", "TE_TEI_BIGEXPLOSION", "TE_TEI_PLASMAHIT", "EF_STARDUST", "TR_ROCKET",
    "TR_GRENADE", "TR_BLOOD", "TR_WIZSPIKE", "TR_SLIGHTBLOOD", "TR_KNIGHTSPIKE", "TR_VORESPIKE",
    "TR_NEHAHRASMOKE", "TR_NEXUIZPLASMA", "TR_GLOWTRAIL", "SVC_PARTICLE",
];

pub fn write_precache(m: &mut SizeBuf, index: usize, name: &str) {
    m.byte(SVCDP_PRECACHE);
    m.short(0x4000 | index as i32);
    m.string(name.as_bytes());
}

fn p(i: usize) -> usize {
    ofs_parm(i)
}

impl Core {
    /// Index of `name`, adding it if new (PF_SV_ForceParticlePrecache).
    /// After loading, new names are also sent to clients. 0 when full.
    pub(crate) fn particle_precache(&mut self, name: &str) -> usize {
        if let Some(i) = self.sv.particle_precache.iter().skip(1).position(|n| n == name) {
            return i + 1;
        }
        if self.sv.particle_precache.len() >= MAX_PARTICLETYPES {
            return 0;
        }
        self.sv.particle_precache.push(name.to_string());
        let i = self.sv.particle_precache.len() - 1;
        if !self.sv.loading {
            write_precache(&mut self.sv.reliable_datagram, i, name);
        }
        i
    }

    /// COM_Effectinfo_Enumerate: DP names, then every `effect` in effectinfo.txt.
    fn precache_effectinfo(&mut self) {
        let Some(data) = self.fs.open("effectinfo.txt") else { return };
        for n in DP_NAMES {
            self.particle_precache(n);
        }
        let text = String::from_utf8_lossy(&data).into_owned();
        for line in text.lines() {
            let Some((tok, rest)) = qcore::cmd::com_parse(line) else { continue };
            if tok == "effect"
                && let Some((name, _)) = qcore::cmd::com_parse(rest)
            {
                self.particle_precache(name);
            }
        }
    }

    pub(crate) fn pf_particleeffectnum(&mut self, vm: &mut Vm) -> Result<()> {
        let s = String::from_utf8_lossy(vm.g_str(p(0))?).into_owned();
        vm.ret_f(0.0);
        if s.is_empty() || !self.fte_particles() {
            return Ok(());
        }
        if self.sv.particle_precache.len() <= 1 && s.starts_with("effectinfo.") {
            self.precache_effectinfo();
        }
        let i = self.particle_precache(&s);
        if i == 0 {
            return Err(vm.run_error("PF_sv_particleeffectnum: overflow"));
        }
        vm.ret_f(i as f32);
        Ok(())
    }

    pub(crate) fn pf_trailparticles(&mut self, vm: &mut Vm) -> Result<()> {
        // DP swapped the first two args; accept both orders.
        let (ent, ef) = if vm.g_i(p(1)) as u32 as usize >= vm.max_edicts() {
            (vm.g_i(p(0)), vm.g_f(p(1)) as i32)
        } else {
            (vm.g_i(p(1)), vm.g_f(p(0)) as i32)
        };
        if ef <= 0 || !self.fte_particles() {
            return Ok(());
        }
        let (start, end) = (vm.g_v(p(2)), vm.g_v(p(3)));
        let pf = self.sv.protocolflags;
        let m = &mut self.sv.datagram;
        m.byte(SVCDP_TRAILPARTICLES);
        m.short(ent);
        m.short(ef);
        for v in start.iter().chain(end.iter()) {
            m.coord(*v, pf);
        }
        Ok(())
    }

    pub(crate) fn pf_pointparticles(&mut self, vm: &mut Vm) -> Result<()> {
        let ef = vm.g_f(p(0)) as i32;
        let org = vm.g_v(p(1));
        let vel = if vm.argc < 3 { [0.0; 3] } else { vm.g_v(p(2)) };
        let count = if vm.argc < 4 { 1 } else { (vm.g_f(p(3)) as i32).min(65535) };
        if ef <= 0 || count < 1 || !self.fte_particles() {
            return Ok(());
        }
        let pf = self.sv.protocolflags;
        let m = &mut self.sv.datagram;
        if count == 1 && vel == [0.0; 3] {
            m.byte(SVCDP_POINTPARTICLES1);
            m.short(ef);
            for v in org {
                m.coord(v, pf);
            }
        } else {
            m.byte(SVCDP_POINTPARTICLES);
            m.short(ef);
            for v in org.iter().chain(vel.iter()) {
                m.coord(*v, pf);
            }
            m.short(count);
        }
        Ok(())
    }

    /// te_particlerain / te_particlesnow.
    pub(crate) fn pf_te_weather(&mut self, vm: &mut Vm, snow: bool) -> Result<()> {
        let (min, max, vel) = (vm.g_v(p(0)), vm.g_v(p(1)), vm.g_v(p(2)));
        let count = vm.g_f(p(3));
        let colour = vm.g_f(p(4)) as i32;
        if count < 1.0 || !self.fte_particles() {
            return Ok(());
        }
        let pf = self.sv.protocolflags;
        let m = &mut self.sv.datagram;
        m.byte(SVC_TEMP_ENTITY);
        m.byte(if snow { TEDP_PARTICLESNOW } else { TEDP_PARTICLERAIN });
        for v in min.iter().chain(max.iter()).chain(vel.iter()) {
            m.coord(*v, pf);
        }
        m.short(count.min(65535.0) as i32);
        m.byte(colour);
        Ok(())
    }
}
