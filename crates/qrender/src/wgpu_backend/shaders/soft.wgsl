// Soft particles (port of fte.frag's soft fade): appended to common.wgsl +
// material.wgsl. DEPTH_T is the single- or multisampled depth texture type;
// the scene depth is bound read-only while these draw.
@group(3) @binding(0) var scene_depth: DEPTH_T;

fn linear_depth(d: f32) -> f32 {
    let n = 4.0;
    let f = 65536.0;
    return n * f / (f - d * (f - n));
}
// du.params: z soft inverse fade distance, w depth samples to take (min over
// them; 1 = sample 0); du.ix.x blend mode (FteBlend order)
@fragment fn fs_poly_soft(i: PolyOut) -> @location(0) vec4<f32> { return poly_soft_frag(i).c; }
@fragment fn fs_poly_soft_glow(i: PolyOut) -> Frag { return poly_soft_frag(i); }
fn poly_soft_frag(i: PolyOut) -> Frag {
    var c = textureSample(t0, s0, i.uv) * i.color;
    if (c.a <= du.params.x) { discard; }
    let px = vec2<i32>(i.clip.xy);
    let ns = max(i32(du.params.w), 1);
    var d = 1.0;
    for (var s = 0; s < ns; s++) {
        d = min(d, textureLoad(scene_depth, px, s));
    }
    let pz = dot(i.wpos - fr.eye.xyz, fr.vfwd.xyz);
    let fade = select(1.0, clamp((linear_depth(d) - pz) * du.params.z, 0.0, 1.0), d < 1.0);
    let bm = du.ix.x;
    // colour-based and premultiplied blends fade their RGB too
    if (bm == 1u || bm == 3u || bm == 4u || bm == 6u || bm == 7u) { c = vec4<f32>(c.rgb * fade, c.a); }
    if (bm == 0u || bm == 2u || bm == 4u || bm == 5u || bm == 7u) { c.a *= fade; }
    let glow = select(poly_glow(c.rgb, bm) * fog_factor(i.wpos), vec3<f32>(0.0), thermal());
    return Frag(poly_fog(c, i.wpos), vec4<f32>(glow, c.a));
}
