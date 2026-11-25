use super::*;

#[test]
fn level_damage_latches_on_one_point_and_resets_at_serverinfo() {
    let mut cl = ClientState::new();
    cl.parse_server_message(&[SVC_DAMAGE, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    assert!(!cl.took_damage);
    cl.parse_server_message(&[SVC_DAMAGE, 1, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    assert!(cl.took_damage);
    cl.events.clear();
    cl.parse_server_message(&[SVC_NOP]).unwrap();
    assert!(cl.took_damage);

    let mut map = qnet::msg::SizeBuf::new(128);
    map.byte(SVC_SERVERINFO.into());
    map.long(PROTOCOL_NETQUAKE as i32);
    map.byte(1);
    map.byte(0);
    map.string(b"Test");
    map.string(b"maps/e1m1.bsp");
    map.string(b"");
    map.string(b"");
    cl.parse_server_message(&map.data).unwrap();
    assert!(!cl.took_damage);
    cl.parse_server_message(&[SVC_DAMAGE, 0, 1, 0, 0, 0, 0, 0, 0]).unwrap();
    assert!(cl.took_damage);
}

#[test]
fn st_and_sts_stufftext_set_stats_without_reaching_the_console() {
    let mut cl = ClientState::new();
    cl.items = 5;
    let mut m = vec![SVC_STUFFTEXT];
    m.extend_from_slice(b"//st 15 0\n//st 40 256 41 1.5\n//sts 50 \"two words\"\n//st 999 1\n//paknames x\n//map e1m1\nbf\n\0");
    cl.parse_server_message(&m).unwrap();
    assert_eq!((cl.stats[40], cl.statsf[40]), (256, 256.0));
    assert_eq!((cl.stats[41], cl.statsf[41]), (1, 1.5));
    assert_eq!(cl.statss[50], "two words");
    assert_eq!(cl.items, 5, "CL_SetStat_f leaves the items alone");
    let stuffed: Vec<_> = cl.take_events().into_iter().filter(|e| matches!(e, ClientEvent::StuffText(_))).collect();
    assert_eq!(stuffed, [ClientEvent::StuffText("bf\n".into())]);
}

#[test]
fn killed_monster_and_found_secret_update_float_stats() {
    let mut cl = ClientState::new();
    cl.parse_server_message(&[SVC_KILLEDMONSTER, SVC_KILLEDMONSTER, SVC_FOUNDSECRET]).unwrap();
    assert_eq!(cl.stats[STAT_MONSTERS], 2);
    assert_eq!(cl.statsf[STAT_MONSTERS], 2.0);
    assert_eq!(cl.stats[STAT_SECRETS], 1);
    assert_eq!(cl.statsf[STAT_SECRETS], 1.0);
}

#[test]
fn v_water_forces_underwater_until_told_otherwise() {
    let mut cl = ClientState::new();
    let stuff = |s: &str| [&[SVC_STUFFTEXT][..], s.as_bytes(), &[0]].concat();
    cl.parse_server_message(&stuff("//v_water 1\n")).unwrap();
    assert!(cl.force_underwater);
    cl.parse_server_message(&stuff("//v_water 0\n")).unwrap();
    assert!(!cl.force_underwater);
    cl.force_underwater = true;
    cl.clear_state();
    assert!(!cl.force_underwater);
    assert!(!cl.take_events().iter().any(|e| matches!(e, ClientEvent::StuffText(_))));
}

#[test]
fn names_and_titles_keep_their_bytes() {
    let mut cl = ClientState::new();
    let mut m = qnet::msg::SizeBuf::new(256);
    m.byte(SVC_SERVERINFO.into());
    m.long(PROTOCOL_NETQUAKE as i32);
    m.byte(2);
    m.byte(0);
    m.string(b"\xd4itle");
    m.string(b"maps/e1m1.bsp");
    m.string(b"");
    m.string(b"");
    m.byte(SVC_UPDATENAME.into());
    m.byte(1);
    m.string(b"\xc3\xef\xec");
    cl.parse_server_message(&m.data).unwrap();
    assert_eq!(qcore::text::wire(&cl.levelname), b"\xd4itle");
    assert_eq!(qcore::text::wire(&cl.scores[1].name), b"\xc3\xef\xec");
}

#[test]
fn another_machines_server_has_no_add_ons() {
    let lines = b"//qsp_portal 0 0 1 2 3 0 0 1 1 0 0\n//qsp_gun 0 10 0 1 0 0 1 2 3 4 5 6 0 0 1\nbf\n\0";
    let m = [&[SVC_STUFFTEXT][..], lines].concat();
    for foreign in [false, true] {
        let mut cl = ClientState::new();
        cl.foreign = foreign;
        cl.clear_state(); // as serverinfo does
        cl.parse_server_message(&m).unwrap();
        assert_eq!(cl.portals.is_empty(), foreign);
        let gun = cl.take_events().iter().any(|e| matches!(e, ClientEvent::Gun(_)));
        assert_eq!(gun, !foreign);
    }
}
