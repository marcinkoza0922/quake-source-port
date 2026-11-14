#version 460
#include "common.glsl"
// Preserve sample 0 exactly; resolving depth or alpha would change AO edges.
layout(location = 0) out vec2 o_source;
void main() {
    ivec2 px = ivec2(gl_FragCoord.xy);
    o_source = vec2(texelFetch(scene_depth_ms, px, 0).r,
                    texelFetch(scene_color_ms, px, 0).a);
}
