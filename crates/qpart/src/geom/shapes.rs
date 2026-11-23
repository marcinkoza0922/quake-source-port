use super::*;

#[inline(always)]
fn push_quad(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>, pos: [Vec3; 4], p: &Particle, c: [u8; 4]) {
    let base = vertices.len() as u32;
    vertices.extend_from_slice(&[
        Vertex { pos: pos[0].to_array(), uv: [p.s1, p.t1], color: c },
        Vertex { pos: pos[1].to_array(), uv: [p.s1, p.t2], color: c },
        Vertex { pos: pos[2].to_array(), uv: [p.s2, p.t2], color: c },
        Vertex { pos: pos[3].to_array(), uv: [p.s2, p.t1], color: c },
    ]);
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

impl<'a> Geometry<'a> {
    pub(super) fn emit_normal_quads(&mut self, l: &Looks, particles: &[Particle]) {
        let view = self.view;
        let premul = l.premul;
        let simple_scale = l.scalefactor == 1.0;
        let cull_behind = self.cull_behind;
        let htan = self.horizontal_tan;
        let vtan = self.vertical_tan;
        if !cull_behind {
            self.quads.reserve(particles.len());
        }
        for p in particles.iter().rev() {
            let need_delta = !simple_scale || cull_behind;
            let (delta, depth) = if need_delta {
                let d = p.org - view.origin;
                (d, d.dot(view.forward))
            } else {
                (Vec3::ZERO, 0.0)
            };
            let scale = if simple_scale {
                p.scale * 0.25
            } else {
                Self::view_scale_depth(depth, p.scale, l, 0.25, 0.001)
            };
            if cull_behind {
                let radius = scale.abs() * 3.0 + 1.0;
                if depth < -radius {
                    continue;
                }
                if let Some(tan) = htan
                    && delta.dot(view.right).abs() > depth * tan + radius * (tan + 1.0)
                {
                    continue;
                }
                if let Some(tan) = vtan
                    && delta.dot(view.up).abs() > depth * tan + radius * (tan + 1.0)
                {
                    continue;
                }
            }
            self.quads.push(Quad {
                pos: p.org.to_array(),
                scale,
                angle: p.angle,
                color: shade(p.rgba, premul),
                uv: [p.s1, p.t1, p.s2, p.t2],
            });
        }
    }

    pub(super) fn emit_normal_particles(&mut self, l: &Looks, particles: &[Particle]) {
        let view = self.view;
        let (pr, pu) = (view.right * 1.5, view.up * 1.5);
        let premul = l.premul;
        let simple_scale = l.scalefactor == 1.0;
        let cull_behind = self.cull_behind;
        let htan = self.horizontal_tan;
        let vtan = self.vertical_tan;
        if !cull_behind {
            self.vertices.reserve(particles.len() * 4);
            self.indices.reserve(particles.len() * 6);
        }
        for p in particles.iter().rev() {
            let need_delta = !simple_scale || cull_behind;
            let (delta, depth) = if need_delta {
                let d = p.org - view.origin;
                (d, d.dot(view.forward))
            } else {
                (Vec3::ZERO, 0.0)
            };
            let scale = if simple_scale {
                p.scale * 0.25
            } else {
                Self::view_scale_depth(depth, p.scale, l, 0.25, 0.001)
            };
            // Normal billboards use right/up axes only. A deliberately oversized
            // sphere covers rotated quads and floating-point error at the eye plane.
            // Simulation and secondary effects have already run; only geometry is skipped.
            if cull_behind {
                let radius = scale.abs() * 3.0 + 1.0;
                if depth < -radius {
                    continue;
                }
                // Unnormalised side planes use the same conservative sphere bound.
                if let Some(tan) = htan {
                    let width = delta.dot(view.right).abs();
                    if width > depth * tan + radius * (tan + 1.0) {
                        continue;
                    }
                }
                if let Some(tan) = vtan {
                    // Unnormalised top/bottom planes: (forward * tan ± up).
                    // radius * (tan + 1) is conservative for both plane lengths.
                    let height = delta.dot(view.up).abs();
                    if height > depth * tan + radius * (tan + 1.0) {
                        continue;
                    }
                }
            }
            let c = shade(p.rgba, premul);
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
            push_quad(&mut self.vertices, &mut self.indices, pos, p, c);
        }
    }

    pub(super) fn emit_udecals(&mut self, l: &Looks, particles: &[Particle]) {
        let premul = l.premul;
        let (sd, td) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        self.vertices.reserve(particles.len() * 4);
        self.indices.reserve(particles.len() * 6);
        for p in particles.iter().rev() {
            let c = shade(p.rgba, premul);
            let pos = if p.angle != 0.0 {
                let (x, y) = (psin(p.angle) * p.scale, pcos(p.angle) * p.scale);
                [p.org - sd * x - td * y, p.org - sd * y + td * x, p.org + sd * x + td * y, p.org + sd * y - td * x]
            } else {
                [p.org - td * p.scale, p.org - sd * p.scale, p.org + td * p.scale, p.org + sd * p.scale]
            };
            push_quad(&mut self.vertices, &mut self.indices, pos, p, c);
        }
    }

    pub(super) fn emit_line_sparks(&mut self, l: &Looks, particles: &[Particle]) {
        let premul = l.premul;
        self.vertices.reserve(particles.len() * 2);
        self.indices.reserve(particles.len() * 2);
        for p in particles.iter().rev() {
            let (c0, c1) = if premul != 0 {
                let a = p.rgba[3].min(1.0);
                let mut c0 = to_color([p.rgba[0] * a, p.rgba[1] * a, p.rgba[2] * a, if premul == 2 { 0.0 } else { a }]);
                if premul == 2 {
                    c0[3] = 0;
                }
                (c0, [0; 4])
            } else {
                let c0 = to_color(p.rgba);
                (c0, [c0[0], c0[1], c0[2], 0])
            };
            let base = self.vertices.len() as u32;
            self.vertices.extend_from_slice(&[
                Vertex { pos: p.org.to_array(), uv: [p.s1, p.t1], color: c0 },
                Vertex { pos: p.org.mul_add(-0.1, p.vel).to_array(), uv: [p.s2, p.t2], color: c1 },
            ]);
            self.indices.extend_from_slice(&[base, base + 1]);
        }
    }

    pub(super) fn emit_fan_sparks(&mut self, l: &Looks, particles: &[Particle]) {
        let view = self.view;
        let premul = l.premul;
        self.vertices.reserve(particles.len() * 3);
        self.indices.reserve(particles.len() * 3);
        for p in particles.iter().rev() {
            let depth = (p.org - view.origin).dot(view.forward);
            let scale = Self::view_scale_depth(depth, p.scale, l, 0.05, 0.0001);
            // vkQuake scales the premultiplied colour by 255 twice here (always
            // saturating); use the same shading as the other kinds instead.
            let c = shade(p.rgba, premul);
            let o2 = p.org.mul_add(-scale, p.vel);
            let cr = (view.origin - o2).cross(p.vel).normalize();
            let base = self.vertices.len() as u32;
            self.vertices.extend_from_slice(&[
                Vertex { pos: p.org.to_array(), uv: [p.s1, p.t1], color: c },
                Vertex { pos: o2.mul_add(-p.scale, cr).to_array(), uv: [p.s1, p.t2], color: c },
                Vertex { pos: o2.mul_add(p.scale, cr).to_array(), uv: [p.s2, p.t1], color: c },
            ]);
            self.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }

    pub(super) fn emit_textured_sparks(&mut self, l: &Looks, particles: &[Particle]) {
        let view_origin = self.view.origin;
        let premul = l.premul;
        self.vertices.reserve(particles.len() * 4);
        self.indices.reserve(particles.len() * 6);
        for p in particles.iter().rev() {
            let c = shade(p.rgba, premul);
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
            let cr = (view_origin - o2).cross(p.vel).normalize();
            let a = o2.mul_add(-half, cr);
            let b = o2.mul_add(half, cr);
            let o2 = p.org.mul_add(length, movedir);
            let cr = (view_origin - o2).cross(p.vel).normalize();
            let pos = [a, b, o2.mul_add(half, cr), o2.mul_add(-half, cr)];
            push_quad(&mut self.vertices, &mut self.indices, pos, p, c);
        }
    }
}
