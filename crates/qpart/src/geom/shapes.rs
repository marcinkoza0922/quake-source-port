use super::*;

impl<'a> Geometry<'a> {
    pub(super) fn textured(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let scale = if l.scalefactor == 1.0 { p.scale * 0.25 } else { self.view_scale(p, l, 0.25, 0.001) };
        // Normal billboards use right/up axes only. A deliberately oversized
        // sphere covers rotated quads and floating-point error at the eye plane.
        // Simulation and secondary effects have already run; only geometry is skipped.
        if self.cull_behind {
            let delta = p.org - self.view.origin;
            let depth = delta.dot(self.view.forward);
            let radius = scale.abs() * 3.0 + 1.0;
            if depth < -radius {
                return;
            }
            if let Some(tan) = self.vertical_tan {
                // Unnormalised top/bottom planes: (forward * tan ± up).
                // radius * (tan + 1) is conservative for both plane lengths.
                let height = delta.dot(self.view.up).abs();
                if height > depth * tan + radius * (tan + 1.0) {
                    return;
                }
            }
        }
        let c = shade(p.rgba, l.premul);
        let (pr, pu) = (self.view.right * 1.5, self.view.up * 1.5);
        let pos = if p.angle != 0.0 {
            let (x, y) = (psin(p.angle) * scale, pcos(p.angle) * scale);
            [
                p.org - pr * x - pu * y,
                p.org - pr * y + pu * x,
                p.org + pr * x + pu * y,
                p.org + pr * y - pu * x,
            ]
        } else {
            [p.org - pu * scale, p.org - pr * scale, p.org + pu * scale, p.org + pr * scale]
        };
        self.quad(ty, pos, p, c);
    }

    pub(super) fn quad(&mut self, ty: TypeId, pos: [Vec3; 4], p: &Particle, c: [u8; 4]) {
        let uv = [[p.s1, p.t1], [p.s1, p.t2], [p.s2, p.t2], [p.s2, p.t1]];
        let v: [Vertex; 4] = std::array::from_fn(|i| Vertex { pos: pos[i].to_array(), uv: uv[i], color: c });
        self.begin(ty, false, &v, &[0, 1, 2, 0, 2, 3]);
    }

    pub(super) fn udecal(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let c = shade(p.rgba, l.premul);
        let (sd, td) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let pos = if p.angle != 0.0 {
            let (x, y) = (psin(p.angle) * p.scale, pcos(p.angle) * p.scale);
            [p.org - sd * x - td * y, p.org - sd * y + td * x, p.org + sd * x + td * y, p.org + sd * y - td * x]
        } else {
            [p.org - td * p.scale, p.org - sd * p.scale, p.org + td * p.scale, p.org + sd * p.scale]
        };
        self.quad(ty, pos, p, c);
    }

    pub(super) fn line_spark(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let (c0, c1) = if l.premul != 0 {
            let a = p.rgba[3].min(1.0);
            let mut c0 = to_color([p.rgba[0] * a, p.rgba[1] * a, p.rgba[2] * a, if l.premul == 2 { 0.0 } else { a }]);
            if l.premul == 2 {
                c0[3] = 0;
            }
            (c0, [0; 4])
        } else {
            let c0 = to_color(p.rgba);
            (c0, [c0[0], c0[1], c0[2], 0])
        };
        let v = [
            Vertex { pos: p.org.to_array(), uv: [p.s1, p.t1], color: c0 },
            Vertex { pos: p.org.mul_add(-0.1, p.vel).to_array(), uv: [p.s2, p.t2], color: c1 },
        ];
        self.begin(ty, true, &v, &[0, 1]);
    }

    pub(super) fn fan_spark(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let scale = self.view_scale(p, l, 0.05, 0.0001);
        // vkQuake scales the premultiplied colour by 255 twice here (always
        // saturating); use the same shading as the other kinds instead.
        let c = shade(p.rgba, l.premul);
        let o2 = p.org.mul_add(-scale, p.vel);
        let cr = (self.view.origin - o2).cross(p.vel).normalize();
        let v = [
            Vertex { pos: p.org.to_array(), uv: [p.s1, p.t1], color: c },
            Vertex { pos: o2.mul_add(-p.scale, cr).to_array(), uv: [p.s1, p.t2], color: c },
            Vertex { pos: o2.mul_add(p.scale, cr).to_array(), uv: [p.s2, p.t1], color: c },
        ];
        self.begin(ty, false, &v, &[0, 1, 2]);
    }

    pub(super) fn textured_spark(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let c = shade(p.rgba, l.premul);
        let half = p.scale * 0.5;
        let (movedir, len) = p.vel.normalize_len();
        let mut length = if l.stretch < 0.0 {
            -l.stretch
        } else if l.stretch != 0.0 {
            len * l.stretch
        } else {
            len * 0.05
        };
        if length < half * l.minstretch {
            length = half * l.minstretch;
        }
        let o2 = p.org.mul_add(-length, movedir);
        let cr = (self.view.origin - o2).cross(p.vel).normalize();
        let a = o2.mul_add(-half, cr);
        let b = o2.mul_add(half, cr);
        let o2 = p.org.mul_add(length, movedir);
        let cr = (self.view.origin - o2).cross(p.vel).normalize();
        let pos = [a, b, o2.mul_add(half, cr), o2.mul_add(-half, cr)];
        self.quad(ty, pos, p, c);
    }
}
