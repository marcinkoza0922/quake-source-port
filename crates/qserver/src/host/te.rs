//! DP_TE_STANDARDEFFECTBUILTINS (#418 to #431): the classic temp entities as
//! builtins (vkQuake PF_sv_te_*). AD's mammoth boss calls `te_explosion2`
//! where its scripted particles are on.

use super::*;

impl Core {
    /// True when `name` is one of these builtins.
    pub(super) fn pf_te_effect(&mut self, vm: &mut Vm, name: &str) -> Result<bool> {
        use qnet::protocol::{
            TE_BEAM, TE_EXPLOSION, TE_EXPLOSION2, TE_GUNSHOT, TE_KNIGHTSPIKE, TE_LAVASPLASH, TE_LIGHTNING1, TE_LIGHTNING2,
            TE_LIGHTNING3, TE_SPIKE, TE_SUPERSPIKE, TE_TAREXPLOSION, TE_TELEPORT, TE_WIZSPIKE,
        };
        let ty = match name {
            "te_gunshot" => TE_GUNSHOT,
            "te_spike" => TE_SPIKE,
            "te_superspike" => TE_SUPERSPIKE,
            "te_explosion" => TE_EXPLOSION,
            "te_tarexplosion" => TE_TAREXPLOSION,
            "te_wizspike" => TE_WIZSPIKE,
            "te_knightspike" => TE_KNIGHTSPIKE,
            "te_lavasplash" => TE_LAVASPLASH,
            "te_teleport" => TE_TELEPORT,
            "te_explosion2" => TE_EXPLOSION2,
            "te_lightning1" => TE_LIGHTNING1,
            "te_lightning2" => TE_LIGHTNING2,
            "te_lightning3" => TE_LIGHTNING3,
            "te_beam" => TE_BEAM,
            _ => return Ok(false),
        };
        let pf = self.sv.protocolflags;
        let beam = matches!(ty, TE_LIGHTNING1 | TE_LIGHTNING2 | TE_LIGHTNING3 | TE_BEAM);
        let owner = if beam { ent_arg(vm, 0)? } else { 0 };
        let m = &mut self.sv.datagram;
        m.byte(i32::from(qnet::protocol::SVC_TEMP_ENTITY));
        m.byte(i32::from(ty));
        if beam {
            m.short(owner as i32);
        }
        for i in if beam { 1..3 } else { 0..1 } {
            for v in vm.g_v(p(i)) {
                m.coord(v, pf);
            }
        }
        if ty == TE_EXPLOSION2 {
            m.byte(vm.g_f(p(1)) as i32);
            m.byte(vm.g_f(p(2)) as i32);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use qcore::cvar::Cvars;
    use qcore::fs::Fs;
    use qvm::ofs_parm;

    #[test]
    fn standard_effects_write_their_temp_entity() {
        let mut fs = Fs::new(Fs::default_basedir());
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps");
        if !fs.basedir().join("id1").is_dir() {
            fs = Fs::new(base);
        }
        if !fs.add_game_dir("id1") || !fs.exists("maps/e1m1.bsp") {
            return;
        }
        let mut sv = crate::Server::new(fs, Cvars::new(), 1);
        sv.spawn_server("e1m1").unwrap();
        let vm = &mut sv.level.as_mut().unwrap().vm;
        let c = &mut sv.core;
        c.sv.protocolflags = 0; // classic 16-bit coordinates
        vm.set_g_v(ofs_parm(0), [8.0, 16.0, 24.0]);
        vm.set_g_f(ofs_parm(1), 224.0);
        vm.set_g_f(ofs_parm(2), 8.0);
        c.sv.datagram.data.clear();
        assert!(c.pf_te_effect(vm, "te_explosion2").unwrap());
        // svc_temp_entity, TE_EXPLOSION2, 3 coords (x8, 16 bit), colour, length
        assert_eq!(c.sv.datagram.data, [23, 12, 64, 0, 128, 0, 192, 0, 224, 8]);
        c.sv.datagram.data.clear();
        vm.set_g_i(ofs_parm(0), 1);
        vm.set_g_v(ofs_parm(1), [8.0, 0.0, 0.0]);
        vm.set_g_v(ofs_parm(2), [0.0, 8.0, 0.0]);
        assert!(c.pf_te_effect(vm, "te_beam").unwrap());
        assert_eq!(c.sv.datagram.data, [23, 13, 1, 0, 64, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0]);
        assert!(!c.pf_te_effect(vm, "te_nonsense").unwrap());
    }
}
