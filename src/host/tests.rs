//! Small host unit tests.

mod menu_controller;

mod relaunch_tests {
    #[test]
    fn strips_game_and_commands() {
        let v = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let out = super::super::mapswitch::relaunch_args(&v("-basedir /q -hipnotic -game foo +map e1m1 +skill 3 -width 800 -height 600"));
        assert_eq!(out, v("-basedir /q -width 800 -height 600"));
        assert_eq!(super::super::mapswitch::relaunch_args(&v("+delay 3 screenshot x -window")), v("-window"));
    }
}

mod pad_default_tests {
    #[test]
    fn pad_defaults_parse() {
        use qinput::keys::*;
        let b = super::super::pad_default_binds();
        assert!(b.contains(&(K_RTRIGGER, "+attack".into())));
        assert!(b.contains(&(K_XBUTTON, "+weaponwheel".into())));
        assert!(b.contains(&(K_LSHOULDER, "impulse 12".into())));
        assert!(b.contains(&(b'9' as i32, "weapon_slot 9".into())));
        assert!(b.contains(&(b'0' as i32, "weapon_slot 0".into())));
        assert_eq!(b.len(), 13);
    }
}
