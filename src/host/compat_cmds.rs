//! Console commands other engines (ironwail, vkQuake) have that mods use:
//! QC `stuffcmd`s them (AD, Alkaline, sewerjam2, qbj3, Mjolnir, the jams)
//! or their configs set them.

use super::*;

pub(super) fn register(c: &mut Commands<Host>) {
    super::quaddicted_cli::register(c);
    // Sky_SkyCommand_f: QC stuffs `sky <name>` to change the skybox (AD,
    // Alkaline, sewerjam2); lasts until the next map loads its own.
    c.add("sky", |h, a| match a.argc() {
        1 => h.print(&format!("\"sky\" is \"{}\"\n", h.level.sky)),
        2 => {
            let name = a.argv(1).to_string();
            let fs = &h.server.core.fs;
            h.level.set_sky(fs, &mut h.r, &name);
            if h.level.sky.is_empty() && !name.is_empty() {
                h.print(&format!("Couldn't load gfx/env/{name}\n"));
            }
        }
        _ => h.print("usage: sky <skyname>\n"),
    });
    // ironwail's HUD style cvar is `hudstyle` (0 classic, 1/2 modern
    // variants, 3 QuakeWorld); qbj3's QC stuffs it from an impulse.
    c.add("hudstyle", |h, a| {
        if a.argc() < 2 {
            let v = h.cv("scr_hudstyle");
            h.print(&format!("\"hudstyle\" is \"{}\" (scr_hudstyle)\n", v as i32));
            return;
        }
        let v = qcore::cvar::q_atof(a.argv(1)) as i32;
        h.exec_line(&format!("scr_hudstyle {}", i32::from(v == 1 || v == 2)));
    });
    // S_Play / S_PlayVol / S_StopAllSoundsC: QC stuffs these too
    // (func_mapjam3, Mjolnir). A missing extension means ".wav".
    c.add("play", |h, a| {
        for i in 1..a.argc() {
            h.play_console_sound(a.argv(i), 1.0);
        }
    });
    c.add("playvol", |h, a| {
        for i in (1..a.argc()).step_by(2) {
            h.play_console_sound(a.argv(i), qcore::cvar::q_atof(a.argv(i + 1)));
        }
    });
    c.add("stopsound", |h, _| {
        if let Some(s) = &h.snd {
            s.event(qsound::SoundEvent::StopAll);
        }
    });
    c.add("v_cshift", |h, a| {
        let n = |i| qcore::cvar::q_atof(a.argv(i)) as i32;
        h.view.set_empty_shift(n(1), n(2), n(3), n(4));
    });
}
