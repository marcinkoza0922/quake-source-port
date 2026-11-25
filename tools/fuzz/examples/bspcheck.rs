//! Parse every .bsp of every game dir and list the rejected ones:
//! bspcheck BASEDIR...
fn main() {
    let (mut n, mut bad) = (0, 0);
    for base in std::env::args().skip(1).map(std::path::PathBuf::from) {
        let mut dirs: Vec<_> = std::fs::read_dir(&base).unwrap().flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());
        for e in dirs {
            let mut fs = qcore::fs::Fs::new(&base);
            fs.add_game_dir(&e.file_name().to_string_lossy());
            for f in fs.list("", ".bsp") {
                let Some(d) = fs.open(&f) else {
                    println!("{}/{f}: unreadable", e.file_name().to_string_lossy());
                    continue;
                };
                n += 1;
                if let Err(err) = qformats::Bsp::parse(&d) {
                    bad += 1;
                    println!("{}/{f}: {err}", e.file_name().to_string_lossy());
                }
            }
        }
    }
    println!("{n} maps, {bad} rejected");
}
