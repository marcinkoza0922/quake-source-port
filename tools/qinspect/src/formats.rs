//! Read-only BSP, MDL and archive dumps using the engine's parsers.
use qcore::fs::{Fs, Pack};
use qformats::{Bsp, Mdl};
use std::path::Path;

pub(super) fn load(file: &str, games: Option<&str>) -> Result<Vec<u8>, String> {
    let Some(games) = games else {
        return std::fs::read(file).map_err(|e| format!("{file}: {e}"));
    };
    let mut fs = Fs::new(Fs::default_basedir());
    fs.add_game_dir("id1");
    for game in games.split(',').filter(|g| !g.is_empty()) {
        if !fs.add_game_dir(game.trim_start_matches('+')) {
            return Err(format!("no game dir {game}"));
        }
    }
    fs.open(file).ok_or_else(|| format!("{file} not found"))
}

pub fn run(kind: &str, args: &[String]) -> Result<(), String> {
    let (suffix, option) = match kind {
        "bsp" => ("[-g GAME[,GAME..]] [--entities | --faces]", "--entities"),
        "mdl" => ("[-g GAME[,GAME..]] [--triangles]", "--triangles"),
        _ => ("[--prefix PATH]", "--prefix"),
    };
    let usage = format!("usage: qinspect {kind} FILE {suffix}");
    let (mut file, mut games, mut detail, mut faces, mut prefix) = (None, None, false, false, "");
    let mut args = args.iter().map(String::as_str);
    while let Some(arg) = args.next() {
        match arg {
            "-h" | "--help" => {
                println!("{usage}");
                return Ok(());
            }
            "-g" if kind != "pak" => games = Some(args.next().ok_or(&usage)?),
            "--prefix" if kind == "pak" => prefix = args.next().ok_or(&usage)?,
            "--faces" if kind == "bsp" => faces = true,
            flag if kind != "pak" && flag == option => detail = true,
            name if !name.starts_with('-') && file.is_none() => file = Some(name),
            _ => return Err(usage),
        }
    }
    let file = file.ok_or(usage)?;
    if kind == "pak" {
        let path = Path::new(file);
        let pack = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pk3"))
        {
            Pack::load_pk3(path)
        } else {
            Pack::load_pak(path)
        }
        .ok_or_else(|| format!("couldn't open archive {file}"))?;
        println!("{file}: {} entries", pack.names().len());
        let prefix = prefix.to_ascii_lowercase();
        let mut names = pack
            .names()
            .iter()
            .filter(|n| n.starts_with(&prefix))
            .collect::<Vec<_>>();
        names.sort_unstable();
        for name in names {
            println!("{:>10}  {name}", pack.entry_size(name).unwrap_or_default());
        }
        return Ok(());
    }
    let bytes = load(file, games)?;
    if kind == "bsp" {
        let map = Bsp::parse(&bytes).map_err(|e| format!("{file}: {e}"))?;
        if detail {
            print!("{}", map.entities);
        } else {
            bsp(file, &map, faces);
        }
    } else {
        let model = Mdl::parse(&bytes).map_err(|e| format!("{file}: {e}"))?;
        mdl(file, &model, detail);
    }
    Ok(())
}

fn bsp(file: &str, map: &Bsp, faces: bool) {
    println!(
        "{file}: {:?}, {} faces, {} vertices, {} models",
        map.version,
        map.faces.len(),
        map.vertexes.len(),
        map.models.len()
    );
    println!(
        "{} planes, {} nodes, {} clipnodes, {} leaves, {} marksurfaces",
        map.planes.len(),
        map.nodes.len(),
        map.clipnodes.len(),
        map.leafs.len(),
        map.marksurfaces.len()
    );
    println!(
        "{} texture slots, {} texinfo, {} edges, {} surfedges",
        map.textures.len(),
        map.texinfo.len(),
        map.edges.len(),
        map.surfedges.len()
    );
    println!(
        "{} light bytes, {} RGB light bytes, {} VIS bytes, {} entity bytes",
        map.lighting.len(),
        map.rgb_lighting.as_ref().map_or(0, Vec::len),
        map.visdata.len(),
        map.entities.chars().count()
    );
    for (i, tex) in map.textures.iter().enumerate() {
        if let Some(tex) = tex {
            println!(
                "texture {i}: {} {}x{} {:?}, {} embedded mips",
                tex.name,
                tex.width,
                tex.height,
                tex.kind,
                tex.mips.len()
            );
        }
    }
    for (i, model) in map.models.iter().enumerate() {
        println!(
            "model *{i}: {:?} to {:?}, origin {:?}, faces {}+{}, headnodes {:?}, {} VIS leaves",
            model.mins,
            model.maxs,
            model.origin,
            model.first_face,
            model.num_faces,
            model.headnode,
            model.visleafs
        );
    }
    if faces {
        for (i, face) in map.faces.iter().enumerate() {
            let texture = map
                .texinfo
                .get(face.texinfo as usize)
                .and_then(|t| map.textures.get(t.miptex as usize))
                .and_then(Option::as_ref)
                .map_or("-", |t| &t.name);
            println!(
                "face {i}: {texture}, plane {} side {}, edges {}+{}, light {} styles {:?}",
                face.plane, face.side, face.first_edge, face.num_edges, face.lightofs, face.styles
            );
        }
    }
}

fn mdl(file: &str, model: &Mdl, triangles: bool) {
    println!(
        "{file}: IDPO v6, {} vertices, {} triangles, {} frames, {} skins ({}x{})",
        model.num_verts,
        model.triangles.len(),
        model.frames.len(),
        model.skins.len(),
        model.skin_width,
        model.skin_height
    );
    println!(
        "scale {:?}, origin {:?}, eye {:?}, radius {}, flags {:#x}, sync {}",
        model.scale,
        model.scale_origin,
        model.eye_position,
        model.bounding_radius,
        model.flags,
        model.synctype
    );
    for (i, skin) in model.skins.iter().enumerate() {
        println!(
            "skin {i}: {} images, intervals {:?}",
            skin.images.len(),
            skin.intervals
        );
    }
    for (i, frame) in model.frames.iter().enumerate() {
        let poses: Vec<_> = frame.poses.iter().map(|p| p.name.as_str()).collect();
        println!(
            "frame {i}: {}, bbox {:?} to {:?}, intervals {:?}",
            poses.join(", "),
            frame.bboxmin.v,
            frame.bboxmax.v,
            frame.intervals
        );
    }
    if triangles {
        for (i, tri) in model.triangles.iter().enumerate() {
            println!(
                "triangle {i}: {:?}, faces_front {}",
                tri.verts, tri.faces_front
            );
        }
    }
}
