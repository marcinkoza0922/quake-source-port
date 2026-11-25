//! Hostile .sav/.qsa: load it, then run a few frames. Needs QUAKE_BASEDIR.
#![no_main]
use std::cell::RefCell;

use libfuzzer_sys::fuzz_target;
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::Server;

thread_local!(static SV: RefCell<Option<Server>> = const { RefCell::new(None) });

fuzz_target!(|data: &[u8]| {
    SV.with_borrow_mut(|slot| {
        let sv = slot.get_or_insert_with(|| {
            let base = std::env::var("QUAKE_BASEDIR").expect("QUAKE_BASEDIR");
            let mut fs = Fs::new(std::path::Path::new(&base));
            fs.add_game_dir("id1");
            Server::new(fs, Cvars::new(), 1)
        });
        if sv.load_game(&qcore::text::decode(data)).is_ok() {
            if let Some(c) = sv.connect_local() {
                for _ in 0..4 {
                    sv.frame(0.05);
                    let _ = (sv.take_reliable(c), sv.take_datagrams(c));
                }
            }
        }
        let _ = (sv.take_text(), sv.take_error());
    });
});
