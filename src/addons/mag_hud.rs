//! Magazine readout of the held add-on gun: rounds loaded per gun, with a
//! bar while reloading, above the status bar in the middle of the screen.
use crate::draw::Canvas;
use qserver::addons::Toy;

/// Half the gap between two guns' readouts.
const GAP: f32 = 34.0;

/// Draws player `owner`'s magazines for the toy they hold; `vh` is the
/// height left above a full-width status bar.
pub fn draw(c: &mut Canvas, vh: f32, cl: &qclient::ClientState, owner: usize, held: Option<Toy>) {
    if cl.intermission != 0 || cl.stats[qnet::protocol::STAT_HEALTH] <= 0 {
        return;
    }
    let Some(m) = held.and_then(|t| cl.mags.iter().find(|m| m.owner as usize == owner && m.toy == t.id())) else { return };
    let n = m.parts.len();
    for (i, p) in m.parts.iter().enumerate() {
        let x = c.vw() / 2.0 + (i as f32 - (n as f32 - 1.0) / 2.0) * GAP * 2.0;
        let y = vh - 44.0;
        let low = p.rounds * 4 <= m.cap;
        let color = if p.rounds == 0 { [1.0, 0.25, 0.2, 1.0] } else if low { [1.0, 0.8, 0.3, 1.0] } else { [0.95, 0.95, 0.95, 1.0] };
        let text = format!("{} / {}", p.rounds, m.cap);
        // a plate behind, as guns reach this far up
        let h = if p.reload_end.is_some() { 18.0 } else { 13.0 };
        c.round_box(x - 24.0, y - 2.0, 48.0, h, 3.0, [0.0, 0.0, 0.0, 0.55], None);
        c.text_raw(x - c.text_w_raw(&text, 8.0) / 2.0, y, &text, 8.0, color);
        if let Some(left) = p.reload_end.map(|t| t - cl.time).filter(|&l| l > 0.0) {
            let k = (left as f32 / p.reload_secs.max(0.1)).clamp(0.0, 1.0);
            c.fill(x - 20.0, y + 10.0, 40.0, 3.0, [0.0, 0.0, 0.0, 0.6]);
            c.fill(x - 20.0, y + 10.0, 40.0 * (1.0 - k), 3.0, [0.9, 0.8, 0.5, 0.9]);
        }
    }
}
