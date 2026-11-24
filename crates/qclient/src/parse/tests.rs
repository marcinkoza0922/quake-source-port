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
