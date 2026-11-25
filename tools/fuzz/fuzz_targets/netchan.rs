//! Raw UDP packets into every datagram decoder.
#![no_main]
use libfuzzer_sys::fuzz_target;
use qnet::qw::{self, Netchan};
use qnet::{Ccrep, Ccreq, Connection};

fuzz_target!(|data: &[u8]| {
    let mut c = Connection::new();
    let mut nc = Netchan::new(27001, 0.0);
    let _ = c.send_reliable(&[7; 4000], 0.0);
    let mut d = data;
    let mut t = 0.0;
    while d.len() > 2 {
        let n = (u16::from_le_bytes([d[0], d[1]]) as usize).min(d.len() - 2);
        let pkt = &d[2..2 + n];
        let _ = (Ccreq::decode(pkt), Ccrep::decode(pkt), qw::oob_payload(pkt));
        let _ = c.handle_packet(pkt, t);
        let _ = c.take_outgoing();
        let _ = nc.process(pkt, t);
        let _ = nc.transmit(b"x");
        let _ = (qw::map_checksum2(pkt), qw::crc_block(pkt));
        t += 0.6;
        d = &d[2 + n..];
    }
    let s = String::from_utf8_lossy(data);
    let mut info = s.to_string();
    let _ = qw::info_value(&s, "name");
    qw::info_set(&mut info, "name", "\\x");
});
