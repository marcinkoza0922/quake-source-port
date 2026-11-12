use super::*;
use crate::Registry;

/// Solid below z=0, water for x > 1000; one large floor polygon at z=0.
struct Floor;

impl World for Floor {
    fn trace_line(&self, start: Vec3, end: Vec3) -> Trace {
        if start.z >= 0.0 && end.z < 0.0 {
            let f = start.z / (start.z - end.z);
            Trace { frac: f, endpos: start.lerp(end, f), normal: Vec3::new(0.0, 0.0, 1.0), entity: 0 }
        } else {
            Trace { frac: 1.0, endpos: end, normal: Vec3::ZERO, entity: 0 }
        }
    }
    fn point_contents(&self, p: Vec3) -> u32 {
        if p.z < 0.0 {
            FTECONTENTS_SOLID
        } else if p.x > 1000.0 {
            FTECONTENTS_WATER
        } else {
            FTECONTENTS_EMPTY
        }
    }
    fn decal_polys(&self, entity: i32, _c: Vec3, _r: f32, _n: Vec3, _np: bool, out: &mut dyn FnMut(&[Vec3])) {
        if entity == 0 {
            let s = 4096.0;
            // Quake winding: clockwise seen from the front, so the edge
            // cross product points into the floor (-z).
            out(&[Vec3::new(-s, s, 0.0), Vec3::new(s, s, 0.0), Vec3::new(s, -s, 0.0), Vec3::new(-s, -s, 0.0)]);
        }
    }
}

fn reg(src: &str) -> Registry {
    let mut pal = vec![0u8; 768];
    for (i, c) in pal.iter_mut().enumerate() {
        *c = (i / 3) as u8;
    }
    let mut r = Registry::with_palette(&pal);
    r.parse_fte("t", false, src);
    assert!(r.messages.iter().all(|m| !m.contains("Unknown")), "{:?}", r.messages);
    r
}

fn id(r: &Registry, name: &str) -> TypeId {
    r.types.iter().position(|t| t.name == name).unwrap()
}

#[derive(Default)]
struct Count {
    particles: usize,
    beams: usize,
    decals: usize,
}

impl Sink for Count {
    fn particle(&mut self, _: TypeId, _: &Particle) {
        self.particles += 1;
    }
    fn beam(&mut self, _: TypeId, _: &BeamSeg, _: &BeamSeg, _: &Particle, _: &Particle) {
        self.beams += 1;
    }
    fn decal(&mut self, _: TypeId, _: &Decal) {
        self.decals += 1;
    }
}

fn step(s: &mut System, r: &Registry, t: f64) -> Count {
    let mut c = Count::default();
    s.frame(r, &Floor, &FrameInput { cl_time: t, ..Default::default() }, &mut c);
    c
}

fn sys() -> System {
    let mut s = System::default();
    s.seed(7);
    s.clear(0.0);
    s
}

#[test]
fn effect_spawns_count_and_expires() {
    let r = reg("r_part burst\n{\ntexture x\nscale 2\ncount 10\ndie 1\nspawnorg 8\n}\n");
    let mut s = sys();
    let b = id(&r, "burst");
    assert!(s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 64.0), None, 1.0, b, None));
    assert_eq!(s.num_particles(), 10);
    for p in s.particles_of(b) {
        assert!((p.org - Vec3::new(0.0, 0.0, 64.0)).length() <= 8.0 * 3f32.sqrt() + 1e-3);
        assert_eq!(p.die, 1.0);
    }
    assert_eq!(step(&mut s, &r, 0.5).particles, 10);
    assert_eq!(step(&mut s, &r, 1.0).particles, 10);
    // Reaping compares against the clock before this frame's advance.
    assert_eq!(step(&mut s, &r, 1.5).particles, 10);
    assert_eq!(step(&mut s, &r, 2.0).particles, 0);
    assert_eq!(s.num_particles(), 0);
    assert!(s.run_list().is_empty());
    // Alpha fades linearly (alphachange = -alpha/die).
    assert!(s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 64.0), None, 1.0, b, None));
    step(&mut s, &r, 2.5);
    let a = s.particles_of(b).next().unwrap().rgba[3];
    assert!((a - 0.5).abs() < 1e-4, "{a}");
}

#[test]
fn velocity_gravity_and_bounce() {
    let r = reg("r_part ball\n{\ntexture x\nscale 1\ncount 1\ndie 5\nvelbias 0 0 -100\ngravity 0\nbounce 1.5\n}\n");
    let mut s = sys();
    let b = id(&r, "ball");
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 20.0), None, 1.0, b, None);
    for i in 1..=10 {
        step(&mut s, &r, i as f64 * 0.05);
    }
    let p = s.particles_of(b).next().unwrap();
    assert!(p.org.z >= 0.0, "{:?}", p.org);
    assert!(p.vel.z > 0.0 && (p.vel.z - 50.0).abs() < 1e-3, "{:?}", p.vel);
}

#[test]
fn cliptype_spawns_child_on_impact() {
    let r = reg(
        "r_part drop\n{\ntexture x\nscale 1\ncount 1\ndie 5\nvelbias 0 0 -400\ncliptype splash\nclipcount 3\n}\n\
         r_part splash\n{\ntexture x\nscale 1\ncount 1\ndie 5\n}\n",
    );
    let mut s = sys();
    let (d, sp) = (id(&r, "drop"), id(&r, "splash"));
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 10.0), None, 1.0, d, None);
    step(&mut s, &r, 0.1);
    assert_eq!(s.particles_of(sp).count(), 3);
    for p in s.particles_of(sp) {
        assert!(p.org.z.abs() < 1e-3);
    }
    step(&mut s, &r, 0.2);
    assert_eq!(s.particles_of(d).count(), 0);
}

#[test]
fn trail_spacing_continues_across_calls() {
    let r = reg("r_part tr\n{\ntexture x\nscale 1\nstep 10\ndie 1\n}\n");
    let mut s = sys();
    let t = id(&r, "tr");
    let mut ts = None;
    s.trail(&r, &Floor, Vec3::new(0.0, 0.0, 10.0), Vec3::new(100.0, 0.0, 10.0), t, 0.1, 1, Some(&mut ts));
    assert_eq!(s.num_particles(), 10);
    // As in C, a partial step still spawns (count-- > 0 with count 0.5)
    // and the overshoot carries over: next particle at 110.
    s.trail(&r, &Floor, Vec3::new(100.0, 0.0, 10.0), Vec3::new(105.0, 0.0, 10.0), t, 0.1, 1, Some(&mut ts));
    assert_eq!(s.num_particles(), 11);
    s.trail(&r, &Floor, Vec3::new(105.0, 0.0, 10.0), Vec3::new(108.0, 0.0, 10.0), t, 0.1, 1, Some(&mut ts));
    assert_eq!(s.num_particles(), 11);
    s.trail(&r, &Floor, Vec3::new(108.0, 0.0, 10.0), Vec3::new(125.0, 0.0, 10.0), t, 0.1, 1, Some(&mut ts));
    assert_eq!(s.num_particles(), 13);
    s.delink_trailstate(&mut ts);
    assert!(ts.is_none());
}

#[test]
fn count_based_trails_accumulate_overflow() {
    let r = reg("r_part tr\n{\ntexture x\nscale 1\ncount 15\ndie 1\n}\n");
    let mut s = sys();
    let t = id(&r, "tr");
    // 15 per second at 0.1 s per call: 1.5 per call.
    for _ in 0..4 {
        s.trail(&r, &Floor, Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 0.0, 10.0), t, 0.1, 0, None);
    }
    assert_eq!(s.num_particles(), 6);
}

#[test]
fn cdecal_effect_clips_to_floor_and_fades() {
    let r = reg("r_part scorch\n{\ntexture x\nscale 32\ncount 1\ndie 2\ntype decal\n}\n");
    let mut s = sys();
    let t = id(&r, "scorch");
    // No direction: traces the six axes and uses the nearest hit.
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 1.0), None, 1.0, t, None);
    // Square fragment fanned into two triangles.
    assert_eq!(s.num_decals(), 2);
    for d in s.decals_of(t) {
        // A 32-unit box at a random rotation around the normal.
        for v in d.vertex {
            assert!(v.z.abs() < 1e-3 && v.length() <= 16.0 * 2f32.sqrt() + 1e-3, "{v:?}");
        }
        for tc in d.texcoords {
            assert!((-0.01..=1.01).contains(&tc[0]) && (-0.01..=1.01).contains(&tc[1]), "{tc:?}");
        }
    }
    assert_eq!(step(&mut s, &r, 1.0).decals, 2);
    // Frame time is clamped to 1 s, so step in smaller increments.
    for t in [2.0, 2.5, 3.0] {
        step(&mut s, &r, t);
    }
    assert_eq!(s.num_decals(), 0);
    assert!(s.run_list().is_empty());
}

#[test]
fn blood_splats_decal_on_impact() {
    let mut r = Registry::new();
    r.import_effectinfo("effectinfo", "effect blood\ntype blood\ncount 1\nsize 16 16\nalpha 256 256 64\nvelocityoffset 0 0 -250\n", false, None);
    let b = id(&r, "blood");
    assert_eq!(r.types[b].cliptype, None);
    assert_eq!(r.types[b].clipbounce, -2.0);
    // vkQuake sets clipbounce -2 for DP blood but no cliptype, so blood is
    // never traced (kept as in C). Give it one to exercise the splat path.
    r.types[b].cliptype = Some(b);
    let mut s = sys();
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 4.0), None, 1.0, b, None);
    step(&mut s, &r, 0.05);
    assert_eq!(s.num_decals(), 2);
    // The splatted particle is marked dead and reaped next frame.
    assert_eq!(s.particles_of(b).next().unwrap().die, -1.0);
    step(&mut s, &r, 0.1);
    assert_eq!(s.particles_of(b).count(), 0);
}

#[test]
fn emitters_spawn_children() {
    let r = reg(
        "r_part em\n{\ntexture x\nscale 1\ncount 1\ndie 10\nemit kid\nemitinterval 0.1\n}\n\
         r_part kid\n{\ntexture x\nscale 1\ncount 1\ndie 10\n}\n",
    );
    let mut s = sys();
    let (e, k) = (id(&r, "em"), id(&r, "kid"));
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 64.0), None, 1.0, e, None);
    for i in 1..=10 {
        step(&mut s, &r, i as f64 * 0.05);
    }
    let n = s.particles_of(k).count();
    assert!((4..=6).contains(&n), "{n}");
}

#[test]
fn trail_emitters_keep_state_per_particle() {
    let r = reg(
        "r_part em\n{\ntexture x\nscale 1\ncount 1\ndie 10\nvelbias 100 0 0\nemit kid\nemitinterval -1\n}\n\
         r_part kid\n{\ntexture x\nscale 1\nstep 10\ndie 10\n}\n",
    );
    let mut s = sys();
    let (e, k) = (id(&r, "em"), id(&r, "kid"));
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 64.0), None, 1.0, e, None);
    for i in 1..=10 {
        step(&mut s, &r, i as f64 * 0.1);
    }
    // 100 qu/s over ~1 s at one particle per 10 qu.
    let n = s.particles_of(k).count();
    assert!((9..=10).contains(&n), "{n}");
}

#[test]
fn beams_link_segments() {
    let r = reg("r_part lg\n{\ntexture x\nscale 4\nstep 16\ndie 0.5\ntype beam\n}\n");
    let mut s = sys();
    let t = id(&r, "lg");
    let mut ts = None;
    s.trail(&r, &Floor, Vec3::new(0.0, 0.0, 10.0), Vec3::new(64.0, 0.0, 10.0), t, 0.1, 0, Some(&mut ts));
    assert_eq!(s.num_beams(), 4);
    let c = step(&mut s, &r, 0.1);
    assert_eq!(c.beams, 3);
    // Continue the trail: the new run links onto the last segment.
    s.trail(&r, &Floor, Vec3::new(64.0, 0.0, 10.0), Vec3::new(96.0, 0.0, 10.0), t, 0.1, 0, Some(&mut ts));
    assert_eq!(s.num_beams(), 6);
    let c = step(&mut s, &r, 0.2);
    assert_eq!(c.beams, 5);
    for i in 3..12 {
        step(&mut s, &r, i as f64 * 0.1);
    }
    assert_eq!(s.num_particles(), 0);
    assert!(s.num_beams() <= 1, "{}", s.num_beams());
}

#[test]
fn zero_lifetime_types_draw_once() {
    let r = reg("r_part flash\n{\ntexture x\nscale 1\ncount 3\ndie 0\nalpha 1\nalphadelta 0\n}\n");
    let mut s = sys();
    let t = id(&r, "flash");
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 64.0), None, 1.0, t, None);
    assert_eq!(step(&mut s, &r, 0.1).particles, 3);
    assert_eq!(s.num_particles(), 0);
    assert_eq!(step(&mut s, &r, 0.2).particles, 0);
}

#[test]
fn inwater_and_overwater() {
    let r = reg(
        "r_part air\n{\ntexture x\nscale 1\ncount 1\ndie 1\ninwater wet\n}\n\
         r_part wet\n{\ntexture x\nscale 1\ncount 2\ndie 1\n}\n\
         r_part dry\n{\ntexture x\nscale 1\ncount 1\ndie 1\nnotunderwater\n}\n",
    );
    let mut s = sys();
    let (a, w, d) = (id(&r, "air"), id(&r, "wet"), id(&r, "dry"));
    s.run_effect(&r, &Floor, Vec3::new(2000.0, 0.0, 10.0), None, 1.0, a, None);
    assert_eq!((s.particles_of(a).count(), s.particles_of(w).count()), (0, 2));
    s.run_effect(&r, &Floor, Vec3::new(2000.0, 0.0, 10.0), None, 1.0, d, None);
    assert_eq!(s.particles_of(d).count(), 0);
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 10.0), None, 1.0, d, None);
    assert_eq!(s.particles_of(d).count(), 1);
}

#[test]
fn assoc_chain_dlights_and_sounds() {
    let r = reg(
        "r_part boom\n{\ntexture x\nscale 1\ncount 2\ndie 1\nlightradius 200\nlightrgb 1 0.5 0\nlighttime 0.5\nsound weapons/r_exp3.wav 1 1\n}\n\
         r_part +boom\n{\ntexture y\nscale 1\ncount 5\ndie 1\n}\n",
    );
    let mut s = sys();
    let b = id(&r, "boom");
    s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 64.0), None, 1.0, b, None);
    assert_eq!(s.num_particles(), 7);
    assert_eq!(s.dlights.len(), 1);
    assert_eq!(s.dlights[0].radius, 200.0);
    assert_eq!(s.dlights[0].die, 0.5);
    assert_eq!(s.sounds.len(), 1);
    assert_eq!(s.sounds[0].name, "weapons/r_exp3.wav");
}

#[test]
fn pool_limit_recycles() {
    let r = reg("r_part p\n{\ntexture x\nscale 1\ncount 100\ndie 100\n}\n");
    let mut s = System::new(Settings { max_particles: 300, ..Settings::default() });
    let t = id(&r, "p");
    for _ in 0..5 {
        s.run_effect(&r, &Floor, Vec3::new(0.0, 0.0, 64.0), None, 1.0, t, None);
    }
    assert_eq!(s.num_particles(), 300);
    step(&mut s, &r, 0.1); // marks 256 dead
    step(&mut s, &r, 0.2); // reaps them
    assert_eq!(s.num_particles(), 44);
}

#[test]
fn trailstates_are_recycled_and_validated() {
    let r = reg("r_part tr\n{\ntexture x\nscale 1\nstep 10\ndie 1\n}\n");
    let mut s = sys();
    let t = id(&r, "tr");
    let mut first = None;
    s.trail(&r, &Floor, Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), t, 0.1, 0, Some(&mut first));
    let h = first.unwrap();
    // Exhaust the ring: the first owner's state gets stolen.
    for _ in 0..MAX_TRAILSTATES {
        let mut other = None;
        s.trail(&r, &Floor, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), t, 0.1, 0, Some(&mut other));
    }
    assert!(!s.valid(h));
    s.trail(&r, &Floor, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), t, 0.1, 0, Some(&mut first));
    assert!(s.valid(first.unwrap()));
}

#[test]
fn sky_tris_emit_rain() {
    let r = reg("r_part rain\n{\ntexture x\nscale 1\ncount 1\ndie 2\n}\n");
    let mut s = sys();
    let t = id(&r, "rain");
    let quad = [
        Vec3::new(-128.0, -128.0, 256.0),
        Vec3::new(128.0, -128.0, 256.0),
        Vec3::new(128.0, 128.0, 256.0),
        Vec3::new(-128.0, 128.0, 256.0),
    ];
    let tris = sky_tris_for_polygon(&quad, Vec3::new(0.0, 0.0, -1.0), t);
    assert_eq!(tris.len(), 2);
    s.set_sky_tris(1, tris);
    let input = FrameInput {
        cl_time: 0.5,
        vieworg: Vec3::new(0.0, 0.0, 128.0),
        rain: vec![RainEmitter {
            model: 1,
            axis: [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)],
            origin: Vec3::ZERO,
        }],
    };
    s.frame(&r, &Floor, &input, &mut NullSink);
    // area 256*256 = 65536 -> one drop per 10000/32768 s per triangle.
    let n = s.particles_of(t).count();
    assert!(n > 0, "{n}");
    for p in s.particles_of(t) {
        assert!(p.org.z > 255.0 && p.org.z < 256.0 && p.org.x.abs() <= 128.0, "{:?}", p.org);
    }
}

#[test]
fn fragment_clipping() {
    let planes = [
        (Vec3::new(1.0, 0.0, 0.0), -8.0),
        (Vec3::new(-1.0, 0.0, 0.0), -8.0),
        (Vec3::new(0.0, 1.0, 0.0), -8.0),
        (Vec3::new(0.0, -1.0, 0.0), -8.0),
        (Vec3::new(0.0, 0.0, -1.0), -8.0),
        (Vec3::new(0.0, 0.0, 1.0), -8.0),
    ];
    let poly = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 0.0), Vec3::new(0.0, 100.0, 0.0)];
    let mut tris = Vec::new();
    // `poly` winds counter-clockwise from +z, so its edge normal is +z.
    fragment_clip_poly(&poly, &planes, Vec3::new(0.0, 0.0, 1.0), true, &mut tris);
    assert_eq!(tris.len(), 2);
    let area: f32 = tris.iter().map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0).sum();
    assert!((area - 64.0).abs() < 1e-3, "{area}");
    // Facing away from the decal normal: rejected.
    tris.clear();
    fragment_clip_poly(&poly, &planes, Vec3::new(0.0, 0.0, -1.0), true, &mut tris);
    assert!(tris.is_empty());
}

#[test]
fn classic_and_weather_fallbacks() {
    let mut r = reg("r_part PE_DEFAULT\n{\ntexture x\nscale 1\ncount 1\ndie 1\n}\nr_part te_snow\n{\ntexture x\nscale 1\ncount 2\ndie 1\n}\n");
    let mut f = |_: &str| None;
    let mut s = sys();
    s.refresh_pe(&mut r, &mut f);
    assert!(s.run_classic_effect(&mut r, &mut f, &Floor, Vec3::new(0.0, 0.0, 64.0), Vec3::ZERO, 75, 10));
    let pe = s.pe.default.unwrap();
    assert_eq!(s.particles_of(pe).count(), 10);
    assert_eq!((r.types[pe].colorindex, r.types[pe].colorrand), (72, 8));
    s.run_weather(&mut r, &mut f, &Floor, Vec3::ZERO, Vec3::splat(100.0), Vec3::ZERO, 5.0, 3, "snow");
    assert_eq!(s.particles_of(id(&r, "te_snow")).count(), 10);
}

