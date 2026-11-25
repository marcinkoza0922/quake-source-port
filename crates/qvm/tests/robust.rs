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
