//! Console commands ironwail and vkQuake have that people's own configs and
//! binds use: `condump`, `randmap`, `skies`, `mods`/`games`, `music_pause`,
//! `music_resume`, `music_loop`, and the mapper's `viewthing` commands. (`inc`, `cycle`, `reset`, `unaliasall` are `qcore::cmd` builtins.)

use super::*;

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("condump", cmd_condump);
    c.add("randmap", |h, _| {
        let names = h.map_names();
        let Some(name) = names.get(h.clock_pick(names.len())).cloned() else {
            h.print("no maps\n");
            return;
        };
        h.print(&format!("Starting map {name}...\n"));
        h.cbuf.add_text(&format!("map {name}\n"));
    });
    c.add("skies", |h, a| {
        // the six faces of a skybox are <name>rt, <name>bk, ...
        let names = h.server.core.fs.list("gfx/env/", "").into_iter().filter_map(|p| {
            let f = p.get("gfx/env/".len()..)?.to_ascii_lowercase();
            let (stem, ext) = f.rsplit_once('.')?;
            let sky = stem.strip_suffix("rt")?;
            (matches!(ext, "tga" | "png" | "jpg" | "pcx") && !sky.is_empty()).then(|| sky.trim_end_matches('_').to_string())
        });
        let list = names.collect();
        print_list(h, list, ["sky", "skies"], a.argv(1));
    });
    for name in ["mods", "games"] {
        c.add(name, |h, a| {
            let fs = &h.server.core.fs;
            let mut roots = vec![fs.basedir()];
            roots.extend(fs.mod_roots().iter().map(|p| p.as_path()));
            let list = qui::mods::scan_roots(&roots).into_iter().map(|m| m.dir).collect();
            print_list(h, list, ["mod", "mods"], a.argv(1));
        });
    }
    c.add("music_pause", |h, _| {
        if let Some(s) = &h.snd {
            s.pause_music(true);
        }
    });
    c.add("music_resume", |h, _| {
        if let Some(s) = &h.snd {
            s.pause_music(false);
        }
    });
    // BGM_Loop_f; the references loop unless told not to
    c.add("music_loop", |h, a| {
        match a.argv(1).to_ascii_lowercase().as_str() {
            "0" | "off" => h.cd.music_once = true,
            "1" | "on" => h.cd.music_once = false,
            "toggle" => h.cd.music_once = !h.cd.music_once,
            _ => {}
        }
        let looping = !h.cd.music_once;
        if let Some(s) = &h.snd {
            s.set_music_loop(looping);
        }
        h.print(if looping { "Music will be looped\n" } else { "Music will not be looped\n" });
    });
    // order numbers belong to tracker music, which we don't play
    c.add("music_jump", |h, a| {
        if a.argc() != 2 {
            h.print("music_jump <ordernum>\n");
        }
    });
    c.add("viewmodel", |h, a| {
        let Some((_, index)) = viewthing(h) else { return };
        let fs = &h.server.core.fs;
        match h.level.named(fs, &mut h.r, a.argv(1)) {
            Some(m) if index < h.level.models.len() => {
                h.level.models[index] = Some(m);
                if let Some(n) = h.cl.model_precache.get_mut(index) {
                    *n = a.argv(1).to_string();
                }
                set_view_frame(h, |_, _| 0);
            }
            _ => h.print(&format!("Can't load {}\n", a.argv(1))),
        }
    });
    c.add("viewframe", |h, a| {
        let want = a.argv(1).parse::<i32>().unwrap_or(0);
        set_view_frame(h, move |_, n| want.min(n - 1));
    });
    c.add("viewnext", |h, _| set_view_frame(h, |f, n| (f + 1).min(n - 1)));
    c.add("viewprev", |h, _| set_view_frame(h, |f, _| (f - 1).max(0)));
    // vkQuake and FTE server info keys; rmj1's quake.rc sets pitch limits,
    // which our server doesn't have
    c.add("serverinfo", |_, _| {});
}

impl Host {
    /// Maps in the search path by name, without `b_*.bsp` item models.
    fn map_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.server.core.fs.list("maps/", ".bsp").iter()
            .filter_map(|p| p.get(5..p.len() - 4).map(str::to_ascii_lowercase))
            .filter(|n| !n.contains('/') && !n.starts_with("b_"))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// An index below `n` from the clock (`rand() % n` in C).
    fn clock_pick(&self, n: usize) -> usize {
        let t = qcore::time::SystemTime::now().duration_since(qcore::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        (t.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 33) as usize % n.max(1)
    }
}

/// FindViewthing: the `viewthing` edict and its model index.
fn viewthing(h: &mut Host) -> Option<(u32, usize)> {
    let f = h.server.core.sv.fields.modelindex;
    let found = h.server.level.as_ref().and_then(|l| {
        let vm = &l.vm;
        (0..vm.num_edicts() as u32).find(|&e| !vm.is_free(e) && vm.classname(e) == "viewthing").map(|e| (e, vm.ent_f(e, f) as usize))
    });
    if found.is_none() {
        h.print("No viewthing on map\n");
    }
    found
}

/// Sets the viewthing's frame to `pick(current, count)` and names it.
fn set_view_frame(h: &mut Host, pick: impl Fn(i32, i32) -> i32) {
    let Some((e, index)) = viewthing(h) else { return };
    let f = h.server.core.sv.fields.frame;
    let names: Vec<String> = h.cl.model_precache.get(index).and_then(|n| h.server.core.fs.open(n))
        .and_then(|d| qformats::Mdl::parse(&d).ok())
        .map(|m| m.frames.iter().map(|f| f.poses.first().map_or(String::new(), |p| p.name.clone())).collect())
        .unwrap_or_default();
    let Some(l) = h.server.level.as_mut() else { return };
    let frame = pick(l.vm.ent_f(e, f) as i32, names.len() as i32).max(0);
    l.vm.set_ent_f(e, f, frame as f32);
    if let Some(n) = names.get(frame as usize) {
        h.print(&format!("frame {frame}: {n}\n"));
    }
}

/// FileList_Print: sorted names, all or those containing `filter`.
fn print_list(h: &mut Host, mut list: Vec<String>, kind: [&str; 2], filter: &str) {
    list.sort_unstable_by_key(|s| s.to_ascii_lowercase());
    list.dedup();
    let f = filter.to_ascii_lowercase();
    list.retain(|n| n.to_ascii_lowercase().contains(&f));
    let mut t: String = list.iter().map(|n| format!("   {n}\n")).collect();
    let n = list.len();
    t += &match (n, filter.is_empty()) {
        (0, true) => format!("no {} found\n", kind[1]),
        (0, false) => format!("no {} found containing \"{filter}\"\n", kind[1]),
        (_, true) => format!("{n} {}\n", kind[usize::from(n != 1)]),
        (_, false) => format!("{n} {} containing \"{filter}\"\n", kind[usize::from(n != 1)]),
    };
    h.print(&t);
}

/// Con_Dump_f: the console's text, colour bits off, to the user dir.
fn cmd_condump(h: &mut Host, a: &Args) {
    let mut rel = if a.argc() >= 2 { a.argv(1).to_string() } else { "condump.txt".to_string() };
    if rel.contains("..") || rel.starts_with(['/', '\\']) || rel.contains(':') {
        h.print("Relative pathnames are not allowed.\n");
        return;
    }
    // COM_AddExtension: any other extension stays (never over a config.cfg)
    if !rel.to_ascii_lowercase().ends_with(".txt") {
        rel.push_str(".txt");
    }
    let text: String = h.con.lines.iter().skip_while(|l| l.iter().all(|&b| b & 0x7f == b' ')).map(|l| {
        let s: String = l.iter().map(|&b| (b & 0x7f) as char).collect();
        format!("{}\n", s.trim_end_matches(' '))
    }).collect();
    let path = h.user_dir().join(&rel);
    let r = path.parent().map_or(Ok(()), qcore::vfs::create_dir_all).and_then(|()| qcore::vfs::write(&path, text.as_bytes()));
    match r {
        Ok(()) => h.print(&format!("Dumped console text to {}.\n", path.display())),
        Err(_) => h.print(&format!("ERROR: couldn't open file {rel}.\n")),
    }
}
