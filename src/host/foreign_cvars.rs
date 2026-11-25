//! Cvars of other engines that map configs set and nothing here reads:
//! registered so those configs load quietly (see also `COMPAT_CVARS`).

pub(super) const FOREIGN_CVARS: &[(&str, &str, u32)] = &[
    // Quoth's optout.cfg switches off DarkPlaces' gameplay fixes. ironwail
    // and vkQuake have none of them but setmodelrealbox, where vkQuake's 0
    // is vanilla's fixed +-16 setmodel box. We always use the model's bounds
    // (ironwail; vkQuake's default): on Quoth maps only decorations differ,
    // and a hanging corpse reaches further than 16 units.
    ("sv_gameplayfix_setmodelrealbox", "1", 0),
    ("sv_gameplayfix_blowupfallenzombies", "0", 0),
    ("sv_gameplayfix_consistentplayerprethink", "0", 0),
    ("sv_gameplayfix_delayprojectiles", "0", 0),
    ("sv_gameplayfix_downtracesupportsongroundflag", "0", 0),
    ("sv_gameplayfix_droptofloorstartsolid", "0", 0),
    ("sv_gameplayfix_droptofloorstartsolid_nudgetocorrect", "0", 0),
    ("sv_gameplayfix_easierwaterjump", "0", 0),
    ("sv_gameplayfix_findradiusdistancetobox", "0", 0),
    ("sv_gameplayfix_fixedcheckwatertransition", "0", 0),
    ("sv_gameplayfix_gravityunaffectedbyticrate", "0", 0),
    ("sv_gameplayfix_grenadebouncedownslopes", "0", 0),
    ("sv_gameplayfix_multiplethinksperframe", "0", 0),
    ("sv_gameplayfix_noairborncorpse", "0", 0),
    ("sv_gameplayfix_noairborncorpse_allowsuspendeditems", "0", 0),
    ("sv_gameplayfix_nogravityonground", "0", 0),
    ("sv_gameplayfix_nostepmoveonsteepslopes", "0", 0),
    ("sv_gameplayfix_nudgeoutofsolid", "0", 0),
    ("sv_gameplayfix_nudgeoutofsolid_separation", "0", 0),
    ("sv_gameplayfix_q1bsptracelinereportstexture", "0", 0),
    ("sv_gameplayfix_q2airaccelerate", "0", 0),
    ("sv_gameplayfix_slidemoveprojectiles", "0", 0),
    ("sv_gameplayfix_stepdown", "0", 0),
    ("sv_gameplayfix_stepmultipletimes", "0", 0),
    ("sv_gameplayfix_stepwhilejumping", "0", 0),
    ("sv_gameplayfix_swiminbmodels", "0", 0),
    ("sv_gameplayfix_unstickentities", "0", 0),
    ("sv_gameplayfix_unstickplayers", "0", 0),
    ("sv_gameplayfix_upwardvelocityclearsongroundflag", "0", 0),
    // Original Quake and old engines' renderer settings in map configs.
    ("gl_flashblend", "0", 0),
    ("r_maxedges", "0", 0),
    ("r_maxsurfs", "0", 0),
    ("gl_exttex", "1", 0),
    ("gl_texquality", "2", 0),
    ("r_oldwater", "0", 0),
    ("r_farclip", "16384", 0),
];
