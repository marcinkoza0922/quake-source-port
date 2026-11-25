//! Depth and unrolled size of the node and clipnode graphs of real maps
//! (the largest of each): treestat BASEDIR...
fn stat(ch: &[[i32; 2]]) -> (u32, f64) {
    let n = ch.len();
    let mut memo: Vec<(u32, f64)> = vec![(0, 0.0); n];
    let mut best = (0, 0.0);
    for root in 0..n {
        let mut st = vec![(root, false)];
        while let Some((i, done)) = st.pop() {
            if memo[i].0 != 0 { continue; }
            let get = |c: i32| usize::try_from(c).ok().filter(|&c| c < n);
            if done {
                let k = ch[i].map(|c| get(c).map_or((0, 0.0), |c| memo[c]));
                memo[i] = (k[0].0.max(k[1].0) + 1, k[0].1 + k[1].1 + 1.0);
            } else {
                st.push((i, true));
                for c in ch[i] { if let Some(c) = get(c) && memo[c].0 == 0 { st.push((c, false)); } }
            }
        }
        best = (u32::max(best.0, memo[root].0), f64::max(best.1, memo[root].1));
    }
    (best.0, best.1 / n.max(1) as f64)
}
fn main() {
    let mut rows = Vec::new();
    for base in std::env::args().skip(1).map(std::path::PathBuf::from) {
        for e in std::fs::read_dir(&base).unwrap().flatten().filter(|e| e.path().is_dir()) {
            let mut fs = qcore::fs::Fs::new(&base);
            fs.add_game_dir(&e.file_name().to_string_lossy());
            for f in fs.list("", ".bsp") {
                let Some(b) = fs.open(&f).and_then(|d| qformats::Bsp::parse(&d).ok()) else { continue };
                if let Some(l) = b.leafs.iter().find(|l| l.contents >= 0) { println!("CONTENTS {} {}/{f}", l.contents, e.file_name().to_string_lossy()); }
                let n = stat(&b.nodes.iter().map(|n| n.children).collect::<Vec<_>>());
                let c = stat(&b.clipnodes.iter().map(|n| n.children).collect::<Vec<_>>());
                rows.push((format!("{}/{f}", e.file_name().to_string_lossy()), n, c));
            }
        }
    }
    println!("{} maps", rows.len());
    for (what, key) in [("depth", 0), ("unrolled/nodes", 1)] {
        for (lump, sel) in [("nodes", 0), ("clipnodes", 1)] {
            let v = |r: &(String, (u32, f64), (u32, f64))| { let s = if sel == 0 { r.1 } else { r.2 }; if key == 0 { f64::from(s.0) } else { s.1 } };
            rows.sort_by(|a, b| v(b).total_cmp(&v(a)));
            println!("{what} {lump}: {}", rows.iter().take(4).map(|r| format!("{} {:.1}", r.0, v(r))).collect::<Vec<_>>().join(", "));
        }
    }
}
