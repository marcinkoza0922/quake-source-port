//! What the debris system needs to know about an alias model: which frames
//! are death frames (by name, so mod monsters work), which of them end a
//! death sequence, and whether it is a gib.
use qformats::mdl::{EF_GIB, Mdl};

pub struct ModelInfo {
    pub mdl: Mdl,
    death: Vec<bool>,
    last: Vec<bool>,
    pub gib: bool,
}

/// Frame-name fragments used for death animations across id1, the mission
/// packs and common mods ("death3", "deathb9", "axdeth5", "die2", "dead").
const DEATH_NAMES: [&str; 5] = ["death", "deth", "dth", "die", "dead"];

pub fn is_death_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    DEATH_NAMES.iter().any(|d| n.contains(d))
}

fn prefix(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
}

impl ModelInfo {
    pub fn load(fs: &qcore::fs::Fs, name: &str) -> Option<ModelInfo> {
        if !name.ends_with(".mdl") {
            return None;
        }
        let mdl = Mdl::parse(&fs.open(name)?).ok()?;
        Some(Self::from_mdl(mdl))
    }

    pub fn from_mdl(mdl: Mdl) -> ModelInfo {
        let names: Vec<String> =
            mdl.frames.iter().map(|f| f.poses.first().map_or(String::new(), |p| p.name.clone())).collect();
        let death: Vec<bool> = names.iter().map(|n| is_death_name(n)).collect();
        let last = (0..names.len())
            .map(|i| death[i] && names.get(i + 1).is_none_or(|next| prefix(next) != prefix(&names[i])))
            .collect();
        let gib = mdl.flags & EF_GIB != 0;
        ModelInfo { mdl, death, last, gib }
    }

    pub fn is_death(&self, frame: i32) -> bool {
        usize::try_from(frame).ok().and_then(|f| self.death.get(f)).copied().unwrap_or(false)
    }

    pub fn is_last(&self, frame: i32) -> bool {
        usize::try_from(frame).ok().and_then(|f| self.last.get(f)).copied().unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_names() {
        for n in ["death1", "deathb9", "axdeth5", "DIE2", "dead"] {
            assert!(is_death_name(n), "{n}");
        }
        for n in ["stand1", "run3", "pain4", "attack1"] {
            assert!(!is_death_name(n), "{n}");
        }
    }

    #[test]
    fn last_of_sequence() {
        let names = ["run1", "death1", "death2", "deathb1", "deathb2", "pain1"];
        let mut mdl = crate::debris::fragment::shard_model();
        let f0 = mdl.frames[0].clone();
        mdl.frames = names
            .iter()
            .map(|n| {
                let mut f = f0.clone();
                f.poses[0].name = n.to_string();
                f
            })
            .collect();
        let i = ModelInfo::from_mdl(mdl);
        let last: Vec<bool> = (0..6).map(|f| i.is_last(f)).collect();
        assert_eq!(last, [false, false, true, false, true, false]);
        assert!(i.is_death(1) && !i.is_death(0) && !i.is_death(5));
    }
}
