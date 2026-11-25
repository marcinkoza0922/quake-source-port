//! Only commands the user types (or puts on the command line) reach the system
//! shell; server, demo, QuakeC, config, alias and bind text never does.
#![cfg(unix)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::super::*;

fn host(cmdline: &[&str]) -> Option<Host> {
    let fs = qcore::fs::Fs::new(std::path::Path::new("."));
    let pal = [[0u8; 3]; 256];
    let r = Renderer::headless(64, 64, &pal).ok()?;
    let mut h = Host::new(fs, r, None, cmdline.iter().map(|s| s.to_string()).collect());
    h.headless = true;
    Some(h)
}

fn marker(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("qsp_shell_{}_{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

fn appears(h: &mut Host, p: &std::path::Path) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while !p.exists() && Instant::now() < end {
        h.frame(1.0 / 72.0, Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
    let ok = p.exists();
    let _ = std::fs::remove_file(p);
    ok
}

/// Every way into the shell, as one command each.
fn attacks(p: &std::path::Path) -> Vec<String> {
    let p = p.display();
    vec![
        format!("!touch {p}"),
        format!("echo x | touch {p}"),
        format!("bash -c \"touch {p}\""),
        format!("sh touch {p}"),
        format!("delay 0 !touch {p}"),
        format!("delay 0 bash touch {p}"),
        format!("delay 0 delay 0 !touch {p}"),
    ]
}

/// Nothing ran, judged after a later, allowed shell command has finished.
fn assert_blocked(h: &mut Host, bad: &std::path::Path, what: &str) {
    for _ in 0..4 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(h.con.jobs.is_empty(), "{what}: started a shell job");
    assert!(h.con.pty.is_none(), "{what}: opened a shell");
    let good = marker("control");
    h.cbuf.add_text_from(&format!("!touch {}\n", good.display()), Origin::User);
    assert!(appears(h, &good), "the control command did not run");
    assert!(!bad.exists(), "{what}: reached the shell");
}

#[test]
fn untrusted_text_never_reaches_the_shell() {
    let Some(mut h) = host(&[]) else { return };
    let bad = marker("bad");
    h.cbuf.add_text("bash\n");
    assert_blocked(&mut h, &bad, "interactive bash");
    for a in attacks(&bad) {
        // exec'd configs, menus
        h.cbuf.add_text(&format!("{a}\n"));
        assert_blocked(&mut h, &bad, &format!("buffer: {a}"));
        // svc_stufftext, as a server or demo sends it
        let mut msg = vec![9u8];
        msg.extend_from_slice(format!("{a}\n\0").as_bytes());
        h.cl.parse_server_message(&msg).expect("stufftext parses");
        h.process_client_events();
        assert!(!h.cbuf.is_empty(), "stufftext was not queued");
        assert_blocked(&mut h, &bad, &format!("stufftext: {a}"));
        // QuakeC localcmd
        h.server.core.cbuf.add_text(&format!("{a}\n"));
        assert_blocked(&mut h, &bad, &format!("localcmd: {a}"));
        // an alias the server set and the user then typed
        h.cmds.set_alias("trap", &format!("{a}\n"));
        h.cbuf.add_text_from("trap\n", Origin::User);
        assert_blocked(&mut h, &bad, &format!("alias: {a}"));
    }
}

#[test]
fn user_commands_still_reach_the_shell() {
    let Some(mut h) = host(&[]) else { return };
    let p = marker("user");
    for a in attacks(&p) {
        h.cbuf.add_text_from(&format!("{a}\n"), Origin::User);
        assert!(appears(&mut h, &p), "{a}");
    }
}

#[test]
fn con_type_is_for_the_user() {
    let Some(mut h) = host(&[]) else { return };
    h.cbuf.add_text("con_type abc\n");
    h.exec_cbuf();
    assert!(h.con.input.is_empty());
    h.cbuf.add_text_from("con_type abc\n", Origin::User);
    h.exec_cbuf();
    assert_eq!(h.con.input, "abc");
}
