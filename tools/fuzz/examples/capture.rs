//! Writes nq_client seeds: real server streams for each protocol.
//! capture BASEDIR OUTDIR
use qcore::cvar::Cvars;
use qserver::{ClientMove, Server};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    std::fs::create_dir_all(&a[2]).unwrap();
    for (proto, pext) in [(15, 0), (666, 0), (999, 0), (999, 2), (999, u32::MAX)] {
        let mut fs = qcore::fs::Fs::new(std::path::Path::new(&a[1]));
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.command(&format!("sv_protocol {proto}"));
        sv.command(&format!("sv_protocol_pext2 {pext}"));
        sv.spawn_server("start").unwrap();
        let c = sv.connect_local().unwrap();
        let mut out = Vec::new();
        for i in 0..120 {
            sv.set_move(c, ClientMove { forward: 200.0, buttons: u16::from(i % 20 < 10), ..Default::default() });
            for m in sv.take_reliable(c).into_iter().chain(sv.take_datagrams(c)) {
                for ch in m.chunks(65535) {
                    out.extend((ch.len() as u16).to_le_bytes());
                    out.extend(ch);
                }
            }
            sv.frame(1.0 / 72.0);
        }
        std::fs::write(format!("{}/p{proto}-{pext}", a[2]), &out).unwrap();
        out.truncate(6000);
        std::fs::write(format!("{}/p{proto}-{pext}-short", a[2]), &out).unwrap();
    }
}
