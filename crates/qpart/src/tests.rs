use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn files(map: &[(&str, &str)]) -> impl FnMut(&str) -> Option<Vec<u8>> {
    let m: HashMap<String, Vec<u8>> = map.iter().map(|(k, v)| (k.to_string(), v.as_bytes().to_vec())).collect();
    move |p: &str| m.get(p).cloned()
}

const CFG: &str = r#"
// comment line
r_part te_rain
{
	texture ball
	count 1
	alpha 0.5
	rgb 255 128 0
	die 2
	scale 1
	stretchfactor -40
	type texturedspark
	cliptype rainsplash
	clipbounce 100
}
r_part rainsplash
{
	randomvel 50 50
	count 1
	die 0.1 0.3
}
r_part +rainsplash
{
	texture ball
	scale 3
	spawnmode ball 2 3
	blend premul_add
	underwater water slime
	sound "weapons/r_exp3.wav" vol=0.5 attn=none
}
r_effect progs/missile.mdl TR_ROCKET replace
r_partredirect old_fx te_rain
"#;

#[test]
fn parses_fte_cfg() {
    let mut r = Registry::new();
    let mut f = files(&[("particles/weather.cfg", CFG)]);
    assert!(r.load_set("weather", false, true, &mut f));
    let rain = r.find("weather.te_rain", &mut f).unwrap();
    let t = &r.types[rain];
    assert_eq!(t.texname, "ball");
    assert_eq!(t.rgb, [1.0, 128.0 / 255.0, 0.0]);
    assert_eq!(t.looks.kind, RenderType::TexturedSpark);
    assert_eq!(t.looks.stretch, -40.0);
    assert_eq!(t.loaded, Loaded::Strong);
    // Old-style alphachange: -1 / die * alpha.
    assert!((t.alphachange - (-0.25)).abs() < 1e-6);
    let splash = t.cliptype.unwrap();
    assert_eq!(r.types[splash].name, "rainsplash");
    // No texture, no scale -> spark.
    assert_eq!(r.types[splash].looks.kind, RenderType::Spark);
    assert_eq!(r.types[splash].die, 0.3);
    assert!((r.types[splash].randdie - 0.2).abs() < 1e-6);
    assert_eq!(r.types[splash].velwrand, [50.0, 50.0, 50.0]);
    assert!(r.types[splash].flags & PT_VELOCITY != 0);

    // +name chains onto the existing effect.
    let chained = r.types[splash].assoc.unwrap();
    let c = &r.types[chained];
    assert_eq!(c.name, "+0rainsplash");
    assert_eq!(c.spawnmode, SpawnMode::Ball);
    assert_eq!((c.spawnparam1, c.spawnparam2), (2.0, 3.0));
    assert_eq!((c.looks.blendmode, c.looks.premul), (BlendMode::Premul, 2));
    assert_eq!(c.flags & PT_TRUNDERWATER, PT_TRUNDERWATER);
    assert_eq!(c.fluidmask, FTECONTENTS_WATER | FTECONTENTS_SLIME);
    assert_eq!(c.sounds[0].vol, 0.5);
    assert_eq!(c.sounds[0].atten, 0.0);
    assert_eq!(c.sounds[0].weight, 1.0);
    assert_eq!(r.sounds_to_precache, vec!["weapons/r_exp3.wav".to_string()]);

    // Redirect and namespace-less lookup.
    assert_eq!(r.find("old_fx", &mut f), Some(rain));
    assert_eq!(r.find("te_rain", &mut f), Some(rain));
    assert_eq!(r.find("nope", &mut f), None);
    let me = r.model_effects("progs/missile.mdl", &mut f);
    assert_eq!(me.flags, EMIT_REPLACE);
    assert_eq!(me.emit, None); // TR_ROCKET isn't defined in this config
}

#[test]
fn reload_replaces_chain_and_weak_does_not_override_strong() {
    let mut r = Registry::new();
    let mut f = files(&[("particles/a.cfg", CFG), ("particles/b.cfg", "r_part namespace a 1\nr_part te_rain\n{\nalpha 0.1\n}\n")]);
    r.load_set("a", false, true, &mut f);
    r.load_set("b", false, true, &mut f);
    let rain = r.find("a.te_rain", &mut f).unwrap();
    assert_eq!(r.types[rain].alpha, 0.5);
    // Reloading the same file via set_desc doesn't duplicate chains.
    let before = r.types.len();
    r.set_desc("a", None, &mut f);
    assert_eq!(r.types.len(), before);
    let s = r.find("a.rainsplash", &mut f).unwrap();
    assert_eq!(r.types[r.types[s].assoc.unwrap()].name, "+0rainsplash");
}

#[test]
fn namespace_loads_on_demand_and_legacy_names() {
    let mut r = Registry::new();
    let mut f = files(&[("particles/fx.cfg", "r_part TR_ROCKET\n{\ntexture x\nscale 2\n}\n")]);
    let id = r.find("fx.t_rocket", &mut f).unwrap();
    assert_eq!(r.types[id].name, "TR_ROCKET");
    assert_eq!(r.types[id].looks.kind, RenderType::Normal);
}

#[test]
fn explosion2_retint() {
    let mut r = Registry::new();
    let mut f = files(&[("particles/q.cfg", "r_part te_explosion2\n{\ntexture x\nscale 2\ncolorindex 0\n}\n")]);
    let id = r.find("q.te_explosion2_32_8", &mut f).unwrap();
    assert_eq!((r.types[id].colorindex, r.types[id].colorrand), (32, 8));
    assert_eq!(r.types[id].name, "te_explosion2_32_8");
}

#[test]
fn ramps_use_palette() {
    let mut pal = vec![0u8; 768];
    pal[3 * 5..3 * 5 + 3].copy_from_slice(&[255, 0, 51]);
    let mut r = Registry::with_palette(&pal);
    let src = "r_part x\n{\ntexture t\nscale 4\nrampmode nearest\nrampindex 5 0.5\nrampindexlist 5 261\nramp 255 0 0 0.3 2\n}\n";
    r.parse_fte("c", false, src);
    let t = &r.types[0];
    assert_eq!(t.rampmode, RampMode::Nearest);
    assert_eq!(t.ramp.len(), 4);
    assert_eq!(t.ramp[0].rgb, [1.0, 0.0, 0.2]);
    assert_eq!((t.ramp[0].alpha, t.ramp[0].scale), (0.5, 4.0));
    assert_eq!(t.ramp[2].alpha, 0.5);
    assert_eq!((t.ramp[3].alpha, t.ramp[3].scale), (0.3, 2.0));
}

#[test]
fn shader_block_is_skipped() {
    let mut r = Registry::new();
    r.parse_fte("c", false, "r_part x\n{\nshader\n{\nmap foo\n{\n}\n}\nscale 3\n}\nr_part y\n{\nscale 1\n}\n");
    assert_eq!(r.types[0].texname, "x");
    assert_eq!(r.types[0].scale, 3.0);
    assert!(r.types[1].is_loaded());
}

const EI: &str = r#"
/* block
comment */
effect TE_SPIKE
count 15
type spark
tex 40 40
color 0x903010 0xFFD030
size 0.4 0.4
alpha 256 256 1024
gravity 1
bounce 1.5
velocityjitter 64 64 64
stretchfactor 0.4
effect TE_SPIKE
type decal
tex 8 16
size 6 6
alpha 256 256 0
// trailing comment
effect blood
type blood
time 1 0.5
"#;

#[test]
fn imports_effectinfo() {
    let mut r = Registry::new();
    r.import_effectinfo("effectinfo", EI, false, None);
    assert!(r.messages.is_empty(), "{:?}", r.messages);
    let mut f = files(&[]);
    let spike = r.find("effectinfo.TE_SPIKE", &mut f).unwrap();
    let t = &r.types[spike];
    assert_eq!(t.looks.kind, RenderType::TexturedSpark);
    assert_eq!(t.count, 15.0);
    assert!((t.rgb[0] - 0x90 as f32 / 255.0).abs() < 1e-6);
    assert!((t.rgbrand[2] - (0x30 - 0x10) as f32 / 255.0).abs() < 1e-6);
    assert_eq!(t.gravity, 800.0);
    assert!((t.looks.stretch - 0.4 * 0.04).abs() < 1e-6);
    // die unspecified: (alpha + alpharand) / -alphachange.
    assert!((t.die - 0.25).abs() < 1e-6);
    // Image 40: column 0, row 5.
    assert_eq!((t.s1, t.s2, t.t1, t.t2), (0.0, 0.125, 0.75, 0.625));
    assert_eq!(t.texsstride, 0.125);

    let d = &r.types[t.assoc.unwrap()];
    assert_eq!(d.name, "0+TE_SPIKE");
    assert_eq!(d.looks.kind, RenderType::CDecal);
    assert_eq!(d.randsmax, 8);
    assert_eq!(d.die, 20.0);

    let b = r.find("effectinfo.blood", &mut f).unwrap();
    assert_eq!(r.types[b].clipbounce, -2.0);
    assert_eq!((r.types[b].die, r.types[b].randdie), (0.5, 0.5));
}

#[test]
fn effectinfo_via_load_set_with_font() {
    let mut r = Registry::new();
    let mut f = files(&[
        ("effectinfo.txt", "effect x\ntex 3 3\n"),
        ("particles/particlefont.txt", "3 0.1 0.2 0.3 0.4\n"),
    ]);
    let id = r.find("effectinfo.x", &mut f).unwrap();
    let t = &r.types[id];
    assert_eq!((t.s1, t.s2, t.t1, t.t2), (0.1, 0.3, 0.4, 0.2));
}

const EI_KEYS: &[&str] = &[
    "count", "countabsolute", "type", "tex", "size", "sizeincrease", "color", "alpha", "velocityoffset",
    "velocityjitter", "originoffset", "originjitter", "gravity", "bounce", "airfriction", "liquidfriction",
    "velocitymultiplier", "trailspacing", "time", "stretchfactor", "blend", "orientation", "lightradius",
    "lightradiusfade", "lightcolor", "lighttime", "lightshadow", "lightcubemapnum", "lightcorona", "rotate",
];

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
    d.join("id1").is_dir().then_some(d)
}

/// Every effectinfo.txt / particles/*.cfg in the game data parses without
/// unknown-token complaints.
#[test]
fn parses_game_data() {
    let Some(base) = basedir() else { return };
    let mut checked = 0;
    let mut dirs: Vec<_> = std::fs::read_dir(&base).unwrap().flatten().filter(|e| e.path().is_dir()).collect();
    dirs.sort_by_key(|e| e.file_name());
    for d in dirs {
        let game = d.file_name().to_string_lossy().into_owned();
        let mut fs = qcore::fs::Fs::new(&base);
        if !fs.add_game_dir(&game) {
            continue;
        }
        let mut names = fs.list("particles/", ".cfg");
        if fs.exists("effectinfo.txt") {
            names.push("effectinfo.txt".into());
        }
        for n in names {
            let mut r = Registry::new();
            let mut src = |p: &str| fs.open(p);
            let set = n.trim_start_matches("particles/").trim_end_matches(".cfg").trim_end_matches(".txt");
            assert!(r.load_set(set, false, true, &mut src), "{game}/{n}");
            let bad: Vec<_> = r
                .messages
                .iter()
                .filter(|m| m.contains("not recognised") || m.contains("Unknown") || m.contains("Bad"))
                // Known keywords with malformed args (e.g. mjolnir's bare
                // `originjitter`) are data bugs; C reports them too.
                .filter(|m| {
                    let tok = m.rsplit(": ").next().unwrap_or("").split(' ').next().unwrap_or("");
                    !EI_KEYS.contains(&tok)
                })
                .collect();
            assert!(bad.is_empty(), "{game}/{n}: {bad:?}");
            assert!(r.types.iter().any(|t| t.is_loaded()), "{game}/{n}: nothing loaded");
            checked += 1;
        }
    }
    eprintln!("checked {checked} particle configs");
}
