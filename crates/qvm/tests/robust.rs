//! Malformed progs.dat.

/// Found by tools/fuzz: the edict size was taken from the header unchecked.
#[test]
fn huge_entityfields_is_rejected() {
    let mut d = vec![0u8; 64];
    d[..4].copy_from_slice(&qvm::PROG_VERSION.to_le_bytes());
    d[56..60].copy_from_slice(&i32::MAX.to_le_bytes());
    let e = qvm::Progs::parse_csqc(&d).expect_err("rejected");
    assert!(e.msg.contains("entityfields"), "{}", e.msg);
}

/// A progs with `globals` globals and nothing else.
fn tiny(globals: usize) -> Vec<u8> {
    let mut d = vec![0u8; 60];
    let mut put = |i: usize, v: i32| d[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    put(0, qvm::PROG_VERSION);
    for lump in [2, 4, 6, 8, 10] {
        put(lump, 60);
    }
    put(11, 1);
    put(12, 61);
    put(13, globals as i32);
    d.push(0);
    // (strings may not touch the end of the file)
    d.resize(61 + globals * 4 + 1, 0);
    d
}

/// Parms, the return value and vectors found by name are read unchecked.
#[test]
fn few_globals_leave_room_for_the_parms() {
    for n in [0, 1, 27, 28, 100] {
        let p = qvm::Progs::parse_csqc(&tiny(n)).unwrap();
        assert!(p.globals.len() >= 28 && p.globals.len() >= n + 15, "{n}");
        assert!(p.entityfields >= 2);
    }
}

/// Functions that call each other through builtins: function 1 is setorigin,
/// function 2 calls it.
fn calls_a_builtin() -> Vec<u8> {
    let (statements, functions, strings, globals) = (60usize, 60 + 24, 60 + 24 + 3 * 36, 60 + 24 + 3 * 36 + 8);
    let mut d = vec![0u8; globals + 40 * 4 + 4];
    let mut put = |at: usize, v: i32| d[at..at + 4].copy_from_slice(&v.to_le_bytes());
    for (i, v) in [qvm::PROG_VERSION, 0, statements as i32, 3, 60, 0, 60, 0, functions as i32, 3, strings, 2, globals as i32, 40, 4].into_iter().enumerate() {
        put(i * 4, v);
    }
    // done; call0 [global 30]; done
    put(statements + 8, 51 | 30 << 16);
    put(globals + 30 * 4, 1);
    put(functions + 36, -2);
    put(functions + 72, 1);
    d
}

struct Again(u32);
impl qvm::Host for Again {
    fn builtin(&mut self, vm: &mut qvm::Vm, _: qvm::Builtin) -> qvm::Result<()> {
        self.0 += 1;
        vm.execute(self, 2)
    }
}

/// A touch that touches again and again used up the native stack (1 MiB on
/// Windows and the web) before QuakeC's own.
#[test]
fn nested_calls_end_in_an_error() {
    let run = || {
        let mut vm = qvm::Vm::new_csqc(qvm::Progs::parse_csqc(&calls_a_builtin()).unwrap(), 8).unwrap();
        for _ in 0..2 {
            let mut host = Again(0);
            let e = vm.execute(&mut host, 2).expect_err("ends");
            assert!(e.msg.contains("stack overflow"), "{}", e.msg);
            assert_eq!(host.0 as usize, qvm::MAX_NESTED, "and starts afresh the next time");
        }
    };
    std::thread::Builder::new().stack_size(1 << 20).spawn(run).unwrap().join().unwrap();
}
