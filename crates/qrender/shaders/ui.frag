#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec4 i_color;
layout(location = 2) flat in uint i_tex;
layout(location = 0) out vec4 o_color;
void main() {
    vec4 c = i_tex == NO_TEX ? vec4(1.0) : tex(i_tex, i_uv);
    o_color = c * i_color;
    if (o_color.a <= 0.0) discard;
}
