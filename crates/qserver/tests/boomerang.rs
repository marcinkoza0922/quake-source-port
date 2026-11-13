//! Boomerang add-on against the real id1 e1m1 (skips without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::addons::{BUTTON_TOY_FIRE, TOY_SHIFT, Toy};
use qserver::{ClientMove, Server};
use qworld::{F, V};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
        });
    d.join("id1").is_dir().then_some(d)
}

fn start() -> Option<(Server, usize)> {
    let mut fs = Fs::new(&basedir()?);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.command("addons_enable 1");
    sv.core.set_addons(true);
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    run(&mut sv, c, 30, ClientMove::default());
    sv.command("notarget");
    // the dog and grunt room: both in view to the west
    sv.command("setpos 250 1400 -190");
    run(&mut sv, c, 5, ClientMove::default());
    Some((sv, c))
}

fn run(sv: &mut Server, c: usize, frames: usize, mv: ClientMove) {
    for _ in 0..frames {
        sv.set_move(c, mv);
        sv.frame(1.0 / 72.0);
        let _ = sv.take_reliable(c);
        let _ = sv.take_datagrams(c);
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
}

fn vec(sv: &Server, e: u32, f: V) -> [f32; 3] {
    sv.level
        .as_ref()
        .unwrap()
        .vm
        .ent_v(e, sv.core.sv.fields.v(f))
}
fn float(sv: &Server, e: u32, f: F) -> f32 {
    sv.level
        .as_ref()
        .unwrap()
        .vm
        .ent_f(e, sv.core.sv.fields.f(f).unwrap())
}

/// Live monsters within `r` of the player, nearest first.
fn monsters(sv: &Server, r: f32) -> Vec<u32> {
    let p = vec(sv, 1, V::Origin);
    let l = sv.level.as_ref().unwrap();
    let mut out: Vec<(f32, u32)> = (2..l.vm.num_edicts() as u32)
        .filter(|&e| {
            !l.vm.is_free(e)
                && float(sv, e, F::Flags) as i32 & 32 != 0
                && float(sv, e, F::Health) > 0.0
        })
        .map(|e| {
            let o = vec(sv, e, V::Origin);
            (((o[0] - p[0]).powi(2) + (o[1] - p[1]).powi(2)).sqrt(), e)
        })
        .filter(|&(d, _)| d < r)
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, e)| e).collect()
}

/// View angles from the player's eye to monster `m`'s centre.
fn aim(sv: &Server, m: u32) -> [f32; 3] {
    let p = vec(sv, 1, V::Origin);
    let eye = [p[0], p[1], p[2] + 22.0];
    let (lo, hi) = (vec(sv, m, V::AbsMin), vec(sv, m, V::AbsMax));
    let d: Vec<f32> = (0..3).map(|i| (lo[i] + hi[i]) * 0.5 - eye[i]).collect();
    let yaw = d[1].atan2(d[0]).to_degrees();
    let pitch = -(d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())).to_degrees();
    [pitch, yaw, 0.0]
}

fn toy(angles: [f32; 3], fire: bool) -> ClientMove {
    let b = Toy::Boomerang.id() << TOY_SHIFT | if fire { BUTTON_TOY_FIRE } else { 0 };
    ClientMove {
        angles,
        buttons: b,
        ..Default::default()
    }
}

#[test]
fn marks_in_order_stuns_each_and_returns() {
    let Some((mut sv, c)) = start() else { return };
    let ms = monsters(&sv, 400.0);
    assert!(ms.len() >= 2, "{ms:?}");
    let (a, b) = (ms[1], ms[0]);
    {
        let mv = toy(aim(&sv, a), true);
        run(&mut sv, c, 3, mv);
    }
    {
        let mv = toy(aim(&sv, b), true);
        run(&mut sv, c, 3, mv);
    }
    assert_eq!(sv.level.as_ref().unwrap().boomerangs.marks(0), &[a, b]);
    {
        let mv = toy(aim(&sv, b), false);
        run(&mut sv, c, 1, mv);
    }
    assert!(sv.level.as_ref().unwrap().boomerangs.flight(0).is_some());
    // can't throw again while it flies
    let mut stunned = Vec::new();
    for _ in 0..72 * 8 {
        {
            let mv = toy(aim(&sv, b), false);
            run(&mut sv, c, 1, mv);
        }
        let l = sv.level.as_ref().unwrap();
        for &(m, _) in &l.boomerangs.stuns {
            if !stunned.contains(&m) {
                stunned.push(m);
                // a stunned monster's think waits for the stun to end
                assert!(
                    float(&sv, m, F::NextThink) as f64 >= l.vm.time + 1.0
                        || float(&sv, m, F::NextThink) == 0.0
                );
            }
        }
        if l.boomerangs.flight(0).is_none() {
            break;
        }
    }
    assert!(
        stunned.contains(&a) && stunned.contains(&b),
        "stunned {stunned:?}"
    );
    assert!(
        sv.level.as_ref().unwrap().boomerangs.flight(0).is_none(),
        "never came back"
    );
    // stuns wear off (default 4 s)
    run(&mut sv, c, 72 * 5, ClientMove::default());
    assert!(sv.level.as_ref().unwrap().boomerangs.stuns.is_empty());
}

#[test]
fn straight_throw_and_vanished_target_still_come_back() {
    let Some((mut sv, c)) = start() else { return };
    // no marks: out and back
    run(&mut sv, c, 2, toy([0.0, 0.0, 0.0], true));
    run(&mut sv, c, 1, toy([0.0, 0.0, 0.0], false));
    assert!(sv.level.as_ref().unwrap().boomerangs.flight(0).is_some());
    run(&mut sv, c, 72 * 10, toy([0.0, 0.0, 0.0], false));
    assert!(sv.level.as_ref().unwrap().boomerangs.flight(0).is_none());
    // a marked target removed mid-flight is skipped
    let m = monsters(&sv, 400.0)[0];
    {
        let mv = toy(aim(&sv, m), true);
        run(&mut sv, c, 3, mv);
    }
    {
        let mv = toy(aim(&sv, m), false);
        run(&mut sv, c, 1, mv);
    }
    {
        let h = sv.core.sv.fields.f(F::Health).unwrap();
        sv.level.as_mut().unwrap().vm.set_ent_f(m, h, 0.0);
    }
    run(&mut sv, c, 72 * 10, ClientMove::default());
    let l = sv.level.as_ref().unwrap();
    assert!(l.boomerangs.flight(0).is_none());
    assert!(!l.boomerangs.stuns.iter().any(|s| s.0 == m));
}
