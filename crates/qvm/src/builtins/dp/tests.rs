use super::text::{strftime, strip_markup};
use super::*;

#[test]
fn markup_codes_are_stripped() {
    assert_eq!(strip_markup(b"^1red^7 ^xf0aRGB ^^caret"), b"red RGB ^caret");
    assert_eq!(strip_markup(b"a^&F-b^U0041^mc"), [b"abA".as_slice(), &[b'c' | 128]].concat());
    assert_eq!(strip_markup(b"^zkept ^x12"), b"^zkept ^x12");
}

#[test]
fn strftime_formats_utc_dates() {
    let f = |fmt: &str, t| String::from_utf8(strftime(fmt.as_bytes(), t)).unwrap();
    assert_eq!(f("%Y-%m-%d %H:%M:%S", 0), "1970-01-01 00:00:00");
    assert_eq!(f("%F %T %a %b %j", 951_827_696), "2000-02-29 12:34:56 Tue Feb 060");
    assert_eq!(f("%I:%M %p%%", 86399), "11:59 PM%");
    assert_eq!(f("%A %d %B %y", 1_763_856_000), "Sunday 23 November 25");
}

struct Quiet;
impl Host for Quiet {
    fn builtin(&mut self, vm: &mut Vm, _: Builtin) -> Result<()> {
        vm.ret_i(0);
        Ok(())
    }
}

enum A<'a> {
    F(f32),
    I(i32),
    S(&'a str),
}

fn vm() -> Option<Vm> {
    let base = std::env::var_os("QUAKE_BASEDIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    let mut vm = Vm::load(&fs.open("progs.dat")?, 1024, 1).ok()?;
    for _ in 0..4 {
        vm.ed_alloc().ok()?;
    }
    Some(vm)
}

fn call(vm: &mut Vm, name: &str, args: &[A]) {
    vm.argc = args.len();
    for (i, a) in args.iter().enumerate() {
        match *a {
            A::F(f) => vm.set_g_f(ofs_parm(i), f),
            A::I(n) => vm.set_g_i(ofs_parm(i), n),
            A::S(s) => {
                let h = vm.strings.temp(s.as_bytes());
                vm.set_g_i(ofs_parm(i), h);
            }
        }
    }
    let f = internal().find(|t| t.0 == name).unwrap_or_else(|| panic!("no {name}")).2;
    f(vm, &mut Quiet).unwrap();
}

const RET: usize = 1; // OFS_RETURN

fn text(vm: &Vm) -> String {
    String::from_utf8_lossy(vm.g_str(RET).unwrap()).into_owned()
}

#[test]
fn string_buffers() {
    use A::*;
    let Some(mut vm) = vm() else { return };
    call(&mut vm, "buf_create", &[]);
    let b = vm.g_f(RET);
    assert_eq!(b, 1.0);
    for s in ["pear", "apple", "fig"] {
        call(&mut vm, "bufstr_add", &[F(b), S(s), F(1.0)]);
    }
    assert_eq!(vm.g_f(RET), 2.0);
    call(&mut vm, "bufstr_free", &[F(b), F(1.0)]);
    call(&mut vm, "bufstr_get", &[F(b), F(1.0)]);
    assert_eq!(vm.g_i(RET), 0);
    call(&mut vm, "bufstr_add", &[F(b), S("kiwi"), F(0.0)]); // fills the hole
    assert_eq!(vm.g_f(RET), 1.0);
    call(&mut vm, "buf_sort", &[F(b), F(0.0), F(0.0)]);
    call(&mut vm, "buf_implode", &[F(b), S(", ")]);
    assert_eq!(text(&vm), "fig, kiwi, pear");
    call(&mut vm, "bufstr_set", &[F(b), F(1e9), S("x")]); // ignored, not allocated
    call(&mut vm, "buf_getsize", &[F(b)]);
    assert_eq!(vm.g_f(RET), 3.0);
    call(&mut vm, "buf_create", &[]);
    let b2 = vm.g_f(RET);
    call(&mut vm, "buf_copy", &[F(b), F(b2)]);
    call(&mut vm, "buf_del", &[F(b)]);
    call(&mut vm, "bufstr_get", &[F(b2), F(2.0)]);
    assert_eq!(text(&vm), "pear");
    for bad in [0.0, -5.0, 900.0, f32::NAN] {
        call(&mut vm, "bufstr_get", &[F(bad), F(0.0)]);
        assert_eq!(vm.g_i(RET), 0);
    }
}

#[test]
fn tokens_uris_and_crc() {
    use A::*;
    let Some(mut vm) = vm() else { return };
    call(&mut vm, "tokenizebyseparator", &[S("10.0.0.1:26000"), S("."), S(":")]);
    assert_eq!(vm.g_f(RET), 5.0);
    call(&mut vm, "argv", &[F(-1.0)]);
    assert_eq!(text(&vm), "26000");
    call(&mut vm, "argv_start_index", &[F(4.0)]);
    assert_eq!(vm.g_f(RET), 9.0);
    call(&mut vm, "argv_end_index", &[F(0.0)]);
    assert_eq!(vm.g_f(RET), 2.0);
    call(&mut vm, "argv_end_index", &[F(9.0)]);
    assert_eq!(vm.g_f(RET), -1.0);
    call(&mut vm, "tokenizebyseparator", &[S("abc"), S("")]); // no endless loop
    assert_eq!(vm.g_f(RET), 1.0);
    call(&mut vm, "tokenize", &[S("  one \"two 2\"")]);
    call(&mut vm, "argv_start_index", &[F(1.0)]);
    assert_eq!(vm.g_f(RET), 6.0);
    call(&mut vm, "uri_escape", &[S("a b/ok-1")]);
    assert_eq!(text(&vm), "a%20b%2Fok-1");
    call(&mut vm, "uri_unescape", &[S("a%20b%2Fok%zz%4")]);
    assert_eq!(text(&vm), "a b/ok%zz%4");
    call(&mut vm, "crc16", &[F(1.0), S("QUAKE")]);
    assert_eq!(vm.g_f(RET), f32::from(crate::crc_block(b"quake")));
}

#[test]
fn chains_and_fields() {
    use A::*;
    let Some(mut vm) = vm() else { return };
    let n = vm.num_edicts() as i32;
    let (cls, health, ch) = (vm.field_ofs("classname").unwrap(), vm.field_ofs("health").unwrap(), vm.field_ofs("chain").unwrap());
    let (a, b) = (n - 2, n - 1);
    let h = vm.strings.engine(b"thing");
    for e in [a, b] {
        vm.set_ent_i(e as u32, cls, h);
        vm.set_ent_f(e as u32, health, 6.0);
    }
    call(&mut vm, "findchain", &[I(cls as i32), S("thing")]);
    assert_eq!((vm.g_i(RET), vm.ent_i(b as u32, ch), vm.ent_i(a as u32, ch)), (b, a, 0));
    call(&mut vm, "findchainfloat", &[I(health as i32), F(5.0)]);
    assert_eq!(vm.g_i(RET), 0);
    call(&mut vm, "findchainflags", &[I(health as i32), F(2.0)]);
    assert_eq!(vm.g_i(RET), b);
    call(&mut vm, "findchain", &[I(1 << 20), S("thing")]); // bad field
    assert_eq!(vm.g_i(RET), 0);
    call(&mut vm, "findentityfield", &[S("health")]);
    let fi = vm.g_f(RET);
    call(&mut vm, "entityfieldname", &[F(fi)]);
    assert_eq!(text(&vm), "health");
    call(&mut vm, "entityfieldtype", &[F(fi)]);
    assert_eq!(vm.g_f(RET), 2.0);
    call(&mut vm, "putentityfieldstring", &[F(fi), I(a), S("42.5")]);
    assert_eq!((vm.g_f(RET), vm.ent_f(a as u32, health)), (1.0, 42.5));
    call(&mut vm, "getentityfieldstring", &[F(fi), I(a)]);
    assert_eq!(text(&vm), "42.500000");
    call(&mut vm, "parseentitydata", &[I(a), S("xx { \"health\" \"7\" } tail"), F(3.0)]);
    assert_eq!((vm.g_f(RET), vm.ent_f(a as u32, health)), (19.0, 7.0));
    call(&mut vm, "isfunction", &[S("worldspawn")]);
    assert_eq!(vm.g_f(RET), 1.0);
    call(&mut vm, "callfunction", &[S("no_such_function")]);
    call(&mut vm, "randomvec", &[]);
    assert!(vm.g_v(RET).iter().map(|x| x * x).sum::<f32>() < 1.0);
}

#[test]
fn vectoangles_with_an_up_vector_rolls() {
    use A::*;
    let Some(mut vm) = vm() else { return };
    for (up, roll) in [([0.0, 0.0, 1.0], 0.0), ([0.0, 1.0, 0.0], -90.0), ([0.0, -1.0, 0.0], 90.0)] {
        vm.set_g_v(ofs_parm(1), up);
        call(&mut vm, "vectoangles", &[F(1.0)]);
        vm.set_g_v(ofs_parm(0), [1.0, 0.0, 0.0]);
        vm.argc = 2;
        pf_vectoangles(&mut vm, &mut Quiet).unwrap();
        let a = vm.g_v(RET);
        assert!((a[2] - roll).abs() < 0.01 && a[0].abs() < 0.01 && a[1].abs() < 0.01, "{up:?}: {a:?}");
    }
}

/// Widths and precisions past what a temp string holds (or Rust formats).
#[test]
fn sprintf_fields_are_bounded() {
    let Some(mut vm) = vm() else { return };
    for f in ["%.70000f", "%99999999999999f", "%2000000000f", "%-2000000000d", "%02000000000x", "%.99999e", "%.70000v", "%.70000g"] {
        call(&mut vm, "sprintf", &[A::S(f), A::F(1.5)]);
        assert!(vm.g_str(RET).unwrap().len() < crate::STRINGTEMP_LENGTH, "{f}");
    }
    call(&mut vm, "sprintf", &[A::S("%.70000s|%2000000000s"), A::S("a"), A::S("b")]);
    call(&mut vm, "sprintf", &[A::S("%*.*f"), A::F(1e30), A::F(1e30), A::F(1.5)]);
    call(&mut vm, "sprintf", &[A::S("%6.2f|"), A::F(1.5)]);
    assert_eq!(vm.g_str(RET).unwrap(), b"  1.50|");
}
