/// Initial particle state for [`ParticlePool::spawn`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleSpawn {
    pub org: [f32; 3],
    pub vel: [f32; 3],
    pub color: u8,
    pub die: f32,
    pub kind: u8,
    pub ramp: f32,
}

/// Mutable view of one particle during [`ParticlePool::update_with`].
pub struct ParticleMut<'a> {
    pub org: [&'a mut f32; 3],
    pub vel: [&'a mut f32; 3],
    pub color: &'a mut u8,
    pub die: &'a mut f32,
    pub kind: &'a mut u8,
    pub ramp: &'a mut f32,
}

/// SoA particle storage, fixed capacity, swap-remove kill (order not preserved).
#[derive(Debug, Clone, Default)]
pub struct ParticlePool {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub z: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    pub vz: Vec<f32>,
    pub color: Vec<u8>,
    pub die: Vec<f32>,
    pub kind: Vec<u8>,
    pub ramp: Vec<f32>,
    max: usize,
}

impl ParticlePool {
    /// Preallocates `max` particles; spawning never allocates.
    pub fn new(max: usize) -> Self {
        Self {
            x: Vec::with_capacity(max),
            y: Vec::with_capacity(max),
            z: Vec::with_capacity(max),
            vx: Vec::with_capacity(max),
            vy: Vec::with_capacity(max),
            vz: Vec::with_capacity(max),
            color: Vec::with_capacity(max),
            die: Vec::with_capacity(max),
            kind: Vec::with_capacity(max),
            ramp: Vec::with_capacity(max),
            max,
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.x.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
    #[inline]
    pub fn max(&self) -> usize {
        self.max
    }

    /// Spawn; returns the index, or `None` when full (Quake drops silently).
    #[inline]
    pub fn spawn(&mut self, p: ParticleSpawn) -> Option<usize> {
        let i = self.len();
        if i >= self.max {
            return None;
        }
        self.x.push(p.org[0]);
        self.y.push(p.org[1]);
        self.z.push(p.org[2]);
        self.vx.push(p.vel[0]);
        self.vy.push(p.vel[1]);
        self.vz.push(p.vel[2]);
        self.color.push(p.color);
        self.die.push(p.die);
        self.kind.push(p.kind);
        self.ramp.push(p.ramp);
        Some(i)
    }

    /// Swap-remove particle `i`; the last particle moves into `i`.
    #[inline]
    pub fn kill(&mut self, i: usize) {
        self.x.swap_remove(i);
        self.y.swap_remove(i);
        self.z.swap_remove(i);
        self.vx.swap_remove(i);
        self.vy.swap_remove(i);
        self.vz.swap_remove(i);
        self.color.swap_remove(i);
        self.die.swap_remove(i);
        self.kind.swap_remove(i);
        self.ramp.swap_remove(i);
    }

    pub fn clear(&mut self) {
        self.x.clear();
        self.y.clear();
        self.z.clear();
        self.vx.clear();
        self.vy.clear();
        self.vz.clear();
        self.color.clear();
        self.die.clear();
        self.kind.clear();
        self.ramp.clear();
    }

    pub fn get(&self, i: usize) -> ParticleSpawn {
        ParticleSpawn {
            org: [self.x[i], self.y[i], self.z[i]],
            vel: [self.vx[i], self.vy[i], self.vz[i]],
            color: self.color[i],
            die: self.die[i],
            kind: self.kind[i],
            ramp: self.ramp[i],
        }
    }

    /// Kill all particles with `die < time`.
    pub fn kill_expired(&mut self, time: f32) {
        let mut i = 0;
        while i < self.len() {
            if self.die[i] < time {
                self.kill(i);
            } else {
                i += 1;
            }
        }
    }

    /// `org += vel * dt` over all particles (tight SoA loops, auto-vectorizable).
    pub fn integrate(&mut self, dt: f32) {
        for (p, v) in self.x.iter_mut().zip(&self.vx) {
            *p += v * dt;
        }
        for (p, v) in self.y.iter_mut().zip(&self.vy) {
            *p += v * dt;
        }
        for (p, v) in self.z.iter_mut().zip(&self.vz) {
            *p += v * dt;
        }
    }

    /// Bulk update: kill expired, then call `f` per particle (kind-specific
    /// ramp/color/velocity logic, à la `CL_RunParticles`), then integrate.
    /// `f` returning false kills that particle.
    pub fn update_with(
        &mut self,
        time: f32,
        dt: f32,
        mut f: impl FnMut(&mut ParticleMut<'_>) -> bool,
    ) {
        self.kill_expired(time);
        let mut i = 0;
        while i < self.len() {
            let mut p = ParticleMut {
                org: [&mut self.x[i], &mut self.y[i], &mut self.z[i]],
                vel: [&mut self.vx[i], &mut self.vy[i], &mut self.vz[i]],
                color: &mut self.color[i],
                die: &mut self.die[i],
                kind: &mut self.kind[i],
                ramp: &mut self.ramp[i],
            };
            if f(&mut p) {
                i += 1;
            } else {
                self.kill(i);
            }
        }
        self.integrate(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_kill_update() {
        let mut ps = ParticlePool::new(3);
        let mk = |die, kind| ParticleSpawn { vel: [1.0, 2.0, 3.0], die, kind, ..Default::default() };
        assert_eq!(ps.spawn(mk(1.0, 0)), Some(0));
        assert_eq!(ps.spawn(mk(5.0, 1)), Some(1));
        assert_eq!(ps.spawn(mk(5.0, 2)), Some(2));
        assert_eq!(ps.spawn(mk(5.0, 3)), None);
        ps.update_with(2.0, 0.5, |p| {
            *p.vel[2] -= 1.0;
            *p.kind != 2
        });
        assert_eq!(ps.len(), 1);
        let p = ps.get(0);
        assert_eq!(p.kind, 1);
        assert_eq!(p.org, [0.5, 1.0, 1.0]);
        ps.kill(0);
        assert!(ps.is_empty());
        let cap = ps.x.capacity();
        for _ in 0..3 {
            ps.spawn(mk(0.0, 0));
        }
        assert_eq!(ps.x.capacity(), cap);
        ps.kill_expired(1.0);
        assert!(ps.is_empty());
    }
}
