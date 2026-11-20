//! The HUD element catalog: every kind of element, its menu name and
//! category, which fonts and options it takes, and its defaults.

/// One kind of HUD element. `id` is its name in `hud_layout`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Health,
    Armor,
    Face,
    HealthBar,
    ArmorBar,
    Ammo,
    AmmoBar,
    AmmoList,
    Shells,
    Nails,
    Rockets,
    Cells,
    Weapons,
    Items,
    Keys,
    Powerups,
    Sigils,
    StatusBar,
    Clock,
    Timer,
    Pb,
    Kills,
    Secrets,
    Speed,
    Fps,
    FrameTime,
    Cpu,
    Ram,
    Gpu,
    Vram,
    Automap,
    Strafe,
    InputSequence,
    Center,
    Notify,
    Captions,
    DamageSkull,
}

/// Text style of an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    /// The status bar's 24-unit number pictures (`num_0`..).
    Big,
    /// Quake's 8×8 characters.
    Quake,
    /// The menu's TTF font.
    Modern,
}

impl Font {
    pub fn id(self) -> char {
        match self {
            Font::Big => 'b',
            Font::Quake => 'q',
            Font::Modern => 'm',
        }
    }
    pub fn from_id(c: char) -> Option<Self> {
        Some(match c {
            'b' => Font::Big,
            'q' => Font::Quake,
            'm' => Font::Modern,
            _ => return None,
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            Font::Big => "big numbers",
            Font::Quake => "Quake",
            Font::Modern => "modern",
        }
    }
}

/// Which of an element's fields an option edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    A,
    B,
    Len,
    /// A cvar shared with the rest of the game (message times, captions).
    Cvar(&'static str),
}

/// The values an option takes.
#[derive(Clone, Copy, Debug)]
pub enum Values {
    Choice(&'static [(&'static str, f32)]),
    /// min, max, step.
    Range(f32, f32, f32),
}

/// A kind-specific option row.
#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub label: &'static str,
    pub field: Field,
    pub values: Values,
}

pub const ON_OFF: &[(&str, f32)] = &[("off", 0.0), ("on", 1.0)];
pub const CLOCK: &[(&str, f32)] = &[("24-hour", 1.0), ("24-hour + seconds", 2.0), ("12-hour", 3.0), ("12-hour + seconds", 4.0)];
pub const PRECISION: &[(&str, f32)] = &[("1:23", 0.0), ("1:23.4", 1.0), ("1:23.45", 2.0)];
pub const PB_KIND: &[(&str, f32)] = &[("any%", 0.0), ("no-hit", 2.0), ("100%", 1.0), ("100% no-hit", 3.0)];
pub const COUNT: &[(&str, f32)] = &[("count", 0.0), ("percent", 1.0)];
pub const SECTIONS: &[(&str, f32)] = &[("status only", 0.0), ("status + inventory", 1.0)];
/// Text colour choices: the element's own, or a crosshair palette colour.
pub const TEXT_COLOR: &[(&str, f32)] = &[
    ("normal", 255.0), ("grey", 6.0), ("white", 0.0), ("yellow", 1.0), ("green", 2.0), ("cyan", 3.0), ("magenta", 4.0),
    ("red", 5.0),
];
pub const COLOR: &[(&str, f32)] =
    &[("grey", 6.0), ("white", 0.0), ("yellow", 1.0), ("green", 2.0), ("cyan", 3.0), ("magenta", 4.0), ("red", 5.0)];

const fn opt(label: &'static str, field: Field, values: Values) -> Opt {
    Opt { label, field, values }
}
const SHOW_ICON: Opt = opt("Show Icon", Field::A, Values::Choice(ON_OFF));
const BAR: &[Opt] = &[opt("Length", Field::Len, Values::Range(24.0, 320.0, 8.0)), opt("Show Value", Field::A, Values::Choice(ON_OFF))];
const ICON: &[Opt] = &[SHOW_ICON];
const FACE: &[Opt] = &[opt("Show Face", Field::A, Values::Choice(ON_OFF))];
const COLUMNS: &[Opt] = &[opt("Columns", Field::A, Values::Range(1.0, 10.0, 1.0))];
const STATUS_BAR: &[Opt] =
    &[opt("Sections", Field::A, Values::Choice(SECTIONS)), opt("Fill Width", Field::B, Values::Choice(ON_OFF))];
const CLOCK_OPTS: &[Opt] = &[opt("Format", Field::A, Values::Choice(CLOCK))];
const TIMER_OPTS: &[Opt] = &[opt("Precision", Field::A, Values::Choice(PRECISION))];
const PB_OPTS: &[Opt] = &[
    // shared with the save cards
    opt("Personal Best Type", Field::Cvar("hud_pb_100"), Values::Choice(PB_KIND)),
    opt("Personal Best Color", Field::Cvar("hud_pb_color"), Values::Choice(COLOR)),
    opt("Precision", Field::A, Values::Choice(PRECISION)),
];
const COUNT_OPTS: &[Opt] = &[opt("Show As", Field::A, Values::Choice(COUNT))];
const SPEED_OPTS: &[Opt] = &[opt("Show Label", Field::A, Values::Choice(ON_OFF))];
const CENTER_OPTS: &[Opt] = &[opt("Center Message Time", Field::Cvar("scr_centertime"), Values::Range(1.0, 10.0, 1.0))];
const NOTIFY_OPTS: &[Opt] = &[opt("Notify Message Time", Field::Cvar("con_notifytime"), Values::Range(1.0, 10.0, 1.0))];
const CAPTION_OPTS: &[Opt] = &[opt("Sound Captions", Field::Cvar("cl_captions"), Values::Choice(ON_OFF))];
const AUTOMAP_VIEW: &[(&str, f32)] = &[("3D wireframe", 0.0), ("Top-down", 1.0)];
const AUTOMAP_REVEAL: &[(&str, f32)] = &[("Explored only", 0.0), ("Full map", 1.0)];
const AUTOMAP_OPTS: &[Opt] = &[
    opt("View Mode", Field::Cvar("cl_automap_view"), Values::Choice(AUTOMAP_VIEW)),
    opt("Rotate with Player", Field::Cvar("cl_automap_rotate"), Values::Choice(ON_OFF)),
    opt("Reveal", Field::Cvar("cl_automap_reveal"), Values::Choice(AUTOMAP_REVEAL)),
    opt("Zoom", Field::Cvar("cl_automap_zoom"), Values::Range(0.5, 3.0, 0.25)),
    opt("Show Markers", Field::Cvar("cl_automap_markers"), Values::Choice(ON_OFF)),
];

/// Menu categories of the Add Element tree; the messages are always there.
pub const CATEGORIES: &[(&str, &[Kind])] = &[
    ("Status", &[Kind::Health, Kind::Armor, Kind::Face, Kind::HealthBar, Kind::ArmorBar]),
    ("Ammo", &[Kind::Ammo, Kind::AmmoBar, Kind::AmmoList, Kind::Shells, Kind::Nails, Kind::Rockets, Kind::Cells]),
    ("Inventory", &[Kind::Weapons, Kind::Items, Kind::Keys, Kind::Powerups, Kind::Sigils]),
    ("Classic", &[Kind::StatusBar]),
    ("Time & Stats", &[Kind::Clock, Kind::Timer, Kind::Pb, Kind::Kills, Kind::Secrets, Kind::Speed, Kind::DamageSkull]),
    ("Performance Metrics", &[Kind::Fps, Kind::FrameTime, Kind::Cpu, Kind::Ram, Kind::Gpu, Kind::Vram]),
    ("Automap", &[Kind::Automap]),
    ("Movement & Input", &[Kind::Strafe, Kind::InputSequence]),
];

impl Kind {
    pub const ALL: [Kind; 37] = [
        Kind::Health, Kind::Armor, Kind::Face, Kind::HealthBar, Kind::ArmorBar, Kind::Ammo, Kind::AmmoBar,
        Kind::AmmoList, Kind::Shells, Kind::Nails, Kind::Rockets, Kind::Cells, Kind::Weapons, Kind::Items,
        Kind::Keys, Kind::Powerups, Kind::Sigils, Kind::StatusBar, Kind::Clock, Kind::Timer, Kind::Pb,
        Kind::Kills, Kind::Secrets, Kind::Speed, Kind::Fps, Kind::FrameTime, Kind::Cpu, Kind::Ram, Kind::Gpu, Kind::Vram,
        Kind::Automap, Kind::Strafe, Kind::InputSequence, Kind::Center, Kind::Notify, Kind::Captions, Kind::DamageSkull,
    ];
    /// Always in the layout: they can be moved but not removed.
    pub const PERMANENT: [Kind; 3] = [Kind::Center, Kind::Notify, Kind::Captions];

    pub fn id(self) -> &'static str {
        use Kind::*;
        match self {
            Health => "health",
            Armor => "armor",
            Face => "face",
            HealthBar => "healthbar",
            ArmorBar => "armorbar",
            Ammo => "ammo",
            AmmoBar => "ammobar",
            AmmoList => "ammolist",
            Shells => "shells",
            Nails => "nails",
            Rockets => "rockets",
            Cells => "cells",
            Weapons => "weapons",
            Items => "items",
            Keys => "keys",
            Powerups => "powerups",
            Sigils => "sigils",
            StatusBar => "statusbar",
            Clock => "clock",
            Timer => "timer",
            Pb => "pb",
            Kills => "kills",
            Secrets => "secrets",
            Speed => "speed",
            Fps => "fps",
            FrameTime => "frametime",
            Cpu => "cpu",
            Ram => "ram",
            Gpu => "gpu",
            Vram => "vram",
            Automap => "automap",
            Strafe => "strafe",
            InputSequence => "inputsequence",
            Center => "center",
            Notify => "notify",
            Captions => "captions",
            DamageSkull => "damageskull",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Kind::ALL.into_iter().find(|k| k.id() == s)
    }

    /// Name in the editor (translated where drawn).
    pub fn label(self) -> &'static str {
        use Kind::*;
        match self {
            Health => "Health",
            Armor => "Armor",
            Face => "Face",
            HealthBar => "Health Bar",
            ArmorBar => "Armor Bar",
            Ammo => "Current Ammo",
            AmmoBar => "Ammo Bar",
            AmmoList => "Ammo Counts",
            Shells => "Shells",
            Nails => "Nails",
            Rockets => "Rockets",
            Cells => "Cells",
            Weapons => "Weapons",
            Items => "Keys & Powerups",
            Keys => "Keys",
            Powerups => "Powerups",
            Sigils => "Sigils",
            StatusBar => "Status Bar",
            Clock => "Clock",
            Timer => "Level Timer",
            Pb => "Personal Best",
            Kills => "Kills",
            Secrets => "Secrets",
            Speed => "Speedometer",
            Fps => "FPS",
            FrameTime => "Frame Time",
            Cpu => "CPU Usage",
            Ram => "Memory Usage",
            Gpu => "GPU Time",
            Vram => "Video Memory",
            Automap => "Automap",
            Strafe => "Strafe & Jump Timing",
            InputSequence => "Input Sequence",
            Center => "Center Message",
            Notify => "Notify Messages",
            Captions => "Sound Captions",
            DamageSkull => "Damage Skull",
        }
    }

    pub fn permanent(self) -> bool {
        Kind::PERMANENT.contains(&self)
    }

    /// Follows the player's stats: hidden with the scoreboard and at
    /// death, drawn per player in split screen, may fade (immersive).
    pub fn is_player(self) -> bool {
        use Kind::*;
        !matches!(self, Clock | Timer | Pb | Kills | Secrets | Speed | Fps | FrameTime | Cpu | Ram | Gpu | Vram | Automap | Center | Notify | Captions | DamageSkull)
    }

    pub fn is_metric(self) -> bool {
        matches!(self, Kind::Fps | Kind::FrameTime | Kind::Cpu | Kind::Ram | Kind::Gpu | Kind::Vram)
    }

    pub fn is_bar(self) -> bool {
        matches!(self, Kind::HealthBar | Kind::ArmorBar | Kind::AmmoBar)
    }

    /// A count drawn next to an icon (big numbers allowed).
    pub fn is_number(self) -> bool {
        use Kind::*;
        matches!(self, Health | Armor | Ammo | Shells | Nails | Rockets | Cells)
    }

    pub fn is_message(self) -> bool {
        self.permanent()
    }

    /// Fonts offered; empty when the element has no text of its own.
    pub fn fonts(self) -> &'static [Font] {
        use Kind::*;
        if self.is_number() {
            &[Font::Big, Font::Quake, Font::Modern]
        } else if matches!(self, Face | Weapons | Items | Keys | Powerups | Sigils | StatusBar | Automap | Strafe | InputSequence | DamageSkull) || self.is_message() {
            &[]
        } else {
            &[Font::Quake, Font::Modern]
        }
    }

    /// Text size choices (units); bars use it as their thickness.
    pub fn sizes(self) -> &'static [(&'static str, f32)] {
        if self.is_number() {
            &[("8", 8.0), ("12", 12.0), ("16", 16.0), ("20", 20.0), ("24", 24.0), ("32", 32.0)]
        } else if self.is_bar() {
            &[("4", 4.0), ("6", 6.0), ("8", 8.0), ("10", 10.0), ("12", 12.0), ("16", 16.0)]
        } else if self.is_message() {
            &[("8", 8.0), ("16", 16.0), ("24", 24.0)]
        } else if self.fonts().is_empty() {
            &[]
        } else {
            &[("8", 8.0), ("10", 10.0), ("12", 12.0), ("16", 16.0), ("20", 20.0), ("24", 24.0)]
        }
    }

    /// Immersive mode (shown briefly after a change) makes sense.
    pub fn can_fade(self) -> bool {
        self.is_player() && !matches!(self, Kind::StatusBar | Kind::Strafe | Kind::InputSequence)
    }

    /// Whether the element exposes a Visibility row in the editor.
    pub fn has_visibility(self) -> bool {
        self != Kind::Captions
    }

    /// Sane default grid anchor when adding a new element of this kind.
    pub fn default_anchor(self) -> super::layout::Anchor {
        use super::layout::Anchor;
        use Kind::*;
        match self {
            Health | Armor | Face | HealthBar | ArmorBar => Anchor::BottomLeft,
            Ammo | AmmoBar | AmmoList | Shells | Nails | Rockets | Cells | Weapons | Items | Keys | Powerups | Sigils => Anchor::BottomRight,
            StatusBar | Captions | InputSequence => Anchor::BottomCenter,
            Strafe | DamageSkull => Anchor::TopCenter,
            Clock | Timer | Pb | Kills | Secrets | Speed | Fps | FrameTime | Cpu | Ram | Gpu | Vram | Automap => Anchor::TopRight,
            Notify => Anchor::TopLeft,
            Center => Anchor::Center,
        }
    }

    /// Text elements may have a dark backing.
    /// Its text colour can be picked (the personal best has its own).
    pub fn has_text_color(self) -> bool {
        (!self.fonts().is_empty() && self != Kind::Pb) || self.is_message()
    }

    /// Its text has a gold label ("FPS", "K") whose colour can be picked.
    pub fn has_label(self) -> bool {
        self.is_metric() || matches!(self, Kind::Kills | Kind::Secrets | Kind::Speed)
    }

    pub fn has_background(self) -> bool {
        !self.fonts().is_empty() && !self.is_number() || matches!(self, Kind::Center | Kind::Notify | Kind::Automap | Kind::Strafe | Kind::InputSequence)
    }

    pub fn opts(self) -> &'static [Opt] {
        use Kind::*;
        match self {
            Health => FACE,
            Armor | Ammo | Shells | Nails | Rockets | Cells => ICON,
            HealthBar | ArmorBar | AmmoBar => BAR,
            Weapons => COLUMNS,
            StatusBar => STATUS_BAR,
            Clock => CLOCK_OPTS,
            Timer => TIMER_OPTS,
            Pb => PB_OPTS,
            Kills | Secrets => COUNT_OPTS,
            Speed => SPEED_OPTS,
            Automap => AUTOMAP_OPTS,
            Center => CENTER_OPTS,
            Notify => NOTIFY_OPTS,
            Captions => CAPTION_OPTS,
            _ => &[],
        }
    }
}

impl Kind {
    /// Help for the element, shown under the editor.
    pub fn tip(self) -> Option<&'static str> {
        Some(match self {
            Kind::Clock => "Shows the time of day in the chosen format",
            Kind::Timer => "The level time, as on the intermission screen; it stops while paused and at the exit",
            Kind::Speed => "Horizontal movement speed in units per second",
            Kind::Fps => "Frames per second, averaged over half a second",
            Kind::FrameTime => "Average time per frame in milliseconds, and the slowest frame",
            Kind::Cpu => "How busy the game keeps the processor, as a share of all its cores",
            Kind::Ram => "Memory the game uses",
            Kind::Gpu => "Graphics card time per frame in milliseconds",
            Kind::Vram => "Video memory in use and what the driver allows, where it reports it",
            Kind::Automap => "Shows a live 3D wireframe or top-down automap on the HUD at all times (single player and co-op only)",
            Kind::Strafe => "Local games. Align white with gold. Jump timing: − early, + late.",
            Kind::InputSequence => "Shows the eight most recent gameplay inputs",
            Kind::Captions => "Shows text for important sounds: doors, alerted enemies, secrets, pickups",
            _ => return None,
        })
    }
}

/// Help for an option row, by its label.
pub fn opt_tip(label: &str) -> Option<&'static str> {
    Some(match label {
        "Show Face" => "Shows the player face next to the health number",
        "Show Label" => "Shows the SPD label before the speed number",
        "Personal Best Type" => "any%: any finish; 100%: every kill and secret. Save cards show this one first",
        "Personal Best Color" => "Color of the personal best on the HUD and save cards",
        "Show As" => "Shown as a count or as a percentage of the level's total",
        "Center Message Time" => "Seconds a center-screen message stays up",
        _ => return None,
    })
}

/// Visibility option's help.
pub const IMMERSIVE_TIP: &str = "Appear on change: shown briefly only when its value changes; hidden: hides without removing";

/// Every text of the catalog, categories, options and editor, for the
/// translation catalog test.
#[cfg(test)]
pub fn lang_keys() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Kind::ALL.iter().map(|k| k.label()).collect();
    out.extend(Kind::ALL.iter().filter_map(|k| k.tip()));
    out.extend(CATEGORIES.iter().map(|c| c.0));
    for k in Kind::ALL {
        for o in k.opts() {
            out.push(o.label);
            out.extend(opt_tip(o.label));
            if let Values::Choice(ch) = o.values {
                out.extend(ch.iter().map(|c| c.0));
            }
        }
    }
    out.extend([Font::Big, Font::Quake, Font::Modern].map(Font::label));
    out.extend(super::layout::Anchor::ALL.map(super::layout::Anchor::label));
    out.extend(super::editor::BUTTONS);
    out.extend(["always", "appear on change", "hidden", IMMERSIVE_TIP, "Text Color", "Label Color", "Sample", "Alert", "Live"]);
    out.extend(TEXT_COLOR.iter().map(|c| c.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_every_addable_kind_is_listed_once() {
        for k in Kind::ALL {
            assert_eq!(Kind::from_id(k.id()), Some(k));
        }
        let listed: Vec<Kind> = CATEGORIES.iter().flat_map(|c| c.1.iter().copied()).collect();
        for k in Kind::ALL {
            let n = listed.iter().filter(|&&l| l == k).count();
            assert_eq!(n, usize::from(!k.permanent()), "{k:?}");
        }
    }
}
