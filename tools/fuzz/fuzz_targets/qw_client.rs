//! Hostile QW server, .qwd or .mvd: flag byte, then u16-length-prefixed messages;
//! the translated NQ messages go through the NQ parser as in the host.
#![no_main]
use libfuzzer_sys::fuzz_target;
use qclient::ClientState;
use qclient::qw::QwState;

fuzz_target!(|data: &[u8]| {
    let Some((&flags, mut d)) = data.split_first() else { return };
    let mut qw = QwState::new();
    qw.demo_mode = flags & 1 != 0;
    qw.mvd = flags & 2 != 0;
    qw.pov_locked = flags & 4 != 0;
    let mut cl = ClientState::new();
    let mut seq = 1u32;
    while d.len() > 2 {
        let n = (u16::from_le_bytes([d[0], d[1]]) as usize).min(d.len() - 2);
        let Ok(p) = qw.parse(&d[2..2 + n], seq, seq, f64::from(seq) * 0.014) else { return };
        for m in &p.nq {
            if cl.parse_server_message(m).is_err() {
                return;
            }
        }
        let _ = cl.take_events();
        cl.frame(0.014);
        let _ = qw.build_cmd(Default::default(), 0, f64::from(seq) * 0.014);
        let _ = (qw.active_players(), qw.pov_angles(), qw.needed_files(), qw.encode_qwd_header(0.0, seq));
        seq += 1;
        d = &d[2 + n..];
    }
});
