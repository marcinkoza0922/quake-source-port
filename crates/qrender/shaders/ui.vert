#version 460
layout(location = 0) in vec2 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;
layout(location = 3) in uint in_tex;
layout(push_constant) uniform P { vec2 inv_size; } pc;
layout(location = 0) out vec2 o_uv;
layout(location = 1) out vec4 o_color;
layout(location = 2) flat out uint o_tex;
void main() {
    o_uv = in_uv; o_color = in_color; o_tex = in_tex;
    gl_Position = vec4(in_pos * pc.inv_size * 2.0 - 1.0, 0.0, 1.0);
}
