#version 460
#include "common.glsl"
// Multiplies the SSAO target into the scene colour after the opaque draws
// (blend: dst * src). Alpha is left alone by the blend state.
layout(location = 0) in vec2 i_uv;
layout(push_constant) uniform P { uint ao_tex; } pc;
layout(location = 0) out vec4 o_color;

// Depth-aware 4x4 blur of the AO target around the scene pixel at uv.
float ao_at(vec2 uv) {
    vec2 sz = vec2(textureSize(textures[nonuniformEXT(pc.ao_tex)], 0));
    ivec2 base = ivec2(floor(uv * sz - 1.5));
    ivec2 cp = ivec2(uv * sz);
    float zc = texelFetch(textures[nonuniformEXT(pc.ao_tex)], clamp(cp, ivec2(0), ivec2(sz) - 1), 0).g;
    float sum = 0.0, wsum = 0.0;
    for (int y = 0; y < 4; y++)
        for (int x = 0; x < 4; x++) {
            ivec2 q = clamp(base + ivec2(x, y), ivec2(0), ivec2(sz) - 1);
            vec2 t = texelFetch(textures[nonuniformEXT(pc.ao_tex)], q, 0).rg;
            float w = 1.0 / (0.001 + abs(t.g - zc) / max(zc, 1.0) * 50.0);
            sum += t.r * w;
            wsum += w;
        }
    return sum / wsum;
}
void main() {
    // scene viewport pixel -> AO target texel (the target may have reserved rows)
    vec2 sz = vec2(textureSize(textures[nonuniformEXT(pc.ao_tex)], 0));
    o_color = vec4(vec3(ao_at(gl_FragCoord.xy / sz)), 1.0);
}
