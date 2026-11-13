//! Original split-shell induction projector. Native MDL geometry, no game assets.
use qformats::mdl::{Frame, Mdl, Pose, Skin, StVert, TriVert, Triangle};
use qcore::math::Vec3;

#[derive(Default)]
struct Mesh {
    points: Vec<[f32; 3]>,
    st: Vec<StVert>,
    triangles: Vec<Triangle>,
}

impl Mesh {
    fn quad(&mut self, p: [[f32; 3]; 4], material: i32) {
        let base = self.points.len() as i32;
        self.points.extend(p);
        self.st.extend([StVert { onseam: 0, s: material * 8 + 4, t: 4 }; 4]);
        for verts in [[base, base + 1, base + 2], [base, base + 2, base + 3]] {
            self.triangles.push(Triangle { faces_front: 1, verts });
        }
    }
    fn block(&mut self, a: [f32; 3], b: [f32; 3], m: i32) {
        let p = |x, y, z| [if x { b[0] } else { a[0] }, if y { b[1] } else { a[1] }, if z { b[2] } else { a[2] }];
        for f in [
            [p(false,false,false),p(false,false,true),p(false,true,true),p(false,true,false)],
            [p(true,false,false),p(true,true,false),p(true,true,true),p(true,false,true)],
            [p(false,false,false),p(true,false,false),p(true,false,true),p(false,false,true)],
            [p(false,true,false),p(false,true,true),p(true,true,true),p(true,true,false)],
            [p(false,false,false),p(false,true,false),p(true,true,false),p(true,false,false)],
            [p(false,false,true),p(true,false,true),p(true,true,true),p(false,true,true)],
        ] { self.quad(f, m); }
    }
    fn shell(&mut self, side: f32) {
        let profile = [(3.5,-2.2),(4.0,-3.0),(5.0,-3.0),(5.8,-2.0),
            (5.8,2.0),(5.0,3.4),(4.0,3.4),(3.5,2.5)];
        let front: [[f32;3];8] = std::array::from_fn(|i| [8.5,profile[i].0*side*0.8,profile[i].1*0.8]);
        let back: [[f32;3];8] = std::array::from_fn(|i| [-3.0,profile[i].0*side,profile[i].1]);
        for i in 0..8 {
            let j = (i+1)%8;
            let mut q = [back[i],back[j],front[j],front[i]];
            if side < 0.0 { q.reverse(); }
            self.quad(q,0);
        }
        for i in 1..7 {
            self.quad([front[0],front[i],front[i+1],front[0]],0);
            self.quad([back[0],back[i+1],back[i],back[0]],0);
        }
    }
    /// Hollow polygonal sleeve along the barrel axis, with contrasting bevels.
    fn sleeve(&mut self, x0: f32, x1: f32, outer: f32, inner: f32, m: i32) {
        let p = |x, r: f32, i: usize| {
            let t = i as f32 * std::f32::consts::TAU / 12.0;
            [x, r * t.cos(), r * t.sin()]
        };
        for i in 0..12 {
            let j = i + 1;
            self.quad([p(x0,outer,i),p(x0,outer,j),p(x1,outer,j),p(x1,outer,i)],m);
            self.quad([p(x0,inner,j),p(x0,inner,i),p(x1,inner,i),p(x1,inner,j)],1);
            self.quad([p(x1,inner,i),p(x1,outer,i),p(x1,outer,j),p(x1,inner,j)],2);
            self.quad([p(x0,outer,i),p(x0,inner,i),p(x0,inner,j),p(x0,outer,j)],2);
        }
    }
}

pub fn model(palette: &[u8]) -> Mdl {
    let mut mesh = Mesh::default();
    // Local +X aims forward; broad ivory side shells surround a black spine.
    mesh.sleeve(-6.0, 8.0, 4.4, 2.5, 1);
    mesh.sleeve(-6.5, -3.5, 4.6, 2.4, 1);
    mesh.block([-6.0,-3.0,2.0],[-1.0,3.0,3.8],0);
    for side in [-1.0, 1.0] {
        let (a,b) = if side < 0.0 { (-5.6,-3.5) } else { (3.5,5.6) };
        mesh.shell(side);
        // Recessed side vents and a narrow identification stripe.
        for x in [-1.0,0.5,2.0,3.5] {
            mesh.block([x,side*5.8-0.08,-0.8],[x+0.65,side*5.8+0.08,1.0],1);
        }
        // Two emitter forks, with blunt ceramic tips and copper hinge blocks.
        mesh.block([6.0,a,-1.7],[17.0,b,0.1],1);
        mesh.block([15.0,a-0.3,-2.0],[19.0,b+0.3,1.0],0);
        mesh.block([5.0,a-0.2,-1.8],[7.0,b+0.2,0.6],3);
        mesh.block([-1.0,a-0.1,3.1],[5.0,b+0.1,3.7],1);
    }
    mesh.block([-5.0,-2.3,-8.0],[-1.0,2.3,-3.0],1);
    mesh.block([-4.0,-2.5,-6.0],[-2.0,2.5,-4.0],3);
    // Exposed stacked induction rings, luminous axial core and dorsal sight.
    mesh.sleeve(8.0, 14.0, 2.0, 0.9, 4);
    for x in [8.0,10.0,12.0,14.0] { mesh.sleeve(x,x+0.65,3.6,2.6,3); }
    mesh.block([-4.0,-1.0,4.0],[3.5,1.0,5.0],1);
    mesh.block([0.0,-0.65,5.0],[2.5,0.65,5.4],4);
    let pack = |p: [f32;3], normal| TriVert {
        v: p.map(|v| ((v + 16.0) * 6.0).round() as u8), normal,
    };
    let mut verts = Vec::new();
    for face in mesh.points.as_chunks::<4>().0 {
        let n = (Vec3::from(face[1]) - Vec3::from(face[0])).cross(Vec3::from(face[2]) - Vec3::from(face[0])).normalize();
        let normal = qcore::anorms::AVERTEXNORMALS.iter().enumerate().max_by(|(_,a),(_,b)| {
            n.dot(Vec3::from(**a)).total_cmp(&n.dot(Vec3::from(**b)))
        }).map_or(0, |(i,_)| i as u8);
        verts.extend(face.iter().map(|p| pack(*p,normal)));
    }
    let min = pack([-7.0,-6.0,-8.0],0);
    let max = pack([20.0,6.0,6.0],0);
    let skins = (0..8).map(|index| {
        let rgb = super::portal_draw::colour(index / 2, index % 2);
        let core = palette.as_chunks::<3>().0.iter().take(255).enumerate().min_by_key(|(_, p)| {
            (0..3).map(|k| (i32::from(p[k]) - i32::from(rgb[k])).pow(2)).sum::<i32>()
        }).map_or(206, |(i,_)| i as u8);
        let nearest = |rgb: [i32;3], fallback| palette.as_chunks::<3>().0.iter().take(224).enumerate()
            .min_by_key(|(_,p)| (0..3).map(|k| (i32::from(p[k])-rgb[k]).pow(2)).sum::<i32>())
            .map_or(fallback, |(i,_)| i as u8);
        let colors = [nearest([200,201,186],12), nearest([28,30,34],2),
            nearest([93,102,108],7), nearest([146,84,50],83), core];
        let mut pixels = vec![0; 64 * 8];
        for y in 0..8 { for x in 0..64 { pixels[y*64+x] = colors[(x/8).min(4)]; } }
        Skin { intervals: vec![], images: vec![pixels] }
    }).collect();
    Mdl { scale: [1.0/6.0;3], scale_origin: [-16.0;3], bounding_radius: 24.0,
        eye_position: [0.0;3], skin_width: 64, skin_height: 8, num_verts: verts.len(),
        synctype: 0, flags: 0, size: 1.0, skins, stverts: mesh.st, triangles: mesh.triangles,
        frames: vec![Frame { bboxmin: min, bboxmax: max, intervals: vec![], poses: vec![Pose {
            name: "inductor".into(), bboxmin: min, bboxmax: max, verts,
        }] }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_owner_and_slot_gets_its_own_core_skin() {
        let mut palette = vec![0;768];
        for i in 0..8 { palette[i*3..i*3+3].copy_from_slice(&crate::addons::portal_draw::colour(i/2,i%2)); }
        let m = model(&palette);
        assert_eq!(m.skins.len(),8);
        for (i,skin) in m.skins.iter().enumerate() {
            assert_eq!(skin.images[0][36],i as u8);
        }
        assert_eq!(m.num_verts,m.stverts.len());
        assert!(m.triangles.iter().all(|t| t.verts.iter().all(|&i| i >= 0 && (i as usize) < m.num_verts)));
        for pose in &m.frames[0].poses {
            assert_eq!(pose.verts.len(),m.num_verts);
            assert!(pose.verts.iter().all(|v| (v.normal as usize) < qcore::anorms::NUMVERTEXNORMALS));
        }
    }
}
