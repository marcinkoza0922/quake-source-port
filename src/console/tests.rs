use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use qinput::keys::*;

use super::Console;
use super::ansi::{self, CellStyle, DEFAULT_FG};
use super::complete::{Catalog, split_unquoted_pipe};
use super::edit::Modifiers;
use super::shell::{PtySession, ShellEnv, ShellJob};
use super::vt::VtGrid;
use super::word_wrap;

#[test]
fn wraps_words() {
    assert_eq!(word_wrap("the quick brown fox", 10), "the quick \nbrown fox");
    assert_eq!(word_wrap("abcdefghijkl xy", 5), "abcde\nfghij\nkl xy");
    assert_eq!(word_wrap("one\ntwo", 40), "one\ntwo");
    assert_eq!(word_wrap("no wrap", 0), "no wrap");
}

#[test]
fn colored_centerprint_wraps_and_centers_by_font_indices() {
    use crate::draw::Canvas;
    use qrender::Draw2D;
    let title = b"\xa0\xd0\xe5\xee\xe4\xf2\xe1\xe7\xef\xee\xa0";
    let mut con = Console::new();
    con.center_print(&qcore::text::decode(title), 2.0, 11);
    assert_eq!(con.centerprint.len(), 1);
    let mut canvas = Canvas::new(640, 480);
    con.draw_center(&mut canvas, usize::MAX);
    let glyphs: Vec<_> = canvas
        .list
        .iter()
        .filter_map(|d| match d {
            Draw2D::Char { x, ch, .. } => Some((*x, *ch)),
            _ => None,
        })
        .collect();
    assert_eq!(glyphs.iter().map(|&(_, ch)| ch).collect::<Vec<_>>(), title);
    assert_eq!(glyphs[0].0, 232.0);
    let wrapped = word_wrap(&qcore::text::decode(b"\xd0\xe5\xee\xe4 \x9e\x9d\x9d"), 5);
    assert_eq!(qcore::text::encode(&wrapped), b"\xd0\xe5\xee\xe4 \n\x9e\x9d\x9d");
    canvas.list.clear();
    con.draw_center(&mut canvas, 2);
    assert_eq!(canvas.list.len(), 2);
}

#[test]
fn ansi_strip_and_sgr_colors() {
    let raw = "\x1b[1;31merror:\x1b[0m \x1b[38;2;16;32;64mok\x1b[0m\n";
    assert_eq!(ansi::strip_ansi(raw), "error: ok\n");

    let mut cur = Vec::new();
    let mut lines = VecDeque::new();
    let mut style = CellStyle::default();
    ansi::append_styled(raw, &mut cur, &mut lines, &mut style, 80, 100, true);
    assert_eq!(lines.len(), 1);
    let row = &lines[0];
    let text: String = row.iter().map(|c| c.ch).collect();
    assert_eq!(text, "error: ok");
    assert!(row[0].style.bold);
    assert_ne!(row[0].style.fg, DEFAULT_FG);
    // TrueColor 16,32,64
    let ok_fg = row[7].style.fg;
    assert!((ok_fg[0] - 16.0 / 255.0).abs() < 0.01);
    assert!((ok_fg[1] - 32.0 / 255.0).abs() < 0.01);
    assert!((ok_fg[2] - 64.0 / 255.0).abs() < 0.01);
}

#[test]
fn vt_grid_cursor_and_alt_screen() {
    let mut vt = VtGrid::new(20, 5);
    vt.feed(b"hello\r\nworld");
    let l0: String = vt.cells[0].iter().map(|c| c.ch).collect::<String>().trim_end().into();
    let l1: String = vt.cells[1].iter().map(|c| c.ch).collect::<String>().trim_end().into();
    assert_eq!(l0, "hello");
    assert_eq!(l1, "world");

    // Enter alt screen, overwrite, then leave alt screen and verify main screen is restored.
    vt.feed(b"\x1b[?1049h\x1b[HALT");
    assert!(vt.in_alt_screen());
    let alt0: String = vt.cells[0].iter().map(|c| c.ch).collect::<String>().trim_end().into();
    assert_eq!(alt0, "ALT");

    vt.feed(b"\x1b[?1049l");
    assert!(!vt.in_alt_screen());
    let r0: String = vt.cells[0].iter().map(|c| c.ch).collect::<String>().trim_end().into();
    assert_eq!(r0, "hello");
}

#[test]
fn fuzzy_completion_and_context_arguments() {
    let mut con = Console::new();
    con.catalog = Catalog {
        cmds: vec!["map".into(), "cvarlist".into(), "changelevel".into()],
        aliases: vec![("qfov".into(), "fov 110".into())],
        cvars: vec![
            ("r_shadows".into(), "1".into(), "0".into()),
            ("con_modern".into(), "1".into(), "1".into()),
        ],
        maps: vec!["e1m1".into(), "e1m2".into(), "start".into()],
        cfgs: vec!["config.cfg".into(), "autoexec.cfg".into()],
        demos: vec!["demo1".into()],
        mods: vec!["id1".into(), "ad".into()],
        saves: vec!["s0".into()],
    };

    // Fuzzy command/cvar match
    for ch in "rshd".chars() {
        con.text_modern(ch);
    }
    assert!(!con.completion.items.is_empty());
    assert_eq!(con.completion.items[0].text, "r_shadows");

    // Map argument completion
    con.input.clear();
    con.cursor = 0;
    for ch in "map e1".chars() {
        con.text_modern(ch);
    }
    assert!(con.completion.items.iter().any(|i| i.text == "e1m1"));

    // Split unquoted pipe
    assert_eq!(
        split_unquoted_pipe("cvarlist | grep ^r_").map(|(l, r)| (l.trim(), r.trim())),
        Some(("cvarlist", "grep ^r_"))
    );
    assert_eq!(split_unquoted_pipe("echo \"a | b\""), None);
}

#[test]
fn readline_shortcuts_and_reverse_search() {
    let mut con = Console::new();
    for ch in "map e1m1 extra".chars() {
        con.text_modern(ch);
    }
    let no_clip = || None;
    let set_clip = |_: &str| {};
    let ctrl = Modifiers { ctrl: true, shift: false, alt: false };
    let plain = Modifiers::default();

    // Ctrl+W deletes last word ("extra")
    con.key_modern(b'w' as i32, ctrl, &no_clip, &set_clip);
    assert_eq!(con.input, "map e1m1 ");

    // Ctrl+A moves to start, Ctrl+E moves to end
    con.key_modern(b'a' as i32, ctrl, &no_clip, &set_clip);
    assert_eq!(con.cursor, 0);
    con.key_modern(b'e' as i32, ctrl, &no_clip, &set_clip);
    assert_eq!(con.cursor, con.input.len());

    // Submit two commands into history
    let submitted = con.key_modern(K_ENTER, plain, &no_clip, &set_clip);
    assert_eq!(submitted.as_deref(), Some("map e1m1 "));
    for ch in "fov 105".chars() {
        con.text_modern(ch);
    }
    let _ = con.key_modern(K_ENTER, plain, &no_clip, &set_clip);

    // Reverse history search with Ctrl+R
    con.key_modern(b'r' as i32, ctrl, &no_clip, &set_clip);
    assert!(con.search.active);
    con.text_modern('e');
    con.text_modern('1');
    assert_eq!(con.search.match_idx, Some(0));
    let exec = con.key_modern(K_ENTER, plain, &no_clip, &set_clip);
    assert_eq!(exec.as_deref(), Some("map e1m1 "));
}

#[test]
fn persistent_history_roundtrip() {
    let dir = std::env::temp_dir().join(format!("qsp_hist_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut con = Console::new();
    con.history = vec!["map e1m1".into(), "cvarlist | grep ^r_".into()];
    con.save_history(&dir);

    let mut con2 = Console::new();
    con2.load_history(&dir);
    assert_eq!(con2.history, con.history);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shell_job_pipes_stdin_and_runs_in_basedir() {
    let basedir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let env = ShellEnv {
        basedir: basedir.clone(),
        gamedir: "id1".into(),
        userdir: basedir.clone(),
        mapname: "e1m1".into(),
    };
    let stdin = Some("alpha\nr_shadows 1\nbeta\nr_wateralpha 0.5\n".to_string());
    let mut job = ShellJob::spawn("grep ^r_ && printf 'dir=%s:%s' \"$QUAKE_GAMEDIR\" \"$QUAKE_MAP\"", stdin, &env)
        .expect("spawn shell job");
    let out = job.drain_with_budget(Duration::from_secs(2));
    assert!(out.contains("r_shadows 1"), "unexpected output: {out}");
    assert!(out.contains("r_wateralpha 0.5"), "unexpected output: {out}");
    assert!(out.contains("dir=id1:e1m1"), "unexpected output: {out}");
}

#[cfg(unix)]
#[test]
fn interactive_pty_bash_opens_in_basedir_and_exits() {
    let basedir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let env = ShellEnv {
        basedir: basedir.clone(),
        gamedir: "id1".into(),
        userdir: basedir.clone(),
        mapname: "start".into(),
    };
    let mut pty = PtySession::spawn(80, 24, &env).expect("spawn bash pty");
    pty.write_bytes(b"printf 'QSP_PTY:%s\\n' \"$PWD\"; exit\n");
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !pty.poll() {
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let text: String = pty
        .snapshot_lines()
        .into_iter()
        .map(|r| r.into_iter().map(|c| c.ch).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    let expected = format!("QSP_PTY:{}", basedir.display());
    assert!(text.contains(&expected), "expected {expected:?} in PTY output:\n{text}");
}

#[test]
fn console_capture_pipes_into_bash_job() {
    let basedir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut con = Console::new();
    con.shell_env = ShellEnv {
        basedir,
        gamedir: "id1".into(),
        userdir: PathBuf::from("."),
        mapname: "e1m1".into(),
    };
    con.begin_capture();
    con.print("con_modern \"1\"\nfov \"90\"\nr_shadows \"0\"\n");
    let captured = con.end_capture();
    con.run_shell_cmd("grep con_modern", Some(captured));

    let deadline = Instant::now() + Duration::from_secs(2);
    while !con.jobs.is_empty() && Instant::now() < deadline {
        con.poll_shell();
        std::thread::sleep(Duration::from_millis(10));
    }
    let printed: String = con
        .modern_lines
        .iter()
        .map(|row| row.iter().map(|c| c.ch).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        printed.contains("con_modern \"1\""),
        "expected piped output in console, got: {printed:?}"
    );
}

fn vt_row(vt: &VtGrid, r: usize) -> String {
    vt.cells[r].iter().map(|c| c.ch).collect::<String>().trim_end().to_string()
}

#[test]
fn vt_restored_cursors_stay_on_a_smaller_screen() {
    for leave in ["\x1b[?1049lx", "\x1b[?1049l\x1b8x", "\x1b[?1049l\x1b[ux"] {
        let mut vt = VtGrid::new(80, 40);
        vt.feed(b"\x1b[38;70H\x1b7\x1b[s\x1b[?1049h\x1b[40;80H");
        vt.resize(30, 20);
        vt.feed(leave.as_bytes());
        assert!(vt.cursor_row < 20 && vt.cells.len() == 20 && vt.cells.iter().all(|r| r.len() == 30), "{leave:?}");
        vt.feed(b"\x1b[J\x1b[K\x1b[P\x1b[@\x1b[X");
    }
}

#[test]
fn vt_random_input_and_resizes_never_panic() {
    let mut seed = 1u32;
    let mut rnd = move |n: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed % n
    };
    for _ in 0..300 {
        let mut vt = VtGrid::new(40, 12);
        for _ in 0..60 {
            let (a, b) = (rnd(60), rnd(60));
            match rnd(9) {
                0 => vt.resize(20 + rnd(60) as usize, 5 + rnd(40) as usize),
                1 => vt.feed(format!("\x1b[{a};{b}H").as_bytes()),
                2 => vt.feed(["\x1b7", "\x1b8", "\x1b[s", "\x1b[u", "\x1bM", "\x1b[?1049h", "\x1b[?1049l"][rnd(7) as usize].as_bytes()),
                3 => vt.feed(format!("\x1b[{a}{}", ['L', 'M', 'S', 'T', 'P', '@', 'X', 'J', 'K'][rnd(9) as usize]).as_bytes()),
                4 => vt.feed(format!("\x1b[{a};{b}r").as_bytes()),
                5 => vt.feed(b"\r\n\r\n"),
                6 => vt.feed(&(0..rnd(50)).map(|i| (i * 37 + a) as u8).collect::<Vec<_>>()),
                _ => vt.feed("some text é €".as_bytes()),
            }
        }
    }
}

#[test]
fn vt_shrinking_keeps_the_prompt() {
    let mut vt = VtGrid::new(40, 24);
    for i in 0..23 {
        vt.feed(format!("line {i}\r\n").as_bytes());
    }
    vt.feed(b"prompt$ ");
    vt.resize(40, 5);
    assert_eq!((vt_row(&vt, vt.cursor_row).as_str(), vt.cursor_row), ("prompt$", 4));
    assert_eq!(vt.scrollback.len(), 19, "the rest is in the scrollback");
    vt.resize(40, 24);
    assert_eq!(vt_row(&vt, vt.cursor_row), "prompt$");
    vt.feed(b"ls");
    assert_eq!(vt_row(&vt, 4), "prompt$ ls");
}

#[test]
fn vt_split_characters_and_endless_escapes() {
    let mut vt = VtGrid::new(40, 5);
    for b in "é€😀x".bytes() {
        vt.feed(&[b]);
    }
    assert_eq!(vt_row(&vt, 0), "é€😀x");
    vt.feed(b"\r\nbad \xff\xfe byte");
    assert_eq!(vt_row(&vt, 1), "bad \u{fffd}\u{fffd} byte");
    // counts are limited to the screen, unfinished sequences to a few KB
    let t = Instant::now();
    vt.feed("\x1b[65535S\x1b[65535T\x1b[65535L\x1b[65535M".repeat(100).as_bytes());
    vt.feed(b"\x1b]0;");
    for _ in 0..500 {
        vt.feed(&[b'a'; 8192]);
    }
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    vt.feed(b"\x1b[2J\x1b[H\x1b]0;title\x07done");
    assert_eq!((vt_row(&vt, 0).as_str(), vt.title.as_str()), ("done", "title"));
}

#[cfg(unix)]
#[test]
fn endless_output_gets_its_share_of_a_frame() {
    let env = ShellEnv { basedir: std::env::temp_dir(), ..Default::default() };
    let mut job = ShellJob::spawn("yes", None, &env).unwrap();
    let t = Instant::now();
    assert!(job.drain_with_budget(Duration::from_millis(30)).len() < 1 << 20);
    for _ in 0..5 {
        assert!(job.poll().len() < 1 << 20);
    }
    assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    drop(job); // `yes` ends with its pipe

    let mut pty = PtySession::spawn(80, 24, &env).unwrap();
    pty.write_bytes(b"yes\n");
    std::thread::sleep(Duration::from_millis(300));
    for _ in 0..20 {
        let t = Instant::now();
        assert!(pty.poll());
        assert!(t.elapsed() < Duration::from_millis(250), "{:?}", t.elapsed());
    }
    // more input than the terminal holds at once is all sent
    let mut pty = PtySession::spawn(80, 24, &env).unwrap();
    pty.write_bytes(b"stty -echo; wc -c\n");
    std::thread::sleep(Duration::from_millis(500));
    pty.send_paste(&"0123456789012345678\n".repeat(4000));
    pty.write_bytes(b"\x04");
    let deadline = Instant::now() + Duration::from_secs(10);
    let shown = |p: &PtySession| p.snapshot_lines().iter().any(|l| l.iter().map(|c| c.ch).collect::<String>().contains("80000"));
    while !shown(&pty) && Instant::now() < deadline {
        pty.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(shown(&pty), "every byte arrived");
}

#[test]
fn classic_console_edits_non_ascii_input() {
    let mut con = Console::new();
    con.modern = false;
    con.history = vec!["say zażółć".into()];
    con.hist_pos = 1;
    let none = |_: &str| Vec::new();
    for key in [K_UPARROW, K_BACKSPACE, K_LEFTARROW, K_LEFTARROW, K_LEFTARROW, K_DEL, K_RIGHTARROW] {
        con.key(key, &none);
    }
    con.text('x');
    assert_eq!(con.input, "say zaóxł");
    con.key(K_HOME, &none);
    for _ in 0..20 {
        con.key(K_RIGHTARROW, &none);
    }
    assert_eq!(con.cursor, con.input.len());
    // the common prefix ends between two characters
    con.input = "z".into();
    con.cursor = 1;
    con.key(K_TAB, &|_: &str| vec!["zé".into(), "zè".into()]);
    assert_eq!(con.input, "z");
}

#[test]
fn game_text_is_not_ansi() {
    let mut con = Console::new();
    let cells = |con: &Console| con.modern_lines.back().unwrap().clone();
    con.print("ammo \x12\x1b\x13 left\n");
    assert_eq!(con.lines.back().unwrap(), b"ammo \x12\x1b\x13 left");
    assert_eq!(cells(&con).iter().map(|c| c.ch).collect::<String>(), "ammo 091 left");
    con.print("\x1b[38;2;10;12;18mhidden\n");
    con.print("error: shown\n");
    assert!(cells(&con).iter().all(|c| c.style == CellStyle::default()));
    // the shell's colours carry over its own lines only
    con.print_ansi("\x1b[31mred\n");
    con.print("game\n");
    assert!(cells(&con).iter().all(|c| c.style == CellStyle::default()));
    con.print_ansi("still red\n");
    assert_ne!(cells(&con)[0].style.fg, DEFAULT_FG);
}
