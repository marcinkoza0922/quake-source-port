#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(push_constant) uniform P { vec4 blend; float gamma; float contrast; uint scene; } pc;
layout(location = 0) out vec4 o_color;
void main() {
    vec3 c = texture(textures[nonuniformEXT(pc.scene)], i_uv).rgb;
    c = mix(c, pc.blend.rgb, pc.blend.a);
    c = pow(max(c * pc.contrast, vec3(0.0)), vec3(pc.gamma));
    o_color = vec4(c, 1.0);
}
