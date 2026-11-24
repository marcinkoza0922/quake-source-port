//! Add-on guns in the players' hands (always shown on a local server; the
//! rest of visible weapons is `cl_vwep`). The rifle goes where the stock gun
//! was, the akimbo pistols into two arms reposed to point them forwards
//! ([`super::arms`]). QC still animates its own weapon, so the bodies are
//! made with every frame that shows the wrong weapon replaced by the one
//! that fits: the rifle's has no axe frames, the pistols' no two-handed ones.
//! The guns are full models moved rigidly with the hands.

use qcore::fs::Fs;
use qcore::math::Vec3;
use qformats::Mdl;
use qserver::addons::Toy;

use super::PLAYER;
use super::arms::{Arms, Posed};
use super::rig::{Rig, rigid};
use super::{classic, enhanced};
use crate::addons::cable_fx::angles_basis;
use crate::assets::md3;
use crate::debris::math::{Mat3, apply, entity_rotation, mul3};
use crate::gfx::Renderer;
use qrender::{ModelId, RenderEntity};

/// Sizes of the guns against their view models.
const PISTOL: f32 = 0.52;
const RIFLE: f32 = 0.6;
/// The rifle's middle from the stock gun's.
const RIFLE_AT: Vec3 = Vec3::new(1.5, 0.3, 0.2);
/// Where the pistols' hands go from the middle of the shoulders: forwards,
/// out, up; and from there with the kick of a shot.
const REACH: Vec3 = Vec3::new(13.0, 5.6, -3.4);
const KICK: Vec3 = Vec3::new(-1.6, 0.0, 1.7);
const KICK_PITCH: f32 = 16.0;

/// What a player slot does with an add-on gun.
#[derive(Clone, Copy, Debug)]
pub struct ToyState {
    pub toy: Toy,
    /// Seconds since the last shot, and which shot or pistol it was.
    pub age: f32,
    pub shot: u32,
}

/// A gun's model-to-player transform in a frame.
type At = (Mat3, Vec3);

struct Gun {
    model: ModelId,
    at: Vec<At>,
    /// The muzzle (in the scaled model) and which of the toy's guns it is.
    muzzle: Option<(usize, Vec3)>,
}

/// A player model and what it holds.
pub struct ToyBody {
    body: ModelId,
    guns: Vec<Gun>,
    names: Vec<String>,
    /// Frames shown while shooting.
    fire: [Option<u32>; 2],
}

/// The frame shown instead of frame `name`.
fn stand_in(toy: Toy, name: &str) -> String {
    let split = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let n: u32 = name[split.len()..].parse().unwrap_or(1);
    match (toy, split) {
        (Toy::Akimbo, "rockrun") => format!("axrun{n}"),
        (Toy::Akimbo, "stand") => format!("axstnd{n}"),
        (Toy::Akimbo, "pain") => format!("axpain{n}"),
        (Toy::Akimbo, "nailatt" | "light" | "rockatt" | "shotatt" | "axatt" | "axattb" | "axattc" | "axattd") => "axstnd1".into(),
        (Toy::Rifle, "axrun") => format!("rockrun{n}"),
        (Toy::Rifle, "axstnd") => format!("stand{}", (n - 1) % 5 + 1),
        (Toy::Rifle, "axpain") => format!("pain{n}"),
        (Toy::Rifle, "axdeth") => format!("deathe{n}"),
        (Toy::Rifle, "axatt" | "axattb" | "axattc" | "axattd") => "stand1".into(),
        _ => name.into(),
    }
}

/// For each frame the index of the one shown instead.
fn sources(toy: Toy, names: &[String]) -> Vec<usize> {
    let find = |n: &str| names.iter().position(|m| m.eq_ignore_ascii_case(n));
    names.iter().enumerate().map(|(i, n)| find(&stand_in(toy, &n.to_ascii_lowercase())).unwrap_or(i)).collect()
}

fn dead(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("death") || n.starts_with("axdeth")
}

fn pitched(deg: f32) -> Mat3 {
    entity_rotation([deg, 0.0, 0.0])
}

/// A model at `scale` of its size.
fn scaled(mut m: Mdl, scale: f32) -> Mdl {
    m.scale = m.scale.map(|s| s * scale);
    m.scale_origin = m.scale_origin.map(|s| s * scale);
    m.bounding_radius *= scale;
    m
}

/// The pistols' body frames with the arms reposed, and each hand's grip per
/// frame. `pos(f)` is frame `f` of the model as it is.
fn akimbo(arms: &Arms, names: &[String], pos: impl Fn(usize) -> Vec<Vec3>) -> Option<(Vec<Posed>, [Vec<At>; 2])> {
    let src = sources(Toy::Akimbo, names);
    let reference = names.iter().position(|n| n.eq_ignore_ascii_case("axstnd1"))?;
    let side = |v: Vec3, s: usize| Vec3::new(v.x, if s == 0 { v.y } else { -v.y }, v.z);
    let aim = |f: usize, kick: Option<usize>| {
        arms.reach(&pos(f), |s, mid| mid + side(REACH, s) + if kick == Some(s) { KICK } else { Vec3::ZERO })
    };
    let base = aim(reference, None);
    let mut grips: [Vec<At>; 2] = Default::default();
    let frames = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            if dead(name) {
                // the arms fall as they do, the pistols in the hands
                let p = pos(src[i]);
                for (s, g) in grips.iter_mut().enumerate() {
                    let fore: Vec<usize> = arms.forearm(s).collect();
                    let pick = |v: &[Vec3]| fore.iter().map(|&k| v[k]).collect::<Vec<_>>();
                    let (r, t) = rigid(&pick(&base.pos), &pick(&p));
                    g.push((r, apply(&r, base.hands[s]) + t));
                }
                let n = p.len();
                return Posed { pos: p, rot: vec![pitched(0.0); n], hands: [Vec3::ZERO; 2] };
            }
            let kick = match name.to_ascii_lowercase().as_str() {
                "nailatt1" => Some(0),
                "nailatt2" => Some(1),
                _ => None,
            };
            let p = aim(src[i], kick);
            for (s, g) in grips.iter_mut().enumerate() {
                g.push((pitched(if kick == Some(s) { KICK_PITCH } else { 0.0 }), p.hands[s]));
            }
            p
        })
        .collect();
    Some((frames, grips))
}

/// Uploads a gun and places it: model point `grip` goes to each `at`.
fn gun(mdl: Mdl, scale: f32, grip: Vec3, at: &[At], muzzle: Option<(usize, Vec3)>, r: &mut Renderer) -> Option<Gun> {
    let model = r.load_alias(&scaled(mdl, scale)).ok()?;
    let at = at.iter().map(|(m, t)| (*m, *t - apply(m, grip * scale))).collect();
    Some(Gun { model, at, muzzle: muzzle.map(|(k, p)| (k, p * scale)) })
}

impl ToyBody {
    pub fn load(toy: Toy, is_md3: bool, fs: &Fs, r: &mut Renderer) -> Option<ToyBody> {
        let player = Mdl::parse(&fs.open(PLAYER)?).ok()?;
        let names: Vec<String> = player.frames.iter().map(|f| f.poses.first().map_or(String::new(), |p| p.name.clone())).collect();
        let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
        let src = sources(toy, &names);
        let (body, rig, grips) = if is_md3 {
            let md5 = md3::parse_md5(fs, PLAYER)?;
            let (rig, mut body, _) = enhanced::rig(&md5, &player)?;
            let grips = if toy == Toy::Akimbo {
                let arms = enhanced::arms(&md5)?;
                let (frames, grips) = akimbo(&arms, &names, |f| enhanced::positions(&md5.md3, f))?;
                enhanced::set_frames(&mut body, &src, Some(&frames));
                Some(grips)
            } else {
                enhanced::set_frames(&mut body, &src, None);
                None
            };
            enhanced::drop_flash(&md5, &mut body);
            let skins = md3::md5_skins(fs, r, PLAYER, &md5, false);
            (r.load_md3(&body, &skins).ok()?, rig, grips)
        } else {
            let (rig, mut body, _) = classic::rig(&player)?;
            let grips = if toy == Toy::Akimbo {
                let arms = classic::arms(&player)?;
                let (frames, grips) = akimbo(&arms, &names, |f| classic::positions(&player, f))?;
                classic::set_frames(&mut body, &src, Some(&frames));
                Some(grips)
            } else {
                classic::set_frames(&mut body, &src, None);
                None
            };
            classic::drop_flash(&mut body, &player);
            (r.load_alias(&body).ok()?, rig, grips)
        };
        let guns = match grips {
            Some(grips) => Self::pistols(&grips, &palette, r)?,
            None => Self::rifle(&rig, &src, &palette, r)?,
        };
        let frame = |n: &str| names.iter().position(|m| m.eq_ignore_ascii_case(n)).map(|f| f as u32);
        Some(ToyBody { body, guns, fire: [frame("nailatt1"), frame("nailatt2")], names })
    }

    fn pistols(grips: &[Vec<At>; 2], palette: &[u8], r: &mut Renderer) -> Option<Vec<Gun>> {
        use crate::addons::akimbo_model::{GRIP, MUZZLE, WELL, pistol};
        let hand = WELL + GRIP * 3.4;
        (0..2).map(|s| gun(pistol(palette, s == 0), PISTOL, hand, &grips[s], Some((s, MUZZLE)), r)).collect()
    }

    fn rifle(rig: &Rig, src: &[usize], palette: &[u8], r: &mut Renderer) -> Option<Vec<Gun>> {
        use crate::addons::rifle_model::{MUZZLE, meshes};
        let at: Vec<At> = src
            .iter()
            .map(|&f| {
                let (m, t) = rig.gun(f);
                (m, apply(&m, rig.grip() + RIFLE_AT) + t)
            })
            .collect();
        // the body's bounds are ±26 along the barrel; hold it a little forward
        // of the magazine
        let grip = Vec3::new(1.0, 0.0, 2.6);
        let muzzle = [Some((0, MUZZLE)), None, None];
        meshes(palette).into_iter().zip(muzzle).map(|(m, z)| gun(m, RIFLE, grip, &at, z, r)).collect()
    }

    /// Turns the body `out` ends with into this one and adds the guns;
    /// returns the muzzles (which gun, world point).
    pub fn attach(&self, state: ToyState, out: &mut Vec<RenderEntity>) -> Vec<(usize, Vec3)> {
        let Some(body) = out.last_mut() else { return Vec::new() };
        body.model = self.body;
        let name = self.names.get(body.frame as usize).map_or("", String::as_str).to_ascii_lowercase();
        let still = ["stand", "axstnd"].iter().any(|s| name.starts_with(s));
        let window = if state.toy == Toy::Rifle { 0.25 } else { 0.1 };
        // the rifle shoulders to fire, as QC's nailguns do; the pistols kick
        // while the player stands
        if state.age < window && !dead(&name) && (still || state.toy == Toy::Rifle)
            && let Some(f) = self.fire[state.shot as usize % 2]
        {
            (body.frame, body.prev_frame, body.lerp) = (f, f, 1.0);
        }
        let body = *body;
        let axes = entity_rotation(body.angles);
        let origin = Vec3::from(body.origin);
        let mut muzzles = Vec::new();
        for (k, g) in self.guns.iter().enumerate() {
            let at = |f: u32| g.at.get(f as usize).or(g.at.first());
            let (Some((m1, t1)), Some((m0, t0))) = (at(body.frame), at(body.prev_frame)) else { continue };
            let l = body.lerp;
            let t = *t0 * (1.0 - l) + *t1 * l;
            let m: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| m0[r][c] * (1.0 - l) + m1[r][c] * l));
            let w = mul3(&axes, &m);
            let col = |c: usize| Vec3::new(w[0][c], w[1][c], w[2][c]).normalize();
            let at = origin + apply(&axes, t);
            if let Some((part, p)) = g.muzzle {
                muzzles.push((part, at + apply(&w, p)));
            }
            out.push(RenderEntity {
                model: g.model,
                origin: at.to_array(),
                angles: angles_basis(col(0), col(2)),
                frame: 0,
                prev_frame: 0,
                lerp: 1.0,
                skin: 0,
                colormap: None,
                key: qrender::attached_key(body.key, k as u32),
                ..body
            });
        }
        muzzles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> Option<(Fs, Mdl, Vec<String>)> {
        let base = std::env::var_os("QUAKE_BASEDIR").unwrap_or_else(|| "vkQuake directory with maps".into());
        let mut fs = Fs::new(base);
        if !fs.add_game_dir("id1") {
            return None;
        }
        let m = Mdl::parse(&fs.open(PLAYER)?).ok()?;
        let names = m.frames.iter().map(|f| f.poses[0].name.clone()).collect();
        Some((fs, m, names))
    }

    #[test]
    fn frames_of_the_wrong_weapon_are_replaced() {
        assert_eq!(stand_in(Toy::Akimbo, "rockrun4"), "axrun4");
        assert_eq!(stand_in(Toy::Akimbo, "shotatt3"), "axstnd1");
        assert_eq!(stand_in(Toy::Akimbo, "deathb2"), "deathb2");
        assert_eq!(stand_in(Toy::Rifle, "axstnd7"), "stand2");
        assert_eq!(stand_in(Toy::Rifle, "axdeth9"), "deathe9");
        assert_eq!(stand_in(Toy::Rifle, "rockrun1"), "rockrun1");
        let Some((_, _, names)) = player() else { return };
        for toy in [Toy::Akimbo, Toy::Rifle] {
            for (i, &s) in sources(toy, &names).iter().enumerate() {
                assert_eq!(names[s].to_ascii_lowercase(), stand_in(toy, &names[i].to_ascii_lowercase()), "{}", names[i]);
            }
        }
    }

    #[test]
    fn both_player_models_point_two_pistols_forwards() {
        let Some((fs, m, names)) = player() else { return };
        let check = |arms: &Arms, pos: &dyn Fn(usize) -> Vec<Vec3>| {
            let (frames, grips) = akimbo(arms, &names, pos).unwrap();
            for (i, n) in names.iter().enumerate() {
                let hands = arms.hands(&frames[i].pos);
                for s in 0..2 {
                    // the pistol is in the hand in every frame, deaths included
                    assert!((hands[s] - grips[s][i].1).length() < 1.5, "{n} {s}: {:?} {:?}", hands[s], grips[s][i].1);
                }
                if !dead(n) {
                    // in front (a hit throws the whole body back), side by side
                    let front = n.contains("pain") || hands.iter().all(|h| h.x > 2.0);
                    assert!(hands[0].y > hands[1].y + 5.0 && front, "{n}: {hands:?}");
                }
            }
        };
        check(&classic::arms(&m).unwrap(), &|f| classic::positions(&m, f));
        if let Some(md5) = md3::parse_md5(&fs, PLAYER) {
            check(&enhanced::arms(&md5).unwrap(), &|f| enhanced::positions(&md5.md3, f));
        }
    }
}
