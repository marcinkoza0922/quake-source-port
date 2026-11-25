//! Hostile NQ/FTE server or demo: u16-length-prefixed messages into the client.
#![no_main]
use libfuzzer_sys::fuzz_target;
use qclient::{ClientState, InputState, MoveSpeeds};

fuzz_target!(|data: &[u8]| {
    let mut cl = ClientState::new();
    let mut d = data;
    while d.len() > 2 {
        let n = (u16::from_le_bytes([d[0], d[1]]) as usize).min(d.len() - 2);
        let ok = cl.parse_server_message(&d[2..2 + n]).is_ok();
        d = &d[2 + n..];
        let _ = cl.take_events();
        if !ok {
            return;
        }
        cl.frame(1.0 / 72.0);
        let cmd = cl.base_move(&InputState::default(), &MoveSpeeds::default(), 0.014);
        let _ = cl.build_move(&cmd);
        let _ = cl.active_entities().count();
        for i in 0..64 {
            let _ = cl.lightstyle_value(i);
        }
    }
});
