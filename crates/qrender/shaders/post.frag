#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(push_constant) uniform P {
    vec4 blend;
    float gamma;
    float contrast;
    uint scene;
    float dither;   // r_softemu 1: screen dither; 0: banding suppression
    uint mode;      // 0 truecolour, 1 palettize + dither, 2 palettize
    float scale;    // scene pixels per output pixel (noise follows scene pixels)
    uint colorblind; // r_colorblind: 0 off, 1 protan, 2 deutan, 3 tritan
} pc;
layout(location = 0) out vec4 o_color;

// 128^3 palette index LUT, packed 4 per uint after the 256 palette entries.
uint lut(ivec3 c) {
    uint i = uint(c.x + c.y * 128 + c.z * 16384);
    return (palette[256u + (i >> 2)] >> ((i & 3u) * 8u)) & 255u;
}
// Daltonize: simulate the deficiency (Viénot/Brettel-style RGB matrices),
// then move the lost difference into channels the viewer can still tell apart.
vec3 daltonize(vec3 c, uint m) {
    mat3 sim;
    if (m == 1u)      sim = mat3(0.56667, 0.55833, 0.0, 0.43333, 0.44167, 0.24167, 0.0, 0.0, 0.75833);
    else if (m == 2u) sim = mat3(0.625, 0.70, 0.0, 0.375, 0.30, 0.30, 0.0, 0.0, 0.70);
    else              sim = mat3(0.95, 0.0, 0.0, 0.05, 0.43333, 0.475, 0.0, 0.56667, 0.525);
    vec3 err = c - sim * c;
    vec3 shift = vec3(0.0, 0.7 * err.r + err.g, 0.7 * err.r + err.b);
    return clamp(c + shift, 0.0, 1.0);
}
void main() {
    vec3 c = texture(textures[nonuniformEXT(pc.scene)], i_uv).rgb;
    if (pc.mode != 0u) {
        if (pc.mode == 1u) {
            vec2 noiseuv = floor(gl_FragCoord.xy * pc.scale) + 0.5;
            c = sqrt(c);
            c += DITHER_NOISE(noiseuv) * pc.dither;
            c *= c;
        }
        // GLPalette_Postprocess: blend/contrast/gamma act on the palette colour
        c = unpack_rgba(palette[lut(ivec3(clamp(c, 0.0, 1.0) * 127.0 + 0.5))]).rgb;
    }
    c = mix(c, pc.blend.rgb, pc.blend.a);
    if (pc.colorblind != 0u)
        c = daltonize(c, pc.colorblind);
    c = pow(max(c * pc.contrast, vec3(0.0)), vec3(pc.gamma));
    if (pc.mode == 0u)
        c += SUPPRESS_BANDING() * pc.dither;
    o_color = vec4(c, 1.0);
}
