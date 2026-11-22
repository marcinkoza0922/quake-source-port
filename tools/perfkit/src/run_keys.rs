//! What `perfkit run` plays: the scenes it selects, and the distinct runs
//! they need. Scenes on the same demo or the same live setup with the same
//! pins share one run, so a vanilla demo is the off run of every scene on it.

use crate::run_cmd::Opts;
use perfkit::live::Live;
use perfkit::plan::{self, PlanScene, Variant};
use perfkit::scenes::Scene;
use perfkit::settings::Profile;

/// The scenes a run covers, and the ones it skips with the reason.
pub fn select(o: &Opts) -> (Vec<&'static PlanScene>, Vec<(&'static PlanScene, String)>) {
    let wanted = |s: &PlanScene| {
        let named = o.only.iter().any(|n| n == s.name);
        if o.full { o.only.is_empty() || named } else { s.group == plan::Group::Vanilla || named }
    };
    let (mut run, mut skip) = (Vec::new(), Vec::new());
    for s in plan::SCENES.iter().filter(|s| wanted(s)) {
        match s.blocked {
            Some(why) => skip.push((s, why.to_string())),
            None => run.push(s),
        }
    }
    (run, skip)
}

/// What a run plays.
#[derive(Clone, Copy)]
pub enum Src {
    Demo(&'static Scene),
    /// A live scene; the name is the first plan scene that needed it.
    Live(&'static str, Live),
}

impl Src {
    /// The same game work: one demo, or one live setup (map and cameras).
    pub fn same(&self, o: &Src) -> bool {
        match (self, o) {
            (Src::Demo(a), Src::Demo(b)) => a.name == b.name,
            (Src::Live(_, a), Src::Live(_, b)) => a == b,
            _ => false,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Src::Demo(d) => d.name,
            Src::Live(n, _) => n,
        }
    }

    pub fn game(&self) -> &'static str {
        match self {
            Src::Demo(d) => d.game,
            Src::Live(_, l) => l.game,
        }
    }
}

/// One distinct engine run: a demo or live scene with one set of pins.
pub struct Key {
    pub src: Src,
    pub profile: Profile,
    pub pins: Vec<(&'static str, &'static str)>,
    /// A readable name: the first scene and variant that needed it.
    pub label: String,
}

/// The distinct runs for `scenes`, and for each (scene, variant) the index
/// of its run in that list.
pub fn keys(scenes: &[&'static PlanScene]) -> (Vec<Key>, Vec<(&'static PlanScene, Variant, usize)>) {
    let (mut keys, mut uses) = (Vec::<Key>::new(), Vec::new());
    for &s in scenes {
        let src = match s.kind {
            plan::Kind::Demo(_) => match s.demo() {
                Some(d) => Src::Demo(d),
                None => continue,
            },
            plan::Kind::Live(l) => Src::Live(s.name, l),
        };
        for v in Variant::of(s) {
            let pins = plan::pins(s, v);
            let i = match keys.iter().position(|k| k.src.same(&src) && k.profile == s.profile() && k.pins == pins) {
                Some(i) => i,
                None => {
                    let label = match (v, src) {
                        (Variant::Off, Src::Demo(d)) => format!("{}-off", d.name),
                        (Variant::Off, Src::Live(_, l)) => format!("{}-off", l.map),
                        _ => format!("{}-{}", s.name, v.name()),
                    };
                    keys.push(Key { src, profile: s.profile(), pins, label });
                    keys.len() - 1
                }
            };
            uses.push((s, v, i));
        }
    }
    (keys, uses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_cmd::parse_args;

    fn opts(a: &[&str]) -> Opts {
        parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn quick_is_vanilla_plus_only_and_shares_off_runs() {
        let o = opts(&["-only", "bloom,ssao,water-droplets"]);
        let (run, skip) = select(&o);
        let names: Vec<_> = run.iter().map(|s| s.name).collect();
        assert_eq!(names, ["combat-e1m3", "spin-e1m1", "spin-sepulcher", "spin-sepulcher-fte", "bloom", "water-droplets", "ssao"]);
        assert!(skip.is_empty());
        let (k, uses) = keys(&run);
        // 4 vanilla offs (bloom and ssao share spin-sepulcher's), bloom on, ssao on and top.
        // Plus water-droplets' own off and on (live runs aren't shared).
        assert_eq!(k.len(), 9, "{:?}", k.iter().map(|k| &k.label).collect::<Vec<_>>());
        let off_of = |name: &str| uses.iter().find(|u| u.0.name == name && u.1 == Variant::Off).map(|u| u.2);
        assert_eq!(off_of("bloom"), off_of("spin-sepulcher"));
        assert!(matches!(k[off_of("water-droplets").unwrap()].src, Src::Live("water-droplets", _)));
        // Live scenes on one setup share their off run.
        let (run, _) = select(&opts(&["-only", "water-planar,water-rt,water-rest"]));
        let (k, uses) = keys(&run);
        let off: Vec<_> = uses.iter().filter(|u| u.1 == Variant::Off && u.0.name.starts_with("water-")).map(|u| u.2).collect();
        assert!(off.len() == 3 && off.iter().all(|&i| i == off[0]) && k[off[0]].label == "e2m1-off", "{off:?}");
        assert_ne!(off_of("spin-sepulcher"), off_of("spin-sepulcher-fte"));
    }

    #[test]
    fn full_runs_every_ready_scene() {
        let (run, skip) = select(&opts(&["-full"]));
        assert_eq!(run.len() + skip.len(), plan::SCENES.len());
        assert!(run.iter().all(|s| s.runnable()));
    }
}
