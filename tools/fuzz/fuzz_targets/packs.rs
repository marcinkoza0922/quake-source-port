//! Hostile pak/pk3 (first byte '0' or '1'): open and read every member.
#![no_main]
use libfuzzer_sys::fuzz_target;
use qcore::fs::Pack;

fuzz_target!(|data: &[u8]| {
    let Some((&k, d)) = data.split_first() else { return };
    let path = std::path::PathBuf::from(format!("/dev/shm/qfuzz-pack-{}", std::process::id()));
    std::fs::write(&path, d).unwrap();
    let p = if k == b'0' { Pack::load_pak(&path) } else { Pack::load_pk3(&path) };
    if let Some(p) = p {
        for n in p.names().iter().take(32) {
            let _ = p.read(n);
        }
    }
});
