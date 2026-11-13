//! Separate cue editors keep hit and kill sounds independently adjustable.
use super::*;

const SOUNDS: &[(&str, f32)] = &[
    ("Click", 0.0), ("Tick", 1.0), ("Pop", 2.0), ("Blip", 3.0), ("Chime", 4.0),
    ("Clink", 5.0), ("Thud", 6.0), ("Knock", 7.0), ("Coin", 8.0),
];

pub(super) const HIT_SOUND: &[Item] = &[
    Item::Choice { label: "Sound", cvar: "s_hit_kind", choices: SOUNDS },
    Item::Run { label: "Preview Sound", run: Run::PreviewHit },
    slider("Volume", "s_hit_volume", 0.0, 1.0, 0.05),
    slider("Time Stretch", "s_hit_length", 0.25, 3.0, 0.25),
    slider("Playback Speed", "s_hit_speed", 0.5, 2.0, 0.1),
    slider("Pitch", "s_hit_pitch", 0.5, 2.0, 0.1),
    slider("Bass", "s_hit_bass", -12.0, 12.0, 1.0),
    slider("Treble", "s_hit_treble", -12.0, 12.0, 1.0),
    slider("Drive", "s_hit_drive", 0.0, 1.0, 0.1),
    slider("Echo", "s_hit_echo", 0.0, 1.0, 0.1),
    slider("Reverb", "s_hit_reverb", 0.0, 1.0, 0.1),
];

pub(super) const KILL_SOUND: &[Item] = &[
    Item::Choice { label: "Sound", cvar: "s_kill_kind", choices: SOUNDS },
    Item::Run { label: "Preview Sound", run: Run::PreviewKill },
    slider("Volume", "s_kill_volume", 0.0, 1.0, 0.05),
    slider("Time Stretch", "s_kill_length", 0.25, 3.0, 0.25),
    slider("Playback Speed", "s_kill_speed", 0.5, 2.0, 0.1),
    slider("Pitch", "s_kill_pitch", 0.5, 2.0, 0.1),
    slider("Bass", "s_kill_bass", -12.0, 12.0, 1.0),
    slider("Treble", "s_kill_treble", -12.0, 12.0, 1.0),
    slider("Drive", "s_kill_drive", 0.0, 1.0, 0.1),
    slider("Echo", "s_kill_echo", 0.0, 1.0, 0.1),
    slider("Reverb", "s_kill_reverb", 0.0, 1.0, 0.1),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_editors_return_to_audio_and_preview_without_enabling_feedback() {
        let mut cv = Cvars::default();
        cv.register("s_hitfeedback", "0", 0);
        for (page, command) in [(Page::HitSound, "hitfeedback_preview hit\n"), (Page::KillSound, "hitfeedback_preview kill\n")] {
            let mut menu = Menu::new();
            menu.open(Screen::OptionsPage(Page::Audio));
            let row = AUDIO.iter().position(|i| matches!(i, Item::Sub { page: p, .. } if *p == page)).unwrap();
            menu.opt_cursor = row;
            menu.key(K_ENTER, &mut cv, false);
            assert_eq!(menu.screen, Screen::OptionsPage(page));
            menu.opt_cursor = 1;
            let actions = menu.key(K_ENTER, &mut cv, false);
            assert_eq!(actions.len(), 1);
            assert!(matches!(&actions[0], Action::Command(c) if c == command));
            assert_eq!(cv.value_of("s_hitfeedback"), 0.0);
            assert_eq!(menu.screen, Screen::OptionsPage(page));
            menu.key(K_ESCAPE, &mut cv, false);
            assert_eq!(menu.screen, Screen::OptionsPage(Page::Audio));
            assert_eq!(menu.opt_cursor, row);
            assert_eq!(parent_page(page).map(|p| p.0), Some(Page::Audio));
        }
    }

    #[test]
    fn cue_edits_are_independent_and_all_controls_are_archived() {
        let mut cv = Cvars::default();
        for name in option_cvars() { cv.register(name, "0", 0); }
        let mut menu = Menu::new();
        menu.open(Screen::OptionsPage(Page::HitSound));
        menu.opt_cursor = 2;
        menu.key(K_RIGHTARROW, &mut cv, false);
        assert_eq!(cv.value_of("s_hit_volume"), 0.05);
        assert_eq!(cv.value_of("s_kill_volume"), 0.0);
        for cue in ["hit", "kill"] {
            for field in ["kind", "volume", "length", "speed", "pitch", "bass", "treble", "drive", "echo", "reverb"] {
                let name = format!("s_{cue}_{field}");
                assert!(option_cvars().any(|c| c == name));
            }
        }
        assert!(option_cvars().any(|c| c == "s_hitfeedback"));
        assert!(option_cvars().any(|c| c == "hud_hitmarkers"));
    }
}
