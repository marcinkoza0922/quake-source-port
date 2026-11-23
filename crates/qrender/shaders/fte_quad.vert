#version 460
#include "common.glsl"
// Instanced normal FTE billboards (40 bytes/instance, 6 verts, no index buffer).
layout(location = 0) in vec3 in_pos;
layout(location = 1) in float in_scale;
layout(location = 2) in float in_angle;
layout(location = 3) in vec4 in_color; // unorm8
layout(location = 4) in vec4 in_uv;    // s1, t1, s2, t2
layout(location = 0) out vec2 o_uv;
layout(location = 1) out vec4 o_color;
layout(location = 2) out vec3 o_wpos;

const int CIDX[6] = int[6](0, 1, 2, 0, 2, 3);

void main() {
    int ci = CIDX[gl_VertexIndex];
    vec3 pr = fr.vright.xyz * 1.5;
    vec3 pu = fr.vup.xyz * 1.5;
    vec3 wpos;
    if (in_angle != 0.0) {
        float x = sin(in_angle) * in_scale;
        float y = cos(in_angle) * in_scale;
        vec3 offs[4] = vec3[4](
            -pr * x - pu * y,
            -pr * y + pu * x,
             pr * x + pu * y,
             pr * y - pu * x
        );
        wpos = in_pos + offs[ci];
    } else {
        vec3 offs[4] = vec3[4](
            -pu * in_scale,
            -pr * in_scale,
             pu * in_scale,
             pr * in_scale
        );
        wpos = in_pos + offs[ci];
    }
    vec2 uvs[4] = vec2[4](
        vec2(in_uv.x, in_uv.y),
        vec2(in_uv.x, in_uv.w),
        vec2(in_uv.z, in_uv.w),
        vec2(in_uv.z, in_uv.y)
    );
    o_uv = uvs[ci];
    o_color = in_color;
    o_wpos = wpos;
    gl_Position = fr.viewproj * vec4(wpos, 1.0);
}
