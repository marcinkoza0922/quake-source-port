#version 460
#include "common.glsl"
#include "water_screen.glsl"
// Evaluates the Enhanced Water screen-droplet field at reduced resolution
// (<= 960x540 RGBA16F) so post.frag does one bilinear sample instead of
// running multi-layer hashed drop evaluation at full 4K.
layout(location = 0) in vec2 i_uv;
layout(push_constant) uniform P {
    float age;
    uint seed;
    uint high;
    float aspect;
    float px;
} pc;
layout(location = 0) out vec4 o_drop;
void main() {
    WsDrop wd = ws_droplets(i_uv, pc.age, pc.seed, pc.high != 0u, pc.aspect, pc.px);
    o_drop = vec4(wd.off * vec2(1.0 / pc.aspect, 1.0) * wd.m, wd.m, wd.spec * wd.m);
}
