// Per-object motion (port of velworld/velalias.vert + vel.frag): screen-space
// displacement since the last frame in view uv units, only where this surface
// is the visible one in the scene depth. Brush vertices are model-local
// (du.model now, du.prev last frame); CPU-posed alias vertices are in world
// space (du.model identity, du.prev = last transform * inverse current).
// DEPTH_T is the single- or multisampled depth texture type.
@group(1) @binding(0) var scene_depth: DEPTH_T;

struct VelOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) cur: vec4<f32>,
    @location(1) prev: vec4<f32>,
};
@vertex fn vs_vel(@location(0) pos: vec3<f32>) -> VelOut {
    var o: VelOut;
    let p = vec4<f32>(pos, 1.0);
    o.cur = fr.viewproj * (du.model * p);
    o.prev = fr.viewproj * (du.prev * p);
    o.clip = o.cur;
    return o;
}

fn linear_depth(d: f32) -> f32 {
    let n = 4.0;
    let f = 65536.0;
    return n * f / (f - d * (f - n));
}
// fr.vel: x scene px per velocity px, zw max scene px
@fragment fn fs_vel(i: VelOut) -> @location(0) vec4<f32> {
    let px = clamp(vec2<i32>(i.clip.xy * fr.vel.x), vec2<i32>(0), vec2<i32>(fr.vel.zw));
    let d = textureLoad(scene_depth, px, 0);
    if (linear_depth(i.clip.z) > linear_depth(d) * 1.02 + 2.0) { discard; }
    let c = i.cur.xy / i.cur.w;
    let p = i.prev.xy / i.prev.w;
    // NDC y is up, uv y down
    return vec4<f32>((c - p) * vec2<f32>(0.5, -0.5), 0.0, 0.0);
}
