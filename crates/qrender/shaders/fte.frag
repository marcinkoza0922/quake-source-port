#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec4 i_color;
layout(location = 2) in vec3 i_wpos;
layout(push_constant) uniform P { vec4 fogmix; uint tex; } pc;
layout(location = 0) out vec4 o_color;
void main() {
    vec4 c = tex(pc.tex, i_uv) * i_color;
    // fogmix.x: 1 fogs towards the fog colour (alpha blending),
    // 0 fades towards black (additive / modulating modes).
    if (fr.fog.w > 0.0) {
        float z = length(i_wpos - fr.eye.xyz) * fr.fog.w;
        float f = clamp(exp(-z * z), 0.0, 1.0);
        c.rgb = mix(fr.fog.rgb * pc.fogmix.x, c.rgb, f);
    }
    o_color = c;
}
