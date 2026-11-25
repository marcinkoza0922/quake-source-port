use super::*;
use qclient::ClientState;
use qnet::SizeBuf;

#[test]
fn qwd_recording_and_playback_round_trip() {
    let mut qw = QwState::new();
    qw.servercount = 7;
    qw.gamedir = "qw".into();
    qw.playernum = 0;
    qw.levelname = "DM4".into();
    qw.sound_names = vec![String::new(), "weapons/r_exp3.wav".into()];
    qw.model_names = vec![String::new(), "maps/dm4.bsp".into(), "progs/player.mdl".into()];
    qw.playerindex = 2;
    qw.players[0].name = "Ranger".into();
    qw.players[0].userinfo = "\\name\\Ranger\\topcolor\\4\\bottomcolor\\13".into();
    qw.players[0].frags = 12;
    qw.stats[qnet::protocol::STAT_HEALTH] = 100;

    let mut qwd = qw.encode_qwd_header(10.0, 1);
    let ucmd = UserCmd { msec: 14, angles: [5.0, 90.0, 0.0], forwardmove: 200, ..Default::default() };
    qwd.extend(qclient::qw::demo::encode_qwd_cmd(10.05, &ucmd, [5.0, 90.0, 0.0]));

    // Build one live QW frame packet (netchan header + svc_playerinfo + svc_packetentities).
    let mut pkt = Vec::new();
    pkt.extend(2u32.to_le_bytes());
    pkt.extend(1u32.to_le_bytes());
    let mut sb = SizeBuf::new(256);
    sb.byte(SVC_PLAYERINFO as i32);
    sb.byte(0);
    sb.short(0);
    for c in [100.0f32, 200.0, 30.0] {
        write_coord(&mut sb, c);
    }
    sb.byte(1);
    sb.byte(SVC_PACKETENTITIES as i32);
    sb.short(0);
    pkt.extend_from_slice(&sb.data);
    qwd.extend(qclient::qw::demo::encode_qwd_read(10.05, &pkt));
    qwd.extend(qclient::qw::demo::encode_qwd_end(10.10));

    assert_eq!(detect_format(&qwd), Some(QwDemoFormat::Qwd));
    let mut demo = crate::demo::Demo::new(qwd);
    let mut cl = ClientState::new();
    cl.demoplayback = true;
    while let Some(m) = demo.next_message() {
        cl.mviewangles[0] = m.angles;
        cl.parse_server_message(m.data).unwrap();
    }
    assert_eq!(cl.signon, 4);
    assert_eq!(cl.mapname, "dm4");
    assert_eq!(cl.stats[qnet::protocol::STAT_HEALTH], 100);
    assert_eq!(cl.scores[0].name, "Ranger");
    assert_eq!(cl.scores[0].frags, 12);
}

#[test]
fn mvd_playback_and_pov_switching() {
    let mut mvd = Vec::new();
    let push_mvd = |mvd: &mut Vec<u8>, msec: u8, cmd: u8, payload: &[u8]| {
        mvd.push(msec);
        mvd.push(cmd);
        mvd.extend((payload.len() as i32).to_le_bytes());
        mvd.extend_from_slice(payload);
    };

    // 1. MVD serverdata + soundlist + modellist + userinfo for 2 players + skins.
    let mut sb = SizeBuf::new(512);
    sb.byte(SVC_SERVERDATA as i32);
    sb.long(PROTOCOL_VERSION_MVD1);
    sb.long(MVD_PEXT1_HIDDEN_MESSAGES as i32);
    sb.long(PROTOCOL_VERSION);
    sb.long(1);
    sb.string(b"qw");
    sb.float(1.0); // MVD float gametime
    sb.string(b"Aerowalk");
    for f in [800.0, 100.0, 320.0, 500.0, 10.0, 0.7, 10.0, 4.0, 1.0, 1.0] {
        sb.float(f);
    }
    sb.byte(SVC_SOUNDLIST as i32);
    sb.byte(0);
    sb.string(b"weapons/ax1.wav");
    sb.byte(0);
    sb.byte(0);
    sb.byte(SVC_MODELLIST as i32);
    sb.byte(0);
    sb.string(b"maps/aerowalk.bsp");
    sb.string(b"progs/player.mdl");
    sb.byte(0);
    sb.byte(0);
    push_mvd(&mut mvd, 0, DEM_ALL, &sb.data);

    sb.clear();
    sb.byte(SVC_STUFFTEXT as i32);
    sb.string(b"cmd spawn 1 0\n");
    for (slot, name, frags) in [(0, "Milton", 15), (1, "bps", 11)] {
        sb.byte(SVC_UPDATEFRAGS as i32);
        sb.byte(slot);
        sb.short(frags);
        sb.byte(SVC_UPDATEUSERINFO as i32);
        sb.byte(slot);
        sb.long(slot + 1);
        let ui = format!("\\name\\{name}\\topcolor\\4\\bottomcolor\\4");
        sb.string(ui.as_bytes());
    }
    push_mvd(&mut mvd, 0, DEM_ALL, &sb.data);

    // Stats for player 0 (health 150) and player 1 (health 80).
    for (slot, hp) in [(0u8, 150i32), (1u8, 80i32)] {
        sb.clear();
        sb.byte(SVC_UPDATESTATLONG as i32);
        sb.byte(qnet::protocol::STAT_HEALTH as i32);
        sb.long(hp);
        push_mvd(&mut mvd, 0, DEM_STATS | (slot << 3), &sb.data);
    }

    // Skins + first frame entities and MVD DF_* playerinfo for both players.
    sb.clear();
    sb.byte(SVC_STUFFTEXT as i32);
    sb.string(b"skins\n");
    sb.byte(SVC_PACKETENTITIES as i32);
    sb.short(0);
    push_mvd(&mut mvd, 20, DEM_ALL, &sb.data);

    sb.clear();
    for (slot, x, yaw) in [(0, 128.0f32, 45.0f32), (1, -256.0f32, 135.0f32)] {
        sb.byte(SVC_PLAYERINFO as i32);
        sb.byte(slot);
        let flags = DF_ORIGIN | (DF_ORIGIN << 1) | (DF_ORIGIN << 2) | (DF_ANGLES << 1) | DF_MODEL;
        sb.short(flags as i32);
        sb.byte(0); // frame
        write_coord(&mut sb, x);
        write_coord(&mut sb, 0.0);
        write_coord(&mut sb, 24.0);
        write_angle16(&mut sb, yaw);
        sb.byte(2); // modelindex
    }
    push_mvd(&mut mvd, 0, DEM_ALL, &sb.data);

    assert_eq!(detect_format(&mvd), Some(QwDemoFormat::Mvd));
    // one that grows too much ends early
    let mut long = mvd.clone();
    for _ in 0..2000 {
        push_mvd(&mut long, 10, DEM_ALL, &sb.data);
    }
    let all = transcode_mvd(long.clone().into(), None).unwrap().0.len();
    let capped = transcode_mvd_within(long.into(), None, 20_000).unwrap().0.len();
    assert!(all > 50_000 && (20_000..30_000).contains(&capped), "{all} {capped}");
    let mut demo = crate::demo::Demo::new(mvd);
    assert!(demo.is_mvd());
    assert_eq!(demo.mvd_pov_name(), Some("Milton"));
    assert_eq!(demo.mvd_players().len(), 2);

    let mut cl = ClientState::new();
    cl.demoplayback = true;
    while let Some(m) = demo.next_message() {
        cl.mviewangles[0] = m.angles;
        cl.parse_server_message(m.data).unwrap();
    }
    assert_eq!(cl.signon, 4);
    assert_eq!(cl.viewentity, 1);
    assert_eq!(cl.stats[qnet::protocol::STAT_HEALTH], 150);
    assert!((cl.mviewangles[0][1] - 45.0).abs() < 0.1);

    // Switch POV to player slot 1 ("bps"): health becomes 80, viewentity becomes 2, yaw becomes 135.
    assert!(demo.set_mvd_pov(1));
    assert_eq!(demo.mvd_pov_name(), Some("bps"));
    let mut cl2 = ClientState::new();
    cl2.demoplayback = true;
    while let Some(m) = demo.next_message() {
        cl2.mviewangles[0] = m.angles;
        cl2.parse_server_message(m.data).unwrap();
    }
    assert_eq!(cl2.viewentity, 2);
    assert_eq!(cl2.stats[qnet::protocol::STAT_HEALTH], 80);
    assert!((cl2.mviewangles[0][1] - 135.0).abs() < 0.1);
}
