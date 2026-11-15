//! Small math helpers and the RNG used by the particle system.
use qcore::math::Vec3;
use std::f32::consts::PI;

const SINTABLE_ENTRIES: usize = 128;

// Precomputed libm `sinf`/`cosf` values for `i * PI / 64.0` (`0..128`) so
// particle billboarding and spiral/field spawns avoid runtime trig calls while
// matching `f32::sin`/`f32::cos` bit-for-bit.
const PSIN: [f32; SINTABLE_ENTRIES] = [
    0.0_f32, 0.049067676_f32, 0.09801714_f32, 0.14673047_f32,
    0.19509032_f32, 0.2429802_f32, 0.2902847_f32, 0.33688986_f32,
    0.38268346_f32, 0.42755508_f32, 0.47139674_f32, 0.51410276_f32,
    0.55557024_f32, 0.5956993_f32, 0.63439333_f32, 0.671559_f32,
    0.70710677_f32, 0.7409512_f32, 0.77301043_f32, 0.8032075_f32,
    0.83146966_f32, 0.85772866_f32, 0.8819213_f32, 0.9039893_f32,
    0.9238795_f32, 0.94154406_f32, 0.95694035_f32, 0.97003126_f32,
    0.9807853_f32, 0.9891765_f32, 0.9951847_f32, 0.99879545_f32,
    1.0_f32, 0.99879545_f32, 0.9951847_f32, 0.9891765_f32,
    0.98078525_f32, 0.97003126_f32, 0.9569403_f32, 0.94154406_f32,
    0.9238795_f32, 0.9039893_f32, 0.88192123_f32, 0.8577286_f32,
    0.83146954_f32, 0.8032075_f32, 0.7730105_f32, 0.74095106_f32,
    0.70710677_f32, 0.67155886_f32, 0.6343933_f32, 0.59569913_f32,
    0.5555702_f32, 0.51410276_f32, 0.47139663_f32, 0.42755505_f32,
    0.38268328_f32, 0.3368898_f32, 0.29028472_f32, 0.24298008_f32,
    0.19509031_f32, 0.14673033_f32, 0.0980171_f32, 0.049067486_f32,
    -8.742278e-08_f32, -0.04906766_f32, -0.09801727_f32, -0.1467305_f32,
    -0.19509049_f32, -0.24298024_f32, -0.29028487_f32, -0.33688995_f32,
    -0.38268343_f32, -0.42755523_f32, -0.47139677_f32, -0.5141029_f32,
    -0.5555703_f32, -0.5956993_f32, -0.6343934_f32, -0.671559_f32,
    -0.7071069_f32, -0.7409512_f32, -0.77301043_f32, -0.8032076_f32,
    -0.8314698_f32, -0.85772854_f32, -0.8819213_f32, -0.9039894_f32,
    -0.9238797_f32, -0.94154406_f32, -0.95694035_f32, -0.9700313_f32,
    -0.98078525_f32, -0.9891765_f32, -0.9951847_f32, -0.99879545_f32,
    -1.0_f32, -0.99879545_f32, -0.9951847_f32, -0.9891765_f32,
    -0.98078525_f32, -0.9700312_f32, -0.95694023_f32, -0.94154406_f32,
    -0.92387944_f32, -0.90398914_f32, -0.8819213_f32, -0.85772854_f32,
    -0.8314695_f32, -0.8032076_f32, -0.77301043_f32, -0.740951_f32,
    -0.70710653_f32, -0.671559_f32, -0.63439316_f32, -0.5956991_f32,
    -0.5555703_f32, -0.5141027_f32, -0.47139654_f32, -0.4275548_f32,
    -0.38268343_f32, -0.3368897_f32, -0.2902844_f32, -0.24298023_f32,
    -0.19509023_f32, -0.14673024_f32, -0.09801677_f32, -0.04906764_f32,
];

const PCOS: [f32; SINTABLE_ENTRIES] = [
    1.0_f32, 0.99879545_f32, 0.9951847_f32, 0.9891765_f32,
    0.98078525_f32, 0.97003126_f32, 0.95694035_f32, 0.94154406_f32,
    0.9238795_f32, 0.9039893_f32, 0.88192123_f32, 0.8577286_f32,
    0.8314696_f32, 0.8032075_f32, 0.77301043_f32, 0.7409511_f32,
    0.70710677_f32, 0.6715589_f32, 0.6343933_f32, 0.5956993_f32,
    0.5555702_f32, 0.5141027_f32, 0.47139665_f32, 0.4275551_f32,
    0.38268343_f32, 0.33688983_f32, 0.29028463_f32, 0.24298012_f32,
    0.19509023_f32, 0.1467305_f32, 0.098017134_f32, 0.04906765_f32,
    -4.371139e-08_f32, -0.04906774_f32, -0.09801722_f32, -0.14673057_f32,
    -0.19509032_f32, -0.2429802_f32, -0.29028472_f32, -0.33688992_f32,
    -0.38268352_f32, -0.42755508_f32, -0.47139683_f32, -0.51410276_f32,
    -0.55557036_f32, -0.59569937_f32, -0.6343933_f32, -0.67155904_f32,
    -0.70710677_f32, -0.74095124_f32, -0.7730105_f32, -0.80320764_f32,
    -0.83146966_f32, -0.8577286_f32, -0.88192135_f32, -0.9039893_f32,
    -0.9238796_f32, -0.9415441_f32, -0.95694035_f32, -0.97003126_f32,
    -0.9807853_f32, -0.9891765_f32, -0.9951847_f32, -0.99879545_f32,
    -1.0_f32, -0.99879545_f32, -0.9951847_f32, -0.9891765_f32,
    -0.98078525_f32, -0.97003126_f32, -0.9569403_f32, -0.94154406_f32,
    -0.9238795_f32, -0.90398926_f32, -0.88192123_f32, -0.85772854_f32,
    -0.83146954_f32, -0.8032075_f32, -0.7730104_f32, -0.7409511_f32,
    -0.70710665_f32, -0.6715589_f32, -0.63439333_f32, -0.5956992_f32,
    -0.55557_f32, -0.5141028_f32, -0.47139668_f32, -0.42755494_f32,
    -0.38268313_f32, -0.33688986_f32, -0.29028454_f32, -0.24297991_f32,
    -0.19509038_f32, -0.14673041_f32, -0.09801693_f32, -0.049067326_f32,
    1.1924881e-08_f32, 0.049067825_f32, 0.09801743_f32, 0.14673042_f32,
    0.19509041_f32, 0.2429804_f32, 0.29028502_f32, 0.3368899_f32,
    0.3826836_f32, 0.42755538_f32, 0.4713967_f32, 0.5141028_f32,
    0.5555704_f32, 0.59569925_f32, 0.63439333_f32, 0.6715591_f32,
    f32::from_bits(0x3f35_04f7), 0.7409511_f32, 0.77301055_f32, 0.8032077_f32,
    0.8314696_f32, 0.85772866_f32, 0.88192135_f32, 0.90398943_f32,
    0.92387956_f32, 0.9415441_f32, 0.9569404_f32, 0.97003126_f32,
    0.9807853_f32, 0.9891766_f32, 0.9951848_f32, 0.99879545_f32,
];

/// FTE's low-resolution sin table (r_part_fte.c redefines sin/cos with it,
/// which visibly quantizes circle/spiral spawn shapes).
#[inline]
pub fn psin(x: f32) -> f32 {
    let i = (x * ((SINTABLE_ENTRIES / 2) as f32 / PI)) as i32;
    PSIN[(i as u32 & (SINTABLE_ENTRIES as u32 - 1)) as usize]
}

#[inline]
pub fn pcos(x: f32) -> f32 {
    let i = (x * ((SINTABLE_ENTRIES / 2) as f32 / PI)) as i32;
    PCOS[(i as u32 & (SINTABLE_ENTRIES as u32 - 1)) as usize]
}

/// COM_Rand-like generator: 15-bit outputs (0..=0x7fff).
#[derive(Clone, Debug)]
pub struct Rng(u32);

pub const RAND_MAX: u32 = 0x7fff;

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed | 1)
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// COM_Rand.
    pub fn rand(&mut self) -> u32 {
        self.next_u32() >> 17
    }
    /// frandom: 0..=1.
    pub fn frandom(&mut self) -> f32 {
        self.rand() as f32 / RAND_MAX as f32
    }
    /// crandom: -1..=1.
    pub fn crandom(&mut self) -> f32 {
        self.rand() as f32 * (2.0 / RAND_MAX as f32) - 1.0
    }
    /// hrandom: -0.5..=0.5.
    pub fn hrandom(&mut self) -> f32 {
        self.frandom() - 0.5
    }
    /// crand: (rand % 32767) / 16383.5 - 1.
    pub fn crand(&mut self) -> f32 {
        (self.rand() % 32767) as f32 / 16383.5 - 1.0
    }
    /// Full-range float for the update pass (ufrandom).
    pub fn ufrandom(&mut self) -> f32 {
        self.next_u32() as f32 * (1.0 / 4_294_967_296.0)
    }
    pub fn ucrandom(&mut self) -> f32 {
        self.next_u32() as f32 * (2.0 / 4_294_967_296.0) - 1.0
    }
}

/// PerpendicularVector (mathlib.c).
pub fn perpendicular(src: Vec3) -> Vec3 {
    let mut pos = 0;
    let mut minelem = 1.0f32;
    for i in 0..3 {
        if src[i].abs() < minelem {
            pos = i;
            minelem = src[i].abs();
        }
    }
    let mut temp = Vec3::ZERO;
    temp[pos] = 1.0;
    // ProjectPointOnPlane (Quake's version divides by |n|^2 twice).
    let inv = 1.0 / src.length_sq();
    (temp - src * (src.dot(temp) * inv * inv)).normalize()
}

/// RotatePointAroundVector: rotate `point` around unit `dir` by `degrees`.
pub fn rotate_around(dir: Vec3, point: Vec3, degrees: f32) -> Vec3 {
    let (s, c) = degrees.to_radians().sin_cos();
    point * c + dir.cross(point) * s + dir * (dir.dot(point) * (1.0 - c))
}

/// VectorVectors: right/up from a forward vector.
pub fn vector_vectors(forward: Vec3) -> (Vec3, Vec3) {
    let right = if forward.x == 0.0 && forward.y == 0.0 {
        Vec3::new(0.0, if forward.z != 0.0 { -1.0 } else { 0.0 }, 0.0)
    } else {
        Vec3::new(forward.y, -forward.x, 0.0).normalize()
    };
    (right, right.cross(forward))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helpers() {
        let p = perpendicular(Vec3::new(0.0, 0.0, 1.0));
        assert!(p.dot(Vec3::new(0.0, 0.0, 1.0)).abs() < 1e-6 && (p.length() - 1.0).abs() < 1e-6);
        let p = perpendicular(Vec3::new(1.0, 2.0, 3.0).normalize());
        assert!(p.dot(Vec3::new(1.0, 2.0, 3.0)).abs() < 1e-5);
        let r = rotate_around(Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), 90.0);
        assert!((r - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6);
        assert!((psin(PI / 2.0) - 1.0).abs() < 1e-6);
        assert!((pcos(-PI) + 1.0).abs() < 1e-6);
        for k in -130..=130 {
            // Test arbitrary angles against the exact index-and-quantize formula PSIN/PCOS replaced.
            let x = (k as f32) * 0.05;
            let idx = ((x * ((SINTABLE_ENTRIES / 2) as f32 / PI)) as i32) as u32 & (SINTABLE_ENTRIES as u32 - 1);
            let quantized = (idx as f32) * (PI / (SINTABLE_ENTRIES / 2) as f32);
            assert_eq!(psin(x).to_bits(), quantized.sin().to_bits(), "psin mismatch at k={k} (idx={idx})");
            assert_eq!(pcos(x).to_bits(), quantized.cos().to_bits(), "pcos mismatch at k={k} (idx={idx})");
        }
        let mut rng = Rng::new(1);
        for _ in 0..1000 {
            let f = rng.frandom();
            assert!((0.0..=1.0).contains(&f));
            assert!((-1.0..=1.0).contains(&rng.crand()));
        }
    }
}
