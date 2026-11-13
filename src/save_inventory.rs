//! Save-card inventory adapters. Masks come from the mounted mod's progs.dat;
//! art comes from that same filesystem, including inherited base-game assets.
use std::collections::HashMap;
use qcore::fs::Fs;
use crate::{saveinfo::SaveInfo, sbar::Pack};

#[derive(Default)]
pub struct Inventory {
    constants: HashMap<String, u32>,
    pub pack: Pack,
    ad: bool,
    keep: bool,
    qbj: bool,
    native_hip_icons: bool,
}

pub struct Icon {
    pub name: String,
    pub fallback: String,
    /// A single frame in QBJ3's counted-key strip.
    pub frame: Option<u32>,
}
impl Icon {
    fn new(name: impl Into<String>, fallback: impl Into<String>) -> Self {
        Self { name: name.into(), fallback: fallback.into(), frame: None }
    }
}

impl Inventory {
    pub fn new(fs: &Fs, pack: Pack) -> Self {
        let mut out = Self { pack, native_hip_icons: fs.open("gfx/weapons/ui_h_weapon_mjolnir_1.lmp").is_some(), ..Default::default() };
        if let Some(progs) = fs.open("progs.dat").and_then(|bytes| qvm::Progs::parse(&bytes).ok()) {
            let fields: Vec<_> = progs.fielddefs.iter().map(|d| progs.string(d.name)).collect();
            out.ad = fields.contains(&b"moditems".as_slice()) && fields.contains(&b"ckeyname1".as_slice());
            out.keep = fields.contains(&b"weaponismoditems".as_slice());
            out.qbj = fields.contains(&b"items_qbj".as_slice());
            for d in &progs.globaldefs {
                let name = String::from_utf8_lossy(progs.string(d.name));
                if d.etype() == qvm::EType::Float && (name.starts_with("IT_") || name.starts_with("IT2_"))
                    && let Some(value) = progs.globals.get(d.ofs as usize) {
                    out.constants.insert(name.into_owned(), f32::from_bits(*value) as u32);
                }
            }
        }
        out
    }
    fn mask(&self, name: &str, fallback: u32) -> u32 { self.constants.get(name).copied().unwrap_or(fallback) }
    fn owns(&self, s: &SaveInfo, field: &str, constant: &str, fallback: u32) -> bool {
        s.bits(field) & self.mask(constant, fallback) != 0
    }

    pub fn weapons(&self, s: &SaveInfo) -> Vec<Icon> {
        let mut out = Vec::new();
        let keep = self.keep;
        for (i, (constant, name)) in [
            ("IT_SHOTGUN", "shotgun"), ("IT_SUPER_SHOTGUN", "sshotgun"),
            ("IT_NAILGUN", "nailgun"), ("IT_SUPER_NAILGUN", "snailgun"),
            ("IT_GRENADE_LAUNCHER", "rlaunch"), ("IT_ROCKET_LAUNCHER", "srlaunch"), ("IT_LIGHTNING", "lightng"),
        ].iter().enumerate() {
            if !self.owns(s, "items", constant, 1 << i) { continue; }
            let upgrade = if i == 1 { Some(("IT_UPGRADE_SSG", 2, "widowm")) }
                else if i == 6 { Some(("IT_UPGRADE_LG", 64, if keep { "plasmaad" } else { "plasma" })) } else { None };
            let upgraded = upgrade.filter(|(c, mask, _)| self.ad && self.owns(s, "moditems", c, *mask));
            let name = upgraded.map_or(*name, |(_,_,n)| n);
            let icon = if self.ad && !keep {
                let name = match name { "rlaunch" => "grenade", "srlaunch" => "rocket", n => n };
                format!("gfx/adw_{name}.lmp")
            } else { format!("inv_{name}") };
            out.push(Icon::new(icon, name));
        }
        let axe_bit = if self.pack == Pack::Rogue || keep { 1 << 11 } else { 1 << 12 };
        if self.owns(s, "items", "IT_AXE", axe_bit) {
            if self.ad && !keep {
                let upgraded = self.owns(s, "moditems", "IT_UPGRADE_AXE", 4096);
                out.push(Icon::new(format!("gfx/adib_axe{}.lmp", if upgraded { 2 } else { 1 }), "Axe"));
            } else if keep {
                let name = if self.owns(s, "perms", "IT_CHAINSAW", 1 << 21) { "chainsaw" }
                    else if self.owns(s,"moditems","IT_UPGRADE_AXE",2048) { "shadax" } else { "axe" };
                out.push(Icon::new(format!("inv_{name}"), name));
            } else if self.qbj { out.push(Icon::new("inv_wrench", "Wrench")); }
            else { out.push(Icon::new("inv_axe", "Axe")); }
        }
        if keep {
            for (field, constant, bit, icon) in [
                ("items","IT_LAVA_NAILGUN",12,"lavang"), ("items","IT_LAVA_SUPER_NAILGUN",13,"lavasng"),
                ("items","IT_MULTI_GRENADE",14,"multigren"), ("items","IT_MULTI_ROCKET",15,"multirock"),
                ("items","IT_PLASMA_GUN",16,"plasmarog"), ("items2","IT2_PROXIMITY_GUN",17,"prox"),
                ("items2","IT2_MJOLNIR",18,"mjolnir"), ("items2","IT2_LASER_CANNON",19,"laser"),
                ("items2","IT2_CHAINLIGHTNING",7,"chainlg"), ("items2","IT2_GRAPPLE",20,"grapple"),
                ("items2","IT2_CROSS",22,"wand"), ("items2","IT2_WAND",23,"wand"),
                ("moditems","IT_ICE",2,"ranstaff"), ("moditems","IT_MACE",3,"mace"),
                ("moditems","IT_VOID_STAFF",4,"snailgun"), ("moditems","IT_FLAK",5,"flak"),
                ("moditems","IT_GUNGNIR",7,"gungnir"), ("moditems","IT_SANGUINATORS",8,"sanguin"),
                ("moditems","IT_SCIMITAR",9,"sshotgun"), ("moditems","IT_RAPIER",10,"nailgun"),
                ("moditems","IT_MIRV",12,"multigren"), ("perms","IT_AIRGUN",10,"airfist"),
                ("perms","IT_FROSTBOW",12,"frostbow"), ("perms","IT_SIDEWINDER",15,"crossbow"),
            ] {
                if self.owns(s,field,constant,1 << bit) { out.push(Icon::new(format!("inv_{icon}"), icon)); }
            }
        } else {
            if self.pack == Pack::Rogue {
                for (i, name) in ["r_lava","r_superlava","r_gren","r_multirock","r_plasma"].iter().enumerate() {
                    if s.items & (1 << (12 + i)) != 0 { out.push(Icon::new(*name, *name)); }
                }
            }
            for (constant, bit, name) in [("IT_MJOLNIR",7,"mjolnir"),("IT_LASER_CANNON",23,"laser"),("IT_PROXIMITY_GUN",16,"prox")] {
                // A named QC constant also covers MG3 without treating it as Hipnotic.
                if (self.constants.contains_key(constant) || self.pack == Pack::Hipnotic) && self.owns(s,"items",constant,1 << bit) {
                    let icon = if self.native_hip_icons && name != "prox" { format!("gfx/weapons/ui_h_weapon_{name}_1.lmp") } else { format!("inv_{name}") };
                    out.push(Icon::new(icon, name));
                }
            }
        }
        // Dwell ships no separate HUD pictures for these additional weapons.
        for (constant, bit, label) in [("IT_QUAD_SHOTGUN",2,"Quad shotgun"),("IT_RAILGUN",3,"Railstaff"),("IT_RIFLE",5,"Rifle")] {
            if self.constants.contains_key(constant) && self.owns(s,"items_dwell",constant,1 << bit) {
                out.push(Icon::new(format!("inv_{}",label.to_ascii_lowercase().replace(' ',"")),label));
            }
        }
        out
    }

    pub fn keys(&self, s: &SaveInfo) -> Vec<Icon> {
        let mut out = Vec::new();
        for i in 0..2 {
            if s.items & (1 << (17 + i)) == 0 { continue; }
            let mut icon = Icon::new(format!("sb_key{}", i+1), format!("Key {}",i+1));
            if self.qbj {
                let worldtype = s.bits("worldtype");
                let theme = match (worldtype >> 8) & 255 { 1 => "r", 2 => "b", _ => "m" };
                icon.name = format!("sb_{theme}key{}",i+1);
                icon.frame = Some((worldtype >> (i*4)) & 15);
            }
            out.push(icon);
        }
        if self.ad {
            for i in 0..4 {
                if !self.owns(s,"moditems",&format!("IT_CKEY{}",i+1),1 << (13+i)) { continue; }
                let skin = s.bits(&format!("ckeyskin{}",i+1));
                let name = s.player.get(&format!("ckeyname{}",i+1)).cloned().unwrap_or_else(|| format!("Custom key {}",i+1));
                out.push(Icon::new(format!("gfx/adib_ckey{}.lmp", if skin < 14 { skin + 1 } else { 1 }), name));
            }
        }
        if self.keep && self.owns(s,"items2","IT2_KEY_SKELETON",1 << 16) {
            out.push(Icon::new("sb_key_skeleton", "Skeleton key"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn preview(items: u32, fields: &[(&str, &str)]) -> SaveInfo {
        SaveInfo { level: "Test".into(), timestamp: "Test".into(), health: 100, armor: 0,
            items, kills: (0, 10), secrets: (0, 1), reason: None, addons: None,
            player: fields.iter().map(|(k,v)| (k.to_string(),v.to_string())).chain([(String::from("items"), items.to_string())]).collect() }
    }
    #[test]
    fn custom_keys_need_ownership_and_preserve_skin() {
        let inv = Inventory { ad: true, ..Default::default() };
        let fields = [("moditems","8192"),("ckeyskin1","7"),("ckeyname1","Emerald key")];
        let keys = inv.keys(&preview(0, &fields));
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "gfx/adib_ckey8.lmp");
        assert_eq!(keys[0].fallback, "Emerald key");
        assert!(inv.keys(&preview(0, &fields[1..])).is_empty());
    }
    #[test]
    fn counted_key_uses_saved_theme_and_count() {
        let inv = Inventory { qbj: true, ..Default::default() };
        let keys = inv.keys(&preview(1<<17, &[("worldtype","514")]));
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "sb_bkey1");
        assert_eq!(keys[0].frame, Some(2)); // third key, zero-based atlas frame
        assert_eq!(inv.weapons(&preview(4096, &[]))[0].name, "inv_wrench");
    }
    #[test]
    fn keep_uses_distinct_inventory_banks() {
        let inv = Inventory { ad: true, keep: true, ..Default::default() };
        let icons = inv.weapons(&preview(2048, &[("items2","262144"),("moditems","2048")]));
        let names: Vec<_> = icons.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"inv_shadax"));
        assert!(names.contains(&"inv_mjolnir"));
        assert!(!names.contains(&"inv_lavang")); // id1 axe bit would be lava nailgun here
    }
    #[test]
    fn installed_mod_constants_and_native_icons() {
        let Some(base) = std::env::var_os("QUAKE_BASEDIR") else { return };
        let base = std::path::PathBuf::from(base);
        for (game, fields, items, expected) in [
            ("mg1", &[][..], 1, "inv_shotgun"),
            ("mg3", &[][..], 128, "gfx/weapons/ui_h_weapon_mjolnir_1.lmp"),
            ("copper", &[][..], 2, "inv_sshotgun"),
            ("ad", &[("moditems","2")][..], 2, "gfx/adw_widowm.lmp"),
            ("mjolnir", &[][..], 2048, "inv_axe"),
            ("qbj3", &[][..], 4096, "inv_wrench"),
        ] {
            if !base.join(game).is_dir() { continue; }
            let mut fs = Fs::new(&base);
            fs.add_game_dir("id1"); fs.add_game_dir(game);
            let inventory = Inventory::new(&fs, Pack::Id);
            let icons = inventory.weapons(&preview(items, fields));
            assert!(icons.iter().any(|icon| icon.name == expected), "{game}: expected {expected}");
            if expected.contains('/') { assert!(fs.open(expected).is_some(), "missing native art: {game}: {expected}"); }
            else {
                let wad = qformats::Wad::parse(&fs.open("gfx.wad").unwrap()).unwrap();
                assert!(wad.get(expected).is_some(), "missing native art: {game}: {expected}");
            }
        }
    }
}
