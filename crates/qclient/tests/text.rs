use qclient::{ClientEvent, ClientState};
use qnet::protocol::{SVC_CENTERPRINT, SVC_CUTSCENE, SVC_FINALE, SVC_PRINT};

#[test]
fn messages_preserve_quake_font_indices() {
    // Includes colored letters/icons and a byte pair that is valid UTF-8.
    let bytes = b"\xa0\xd0\xe5\xee\xe4\xf2\xe1\xe7\xef\xee\xa0\n\x9e\x9d\x9d\xc3\xa9";
    for svc in [SVC_CENTERPRINT, SVC_FINALE, SVC_CUTSCENE, SVC_PRINT] {
        let mut cl = ClientState::default();
        let mut msg = vec![svc];
        msg.extend_from_slice(bytes);
        msg.push(0);
        cl.parse_server_message(&msg).unwrap();
        let events = cl.take_events();
        let text = events.iter().find_map(|e| match e {
            ClientEvent::CenterPrint(s) | ClientEvent::Print(s) => Some(s),
            _ => None,
        }).unwrap();
        assert_eq!(qcore::text::encode(text), bytes);
        if svc != SVC_PRINT {
            assert_eq!(qcore::text::encode(&cl.centerprint), bytes);
        }
    }
}

#[test]
fn qbj3_door_label_survives_entity_load_and_save_restore() {
    use qcore::{cvar::Cvars, fs::Fs};
    use qserver::Server;
    let base = Fs::default_basedir();
    if !base.join("qbj3/progs.dat").is_file() {
        return;
    }
    let mut fs = Fs::new(base);
    fs.add_game_dir("id1");
    fs.add_game_dir("qbj3");
    let bsp = qformats::Bsp::parse(&fs.open("maps/start.bsp").unwrap()).unwrap();
    let entities = qformats::parse_entities(&bsp.entities).unwrap();
    let label = entities.iter().filter_map(|e| e.get("message"))
        .find(|s| s.contains("Avg Playtime")).unwrap();
    let expected = qcore::text::encode(&label.replace("\\n", "\n"));
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.spawn_server("start").unwrap();
    assert!(sv.error().is_none(), "{:?}", sv.error());
    let l = sv.level.as_ref().unwrap();
    let ofs = l.vm.field_ofs("message").unwrap();
    let (entity, bytes) = (0..l.vm.num_edicts() as u32).find_map(|e| {
        let bytes = l.vm.ent_str(e, ofs).ok()?;
        bytes.windows(12).any(|w| w == b"Avg Playtime").then(|| (e, bytes.to_vec()))
    }).unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(&bytes[..11], b"\xa0\xd0\xe5\xee\xe4\xf2\xe1\xe7\xef\xee\xa0");
    assert!(bytes.ends_with(b"\x9e\x9d\x9d"));
    assert!(label.contains("\\n"));
    let mut cl = ClientState::default();
    let mut msg = vec![SVC_CENTERPRINT];
    msg.extend_from_slice(&bytes);
    msg.push(0);
    cl.parse_server_message(&msg).unwrap();
    assert_eq!(qcore::text::encode(&cl.centerprint), bytes);
    let saved = qcore::text::encode(&sv.save_game());
    sv.load_game(&qcore::text::decode(&saved)).unwrap();
    let l = sv.level.as_ref().unwrap();
    assert_eq!(l.vm.ent_str(entity, ofs).unwrap(), bytes);
}

#[test]
fn hit_feedback_comments_become_events_without_commands() {
    use qnet::protocol::SVC_STUFFTEXT;
    let mut cl = ClientState::new();
    // Split a line across packets: no feedback until its newline arrives.
    let mut first = b"\x09//qhit \0".to_vec();
    first.resize(512, 1); // svc_nop: large packet retains a partial line
    cl.parse_server_message(&first).unwrap();
    assert!(cl.take_events().is_empty());
    let mut msg = vec![SVC_STUFFTEXT];
    msg.extend_from_slice(b"0\n//qhit 1\nbf\n\0");
    cl.parse_server_message(&msg).unwrap();
    assert_eq!(cl.take_events(), vec![
        ClientEvent::HitFeedback { kill: false },
        ClientEvent::HitFeedback { kill: true },
        ClientEvent::StuffText("bf\n".into()),
    ]);
}

#[test]
fn portal_feedback_preserves_owner_and_never_executes_as_commands() {
    let mut cl = ClientState::new();
    let mut msg = vec![qnet::protocol::SVC_STUFFTEXT];
    msg.extend_from_slice(b"//qsp_portalfx 3 1 1 1 2 3 4 5 6\n//qsp_portalfx 1 7 0 1 2 3 4 5 6\n\0");
    cl.parse_server_message(&msg).unwrap();
    assert_eq!(cl.take_events(),vec![ClientEvent::PortalEffect(qclient::PortalEffect {
        owner: 3, slot: 1, kind: 1, start: [1.0,2.0,3.0], end: [4.0,5.0,6.0],
    })]);
}
