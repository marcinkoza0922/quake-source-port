//! Part 1 visual parity stills (`docs/perf/VANILLA.md`, Visual parity).
//!
//! Each map gets one short demo, recorded by our engine: noclip, notarget,
//! then `setpos` to each view in turn, holding it [`HOLD`] seconds. A
//! `screenshot` stufftext is patched into the demo inside every hold
//! ([`crate::demofile`]), and each engine plays it with `playdemo`. So
//! every engine shoots (within a frame) the same moment of the same demo,
//! without console scripts the references don't share (vkQuake runs a
//! startup `map` only once the command buffer is empty, so `wait` chains
//! after it never see the map). Not `timedemo`: at ~650 fps vkQuake reached
//! the first view while its startup console was still sliding away.

use crate::engine::Engine;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub name: &'static str,
    pub game: &'static str,
    pub map: &'static str,
    /// `setpos` arguments: x y z pitch yaw roll.
    pub pos: [f32; 6],
    /// Recorded and played with FTE particles (ours and vkQuake only
    /// compared; ironwail is shown without them).
    pub fte: bool,
    /// What the view is there to catch.
    pub what: &'static str,
    /// Seconds on the map before the first view of its demo: [`LEAD`], or
    /// longer where the map holds the player in an intro (ad_tears moves
    /// the player and ignores `setpos` for its first ~10 s).
    pub lead: f32,
}

const fn v(name: &'static str, game: &'static str, map: &'static str, pos: [f32; 6], what: &'static str) -> View {
    View { name, game, map, pos, fte: false, what, lead: LEAD }
}

pub const VIEWS: &[View] = &[
    v("e1m1-start", "id1", "e1m1", [480.0, -352.0, 88.0, 0.0, 90.0, 0.0], "world, lightmaps, flame models"),
    v("e1m1-back", "id1", "e1m1", [480.0, -352.0, 88.0, 0.0, 270.0, 0.0], "world, lightmaps"),
    v("e1m1-grunts", "id1", "e1m1", [48.0, 1390.0, -200.0, 0.0, 90.0, 0.0], "md5 grunt and dog close-up"),
    v("e2m2-start", "id1", "e2m2", [-256.0, -1952.0, 280.0, 0.0, 90.0, 0.0], "world, lightmaps"),
    v("mg1-start", "mg1", "start", [0.0, -640.0, 24.0, 0.0, 90.0, 0.0], "fog, fence textures"),
    v("mg1-back", "mg1", "start", [0.0, -640.0, 24.0, 0.0, 270.0, 0.0], "fog"),
    // Compatibility audit (2026-11-17): each priority game at its start.
    v("hip1m1-start", "hipnotic", "hip1m1", [16.0, 304.0, -40.0, 0.0, 90.0, 0.0], "hipnotic world, status bar art"),
    v("r1m1-start", "rogue", "r1m1", [408.0, 3360.0, 232.0, 0.0, 90.0, 0.0], "rogue world"),
    v("mg3-start", "mg3", "start", [0.0, -2136.0, -224.0, 0.0, 90.0, 0.0], "mg3 hub, fog"),
    v("dopa-start", "dopa", "start", [32.0, -184.0, 160.0, 0.0, 0.0, 0.0], "dopa hub"),
    v("qbj3-start", "qbj3", "start", [0.0, -1648.0, -968.0, 0.0, 90.0, 0.0], "qbj3 hub, external textures, skybox"),
    v("alk-start", "alk1.2", "start", [832.0, 0.0, 536.0, 0.0, 180.0, 0.0], "Alkaline hub, csqc-free HUD"),
    v("sewerjam2-start", "sewerjam2", "start", [240.0, -336.0, 88.0, 0.0, 237.0, 0.0], "sewerjam2 hub, liquids"),
    v("q64-start", "q64", "start", [480.0, -352.0, 88.0, 0.0, 90.0, 0.0], "Quake64 RGB lighting, 16-bit lightmaps"),
    v("sepulcher-start", "ad", "ad_sepulcher", [-2560.0, 2464.0, -8.0, 0.0, 270.0, 0.0], "coloured light, fog, sky"),
    View { lead: 12.0, ..v("tears-waterfall", "ad", "ad_tears", [847.0, -256.0, -449.0, 0.0, 180.0, 0.0], "coloured light, water, waterfall") },
    View {
        name: "sepulcher-fte-start",
        game: "ad",
        map: "ad_sepulcher",
        pos: [-2560.0, 2464.0, -8.0, 0.0, 270.0, 0.0],
        fte: true,
        what: "AD sky rain (FTE), misc_gtkmodel decorations",
        lead: LEAD,
    },
];

/// Seconds each view is held while recording; the first view starts after
/// [`LEAD`] seconds on the map.
pub const HOLD: f32 = 2.0;
pub const LEAD: f32 = 2.0;
/// Echoed by the demo's last block: the run is over.
pub const DONE: &str = "PERFKIT_STILLS_DONE";
/// Demo blocks a view must be held for to count (72 Hz server: ~144 per hold).
pub const MIN_RUN: usize = 60;

/// One demo: the views of one map, with or without FTE particles.
#[derive(Clone, Debug)]
pub struct Group {
    pub game: &'static str,
    pub map: &'static str,
    pub fte: bool,
    pub views: Vec<&'static View>,
}

impl Group {
    /// Seconds on the map before the first view.
    pub fn lead(&self) -> f32 {
        self.views.iter().map(|v| v.lead).fold(LEAD, f32::max)
    }

    pub fn name(&self) -> String {
        format!("still-{}-{}{}", self.game, self.map, if self.fte { "-fte" } else { "" })
    }
}

/// Views grouped into demos, in catalogue order.
pub fn groups(only: &[String]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for v in VIEWS.iter().filter(|v| only.is_empty() || only.iter().any(|n| n == v.name)) {
        match out.iter_mut().find(|g| g.game == v.game && g.map == v.map && g.fte == v.fte) {
            Some(g) => g.views.push(v),
            None => out.push(Group { game: v.game, map: v.map, fte: v.fte, views: vec![v] }),
        }
    }
    out
}

/// Our engine's recording command line for a group (without the binary);
/// the script is [`record_cfg`], `exec`ed from `<game>/perfkit/still.cfg`
/// (on the command line a `+command` ends at the next token starting with
/// `-`, which cut `setpos` short at its first negative coordinate).
pub fn record_args(g: &Group, base: &Path) -> Vec<String> {
    let end = g.lead() + HOLD * g.views.len() as f32 + 0.5;
    let mut v: Vec<String> = vec!["-basedir".into(), base.display().to_string()];
    v.extend(["-headless", "-game", g.game, "-width", "320", "-height", "240", "-fixedstep", "250"].map(String::from));
    v.extend(["-autoquit".into(), format!("{}", end.ceil() as u32 + 10), "+exec".into(), "perfkit/still.cfg".into()]);
    v
}

/// The recording script. Each view is preceded by a quarter second at the
/// same spot turned 45°, so two views with equal angles are still separate
/// runs of demo blocks for [`crate::demofile::Demo::shot_blocks`].
/// `(separator, view)` angles of a view, as [`record_cfg`] holds them.
pub fn shot_angles(v: &View) -> ([f32; 3], [f32; 3]) {
    let [_, _, _, p, yaw, r] = v.pos;
    ([p, yaw + SEPARATOR_YAW, r], [p, yaw, r])
}

/// How far the separator before each view is turned.
const SEPARATOR_YAW: f32 = 45.0;

pub fn record_cfg(g: &Group) -> Vec<String> {
    let lead = g.lead();
    let end = lead + HOLD * g.views.len() as f32 + 0.5;
    let fte = if g.fte { "1" } else { "0" };
    let mut v = vec!["sv_autosave 0".to_string(), format!("sv_fteparticles {fte}"), format!("r_fteparticles {fte}")];
    v.push(format!("record perfkit_{} {}", g.name(), g.map));
    // Just before the first view: an intro may reset the movetype.
    for c in ["god", "notarget", "noclip"] {
        v.push(format!("delay {} {c}", lead - 0.5));
    }
    for (i, view) in g.views.iter().enumerate() {
        let t = lead + HOLD * i as f32;
        let [x, y, z, p, yaw, r] = view.pos;
        v.push(format!("delay {} setpos {x} {y} {z} {p} {} {r}", t - 0.25, yaw + SEPARATOR_YAW));
        v.push(format!("delay {t} setpos {x} {y} {z} {p} {yaw} {r}"));
    }
    v.push(format!("delay {end} stop"));
    v
}

/// Console lines that hide everything but the 3D view: no status bar, no
/// crosshair, no weapon, no notify lines. The references hide the HUD with
/// `viewsize 130`; ours ignores viewsize and hides each HUD element instead.
pub fn hud_off(e: Engine) -> Vec<String> {
    let common = ["crosshair 0", "r_drawviewmodel 0", "con_notifytime 0", "scr_showfps 0"];
    let own: &[&str] = match e {
        Engine::Ours => &[
            "scr_hudstyle 1", "hud_health 0", "hud_armor 0", "hud_ammo 0", "hud_ammolist 0", "hud_weapons 0", "hud_items 0",
            "hud_face 0", "hud_damageindicator 0",
        ],
        Engine::Ironwail | Engine::VkQuake => &["viewsize 130"],
    };
    common.iter().chain(own).map(|s| s.to_string()).collect()
}

/// Image files directly in `dirs` (not links into the user's data).
pub fn images(dirs: &[PathBuf]) -> Vec<(std::time::SystemTime, PathBuf)> {
    let mut v = Vec::new();
    for d in dirs {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            let is_img = p.extension().is_some_and(|x| ["png", "tga", "jpg"].contains(&x.to_string_lossy().to_ascii_lowercase().as_str()));
            if let (true, Ok(m)) = (is_img, std::fs::symlink_metadata(&p))
                && m.is_file()
            {
                v.push((m.modified().unwrap_or(std::time::UNIX_EPOCH), p));
            }
        }
    }
    v.sort();
    v
}

/// Where each engine writes `screenshot` images in the sandbox.
pub fn shot_dirs(e: Engine, sandbox: &Path, userdata: &Path, game: &str) -> Vec<PathBuf> {
    match e {
        Engine::Ours => vec![userdata.join("quake-source-port").join(game.to_ascii_lowercase())],
        Engine::Ironwail => vec![sandbox.join(game).join("screenshots"), sandbox.join(game)],
        Engine::VkQuake => vec![sandbox.join(game), sandbox.join(game).join("screenshots")],
    }
}

/// Structural dissimilarity (0 = identical) from ImageMagick 7.
pub fn dssim(a: &Path, b: &Path) -> Result<f64, String> {
    let out = Command::new("magick").arg("compare").args(["-metric", "DSSIM"]).arg(a).arg(b).arg("null:").output().map_err(|e| format!("magick: {e}"))?;
    let text = String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout);
    parse_metric(&text).ok_or_else(|| format!("magick compare: {}", text.trim()))
}

/// The normalised value in `"1626.31 (0.024816)"`.
fn parse_metric(s: &str) -> Option<f64> {
    let open = s.find('(')?;
    let close = open + s[open..].find(')')?;
    s[open + 1..close].trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_cover_every_view_once() {
        let all = groups(&[]);
        assert_eq!(all.iter().map(|g| g.views.len()).sum::<usize>(), VIEWS.len());
        for (i, a) in VIEWS.iter().enumerate() {
            assert!(VIEWS[i + 1..].iter().all(|b| b.name != a.name), "{}", a.name);
        }
        assert_eq!(groups(&["mg1-back".into()])[0].name(), "still-mg1-start");
    }

    #[test]
    fn record_script_holds_each_view() {
        let g = &groups(&["e1m1-start".into(), "e1m1-back".into()])[0];
        let a = record_args(g, Path::new("/sb")).join(" ");
        assert!(a.contains("-headless") && a.ends_with("+exec perfkit/still.cfg"), "{a}");
        let c = record_cfg(g);
        assert_eq!(c[0], "sv_autosave 0");
        assert!(c.contains(&"record perfkit_still-id1-e1m1 e1m1".to_string()), "{c:?}");
        assert!(c.contains(&"delay 1.5 noclip".to_string()), "{c:?}");
        let tears = &groups(&["tears-waterfall".into()])[0];
        assert_eq!(tears.lead(), 12.0);
        assert!(record_cfg(tears).contains(&"delay 12 setpos 847 -256 -449 0 180 0".to_string()));
        let tail = c[c.len() - 5..].join("|");
        assert_eq!(
            tail,
            "delay 1.75 setpos 480 -352 88 0 135 0|delay 2 setpos 480 -352 88 0 90 0|\
             delay 3.75 setpos 480 -352 88 0 315 0|delay 4 setpos 480 -352 88 0 270 0|delay 6.5 stop"
        );
    }

    #[test]
    fn parses_imagemagick_metric() {
        assert_eq!(parse_metric("1626.31 (0.024816)"), Some(0.024816));
        assert_eq!(parse_metric("0 (0)"), Some(0.0));
        assert_eq!(parse_metric("compare: unable to open"), None);
    }
}
