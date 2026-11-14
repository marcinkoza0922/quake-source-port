//! Stress-test helpers for tools/qprof's "ultimate" scenario:
//! `sv_horde N` spawns N extra copies of every monster at map load, placed
//! on a ring around the original (positions near a mapper-placed monster
//! are usually open floor), and `prof_wake` makes every living monster hunt
//! player 1 at once. Debug only; nothing touches them by default.

use crate::server::{Server, exec};

/// FL_MONSTER in the QC `flags` field.
const FL_MONSTER: i32 = 32;

/// Entity lump text with `n` extra copies of each `monster_*` entity.
/// Copies drop `target`/`killtarget` so their deaths don't re-fire the
/// level's scripting.
pub fn multiply(ents: &str, n: usize) -> String {
    if n == 0 {
        return ents.to_string();
    }
    let mut out = String::with_capacity(ents.len() * 2);
    let mut extra = String::new();
    for block in blocks(ents) {
        out.push_str(block);
        out.push('\n');
        let pairs = pairs(block);
        let get = |k: &str| pairs.iter().find(|p| p.0 == k).map(|p| p.1);
        if !get("classname").is_some_and(|c| c.starts_with("monster_")) {
            continue;
        }
        let Some(o) = get("origin").map(|s| s.split_whitespace().filter_map(|v| v.parse::<f32>().ok()).collect::<Vec<_>>())
            .filter(|o| o.len() == 3) else { continue };
        for k in 0..n {
            // Rings of 6 at 56, 112, ... units.
            let ring = (k / 6 + 1) as f32 * 56.0;
            let a = (k % 6) as f32 * std::f32::consts::TAU / 6.0 + (k / 6) as f32 * 0.5;
            extra.push_str("{\n");
            for (key, v) in &pairs {
                match *key {
                    "target" | "killtarget" => {}
                    "origin" => extra.push_str(&format!(
                        "\"origin\" \"{} {} {}\"\n", o[0] + ring * a.cos(), o[1] + ring * a.sin(), o[2] + 8.0
                    )),
                    _ => extra.push_str(&format!("\"{key}\" \"{v}\"\n")),
                }
            }
            extra.push_str("}\n");
        }
    }
    out.push_str(&extra);
    out
}

/// Top-level `{ ... }` blocks (entity lumps never nest braces outside quotes).
fn blocks(s: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let (mut start, mut quoted) = (None, false);
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '{' if !quoted && start.is_none() => start = Some(i),
            '}' if !quoted => {
                if let Some(b) = start.take() {
                    v.push(&s[b..=i]);
                }
            }
            _ => {}
        }
    }
    v
}

/// `"key" "value"` pairs of one block.
fn pairs(block: &str) -> Vec<(&str, &str)> {
    let q: Vec<&str> = block.split('"').skip(1).step_by(2).collect();
    q.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect()
}

/// `prof_wake`: every living monster targets player 1 and starts running
/// (id1 FoundTarget/HuntTarget: enemy, goalentity, then `th_run`).
pub(crate) fn cmd_prof_wake(s: &mut Server) {
    let player = 1u32;
    let f = s.core.sv.fields.clone();
    let g = s.core.sv.globals.clone();
    let Some(l) = s.level.as_mut() else { return };
    let th_run = l.vm.progs.find_field("th_run").map(|d| d.ofs as usize);
    let mut woken = 0;
    for e in 2..l.vm.num_edicts() as u32 {
        let vm = &mut l.vm;
        if vm.is_free(e) || vm.ent_f(e, f.flags) as i32 & FL_MONSTER == 0 || vm.ent_f(e, f.health) <= 0.0 {
            continue;
        }
        vm.set_ent_i(e, f.enemy, player as i32);
        vm.set_ent_i(e, f.goalentity, player as i32);
        if let Some(ofs) = th_run {
            let func = vm.ent_i(e, ofs);
            vm.set_g_i(g.self_, e as i32);
            vm.set_g_i(g.other, player as i32);
            exec(vm, &mut s.core, &mut l.world, func);
        }
        woken += 1;
    }
    s.core.print(&format!("prof_wake: {woken} monsters hunting\n"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn copies_monsters_only() {
        let ents = "{\n\"classname\" \"worldspawn\"\n}\n{\n\"classname\" \"monster_army\"\n\"origin\" \"0 0 24\"\n\"target\" \"t1\"\n}\n";
        let out = super::multiply(ents, 2);
        assert_eq!(super::blocks(&out).len(), 4);
        assert_eq!(out.matches("monster_army").count(), 3);
        assert_eq!(out.matches("\"target\"").count(), 1);
        assert!(out.contains("\"origin\" \"56 0 32\""));
        assert_eq!(super::multiply(ents, 0), ents);
    }
}
