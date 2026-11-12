use super::*;
use crate::{ClientEvent, ClientState};

fn w_coord(b: &mut SizeBuf, v: [f32; 3]) {
    for x in v {
        write_coord(b, x);
    }
}

fn serverdata(b: &mut SizeBuf, playernum: u8) {
    b.byte(SVC_SERVERDATA as i32);
    b.long(PROTOCOL_VERSION);
    b.long(7);
    b.string(b"qw");
    b.byte(playernum as i32);
    b.string(b"The Test Level");
    for f in [800.0, 100.0, 320.0, 500.0, 10.0, 0.7, 10.0, 4.0, 1.0, 1.0] {
        b.float(f);
    }
}

fn list(b: &mut SizeBuf, cmd: u8, names: &[&str]) {
    b.byte(cmd as i32);
    b.byte(0);
    for n in names {
        b.string(n.as_bytes());
    }
    b.string(b"");
    b.byte(0);
}

fn stuff(b: &mut SizeBuf, s: &str) {
    b.byte(SVC_STUFFTEXT as i32);
    b.string(s.as_bytes());
}

fn playerinfo(b: &mut SizeBuf, num: u8, origin: [f32; 3], vel_x: i16) {
    b.byte(SVC_PLAYERINFO as i32);
    b.byte(num as i32);
    b.short((PF_COMMAND | PF_VELOCITY1) as i32);
    w_coord(b, origin);
    b.byte(3); // frame
    write_delta_usercmd(b, &UserCmd::default(), &UserCmd { angles: [0.0, 90.0, 0.0], ..Default::default() });
    b.short(vel_x as i32);
}

/// Full update: entity 40 (model 3 = "*1") and entity 41 (model 2).
fn packet_entities(b: &mut SizeBuf, x40: f32) {
    b.byte(SVC_PACKETENTITIES as i32);
    b.short((40 | U_MOREBITS | U_ORIGIN1) as i32);
    b.byte(U_MODEL as i32);
    b.byte(3);
    write_coord(b, x40);
    b.short(41 | (U_MOREBITS | U_FRAME) as i32);
    b.byte(U_MODEL as i32);
    b.byte(2);
    b.byte(5);
    b.short(0);
}

struct Harness {
    qw: QwState,
    cl: ClientState,
    seq: u32,
    cmds: Vec<String>,
}

impl Harness {
    fn new() -> Self {
        Self { qw: QwState::new(), cl: ClientState::new(), seq: 0, cmds: Vec::new() }
    }
    /// Deliver one server message as if it answered our latest command.
    fn feed(&mut self, b: &SizeBuf, t: f64) -> Vec<QwAction> {
        self.seq += 1;
        let ack = self.qw.outgoing_sequence.saturating_sub(1);
        let p = self.qw.parse(&b.data, ack, self.seq, t).unwrap();
        for m in &p.nq {
            self.cl.parse_server_message(m).unwrap();
        }
        for a in &p.actions {
            if let QwAction::Cmd(c) = a {
                self.cmds.push(c.clone());
            }
        }
        p.actions
    }
    fn send_cmd(&mut self, t: f64) -> Vec<u8> {
        self.qw.build_cmd(UserCmd { msec: 13, forwardmove: 400, ..Default::default() }, 0, t)
    }
}

/// The signon handshake and a first frame, through to a live NQ client.
#[test]
fn signon_to_active_frame() {
    let mut h = Harness::new();
    let mut b = SizeBuf::new(4096);
    serverdata(&mut b, 2);
    stuff(&mut b, "fullserverinfo \"\\deathmatch\\1\\maxclients\\8\"\n");
    h.feed(&b, 0.0);
    assert_eq!(h.cmds, ["soundlist 7 0"]);
    assert!(h.qw.deathmatch());

    let mut b = SizeBuf::new(4096);
    list(&mut b, SVC_SOUNDLIST, &["weapons/r_exp3.wav"]);
    h.feed(&b, 0.0);
    assert_eq!(h.cmds.last().unwrap(), "modellist 7 0");

    let mut b = SizeBuf::new(4096);
    list(&mut b, SVC_MODELLIST, &["maps/test.bsp", "progs/player.mdl", "*1"]);
    let a = h.feed(&b, 0.0);
    assert_eq!(a, [QwAction::Precache]);
    assert_eq!(h.qw.playerindex, 2);
    assert_eq!(h.qw.needed_files(), ["maps/test.bsp", "progs/player.mdl", "sound/weapons/r_exp3.wav"]);
    let (msg, cmd) = h.qw.finish_precache(0xdead_beef);
    assert_eq!(cmd, format!("prespawn 7 0 {}", 0xdead_beefu32 as i32));
    h.cl.parse_server_message(&msg).unwrap();
    assert_eq!((h.cl.signon, h.cl.maxclients, h.cl.viewentity), (1, 32, 3));
    assert_eq!(h.cl.mapname, "test");
    assert_eq!(h.cl.gametype, nq::GAME_DEATHMATCH);
    assert!(h.cl.take_events().contains(&ClientEvent::NewMap));

    // Prespawn data, then spawn and skins.
    let mut b = SizeBuf::new(4096);
    b.byte(SVC_SPAWNBASELINE as i32);
    b.short(41);
    for v in [2, 0, 0, 0] {
        b.byte(v);
    }
    for _ in 0..3 {
        write_coord(&mut b, 64.0);
        write_angle(&mut b, 0.0);
    }
    stuff(&mut b, "cmd spawn 7 0\n");
    h.feed(&b, 0.1);
    assert_eq!(h.cl.signon, 2);
    assert_eq!(h.cmds.last().unwrap(), "spawn 7 0");
    let mut b = SizeBuf::new(4096);
    b.byte(SVC_UPDATEUSERINFO as i32);
    b.byte(2);
    b.long(99);
    b.string(b"\\name\\alice\\topcolor\\4\\bottomcolor\\12");
    b.byte(SVC_UPDATESTAT as i32);
    b.byte(nq::STAT_HEALTH as i32);
    b.byte(100);
    stuff(&mut b, "skins\n");
    h.feed(&b, 0.2);
    assert_eq!(h.cl.signon, 3);
    assert_eq!(h.cmds.last().unwrap(), "begin 7");
    assert_eq!(h.cl.scores[2].name, "alice");
    assert_eq!(h.cl.scores[2].colors, 4 << 4 | 12);

    // First frame answers command 2 (sequence 0 means "no frame" in QW).
    for _ in 0..3 {
        h.send_cmd(0.25);
    }
    let mut b = SizeBuf::new(4096);
    playerinfo(&mut b, 2, [10.0, 20.0, 30.0], 320);
    playerinfo(&mut b, 5, [100.0, 0.0, 0.0], 0);
    packet_entities(&mut b, 128.0);
    h.feed(&b, 0.3);
    assert_eq!(h.qw.stage, Stage::Active);
    assert_eq!(h.cl.signon, 4);
    assert_eq!(h.cl.stats[nq::STAT_HEALTH], 100);
    assert_eq!(h.cl.mvelocity[0][0], 320.0);
    h.cl.frame(1.0);
    let me = &h.cl.entities[3];
    assert_eq!(me.origin, [10.0, 20.0, 30.0]);
    let other = &h.cl.entities[6];
    assert_eq!((other.model, other.colormap, other.frame), (2, 6, 3));
    assert!((other.angles[1] - 90.0).abs() < 0.01);
    assert_eq!(h.cl.entities[40].origin[0], 128.0);
    assert_eq!(h.cl.entities[41].model, 2);
    assert_eq!(h.cl.entities[41].origin, [64.0; 3]);
    assert_eq!(h.cl.entities[41].frame, 5);
    assert!(h.cl.visedicts.contains(&6) && !h.cl.visedicts.contains(&3));

    // Delta from that frame: move 40, remove 41.
    let msg = h.send_cmd(0.3);
    assert_eq!(&msg[msg.len() - 2..], [CLC_DELTA, 2]);
    let mut b = SizeBuf::new(4096);
    playerinfo(&mut b, 2, [12.0, 20.0, 30.0], 320);
    b.byte(SVC_DELTAPACKETENTITIES as i32);
    b.byte(2);
    b.short((40 | U_ORIGIN1) as i32);
    write_coord(&mut b, 140.0);
    b.short((41 | U_REMOVE) as i32);
    b.short(0);
    h.feed(&b, 0.35);
    h.cl.frame(1.0);
    assert_eq!(h.cl.entities[40].origin[0], 140.0);
    assert_eq!(h.cl.entities[41].model, 0);
    assert_eq!(h.qw.validsequence, 3);
}

#[test]
fn move_checksum_and_deltas() {
    let mut qw = QwState::new();
    let a = qw.build_cmd(UserCmd { msec: 10, forwardmove: 200, ..Default::default() }, 0, 0.0);
    let b = qw.build_cmd(UserCmd { msec: 10, forwardmove: 200, buttons: 2, ..Default::default() }, 0, 0.0);
    assert_eq!(a[0], CLC_MOVE);
    assert_eq!(a[1], sequence_crc_byte(&a[2..], 0));
    assert_eq!(b[1], sequence_crc_byte(&b[2..], 1));
    // Three delta-coded commands: oldest, previous, newest.
    let mut m = MsgReader::new(&b[3..]);
    let c0 = read_delta_usercmd(&mut m, &UserCmd::default());
    let c1 = read_delta_usercmd(&mut m, &c0);
    let c2 = read_delta_usercmd(&mut m, &c1);
    assert!(m.at_end());
    assert_eq!((c0.forwardmove, c1.forwardmove, c2.buttons), (0, 200, 2));
    assert_eq!(qw.outgoing_sequence, 2);
}

#[test]
fn temp_entities_and_prints() {
    let mut h = Harness::new();
    h.qw.nq_ready = true;
    h.cl.protocol = nq::PROTOCOL_RMQ;
    h.cl.protocolflags = NQ_FLAGS;
    let mut b = SizeBuf::new(1024);
    b.byte(SVC_PRINT as i32);
    b.byte(PRINT_CHAT);
    b.string(b"bob: hi\n");
    b.byte(SVC_TEMP_ENTITY as i32);
    b.byte(TE_GUNSHOT as i32);
    b.byte(2);
    w_coord(&mut b, [1.0, 2.0, 3.0]);
    b.byte(SVC_TEMP_ENTITY as i32);
    b.byte(TE_EXPLOSION as i32);
    w_coord(&mut b, [1.0, 2.0, 3.0]);
    b.byte(SVC_TEMP_ENTITY as i32);
    b.byte(TE_LIGHTNING2 as i32);
    b.short(1);
    w_coord(&mut b, [0.0; 3]);
    w_coord(&mut b, [64.0, 0.0, 0.0]);
    stuff(&mut b, "bf\n");
    h.feed(&b, 0.0);
    let ev = h.cl.take_events();
    assert!(ev.contains(&ClientEvent::Print("\x01bob: hi\n".into())), "{ev:?}");
    assert!(ev.contains(&ClientEvent::StuffText("bf\n".into())));
    assert!(ev.iter().any(|e| matches!(e, ClientEvent::TempSound { name: "weapons/r_exp3.wav", .. })));
    assert!(!h.cl.particles.is_empty());
    assert_eq!(h.cl.beams.as_mut_slice().iter().filter(|b| b.entity == 1).count(), 1);
}

#[test]
fn nails_unpack() {
    let mut qw = QwState::new();
    let mut b = SizeBuf::new(64);
    // x = 100, y = -50, z = 8, pitch 4/16, yaw 64/256.
    let enc = |v: i32| ((v + 4096) >> 1) as u32;
    let (x, y, z) = (enc(100), enc(-50), enc(8));
    b.byte(1);
    b.byte((x & 255) as i32);
    b.byte(((x >> 8) | (y & 15) << 4) as i32);
    b.byte((y >> 4) as i32);
    b.byte((z & 255) as i32);
    b.byte(((z >> 8) | 4 << 4) as i32);
    b.byte(64);
    qw.parse_nails(&mut MsgReader::new(&b.data));
    assert_eq!(qw.nails, [([100.0, -50.0, 8.0], [90.0, 90.0, 0.0])]);
}
