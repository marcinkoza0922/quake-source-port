use super::*;

pub(super) fn ramp_decal(t: &PartType, d: &mut Decal, pt: f32, ft: f32) {
    let n = t.ramp.len();
    let frac = n as f32 * (t.die - (d.die - pt)) / t.die;
    match t.rampmode {
        RampMode::Nearest if n > 0 => {
            let r = &t.ramp[(frac.max(0.0) as usize).min(n - 1)];
            d.rgba = [r.rgb[0], r.rgb[1], r.rgb[2], r.alpha];
        }
        RampMode::Lerp if n > 0 => {
            let s1 = (frac.max(0.0) as usize).min(n - 1);
            let s2 = (s1 + 1).min(n - 1);
            let f = frac - s1 as f32;
            let (a, b) = (&t.ramp[s1], &t.ramp[s2]);
            for k in 0..3 {
                d.rgba[k] = a.rgb[k] + (b.rgb[k] - a.rgb[k]) * f;
            }
            d.rgba[3] = a.alpha + (b.alpha - a.alpha) * f;
        }
        RampMode::Delta if n > 0 => {
            let r = &t.ramp[(frac.max(0.0) as usize).min(n - 1)];
            for k in 0..3 {
                d.rgba[k] += ft * r.rgb[k];
            }
            d.rgba[3] -= ft * r.alpha;
        }
        _ => {
            if pt < d.die - t.die + t.rgbchangetime {
                for k in 0..3 {
                    d.rgba[k] += ft * t.rgbchange[k];
                }
            }
            d.rgba[3] += ft * t.alphachange;
        }
    }
}

pub(super) const FRAG_EPSILON: f32 = 1.0 / 32.0;
pub(super) const MAXFRAGMENTVERTS: usize = 128 * 3;

/// Fragment_ClipPolyToPlane.
pub(super) fn clip_poly_to_plane(input: &[Vec3], plane: Vec3, dist: f32, out: &mut Vec<Vec3>) {
    out.clear();
    let n = input.len();
    let dots: Vec<f32> = input.iter().map(|v| v.dot(plane) - dist).collect();
    let keep: Vec<u8> = dots
        .iter()
        .map(|&d| if d < -FRAG_EPSILON { 0 } else if d > FRAG_EPSILON { 1 } else { 2 })
        .collect();
    let clipped = keep.iter().filter(|&&k| k == 0).count();
    if clipped == n {
        return;
    }
    if clipped == 0 {
        out.extend_from_slice(input);
        return;
    }
    for i in 0..n {
        let (k0, k1) = (keep[i], keep[(i + 1) % n]);
        if k0 == 2 {
            out.push(input[i]);
            continue;
        }
        if k0 == 1 {
            out.push(input[i]);
        }
        if k1 == 2 || k0 == k1 {
            continue;
        }
        let (d0, d1) = (dots[i], dots[(i + 1) % n]);
        let mut d = d0 - d1;
        if d != 0.0 {
            d = d0 / d;
        }
        out.push(input[i].lerp(input[(i + 1) % n], d));
    }
}

/// Fragment_ClipPoly: clips a surface polygon to the decal box and fans it
/// into triangles.
pub(super) fn fragment_clip_poly(poly: &[Vec3], planes: &[(Vec3, f32); 6], normal: Vec3, noperp: bool, tris: &mut Vec<[Vec3; 3]>) {
    if poly.len() > MAXFRAGMENTVERTS || poly.len() < 3 {
        return;
    }
    if noperp {
        let d1 = poly[1] - poly[0];
        let mut n = Vec3::ZERO;
        let mut found = false;
        for v in &poly[2..] {
            n = d1.cross(*v - poly[0]);
            if n.dot(n) > 0.1 {
                found = true;
                break;
            }
        }
        if !found || n.normalize().dot(normal) < 0.1 {
            return;
        }
    }
    let mut a = poly.to_vec();
    let mut b = Vec::with_capacity(poly.len() + 6);
    for (pn, pd) in planes {
        clip_poly_to_plane(&a, *pn, *pd, &mut b);
        if b.len() < 3 {
            return;
        }
        std::mem::swap(&mut a, &mut b);
    }
    let mut n = a.len();
    while n > 2 {
        n -= 1;
        tris.push([a[0], a[n - 1], a[n]]);
    }
}
