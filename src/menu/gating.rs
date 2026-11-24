//! Rows that a setting switches off. A row with master toggles works only
//! while they are all on; when one of them is a row on the same page, the
//! row is drawn as that row's dependent (indented, own colour).

use super::*;

/// Master toggles that open a sub-page.
fn sub_masters(page: Page) -> &'static [&'static str] {
    match page {
        Page::AutoAim => &["assist_autoaim"],
        Page::GrenadeArc => &["assist_grenadearc"],
        Page::Autosave => &["sv_autosave"],
        Page::WeaponPriority => &["modifier_autoswitch"],
        Page::Automap => &["cl_automap"],
        Page::Water => &["r_enhancedwater"],
        Page::Bloom => &["r_bloom"],
        Page::RtLights => &["r_shadow_realtime_dlight"],
        Page::Impacts => &["r_enhancedfx"],
        Page::Blood => &["r_blood"],
        Page::Debris => &["r_debris"],
        Page::Goggles => &["addon_goggles"],
        Page::Hitstop | Page::HitstopAdvanced => &["addon_hitstop"],
        _ => &[],
    }
}

fn any_off(masters: &[&str], cv: &Cvars) -> bool {
    masters.iter().any(|m| cv.value_of(m) == 0.0)
}

/// Sub-pages that are switched off by their parent setting.
pub(super) fn sub_disabled(page: Page, cv: &Cvars) -> bool {
    // the order only matters to Better weapon
    any_off(sub_masters(page), cv) || (page == Page::WeaponPriority && cv.value_of("modifier_autoswitch") != 1.0)
}

/// The cvar a row sets, for rows that set one directly.
fn row_cvar(item: Item) -> Option<&'static str> {
    match item {
        Item::Slider { cvar, .. } | Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } | Item::Text { cvar, .. } => Some(cvar),
        _ => None,
    }
}

impl Menu {
    /// Toggles that must all be on for the row to work.
    pub(super) fn masters(&self, item: Item) -> &'static [&'static str] {
        match item {
            Item::Sub { page, .. } => sub_masters(page),
            Item::Slider { cvar: "r_deluxemapping_specular", .. } => &["r_deluxemapping"],
            Item::Slider { cvar: "r_motionblur_amount", .. } | Item::Choice { cvar: "r_motionblur_quality", .. } => &["r_motionblur"],
            Item::Slider { cvar: "r_waterripples_amount", .. } => &["r_waterripples"],
            Item::Slider { cvar: "r_watercaustics_amount", .. } => &["r_watercaustics"],
            Item::Choice { cvar: "r_particle_quality" | "r_impact_fx", .. }
            | Item::Toggle { cvar: "r_impact_bullets" | "r_impact_nails" | "r_impact_scorches", .. }
            | Item::Slider { cvar: "r_impact_lifetime" | "r_impact_limit", .. } => &["r_enhancedparticles"],
            Item::Slider { cvar: "r_wateralpha", .. } => &["r_enhancedwater", "r_watertranslucency"],
            Item::Slider { cvar: "r_water_reflections_amount", .. } => &["r_water_reflections"],
            Item::Slider { cvar: "s_reverb_amount", .. } => &["s_reverb"],
            Item::Slider { cvar: "s_footsteps_volume", .. } => &["s_footsteps"],
            Item::Choice { cvar: "r_portal_quality", .. } => match self.screen {
                Screen::OptionsPage(Page::Enhancements) => &["r_teleportwindows"],
                Screen::OptionsPage(Page::PortalGun) => &["addon_portal_view"],
                _ => &[],
            },
            Item::Choice { cvar: "addon_portal_max" | "addon_portal_depth", .. } | Item::Slider { cvar: "addon_portal_dist", .. } => {
                &["addon_portal_view"]
            }
            _ => &[],
        }
    }

    /// Shared by keyboard adjustment, pointer dragging and drawing. Sub rows
    /// are gated by `sub_disabled` instead.
    pub(super) fn item_disabled(&self, item: Item, cv: &Cvars) -> bool {
        self.game_locked(item, cv)
            || (matches!(item, Item::Run { run: Run::ExportClip, .. }) && cv.value_of("_export_hasclip") == 0.0)
            || (!matches!(item, Item::Sub { .. }) && any_off(self.masters(item), cv))
            // classic styles replaced by an enhanced one
            || (matches!(item, Item::Choice { cvar: "r_waterwarp", .. }) && cv.value_of("r_enhancedwater") != 0.0)
            || (matches!(item, Item::Choice { cvar: "r_particles", .. }) && cv.value_of("r_enhancedparticles") != 0.0)
            || (matches!(item, Item::Choice { cvar: "r_drawviewmodel", .. }) && cv.value_of("cl_camera") != 0.0)
    }

    /// Whether row `i` of `page` hangs off a master toggle shown on the same page.
    pub(super) fn is_dependent(&self, page: Page, i: usize) -> bool {
        let masters = self.masters(self.page_item(page, i));
        !masters.is_empty()
            && (0..self.page_len(page)).any(|k| row_cvar(self.page_item(page, k)).is_some_and(|c| masters.contains(&c)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(m: &Menu, page: Page, label: &str) -> usize {
        (0..m.page_len(page))
            .find(|&i| match m.page_item(page, i) {
                Item::Slider { label: l, .. } | Item::Toggle { label: l, .. } | Item::Choice { label: l, .. } | Item::Sub { label: l, .. } => l == label,
                _ => false,
            })
            .unwrap_or_else(|| panic!("no row {label}"))
    }

    #[test]
    fn rows_depend_on_a_master_toggle_shown_on_their_page() {
        let mut m = Menu::new();
        m.open(Screen::OptionsPage(Page::Enhancements));
        let p = Page::Enhancements;
        for dep in ["Motion Blur Quality", "Motion Blur Amount", "Bloom Settings", "Water Settings", "Portal View Quality", "Debris Settings"] {
            assert!(m.is_dependent(p, row(&m, p, dep)), "{dep} should be dependent");
        }
        for top in ["Motion Blur", "Speed Blur", "Screen Shader", "Ambient Occlusion"] {
            assert!(!m.is_dependent(p, row(&m, p, top)), "{top} should not be dependent");
        }
        // Water Opacity needs Enhanced Water too, but hangs off Translucent Water here
        m.open(Screen::OptionsPage(Page::Water));
        assert!(m.is_dependent(Page::Water, row(&m, Page::Water, "Water Opacity")));
        // the whole Hitstop page needs addon_hitstop, which is not a row on it
        m.open(Screen::OptionsPage(Page::Hitstop));
        assert!(!m.is_dependent(Page::Hitstop, row(&m, Page::Hitstop, "Advanced Settings")));
        // the classic particle style is replaced, not unlocked, by Enhanced Particles
        m.open(Screen::OptionsPage(Page::Graphics));
        assert!(!m.is_dependent(Page::Graphics, row(&m, Page::Graphics, "Particles")));
    }

    #[test]
    fn headings_are_skipped_by_cursor_and_pointer() {
        let mut cv = Cvars::default();
        let mut m = Menu::new();
        m.opt_cursor = 0;
        m.open(Screen::OptionsPage(Page::Enhancements));
        let p = Page::Enhancements;
        assert!(matches!(ENHANCEMENTS[0], Item::Heading(_)));
        assert_eq!(m.opt_cursor, 1, "opening lands below the first heading");
        // up from the first row wraps to the last, never onto the heading
        m.key(K_UPARROW, &mut cv, false);
        assert_eq!(m.opt_cursor, m.page_len(p) - 1);
        // walking down never rests on a heading
        for _ in 0..m.page_len(p) * 2 {
            m.key(K_DOWNARROW, &mut cv, false);
            assert!(!matches!(m.page_item(p, m.opt_cursor), Item::Heading(_)));
        }
        // the pointer can't select a heading
        let h = ENHANCEMENTS.iter().rposition(|i| matches!(i, Item::Heading(_))).unwrap();
        m.set_page_scroll(p, 0);
        let y = m.page_row_y(p, h) + 2.0;
        if y < m.page_bottom(p) {
            assert_eq!(m.item_at((LABEL_X + 10.0, y)), None);
        }
        // scrolling the cursor into view never leaves it on a heading
        for sc in 0..m.page_len(p) {
            m.set_page_scroll(p, sc);
            assert!(!matches!(m.page_item(p, m.opt_cursor), Item::Heading(_)), "scroll {sc}");
        }
    }
}
