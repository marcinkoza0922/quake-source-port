//! Particle type definitions (part_type_t, plooks_t, ramp_t) from vkQuake
//! `r_part_fte.c`.

/// Index of a particle type in [`crate::Registry::types`].
pub type TypeId = usize;

/// Blend modes (blendmode_t).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// SRC_ALPHA, ONE_MINUS_SRC_ALPHA
    #[default]
    Blend,
    /// SRC_COLOR, ONE_MINUS_SRC_COLOR
    BlendColour,
    /// SRC_ALPHA, ONE
    AddA,
    /// SRC_COLOR, ONE
    AddC,
    /// SRC_ALPHA, ONE_MINUS_SRC_COLOR
    Subtract,
    /// ZERO, ONE_MINUS_SRC_ALPHA
    InvModA,
    /// ZERO, ONE_MINUS_SRC_COLOR
    InvModC,
    /// ONE, ONE_MINUS_SRC_ALPHA
    Premul,
}

/// Render type (plooks_t::type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RenderType {
    #[default]
    Normal,
    Spark,
    SparkFan,
    TexturedSpark,
    Beam,
    CDecal,
    UDecal,
    Invisible,
}

/// Static render state shared by particles of a type (plooks_t).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Looks {
    pub kind: RenderType,
    pub blendmode: BlendMode,
    pub nearest: bool,
    pub scalefactor: f32,
    pub invscalefactor: f32,
    pub stretch: f32,
    /// Limits the particle's length to a multiple of its width.
    pub minstretch: f32,
    /// 0: direct rgba. 1: rgb*a,a (blend). 2: rgb*a,0 (add).
    pub premul: u8,
}

/// Spawn modes (SM_*).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpawnMode {
    #[default]
    Box,
    Circle,
    Ball,
    Spiral,
    Tracer,
    TeleBox,
    LavaSplash,
    UniCircle,
    Field,
    DistBall,
    MeshSurface,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RampMode {
    #[default]
    None,
    Delta,
    Nearest,
    Lerp,
}

/// Deltas or absolutes depending on the ramp mode.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ramp {
    pub rgb: [f32; 3],
    pub alpha: f32,
    pub scale: f32,
    pub rotation: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartSound {
    pub name: String,
    pub vol: f32,
    pub atten: f32,
    pub delay: f32,
    pub pitch: f32,
    pub weight: f32,
}

pub const PT_VELOCITY: u32 = 0x0001;
pub const PT_FRICTION: u32 = 0x0002;
pub const PT_CHANGESCOLOUR: u32 = 0x0004;
pub const PT_CITRACER: u32 = 0x0008;
pub const PT_INVFRAMETIME: u32 = 0x0010;
pub const PT_AVERAGETRAIL: u32 = 0x0020;
pub const PT_NOSTATE: u32 = 0x0040;
pub const PT_NOSPREADFIRST: u32 = 0x0080;
pub const PT_NOSPREADLAST: u32 = 0x0100;
pub const PT_TROVERWATER: u32 = 0x0200;
pub const PT_TRUNDERWATER: u32 = 0x0400;
pub const PT_NODLSHADOW: u32 = 0x0800;
pub const PT_WORLDSPACERAND: u32 = 0x1000;

pub const FTECONTENTS_EMPTY: u32 = 0;
pub const FTECONTENTS_SOLID: u32 = 1;
pub const FTECONTENTS_WATER: u32 = 2;
pub const FTECONTENTS_SLIME: u32 = 4;
pub const FTECONTENTS_LAVA: u32 = 8;
pub const FTECONTENTS_SKY: u32 = 16;
pub const FTECONTENTS_FLUID: u32 =
    FTECONTENTS_WATER | FTECONTENTS_SLIME | FTECONTENTS_LAVA | FTECONTENTS_SKY;
pub const FTECONTENTS_PLAYERCLIP: u32 = 0;

/// How a type was loaded (part_type_t::loaded).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Loaded {
    #[default]
    No,
    /// Implicitly / weakly loaded; may be replaced by other weak configs.
    Weak,
    /// Explicitly loaded; weak configs don't replace it.
    Strong,
}

/// One particle effect description (part_type_t, static parts only).
#[derive(Clone, Debug, PartialEq)]
pub struct PartType {
    pub name: String,
    pub config: String,
    pub texname: String,
    pub sounds: Vec<PartSound>,

    pub rgb: [f32; 3],
    pub alpha: f32,
    pub rgbchange: [f32; 3],
    pub alphachange: f32,
    pub rgbrand: [f32; 3],
    pub alpharand: f32,
    /// Palette colour (-1: none).
    pub colorindex: i32,
    pub colorrand: i32,
    pub rgbchangetime: f32,
    pub rgbrandsync: [f32; 3],
    pub scale: f32,
    pub scalerand: f32,
    pub die: f32,
    pub randdie: f32,
    pub veladd: f32,
    pub randomveladd: f32,
    pub orgadd: f32,
    pub randomorgadd: f32,
    pub spawnvel: f32,
    pub spawnvelvert: f32,
    pub orgbias: [f32; 3],
    pub velbias: [f32; 3],
    pub orgwrand: [f32; 3],
    pub velwrand: [f32; 3],
    pub flurry: f32,
    pub surfflagmatch: u32,
    pub surfflagmask: u32,

    pub s1: f32,
    pub t1: f32,
    pub s2: f32,
    pub t2: f32,
    pub texsstride: f32,
    pub randsmax: i32,

    pub looks: Looks,

    pub spawntime: f32,
    pub spawnchance: f32,
    pub rotationstartmin: f32,
    pub rotationstartrand: f32,
    pub rotationmin: f32,
    pub rotationrand: f32,

    pub scaledelta: f32,
    pub countextra: f32,
    pub count: f32,
    pub countrand: f32,
    pub countspacing: f32,
    pub countoverflow: f32,
    pub rainfrequency: f32,

    pub assoc: Option<TypeId>,
    pub cliptype: Option<TypeId>,
    pub inwater: Option<TypeId>,
    pub clipcount: f32,
    pub emit: Option<TypeId>,
    pub emittime: f32,
    pub emitrand: f32,
    pub emitstart: f32,

    pub areaspread: f32,
    pub areaspreadvert: f32,
    pub spawnparam1: f32,
    pub spawnparam2: f32,
    pub spawnmode: SpawnMode,

    pub gravity: f32,
    pub friction: [f32; 3],
    pub clipbounce: f32,
    pub stainonimpact: f32,

    pub dl_rgb: [f32; 3],
    pub dl_radius: [f32; 2],
    pub dl_time: f32,
    pub dl_decay: [f32; 4],
    pub dl_corona_intensity: f32,
    pub dl_corona_scale: f32,
    pub dl_scales: [f32; 3],
    pub dl_cubemapnum: i32,

    pub rampmode: RampMode,
    pub ramp: Vec<Ramp>,

    pub loaded: Loaded,
    pub flags: u32,
    pub fluidmask: u32,
}

impl PartType {
    /// A freshly allocated, unloaded type (P_GetParticleType: memset 0).
    pub fn empty(config: &str, name: &str) -> Self {
        PartType {
            name: name.to_string(),
            config: config.to_string(),
            texname: String::new(),
            sounds: Vec::new(),
            rgb: [0.0; 3],
            alpha: 0.0,
            rgbchange: [0.0; 3],
            alphachange: 0.0,
            rgbrand: [0.0; 3],
            alpharand: 0.0,
            colorindex: 0,
            colorrand: 0,
            rgbchangetime: 0.0,
            rgbrandsync: [0.0; 3],
            scale: 0.0,
            scalerand: 0.0,
            die: 0.0,
            randdie: 0.0,
            veladd: 0.0,
            randomveladd: 0.0,
            orgadd: 0.0,
            randomorgadd: 0.0,
            spawnvel: 0.0,
            spawnvelvert: 0.0,
            orgbias: [0.0; 3],
            velbias: [0.0; 3],
            orgwrand: [0.0; 3],
            velwrand: [0.0; 3],
            flurry: 0.0,
            surfflagmatch: 0,
            surfflagmask: 0,
            s1: 0.0,
            t1: 0.0,
            s2: 0.0,
            t2: 0.0,
            texsstride: 0.0,
            randsmax: 0,
            looks: Looks::default(),
            spawntime: 0.0,
            spawnchance: 0.0,
            rotationstartmin: 0.0,
            rotationstartrand: 0.0,
            rotationmin: 0.0,
            rotationrand: 0.0,
            scaledelta: 0.0,
            countextra: 0.0,
            count: 0.0,
            countrand: 0.0,
            countspacing: 0.0,
            countoverflow: 0.0,
            rainfrequency: 0.0,
            assoc: None,
            cliptype: None,
            inwater: None,
            clipcount: 0.0,
            emit: None,
            emittime: 0.0,
            emitrand: 0.0,
            emitstart: 0.0,
            areaspread: 0.0,
            areaspreadvert: 0.0,
            spawnparam1: 0.0,
            spawnparam2: 0.0,
            spawnmode: SpawnMode::Box,
            gravity: 0.0,
            friction: [0.0; 3],
            clipbounce: 0.0,
            stainonimpact: 0.0,
            dl_rgb: [0.0; 3],
            dl_radius: [0.0; 2],
            dl_time: 0.0,
            dl_decay: [0.0; 4],
            dl_corona_intensity: 0.0,
            dl_corona_scale: 0.0,
            dl_scales: [0.0; 3],
            dl_cubemapnum: 0,
            rampmode: RampMode::None,
            ramp: Vec::new(),
            loaded: Loaded::No,
            flags: 0,
            fluidmask: 0,
        }
    }

    /// P_ResetToDefaults: keeps name/config, sets the non-zero defaults.
    pub fn reset_to_defaults(&mut self) {
        let (name, config) = (std::mem::take(&mut self.name), std::mem::take(&mut self.config));
        *self = PartType::empty(&config, &name);
        self.rainfrequency = 1.0;
        self.fluidmask = FTECONTENTS_FLUID;
        self.alpha = 1.0;
        self.alphachange = 1.0;
        self.clipbounce = 0.8;
        self.clipcount = 1.0;
        self.colorindex = -1;
        self.rotationstartmin = -std::f32::consts::PI;
        self.rotationstartrand = std::f32::consts::PI - self.rotationstartmin;
        self.spawnchance = 1.0;
        self.dl_rgb = [1.0; 3];
        self.dl_corona_intensity = 0.25;
        self.dl_corona_scale = 0.5;
        self.dl_scales = [0.0, 1.0, 1.0];
        self.looks.stretch = 0.05;
        self.randsmax = 1;
        self.s2 = 1.0;
        self.t2 = 1.0;
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded != Loaded::No
    }
}

/// Renderer feature toggles consulted by FinishParticleType
/// (r_part_sparks, r_part_sparks_textured, r_part_sparks_trifan, r_part_beams).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderOptions {
    pub sparks: f32,
    pub sparks_textured: bool,
    pub sparks_trifan: bool,
    pub beams: f32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { sparks: 1.0, sparks_textured: true, sparks_trifan: true, beams: 1.0 }
    }
}

fn dot(a: [f32; 3]) -> f32 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

impl PartType {
    /// FinishParticleType (minus texture loading, which the renderer does).
    /// Returns warnings.
    pub fn finish(&mut self, opts: &RenderOptions, warn: &mut Vec<String>) {
        if self.gravity != 0.0
            || self.veladd != 0.0
            || self.spawnvel != 0.0
            || self.spawnvelvert != 0.0
            || dot(self.velwrand) != 0.0
            || dot(self.velbias) != 0.0
            || self.flurry != 0.0
        {
            self.flags |= PT_VELOCITY;
        }
        if dot(self.velbias) != 0.0 || dot(self.velwrand) != 0.0 || dot(self.orgwrand) != 0.0 {
            self.flags |= PT_WORLDSPACERAND;
        }
        if self.friction != [0.0; 3] {
            self.flags |= PT_FRICTION;
        }
        if self.dl_decay[3] != 0.0 && self.dl_time == 0.0 {
            self.dl_time = self.dl_radius[0] / self.dl_decay[3];
        }
        if self.looks.scalefactor > 1.0 && self.looks.invscalefactor == 0.0 {
            self.scale *= self.looks.scalefactor;
            self.scalerand *= self.looks.scalefactor;
            self.looks.scalefactor = 1.0;
        }
        self.looks.invscalefactor = 1.0 - self.looks.scalefactor;

        use RenderType as R;
        if self.looks.kind == R::TexturedSpark && self.looks.stretch == 0.0 {
            self.looks.stretch = 0.05;
        }
        if self.looks.kind == R::Spark && opts.sparks < 0.0 {
            self.looks.kind = R::Invisible;
        }
        if self.looks.kind == R::TexturedSpark && !opts.sparks_textured {
            self.looks.kind = R::Spark;
        }
        if self.looks.kind == R::SparkFan && !opts.sparks_trifan {
            self.looks.kind = R::Spark;
        }
        if self.looks.kind == R::Spark && opts.sparks == 0.0 {
            self.looks.kind = R::Invisible;
        }
        if self.looks.kind == R::Beam && opts.beams <= 0.0 {
            self.looks.kind = R::Invisible;
        }
        if self.rampmode != RampMode::None && self.ramp.is_empty() {
            self.rampmode = RampMode::None;
            warn.push(format!("{}.{}: Particle has a ramp mode but no ramp", self.config, self.name));
        } else if !self.ramp.is_empty() && self.rampmode == RampMode::None {
            warn.push(format!("{}.{}: Particle has a ramp but no ramp mode", self.config, self.name));
        }
    }
}
