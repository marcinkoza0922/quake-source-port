// Ray-traced reflections (src/rt): one alias model surface of the frame.
// Mirrors `RtDraw` in src/rt.rs (std430, 144 bytes).
struct RtDraw {
    vec4 m[3];      // model-to-world rows (3x4 affine)
    vec4 light;     // rgb (R_SetupAliasLighting / 200), alpha
    vec4 shadevec;  // xyz shade direction, w pose blend
    uvec4 poses;    // pose1 base, pose2 base, shirt row, pants row
    uvec4 texs;     // base, fullbright, colormap, flags (as alias.vert)
    uvec4 addr;     // vertex buffer address lo/hi, index buffer address lo/hi
    uvec4 range;    // first index, triangles, first output triangle, 0
};

// TLAS instance custom index: brush instances carry their submodel (and
// the alternate-texture bit); the alias instance is RT_ALIAS.
const uint RT_ALIAS = 0x800000u;
const uint RT_ALT = 0x400000u;
// Per-frame texture animation table: resolved texture per BSP texture,
// primary sequence first, alternates from RT_ANIM_ALT.
const uint RT_ANIM_ALT = 4096u;
