//! Hostile remote client: ops (moves, string commands, acks, frames) into a
//! live server. Needs game data (QUAKE_BASEDIR); QFUZZ_MAP picks the map.
#![no_main]
use std::cell::RefCell;

use libfuzzer_sys::fuzz_target;
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::{ClientMove, Server};

fn server() -> Server {
    let base = std::env::var("QUAKE_BASEDIR").expect("QUAKE_BASEDIR");
    let mut fs = Fs::new(std::path::Path::new(&base));
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 2);
    sv.command("skill 3");
    sv.spawn_server(&std::env::var("QFUZZ_MAP").unwrap_or("start".into())).expect("map");
    sv
}

thread_local!(static SV: RefCell<Option<(Server, u32)>> = const { RefCell::new(None) });

fuzz_target!(|data: &[u8]| {
    SV.with_borrow_mut(|slot| {
        // a fresh level now and then keeps crashes reproducible-ish
        if slot.as_ref().is_none_or(|(sv, n)| *n > 200 || !sv.active()) {
            *slot = Some((server(), 0));
        }
        let (sv, n) = slot.as_mut().unwrap();
        *n += 1;
        for i in 0..2 {
            sv.drop_client(i, true);
        }
        let i = sv.core.clients.iter().position(|c| !c.active).unwrap();
        sv.connect_client(i);
        let mut d = data;
        let mut frames = 0;
        let f = |d: &[u8], k: usize| f32::from_bits(u32::from_le_bytes(std::array::from_fn(|j| d.get(k * 4 + j).copied().unwrap_or(0))));
        let short = |d: &[u8], o: usize| f32::from(i16::from_le_bytes(std::array::from_fn(|j| d.get(o + j).copied().unwrap_or(0))));
        while let Some((&op, rest)) = d.split_first() {
            d = rest;
            let take = |d: &mut &[u8], n: usize| -> Vec<u8> {
                let n = n.min(d.len());
                let (a, b) = d.split_at(n);
                *d = b;
                a.to_vec()
            };
            match op % 8 {
                0..=2 => {
                    let n = d.iter().position(|&b| b == b'\n').unwrap_or(d.len());
                    let s = take(&mut d, n + 1);
                    sv.queue_client_command(i, String::from_utf8_lossy(&s).trim_end_matches('\n'));
                }
                3 | 4 => {
                    let b = take(&mut d, 26);
                    sv.record_ping(i, f(&b, 0));
                    sv.set_move(i, ClientMove {
                        // as clc_move carries them: 16-bit angles and moves
                        angles: [4, 6, 8].map(|o| short(&b, o) * (360.0 / 65536.0)),
                        forward: short(&b, 10),
                        side: short(&b, 12),
                        up: short(&b, 14),
                        buttons: u16::from(b.get(24).copied().unwrap_or(0)),
                        impulse: b.get(25).copied().unwrap_or(0),
                    });
                }
                5 => {
                    let b = take(&mut d, 6);
                    sv.ack_frame(i, f(&b, 0).to_bits() as i32);
                    sv.record_move_seq(i, u16::from(b.get(4).copied().unwrap_or(0)));
                }
                _ => {
                    frames += 1;
                    if frames > 64 {
                        break;
                    }
                    sv.frame(1.0 / 72.0);
                    let _ = (sv.take_reliable(i), sv.take_datagrams(i));
                }
            }
        }
        sv.frame(1.0 / 72.0);
        let _ = (sv.take_reliable(i), sv.take_datagrams(i), sv.take_text(), sv.take_error());
    });
});
