//! FTE `r_part` config parser (PScript_ParseParticleEffectFile).
use crate::num::{atof, atoi, strtoul};
use crate::types::*;
use crate::{Registry, TypeId};
use qcore::cmd::Args;
use std::f32::consts::PI;

const DEG: f32 = PI / 180.0;

struct Lines<'a> {
    rest: &'a str,
}

impl<'a> Lines<'a> {
    /// PScript_ReadLine: next line including its newline, None at EOF.
    fn next(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            return None;
        }
        let end = self.rest.find('\n').map_or(self.rest.len(), |i| i + 1);
        let (l, r) = self.rest.split_at(end);
        self.rest = r;
        Some(l)
    }
}

fn trim_ws(s: &str) -> &str {
    s.trim_start_matches(|c: char| c <= ' ')
}

impl Registry {
    pub(crate) fn parse_fte(&mut self, config: &str, mut weak: bool, text: &str) {
        let mut ns = config.to_string();
        let mut lines = Lines { rest: text };
        let mut pending: Option<&str> = None;
        loop {
            let line = match pending.take().or_else(|| lines.next()) {
                Some(l) => l,
                None => return,
            };
            let args = Args::tokenize(line);
            let var = args.argv(0);
            match var {
                "r_effect" | "r_trail" => {
                    self.associate_effect(&args);
                    continue;
                }
                "r_partredirect" => {
                    self.redirect(&args);
                    continue;
                }
                "r_part" => {}
                _ => {
                    if !var.is_empty() {
                        self.msg(format!("Unknown particle command \"{var}\""));
                    }
                    continue;
                }
            }
            if args.argc() != 2 {
                if args.argv(1) == "namespace" {
                    ns = args.argv(2).to_string();
                    if args.argc() >= 4 {
                        weak = atoi(args.argv(3)) != 0;
                    }
                } else {
                    self.msg("No name for particle effect".into());
                }
                continue;
            }
            let Some(brace) = lines.next() else { return };
            if !trim_ws(brace).starts_with('{') {
                self.msg("This is a multiline command and should be used within config files".into());
                pending = Some(brace);
                continue;
            }
            let raw = args.argv(1);
            let plus = raw.starts_with('+');
            let mut id = self.allocate(&ns, raw.strip_prefix('+').unwrap_or(raw));

            if weak && self.types[id].loaded == Loaded::Strong {
                let mut depth = 1;
                loop {
                    let Some(l) = lines.next() else { return };
                    let l = trim_ws(l);
                    if l.starts_with('{') {
                        depth += 1;
                    } else if l.starts_with('}') {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                continue;
            }

            if plus {
                if self.types[id].is_loaded() {
                    let mut ok = false;
                    for i in 0..64 {
                        let parent = id;
                        let newname = format!("+{i}{}", self.types[id].name);
                        id = self.allocate(&ns, &newname);
                        if !self.types[id].is_loaded() {
                            if self.types[parent].assoc.is_some() {
                                self.msg(format!("warning: assoc on particle chain {} overridden", &raw[1..]));
                            }
                            self.types[parent].assoc = Some(id);
                            ok = true;
                            break;
                        }
                    }
                    if !ok {
                        self.msg("Too many duplicate names, gave up".into());
                        return;
                    }
                }
            } else if self.types[id].is_loaded() {
                let mut assoc = self.types[id].assoc;
                let mut guard = 0;
                while let Some(a) = assoc {
                    if !self.types[a].name.starts_with('+') || guard > self.types.len() {
                        break;
                    }
                    self.types[a].loaded = Loaded::No;
                    assoc = self.types[a].assoc;
                    guard += 1;
                }
            }

            self.types[id].reset_to_defaults();
            let mut st = BodyState::default();
            loop {
                let Some(l) = lines.next() else {
                    let n = self.types[id].name.clone();
                    self.msg(format!("Unexpected end of buffer with effect {n}"));
                    return;
                };
                let mut buf = trim_ws(l);
                // `shader` may swallow the next line; loop to reparse it.
                loop {
                    if buf.starts_with('}') {
                        break;
                    }
                    match self.parse_field(id, &ns, buf, &mut lines, &mut st) {
                        Some(next) => buf = trim_ws(next),
                        None => break,
                    }
                }
                if buf.starts_with('}') {
                    break;
                }
            }
            self.finish_fte(id, weak, &st);
        }
    }

    fn finish_fte(&mut self, id: TypeId, weak: bool, st: &BodyState) {
        let opts = self.opts;
        let mut warn = Vec::new();
        let t = &mut self.types[id];
        t.loaded = if weak { Loaded::Weak } else { Loaded::Strong };
        if t.clipcount < 1.0 {
            t.clipcount = 1.0;
        }
        if !st.settype {
            if t.looks.kind == RenderType::Normal && t.texname.is_empty() {
                t.looks.kind = if t.scale != 0.0 { RenderType::SparkFan } else { RenderType::Spark };
            } else if t.looks.kind == RenderType::Spark {
                if !t.texname.is_empty() {
                    t.looks.kind = RenderType::TexturedSpark;
                } else if t.scale != 0.0 {
                    t.looks.kind = RenderType::SparkFan;
                }
            }
        }
        if !st.setalphadelta {
            t.alphachange = (-t.alphachange / t.die) * t.alpha;
        }
        t.finish(&opts, &mut warn);
        if t.looks.kind == RenderType::Beam && !st.setbeamlen {
            t.rotationstartmin = 1.0 / 128.0;
        }
        for s in &t.sounds {
            if !s.name.is_empty() && !self.sounds_to_precache.contains(&s.name) {
                self.sounds_to_precache.push(s.name.clone());
            }
        }
        self.messages.extend(warn);
        self.dirty = true;
    }

    /// Parses one body line. Returns a line to reparse (after `shader`
    /// without a block), else None.
    fn parse_field<'a>(
        &mut self,
        id: TypeId,
        ns: &str,
        buf: &'a str,
        lines: &mut Lines<'a>,
        st: &mut BodyState,
    ) -> Option<&'a str> {
        let a = Args::tokenize(buf);
        let var = a.argv(0);
        let value = a.argv(1);
        let argc = a.argc();
        let f = |i: usize| atof(a.argv(i));
        let pal = |r: &Registry, i: i32| r.palette_rgb(i as usize);

        match var {
            "shader" => {
                let t = &mut self.types[id];
                t.texname = t.name.clone();
                let next = lines.next()?;
                if !trim_ws(next).starts_with('{') {
                    return Some(next);
                }
                let mut nest = 1;
                while nest > 0 {
                    let Some(l) = lines.next() else {
                        let n = self.types[id].name.clone();
                        self.msg(format!("Unexpected end of buffer with effect {n}"));
                        break;
                    };
                    let l = trim_ws(l);
                    if l.starts_with('}') {
                        nest -= 1;
                    }
                    if l.starts_with('{') {
                        nest += 1;
                    }
                }
                return None;
            }
            "assoc" | "inwater" => {
                let r = self.check_association(ns, value, id);
                if var == "assoc" {
                    self.types[id].assoc = r;
                } else {
                    self.types[id].inwater = r;
                }
                return None;
            }
            "cliptype" => {
                let r = self.allocate(ns, value);
                self.types[id].cliptype = Some(r);
                return None;
            }
            "emit" => {
                let r = self.allocate(ns, value);
                self.types[id].emit = Some(r);
                return None;
            }
            "rampindexlist" => {
                for i in 1..argc {
                    let cidx = atoi(a.argv(i));
                    let rgb = pal(self, cidx);
                    let t = &mut self.types[id];
                    t.ramp.push(Ramp { rgb, alpha: if cidx > 255 { 0.5 } else { 1.0 }, scale: t.scale, rotation: 0.0 });
                }
                return None;
            }
            "rampindex" => {
                let cidx = atoi(value);
                let rgb = pal(self, cidx);
                let t = &mut self.types[id];
                let mut alpha = if cidx > 255 { 0.5 } else { 1.0 };
                if argc > 2 {
                    alpha *= f(2);
                }
                let scale = if argc > 3 { f(3) } else { t.scale };
                t.ramp.push(Ramp { rgb, alpha, scale, rotation: 0.0 });
                return None;
            }
            _ => {}
        }

        let mut msgs: Vec<String> = Vec::new();
        let t = &mut self.types[id];
        let tag = format!("{}.{}", t.config, t.name);
        match var {
            "texture" | "linear_texture" | "nearest_texture" | "nearesttexture" => {
                t.texname = value.to_string();
                t.looks.nearest = var.starts_with("nearest");
            }
            "tcoords" => {
                let mut tscale = f(5);
                if tscale <= 0.0 {
                    tscale = 1.0;
                }
                t.s1 = f(1) / tscale;
                t.t1 = f(2) / tscale;
                t.s2 = f(3) / tscale;
                t.t2 = f(4) / tscale;
                t.randsmax = atoi(a.argv(6));
                t.texsstride = if argc > 7 { f(7) } else { 1.0 / tscale };
                if t.randsmax < 1 || t.texsstride == 0.0 {
                    t.randsmax = 1;
                }
            }
            "atlas" => {
                let dims = (f(1) as i32).max(1);
                let i = atoi(a.argv(2));
                let mut m = atoi(a.argv(3));
                if m > (m / dims) * dims + dims - 1 {
                    m = (m / dims) * dims + dims - 1;
                    msgs.push(format!("effect {} wraps across an atlased line", t.name));
                }
                if m < i {
                    m = i;
                }
                let d = 1.0 / dims as f32;
                t.s1 = d * (i % dims) as f32;
                t.s2 = d * (1 + i % dims) as f32;
                t.t1 = d * (i / dims) as f32;
                t.t2 = d * (1 + i / dims) as f32;
                t.randsmax = m - i + 1;
                t.texsstride = t.s2 - t.s1;
            }
            "rotation" => {
                t.rotationstartmin = f(1) * DEG;
                t.rotationstartrand = if argc > 2 { f(2) * DEG - t.rotationstartmin } else { 0.0 };
                t.rotationmin = f(3) * DEG;
                t.rotationrand = if argc > 4 { f(4) * DEG - t.rotationmin } else { 0.0 };
            }
            "rotationstart" => {
                t.rotationstartmin = f(1) * DEG;
                t.rotationstartrand = if argc > 2 { f(2) * DEG - t.rotationstartmin } else { 0.0 };
            }
            "rotationspeed" => {
                t.rotationmin = f(1) * DEG;
                t.rotationrand = if argc > 2 { f(2) * DEG - t.rotationmin } else { 0.0 };
            }
            "beamtexstep" => {
                t.rotationstartmin = 1.0 / f(1);
                t.rotationstartrand = 0.0;
                st.setbeamlen = true;
            }
            "beamtexspeed" => t.rotationmin = f(1),
            "scale" => {
                t.scale = f(1);
                if argc > 2 {
                    t.scalerand = f(2) - t.scale;
                }
            }
            "scalerand" => t.scalerand = f(1),
            "scalefactor" => t.looks.scalefactor = f(1),
            "scaledelta" => t.scaledelta = f(1),
            "stretchfactor" => {
                t.looks.stretch = f(1);
                t.looks.minstretch = if argc > 2 { f(2) } else { 0.0 };
            }
            "step" => {
                t.countspacing = f(1);
                t.count = 1.0 / f(1);
                if argc > 2 {
                    t.countrand = 1.0 / f(2);
                }
                if argc > 3 {
                    t.countextra = f(3);
                }
            }
            "count" => {
                t.countspacing = 0.0;
                t.count = f(1);
                if argc > 2 {
                    t.countrand = f(2);
                }
                if argc > 3 {
                    t.countextra = f(3);
                }
            }
            "rainfrequency" => t.rainfrequency = f(1),
            "alpha" => t.alpha = f(1),
            "alpharand" => t.alpharand = f(1),
            "alphachange" => t.alphachange = f(1),
            "alphadelta" => {
                t.alphachange = f(1);
                st.setalphadelta = true;
            }
            "die" => {
                t.die = f(1);
                if argc > 2 {
                    let (mut mn, mut mx) = (t.die, f(2));
                    if mn > mx {
                        std::mem::swap(&mut mn, &mut mx);
                    }
                    t.die = mx;
                    t.randdie = mx - mn;
                }
            }
            "diesubrand" => t.randdie = f(1),
            "randomvel" => {
                t.velbias[0] = 0.0;
                t.velbias[1] = 0.0;
                t.velwrand[0] = f(1);
                t.velwrand[1] = f(1);
                if argc > 3 {
                    t.velbias[2] = f(2);
                    t.velwrand[2] = f(3);
                    t.velwrand[2] -= t.velbias[2];
                    t.velwrand[2] /= 2.0;
                    t.velbias[2] += t.velwrand[2];
                } else if argc > 2 {
                    t.velwrand[2] = f(2);
                    t.velbias[2] = 0.0;
                } else {
                    t.velwrand[2] = t.velwrand[0];
                    t.velbias[2] = 0.0;
                }
            }
            "veladd" => {
                t.veladd = f(1);
                t.randomveladd = if argc > 2 { f(2) - t.veladd } else { 0.0 };
            }
            "orgadd" => {
                t.orgadd = f(1);
                t.randomorgadd = if argc > 2 { f(2) - t.orgadd } else { 0.0 };
            }
            "orgbias" => t.orgbias = [f(1), f(2), f(3)],
            "orgwrand" => t.orgwrand = [f(1), f(2), f(3)],
            "velbias" => t.velbias = [f(1), f(2), f(3)],
            "velwrand" => t.velwrand = [f(1), f(2), f(3)],
            "friction" => {
                t.friction = [f(1); 3];
                if argc > 3 {
                    t.friction[2] = f(3);
                    t.friction[1] = f(2);
                } else if argc > 2 {
                    t.friction[2] = f(2);
                }
            }
            "gravity" => t.gravity = f(1),
            "flurry" => t.flurry = f(1),
            "underwater" | "notunderwater" => {
                t.flags |= if var == "underwater" { PT_TRUNDERWATER } else { PT_TROVERWATER };
                if t.flags & (PT_TRUNDERWATER | PT_TROVERWATER) == (PT_TRUNDERWATER | PT_TROVERWATER) {
                    t.flags &= !PT_TRUNDERWATER;
                    msgs.push(format!("{tag}: both over and under water"));
                }
                if argc == 1 {
                    t.fluidmask = FTECONTENTS_FLUID;
                } else {
                    t.fluidmask = 0;
                    for i in (1..argc).rev() {
                        t.fluidmask |= match a.argv(i) {
                            "water" => FTECONTENTS_WATER,
                            "slime" => FTECONTENTS_SLIME,
                            "lava" => FTECONTENTS_LAVA,
                            "sky" => FTECONTENTS_SKY,
                            "fluid" => FTECONTENTS_FLUID,
                            "solid" => FTECONTENTS_SOLID,
                            "playerclip" => FTECONTENTS_PLAYERCLIP,
                            "none" => 0,
                            v => {
                                msgs.push(format!("{tag}: unknown contents: {v}"));
                                0
                            }
                        };
                    }
                }
            }
            "model" | "viewspace" | "spawnstain" => {}
            "sound" => {
                let mut s = PartSound {
                    name: a.argv(1).to_string(),
                    vol: 1.0,
                    atten: 1.0,
                    pitch: 100.0,
                    delay: 0.0,
                    weight: 0.0,
                };
                let (_, rest) = strtoul(a.argv(2), 0);
                if !rest.trim_start_matches([' ', '\t']).is_empty() {
                    for p in 2..argc {
                        let e = a.argv(p);
                        let lower = e.to_ascii_lowercase();
                        let val = e.split_once('=').map_or("", |(_, v)| v);
                        if lower.starts_with("vol=") || lower.starts_with("volume=") {
                            s.vol = atof(val);
                        } else if lower.starts_with("attn=") || lower.starts_with("atten=") || lower.starts_with("attenuation=") {
                            s.atten = match val {
                                "none" => 0.0,
                                "normal" => 1.0,
                                v => atof(v),
                            };
                        } else if lower.starts_with("pitch=") {
                            s.pitch = atof(val);
                        } else if lower.starts_with("delay=") {
                            s.delay = atof(val);
                        } else if lower.starts_with("weight=") {
                            s.weight = atof(val);
                        } else {
                            msgs.push(format!("Bad named argument: {e}"));
                        }
                    }
                } else {
                    let or = |v: f32, d: f32| if v == 0.0 { d } else { v };
                    s.vol = or(f(2), 1.0);
                    s.atten = or(f(3), 1.0);
                    s.pitch = or(f(4), 100.0);
                    s.delay = f(5);
                    s.weight = f(6);
                }
                if s.weight == 0.0 {
                    s.weight = 1.0;
                }
                t.sounds.push(s);
            }
            "colorindex" => {
                if argc > 2 {
                    t.colorrand = strtoul(a.argv(2), 0).0 as i32;
                }
                t.colorindex = strtoul(value, 0).0 as i32;
            }
            "colorrand" => t.colorrand = atoi(value),
            "citracer" => t.flags |= PT_CITRACER,
            "red" => t.rgb[0] = f(1) / 255.0,
            "green" => t.rgb[1] = f(1) / 255.0,
            "blue" => t.rgb[2] = f(1) / 255.0,
            "rgb" | "rgbf" | "rgbdelta" | "rgbdeltaf" | "rgbrand" | "rgbrandf" | "rgbrandsync" => {
                let div = if matches!(var, "rgb" | "rgbdelta" | "rgbrand") { 255.0 } else { 1.0 };
                let v0 = f(1) / div;
                let mut v = [v0; 3];
                if argc > 3 {
                    v[1] = f(2) / div;
                    v[2] = f(3) / div;
                }
                match var {
                    "rgb" | "rgbf" => t.rgb = v,
                    "rgbdelta" | "rgbdeltaf" => {
                        t.rgbchange = v;
                        if t.rgbchangetime == 0.0 {
                            t.rgbchangetime = t.die;
                        }
                    }
                    "rgbrandsync" => t.rgbrandsync = v,
                    _ => t.rgbrand = v,
                }
            }
            "reddelta" | "greendelta" | "bluedelta" => {
                let i = ["reddelta", "greendelta", "bluedelta"].iter().position(|x| *x == var).unwrap_or(0);
                t.rgbchange[i] = f(1) / 255.0;
                if t.rgbchangetime == 0.0 {
                    t.rgbchangetime = t.die;
                }
            }
            "rgbdeltatime" => t.rgbchangetime = f(1),
            "redrand" => t.rgbrand[0] = f(1) / 255.0,
            "greenrand" => t.rgbrand[1] = f(1) / 255.0,
            "bluerand" => t.rgbrand[2] = f(1) / 255.0,
            "redrandsync" => t.rgbrandsync[0] = f(1),
            "greenrandsync" => t.rgbrandsync[1] = f(1),
            "bluerandsync" => t.rgbrandsync[2] = f(1),
            "stains" => t.stainonimpact = f(1),
            "blend" => {
                t.looks.premul = 0;
                t.looks.blendmode = match value {
                    "adda" | "add" => BlendMode::AddA,
                    "addc" => BlendMode::AddC,
                    "subtract" => BlendMode::Subtract,
                    "invmoda" | "invmod" => BlendMode::InvModA,
                    "invmodc" => BlendMode::InvModC,
                    "blendcolour" | "blendcolor" => BlendMode::BlendColour,
                    "blendalpha" | "blend" => BlendMode::Blend,
                    "premul_subtract" => {
                        t.looks.premul = 1;
                        BlendMode::InvModC
                    }
                    "premul_add" => {
                        t.looks.premul = 2;
                        BlendMode::Premul
                    }
                    "premul_blend" => {
                        t.looks.premul = 1;
                        BlendMode::Premul
                    }
                    _ => BlendMode::Blend,
                };
            }
            "spawnmode" => {
                t.spawnmode = match value {
                    "circle" => SpawnMode::Circle,
                    "ball" => SpawnMode::Ball,
                    "spiral" => SpawnMode::Spiral,
                    "tracer" => SpawnMode::Tracer,
                    "telebox" => SpawnMode::TeleBox,
                    "lavasplash" => SpawnMode::LavaSplash,
                    "uniformcircle" => SpawnMode::UniCircle,
                    "syncfield" => {
                        t.spawnparam1 = 16.0;
                        t.spawnparam2 = 0.0;
                        SpawnMode::Field
                    }
                    "distball" => SpawnMode::DistBall,
                    _ => SpawnMode::Box,
                };
                if argc > 2 {
                    if argc > 3 {
                        t.spawnparam2 = f(3);
                    }
                    t.spawnparam1 = f(2);
                }
            }
            "type" => {
                t.looks.kind = match value {
                    "beam" => RenderType::Beam,
                    "spark" | "linespark" => RenderType::Spark,
                    "sparkfan" | "trianglefan" => RenderType::SparkFan,
                    "texturedspark" => RenderType::TexturedSpark,
                    "decal" | "cdecal" => RenderType::CDecal,
                    "udecal" => RenderType::UDecal,
                    _ => RenderType::Normal,
                };
                st.settype = true;
            }
            "clippeddecal" => {
                if argc >= 2 {
                    t.surfflagmask = strtoul(a.argv(1), 0).0;
                    t.surfflagmatch = t.surfflagmask;
                    if argc >= 3 {
                        t.surfflagmatch = strtoul(a.argv(2), 0).0;
                    }
                }
                t.looks.kind = RenderType::CDecal;
                st.settype = true;
            }
            "isbeam" => {
                t.looks.kind = RenderType::Beam;
                st.settype = true;
            }
            "spawntime" => t.spawntime = f(1),
            "spawnchance" => t.spawnchance = f(1),
            "clipcount" => t.clipcount = f(1),
            "clipbounce" => {
                t.clipbounce = f(1);
                if t.clipbounce < 0.0 && t.cliptype.is_none() {
                    t.cliptype = Some(id);
                }
            }
            "bounce" => {
                t.cliptype = Some(id);
                t.clipbounce = f(1);
            }
            "emitinterval" => t.emittime = f(1),
            "emitintervalrand" => t.emitrand = f(1),
            "emitstart" => t.emitstart = f(1),
            "areaspread" => t.areaspread = f(1),
            "areaspreadvert" => t.areaspreadvert = f(1),
            "offsetspread" => t.spawnvel = f(1),
            "offsetspreadvert" => t.spawnvelvert = f(1),
            "spawnorg" => {
                t.areaspread = f(1);
                t.areaspreadvert = if argc > 2 { f(2) } else { t.areaspread };
            }
            "spawnvel" => {
                t.spawnvel = f(1);
                t.spawnvelvert = if argc > 2 { f(2) } else { t.spawnvel };
            }
            "spawnparam1" => t.spawnparam1 = f(1),
            "spawnparam2" => t.spawnparam2 = f(1),
            "up" => t.orgbias[2] = f(1),
            "rampmode" => {
                t.rampmode = match value {
                    "none" => RampMode::None,
                    "absolute" | "nearest" => RampMode::Nearest,
                    "lerp" => RampMode::Lerp,
                    _ => RampMode::Delta,
                };
            }
            "ramp" => {
                let r0 = f(1) / 255.0;
                let ramp = if argc > 3 {
                    let (alpha, scale) = if argc > 4 {
                        (f(4), if argc > 5 { f(5) } else { t.scaledelta })
                    } else {
                        (t.alpha, t.scaledelta)
                    };
                    Ramp { rgb: [r0, f(2) / 255.0, f(3) / 255.0], alpha, scale, rotation: 0.0 }
                } else {
                    Ramp { rgb: [r0; 3], alpha: t.alpha, scale: t.scaledelta, rotation: 0.0 }
                };
                t.ramp.push(ramp);
            }
            "perframe" => t.flags |= PT_INVFRAMETIME,
            "averageout" => t.flags |= PT_AVERAGETRAIL,
            "nostate" => t.flags |= PT_NOSTATE,
            "nospreadfirst" => t.flags |= PT_NOSPREADFIRST,
            "nospreadlast" => t.flags |= PT_NOSPREADLAST,
            "lightradius" => {
                t.dl_radius = [f(1); 2];
                if argc > 2 {
                    t.dl_radius[1] = f(2);
                }
                t.dl_radius[1] -= t.dl_radius[0];
            }
            "lightradiusfade" => t.dl_decay[3] = f(1),
            "lightrgb" => t.dl_rgb = [f(1), f(2), f(3)],
            "lightrgbfade" => {
                t.dl_decay[0] = f(1);
                t.dl_decay[1] = f(2);
                t.dl_decay[2] = f(3);
            }
            "lightcorona" => {
                t.dl_corona_intensity = f(1);
                t.dl_corona_scale = f(2);
            }
            "lighttime" => t.dl_time = f(1),
            "lightshadows" => t.flags = (t.flags & !PT_NODLSHADOW) | if f(1) != 0.0 { 0 } else { PT_NODLSHADOW },
            "lightcubemap" => t.dl_cubemapnum = atoi(value),
            "lightscales" => t.dl_scales = [f(1), f(2), f(3)],
            _ => {}
        }
        self.messages.extend(msgs);
        None
    }
}

#[derive(Default)]
struct BodyState {
    settype: bool,
    setalphadelta: bool,
    setbeamlen: bool,
}
