//! CSQC entry-point, global and entity-field descriptors resolved from `csprogs.dat`.

use qvm::Progs;

/// Entry-point function indices discovered in `csprogs.dat`.
#[derive(Default, Clone, Copy)]
pub struct Entries {
    pub init: Option<usize>,
    pub shutdown: Option<usize>,
    pub draw_hud: Option<usize>,
    pub draw_scores: Option<usize>,
    pub update_view: Option<usize>,
    pub parse_event: Option<usize>,
    pub parse_print: Option<usize>,
    pub parse_centerprint: Option<usize>,
    pub parse_stuffcmd: Option<usize>,
    pub console_command: Option<usize>,
}

/// Optional standard CSQC globals (0 = absent in this `csprogs.dat`).
#[derive(Default, Clone, Copy)]
pub struct CsqcGlobals {
    pub time: usize,
    pub frametime: usize,
    pub cltime: usize,
    pub clframetime: usize,
    pub mapname: usize,
    pub world: usize,
    pub deathmatch: usize,
    pub coop: usize,
    pub total_monsters: usize,
    pub total_secrets: usize,
    pub killed_monsters: usize,
    pub found_secrets: usize,
    pub player_localentnum: usize,
    pub player_localnum: usize,
    pub maxclients: usize,
    pub intermission: usize,
    pub intermission_time: usize,
    pub sb_showscores: usize,
    pub v_forward: usize,
    pub v_right: usize,
    pub v_up: usize,
    pub view_angles: usize,
    pub pmove_org: usize,
    pub pmove_vel: usize,
    pub input_angles: usize,
    pub trace_allsolid: usize,
    pub trace_startsolid: usize,
    pub trace_fraction: usize,
    pub trace_endpos: usize,
    pub trace_plane_normal: usize,
    pub trace_plane_dist: usize,
    pub trace_ent: usize,
    pub trace_inopen: usize,
    pub trace_inwater: usize,
}

/// Optional standard CSQC entity fields (`usize::MAX` = absent).
#[derive(Clone, Copy)]
pub struct CsqcFields {
    pub origin: usize,
    pub angles: usize,
    pub model: usize,
    pub modelindex: usize,
    pub solid: usize,
    pub message: usize,
    pub frame: usize,
    pub frame2: usize,
    pub lerpfrac: usize,
    pub skin: usize,
    pub colormap: usize,
    pub alpha: usize,
    pub effects: usize,
    pub renderflags: usize,
    pub drawmask: usize,
    pub mins: usize,
    pub maxs: usize,
}

impl Default for CsqcFields {
    fn default() -> Self {
        Self {
            origin: usize::MAX,
            angles: usize::MAX,
            model: usize::MAX,
            modelindex: usize::MAX,
            solid: usize::MAX,
            message: usize::MAX,
            frame: usize::MAX,
            frame2: usize::MAX,
            lerpfrac: usize::MAX,
            skin: usize::MAX,
            colormap: usize::MAX,
            alpha: usize::MAX,
            effects: usize::MAX,
            renderflags: usize::MAX,
            drawmask: usize::MAX,
            mins: usize::MAX,
            maxs: usize::MAX,
        }
    }
}

pub(super) fn find_fn(p: &Progs, name: &str) -> Option<usize> {
    p.find_function(name).filter(|&i| p.functions[i].first_statement > 0)
}

pub(super) fn resolve_entries(p: &Progs) -> Entries {
    Entries {
        init: find_fn(p, "CSQC_Init"),
        shutdown: find_fn(p, "CSQC_Shutdown"),
        draw_hud: find_fn(p, "CSQC_DrawHud"),
        draw_scores: find_fn(p, "CSQC_DrawScores"),
        update_view: find_fn(p, "CSQC_UpdateView"),
        parse_event: find_fn(p, "CSQC_Parse_Event"),
        parse_print: find_fn(p, "CSQC_Parse_Print"),
        parse_centerprint: find_fn(p, "CSQC_Parse_CenterPrint"),
        parse_stuffcmd: find_fn(p, "CSQC_Parse_StuffCmd"),
        console_command: find_fn(p, "CSQC_ConsoleCommand"),
    }
}

pub(super) fn resolve_globals(p: &Progs) -> CsqcGlobals {
    let g = |n: &str| p.find_global(n).map_or(0, |d| d.ofs as usize);
    CsqcGlobals {
        time: g("time"),
        frametime: g("frametime"),
        cltime: g("cltime"),
        clframetime: g("clframetime"),
        mapname: g("mapname"),
        world: g("world"),
        deathmatch: g("deathmatch"),
        coop: g("coop"),
        total_monsters: g("total_monsters"),
        total_secrets: g("total_secrets"),
        killed_monsters: g("killed_monsters"),
        found_secrets: g("found_secrets"),
        player_localentnum: g("player_localentnum"),
        player_localnum: g("player_localnum"),
        maxclients: g("maxclients"),
        intermission: g("intermission"),
        intermission_time: g("intermission_time"),
        sb_showscores: g("sb_showscores"),
        v_forward: g("v_forward"),
        v_right: g("v_right"),
        v_up: g("v_up"),
        view_angles: g("view_angles"),
        pmove_org: g("pmove_org"),
        pmove_vel: g("pmove_vel"),
        input_angles: g("input_angles"),
        trace_allsolid: g("trace_allsolid"),
        trace_startsolid: g("trace_startsolid"),
        trace_fraction: g("trace_fraction"),
        trace_endpos: g("trace_endpos"),
        trace_plane_normal: g("trace_plane_normal"),
        trace_plane_dist: g("trace_plane_dist"),
        trace_ent: g("trace_ent"),
        trace_inopen: g("trace_inopen"),
        trace_inwater: g("trace_inwater"),
    }
}

pub(super) fn resolve_fields(p: &Progs) -> CsqcFields {
    let f = |n: &str| p.find_field(n).map_or(usize::MAX, |d| d.ofs as usize);
    CsqcFields {
        origin: f("origin"),
        angles: f("angles"),
        model: f("model"),
        modelindex: f("modelindex"),
        solid: f("solid"),
        message: f("message"),
        frame: f("frame"),
        frame2: f("frame2"),
        lerpfrac: f("lerpfrac"),
        skin: f("skin"),
        colormap: f("colormap"),
        alpha: f("alpha"),
        effects: f("effects"),
        renderflags: f("renderflags"),
        drawmask: f("drawmask"),
        mins: f("mins"),
        maxs: f("maxs"),
    }
}
