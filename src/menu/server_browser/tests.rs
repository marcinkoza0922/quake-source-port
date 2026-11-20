use super::*;
use crate::net::browser::ServerProto;

fn commands(acts: Vec<Action>) -> String {
    acts.into_iter().filter_map(|a| match a {
        Action::Command(command) => Some(command),
        _ => None,
    }).collect()
}

fn entry(port: u16, name: &str, protocol: ServerProto, players: u8, ping: u32) -> ServerEntry {
    ServerEntry {
        address: format!("192.0.2.1:{port}"),
        hostname: name.into(),
        map: "dm4".into(),
        max_players: 8,
        players,
        ping_ms: Some(ping),
        protocol,
        ..Default::default()
    }
}

#[test]
fn controller_can_browse_filter_and_join_across_tabs() {
    let mut m = Menu::new();
    let mut cv = Cvars::default();
    cv.register("_mp_address", "", 0);
    m.open_multiplayer("join");
    assert_eq!(m.page_len(Page::Join), 3);
    m.key(K_DPAD_DOWN, &mut cv, false);
    m.key(K_DPAD_DOWN, &mut cv, false);
    assert_eq!(commands(m.key(K_ABUTTON, &mut cv, false)), "slist\n");
    assert_eq!(m.screen, Screen::ServerBrowser);
    m.set_server_search_status(ServerTab::Lan, SearchStatus::Searching);
    assert!(commands(m.key(K_XBUTTON, &mut cv, false)).is_empty());

    let mut saved = SavedServers::default();
    saved.record_recent("192.0.2.2:27500", true, None);
    m.replace_server_results(ServerTab::Lan, vec![entry(26000, "Local", ServerProto::NetQuake, 1, 4)], &saved);
    assert_eq!(commands(m.key(K_RSHOULDER, &mut cv, false)), "slist_internet\n");
    m.replace_server_results(ServerTab::Internet, vec![
        entry(27500, "QW active", ServerProto::QuakeWorld, 3, 51),
        entry(27501, "QW empty", ServerProto::QuakeWorld, 0, 99),
        entry(26001, "NQ empty", ServerProto::NetQuake, 0, 70),
        entry(27502, "QW nearby", ServerProto::QuakeWorld, 0, 49),
        entry(27503, "QW distant", ServerProto::QuakeWorld, 0, 100),
    ], &saved);
    m.set_server_search_status(ServerTab::Internet, SearchStatus::Complete);

    // Internet ping and population filters combine with protocol selection.
    m.key(K_LTRIGGER, &mut cv, false);
    m.key(K_DPAD_RIGHT, &mut cv, false);
    m.key(K_DPAD_RIGHT, &mut cv, false);
    // Below 100 ms includes the 49 ms server and excludes 100 ms and unknown.
    assert_eq!(m.server_browser.current().visible, vec![3, 0, 2, 1]);
    m.key(K_DPAD_DOWN, &mut cv, false);
    m.key(K_ABUTTON, &mut cv, false);
    m.key(K_ABUTTON, &mut cv, false);
    m.key(K_DPAD_DOWN, &mut cv, false);
    m.key(K_ABUTTON, &mut cv, false);
    assert_eq!(m.server_browser.current().visible, vec![3, 1]);
    m.key(K_BBUTTON, &mut cv, false);
    assert_eq!(m.screen, Screen::ServerBrowser);

    m.key(K_YBUTTON, &mut cv, false);
    assert!(m.osk_open());
    m.key(K_ABUTTON, &mut cv, false); // OSK types '1' and narrows by address
    assert_eq!(m.server_browser.current().filter.query, "1");
    m.key(K_XBUTTON, &mut cv, false);
    m.key(K_BBUTTON, &mut cv, false);
    assert!(m.server_browser.current().filter.query.is_empty());
    m.key(K_RTRIGGER, &mut cv, false);
    assert_eq!(m.server_browser.current().filter.sort, 1);

    assert!(commands(m.key(K_LSHOULDER, &mut cv, false)).is_empty());
    assert_eq!(m.server_browser.current().selected().unwrap().hostname, "Local");
    assert!(commands(m.key(K_RSHOULDER, &mut cv, false)).is_empty());
    m.key(K_DPAD_DOWN, &mut cv, false);
    assert_eq!(commands(m.key(K_RTHUMB, &mut cv, false)), "server_fav \"qw://192.0.2.1:27501\"\n");
    assert_eq!(commands(m.key(K_ABUTTON, &mut cv, false)), "connect qw://192.0.2.1:27501\n");
    assert_eq!(m.screen, Screen::None);
}

#[test]
fn mouse_button_and_list_use_visible_hit_targets() {
    let mut m = Menu::new();
    let mut cv = Cvars::default();
    m.open_multiplayer("join");
    let row = m.page_row_y(Page::Join, 2);
    m.mouse_move((PANEL_X + 3.0, row + 2.0), &mut cv);
    assert!(commands(m.key(K_MOUSE1, &mut cv, false)).is_empty());
    m.mouse_move((320.0, row + 2.0), &mut cv);
    assert_eq!(commands(m.key(K_MOUSE1, &mut cv, false)), "slist\n");

    let servers: Vec<_> = (0..30).map(|n| entry(26000 + n, &format!("Local {n}"), ServerProto::NetQuake, 0, n.into())).collect();
    m.replace_server_results(ServerTab::Lan, servers.clone(), &SavedServers::default());
    m.key(K_END, &mut cv, false);
    assert_eq!(m.server_browser.current().scroll, 30 - ROWS);
    let mut reversed = servers;
    reversed.reverse();
    m.replace_server_results(ServerTab::Lan, reversed, &SavedServers::default());
    assert_eq!(m.server_browser.current().selected().unwrap().hostname, "Local 29");

    let (x, _, _) = m.server_browser.current().scrollbar().unwrap();
    m.mouse_move((x, TOP + 1.0), &mut cv);
    m.key(K_MOUSE1, &mut cv, false);
    m.mouse_up();
    assert_eq!(m.server_browser.current().scroll, 0);
    m.mouse_move((X + 10.0, TOP + 1.0), &mut cv);
    m.key(K_MOUSE1, &mut cv, false);
    assert_eq!(m.server_browser.current().selected().unwrap().hostname, "Local 0");
    m.key(K_BBUTTON, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::Join));
    assert_eq!(m.opt_cursor, 2);
}
