//! Renderer debugging commands: `r_visibility` (hide/isolate faces and
//! entities), `r_testlight` (fixed dynamic lights) and `r_partinfo`.

use super::*;

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("r_visibility", |h, a| {
        let message = h.r.debug_visibility(&a.all()[1..]);
        h.print(&message);
        if a.argv(1) == "isolate"
            && let Some(entity) = h.cl.entities.get(h.cl.viewentity)
        {
            let mut eye = entity.origin;
            eye[2] += h.cl.viewheight;
            h.print(&format!("Client eye {eye:?}, angles {:?}\n", h.cl.viewangles));
        }
    });
    // debug: fixed dynamic lights (real-time dynamic light shadows, screenshots)
    c.add("r_testlight", |h, a| {
        let f = |i: usize, d: f32| a.argv(i).parse::<f32>().unwrap_or(d);
        match a.argc() {
            2 if a.argv(1) == "clear" => h.test_lights.clear(),
            5 | 8 => h.test_lights.push(qrender::DLight {
                origin: [f(1, 0.0), f(2, 0.0), f(3, 0.0)],
                radius: f(4, 200.0),
                minlight: 0.0,
                color: [f(5, 1.0), f(6, 1.0), f(7, 1.0)],
                key: 0,
            }),
            _ => h.print(&format!("r_testlight x y z radius [r g b] | clear ({} lights)\n", h.test_lights.len())),
        }
    });
    c.add("r_partinfo", |h, _| {
        let s = h.fte.info(&h.cl);
        h.con.print(&s);
    });
}
