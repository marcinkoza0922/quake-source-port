#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec3 i_wpos;
layout(push_constant) uniform P { vec4 color; uint tex; } pc;
layout(location = 0) out vec4 o_color;
void main() {
    vec4 c = tex(pc.tex, i_uv) * pc.color;
    if (c.a < 0.666 * pc.color.a) discard;
    o_color = vec4(apply_fog(c.rgb, i_wpos), c.a);
}
