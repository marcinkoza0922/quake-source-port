//! `qwfake [-port N] [-map NAME] [-basedir DIR]`: run the scripted QW test
//! server (see the library docs) until killed. Connect with
//! `qwconnect 127.0.0.1:<port>` (default port 27500).

use std::net::Ipv4Addr;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let val = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let port: u16 = val("-port").and_then(|p| p.parse().ok()).unwrap_or(qnet::qw::PORT_SERVER);
    let map = format!("maps/{}.bsp", val("-map").unwrap_or_else(|| "e1m1".into()));
    let base = val("-basedir").map(std::path::PathBuf::from).unwrap_or_else(qcore::fs::Fs::default_basedir);
    let mut fs = qcore::fs::Fs::new(&base);
    fs.add_game_dir("id1");
    let Some(bsp) = fs.open(&map) else {
        eprintln!("qwfake: {map} not found under {}", base.display());
        std::process::exit(1);
    };
    let sock = qnet::UdpTransport::bind((Ipv4Addr::UNSPECIFIED, port).into()).expect("bind");
    let mut sv = qwfake::FakeServer::new(sock, &map, &bsp).expect("bad map");
    eprintln!("qwfake: serving {map} on port {}", sv.port());
    let mut info = String::new();
    let mut logged = 0;
    loop {
        sv.frame(&mut info);
        for l in &sv.log[logged..] {
            eprintln!("qwfake: {l}");
        }
        logged = sv.log.len();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
