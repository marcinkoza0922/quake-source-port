//! DarkPlaces `effectinfo.txt` importer (P_ImportEffectInfo).
use crate::Registry;
use crate::num::{atof, atoi, strtoul};
use crate::types::*;
use crate::TypeId;
use qcore::cmd::com_parse;
use std::f32::consts::PI;

/// Per-image texcoords in `particles/particlefont`: [s1, s2, t1, t2].
fn tex_images(font: Option<&str>) -> Vec<[f32; 4]> {
    let mut ti: Vec<[f32; 4]> = (0..256)
        .map(|i| {
            let (c, r) = ((i & 7) as f32, (i >> 3) as f32);
            [c / 8.0, (c + 1.0) / 8.0, (r + 1.0) / 8.0, r / 8.0]
        })
        .collect();
    for line in font.unwrap_or("").lines() {
        let mut toks = Vec::new();
        let mut s = line;
        while let Some((t, r)) = com_parse(s) {
            toks.push(t);
            s = r;
        }
        // Index and four coords; shorter lines are ignored.
        if toks.len() >= 5 {
            let i = atoi(toks[0]);
            if (0..256).contains(&i) {
                let v = |k: usize| atof(toks[k]);
                ti[i as usize] = [v(1), v(3), v(4), v(2)];
            }
        }
    }
    ti
}

fn finish_effectinfo(t: &mut PartType, blood_decal: bool, opts: &RenderOptions, warn: &mut Vec<String>) {
    match t.looks.kind {
        RenderType::CDecal => {
            if t.die == 9999.0 {
                t.die = 20.0;
            }
            t.alphachange = -(t.alpha / t.die);
        }
        RenderType::UDecal => {
            t.looks.stretch *= 1.0 / std::f32::consts::SQRT_2;
            t.scale *= t.looks.stretch;
            t.scalerand *= t.looks.stretch;
            t.scaledelta *= t.looks.stretch;
            t.looks.stretch = 1.0;
        }
        RenderType::Normal => {
            t.scale *= 2.0 * t.looks.stretch;
            t.scalerand *= 2.0 * t.looks.stretch;
            t.scaledelta *= 2.0 * 2.0 * t.looks.stretch;
            t.looks.stretch = 1.0;
        }
        _ => {}
    }
    if blood_decal {
        t.clipbounce = -2.0;
    }
    if t.looks.kind == RenderType::TexturedSpark {
        t.looks.stretch *= 0.04;
        if t.looks.stretch < 0.0 {
            t.looks.stretch = 0.000001;
        }
    }
    if t.die == 9999.0 {
        t.die = if t.alphachange != 0.0 { (t.alpha + t.alpharand) / -t.alphachange } else { 15.0 };
    }
    t.looks.minstretch = 0.5;
    t.finish(opts, warn);
}

impl Registry {
    fn finish_ei(&mut self, id: TypeId, blood: bool) {
        let opts = self.opts;
        let mut warn = Vec::new();
        finish_effectinfo(&mut self.types[id], blood, &opts, &mut warn);
        self.messages.extend(warn);
    }

    /// Imports DP effectinfo text into namespace `config`.
    pub fn import_effectinfo(&mut self, config: &str, text: &str, weak: bool, font: Option<&str>) {
        let ti = tex_images(font);
        let mut cur: Option<TypeId> = None;
        let mut blood = false;
        let mut rest = text;
        while !rest.is_empty() {
            rest = rest.trim_start_matches([' ', '\t']);
            if let Some(r) = rest.strip_prefix("/*") {
                rest = r.find("*/").map_or("", |i| &r[i + 2..]);
                continue;
            }
            let (line, next) = match rest.find('\n') {
                Some(i) => (&rest[..i], &rest[i + 1..]),
                None => (rest, ""),
            };
            rest = next;
            let mut arg: Vec<&str> = Vec::new();
            let mut s = line;
            while let Some((t, r)) = com_parse(s) {
                if arg.len() < 8 {
                    arg.push(t);
                }
                s = r;
            }
            let args = arg.len();
            if args == 0 {
                continue;
            }
            let f = |i: usize| atof(arg[i]);

            if arg[0] == "effect" {
                if let Some(id) = cur {
                    self.finish_ei(id, blood);
                }
                blood = false;
                let name = arg.get(1).copied().unwrap_or("");
                let mut id = self.allocate(config, name);
                if self.types[id].is_loaded() {
                    let mut ok = false;
                    for i in 0..64 {
                        let parent = id;
                        id = self.allocate(config, &format!("{i}+{name}"));
                        if !self.types[id].is_loaded() {
                            self.types[parent].assoc = Some(id);
                            ok = true;
                            break;
                        }
                    }
                    if !ok {
                        self.msg("Too many duplicate names, gave up".into());
                        cur = None;
                        break;
                    }
                }
                let t = &mut self.types[id];
                t.reset_to_defaults();
                t.loaded = if weak { Loaded::Weak } else { Loaded::Strong };
                t.scale = 1.0;
                t.alpha = 0.0;
                t.alpharand = 1.0;
                t.alphachange = -1.0;
                t.die = 9999.0;
                t.texname = "particles/particlefont".into();
                t.rgb = [1.0; 3];
                t.colorindex = -1;
                t.spawnchance = 1.0;
                t.looks.scalefactor = 2.0;
                t.looks.invscalefactor = 0.0;
                t.looks.kind = RenderType::Normal;
                t.looks.blendmode = BlendMode::Premul;
                t.looks.premul = 1;
                t.looks.stretch = 1.0;
                t.dl_time = 0.0;
                let [s1, s2, t1, t2] = ti[63];
                (t.s1, t.s2, t.t1, t.t2) = (s1, s2, t1, t2);
                t.texsstride = 0.0;
                t.randsmax = 1;
                cur = Some(id);
                continue;
            }
            let Some(id) = cur else {
                self.msg("Bad effectinfo file".into());
                break;
            };
            let mut msg = None;
            let t = &mut self.types[id];
            match (arg[0], args) {
                ("countabsolute", 2) => t.countextra = f(1),
                ("count", 2) => t.count = f(1),
                ("type", 2) => {
                    let (kind, bm, pm) = match arg[1] {
                        "decal" | "cdecal" => (RenderType::CDecal, BlendMode::InvModC, 2),
                        "udecal" => (RenderType::UDecal, BlendMode::InvModC, 2),
                        "alphastatic" => (RenderType::Normal, BlendMode::Premul, 1),
                        "static" | "smoke" | "bubble" => (RenderType::Normal, BlendMode::Premul, 2),
                        "spark" => (RenderType::TexturedSpark, BlendMode::Premul, 2),
                        "blood" => {
                            t.gravity = 800.0;
                            blood = true;
                            (RenderType::Normal, BlendMode::InvModC, 2)
                        }
                        "beam" => (RenderType::Beam, BlendMode::Premul, 2),
                        "snow" => {
                            t.flurry = 32.0;
                            (RenderType::Normal, BlendMode::Premul, 2)
                        }
                        other => {
                            msg = Some(format!("effectinfo type {other} not supported"));
                            (t.looks.kind, t.looks.blendmode, t.looks.premul)
                        }
                    };
                    t.looks.kind = kind;
                    t.looks.blendmode = bm;
                    t.looks.premul = pm;
                }
                ("tex", 3) => {
                    let mini = (atoi(arg[1]) & 255) as usize;
                    let maxi = atoi(arg[2]);
                    let [s1, s2, t1, t2] = ti[mini];
                    (t.s1, t.s2, t.t1, t.t2) = (s1, s2, t1, t2);
                    t.texsstride = ti[(mini + 1) & 255][0] - ti[mini][0];
                    t.randsmax = (maxi - mini as i32).max(1);
                }
                ("size", 3) => {
                    t.scale = f(1);
                    t.scalerand = f(2) - f(1);
                }
                ("sizeincrease", 2) => t.scaledelta = f(1),
                ("color", 3) => {
                    let (c1, c2) = (strtoul(arg[1], 0).0, strtoul(arg[2], 0).0);
                    for i in 0..3 {
                        let sh = 16 - i * 8;
                        let (a, b) = ((c1 >> sh) & 0xff, (c2 >> sh) & 0xff);
                        t.rgb[i] = a as f32 / 255.0;
                        t.rgbrand[i] = (b as i32 - a as i32) as f32 / 255.0;
                        t.rgbrandsync[i] = 1.0;
                    }
                }
                ("alpha", 4) => {
                    let (a1, a2, fade) = (f(1), f(2), f(3));
                    if a1 > a2 {
                        t.alpha = a2 / 256.0;
                        t.alpharand = (a1 - a2) / 256.0;
                    } else {
                        t.alpha = a1 / 256.0;
                        t.alpharand = (a2 - a1) / 256.0;
                    }
                    t.alphachange = -fade / 256.0;
                }
                ("velocityoffset", 4) => t.velbias = [f(1), f(2), f(3)],
                ("velocityjitter", 4) => t.velwrand = [f(1), f(2), f(3)],
                ("originoffset", 4) => t.orgbias = [f(1), f(2), f(3)],
                ("originjitter", 4) => t.orgwrand = [f(1), f(2), f(3)],
                ("gravity", 2) => t.gravity = 800.0 * f(1),
                ("bounce", 2) => {
                    t.clipbounce = f(1);
                    if t.clipbounce < 0.0 {
                        t.cliptype = Some(id);
                    }
                }
                ("airfriction", 2) => t.friction = [f(1); 3],
                ("liquidfriction", 2) => {}
                ("underwater", 1) => t.flags |= PT_TRUNDERWATER,
                ("notunderwater", 1) => t.flags |= PT_TROVERWATER,
                ("velocitymultiplier", 2) => t.veladd = f(1),
                ("trailspacing", 2) => {
                    t.countspacing = f(1);
                    t.count = 1.0 / t.countspacing;
                }
                ("time", 3) => {
                    t.die = f(1);
                    t.randdie = f(2) - t.die;
                    if t.randdie < 0.0 {
                        t.die = f(2);
                        t.randdie = f(1) - t.die;
                    }
                }
                ("stretchfactor", 2) => t.looks.stretch = f(1),
                ("blend", 2) => match arg[1] {
                    "invmod" => (t.looks.blendmode, t.looks.premul) = (BlendMode::InvModC, 2),
                    "alpha" => (t.looks.blendmode, t.looks.premul) = (BlendMode::Premul, 1),
                    "add" => (t.looks.blendmode, t.looks.premul) = (BlendMode::Premul, 2),
                    o => msg = Some(format!("effectinfo 'blend {o}' not supported")),
                },
                ("orientation", 2) => match arg[1] {
                    "billboard" => t.looks.kind = RenderType::Normal,
                    "spark" => t.looks.kind = RenderType::TexturedSpark,
                    "oriented" => {
                        if t.looks.kind != RenderType::CDecal {
                            t.looks.kind = RenderType::UDecal;
                        }
                    }
                    "beam" => t.looks.kind = RenderType::Beam,
                    o => msg = Some(format!("effectinfo 'orientation {o}' not supported")),
                },
                ("lightradius", 2) => t.dl_radius = [f(1), 0.0],
                ("lightradiusfade", 2) => t.dl_decay[3] = f(1),
                ("lightcolor", 4) => t.dl_rgb = [f(1), f(2), f(3)],
                ("lighttime", 2) => t.dl_time = f(1),
                ("lightshadow", 2) => {
                    t.flags = (t.flags & !PT_NODLSHADOW) | if atoi(arg[1]) == 0 { PT_NODLSHADOW } else { 0 }
                }
                ("lightcubemapnum", 2) => t.dl_cubemapnum = atoi(arg[1]),
                ("lightcorona", 3) => {
                    t.dl_corona_intensity = f(1) * 0.25;
                    t.dl_corona_scale = f(2);
                }
                ("staincolor" | "stainalpha" | "stainsize" | "staintex", 3) | ("stainless", 2) => {}
                ("rotate", 5) => {
                    let d = PI / 180.0;
                    t.rotationstartmin = f(1) * d + PI / 4.0;
                    t.rotationstartrand = (f(2) - f(1)) * d;
                    t.rotationmin = f(3) * d;
                    t.rotationrand = (f(4) - f(3)) * d;
                }
                _ => {
                    msg = Some(format!(
                        "Particle effect token not recognised, or invalid args: {}",
                        arg[..args.min(6)].join(" ")
                    ))
                }
            }
            if let Some(m) = msg {
                self.msg(m);
            }
        }
        if let Some(id) = cur {
            self.finish_ei(id, blood);
        }
        self.dirty = true;
    }
}
