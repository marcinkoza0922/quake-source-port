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
