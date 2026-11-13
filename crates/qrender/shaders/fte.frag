#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec4 i_color;
layout(location = 2) in vec3 i_wpos;
// fogmix: x fog mode, y soft inverse fade distance (0 = hard),
// z scene MSAA samples, w blend mode (FteBlend order).
layout(push_constant) uniform P { vec4 fogmix; uint tex; } pc;
layout(location = 0) out vec4 o_color;

// View-space depth of a [0,1] depth value (setup_view: near 4, far 65536).
float linear_depth(float d) {
    const float n = 4.0, f = 65536.0;
    return n * f / (f - d * (f - n));
}

void main() {
    if (pc.fogmix.w == 8.0) {
        // add-on portal: oval mask over the quad, the view out of the linked
        // portal sampled in screen space (or a dim fill), a glowing rim
        vec2 q = i_uv * 2.0 - 1.0;
        float d = length(q);
        float aa = fwidth(d) * 1.5;
        if (d > 1.0) discard;
        vec3 inner = pc.tex != NO_TEX
            ? texture(textures[nonuniformEXT(pc.tex)], gl_FragCoord.xy / vec2(textureSize(textures[nonuniformEXT(pc.tex)], 0))).rgb
            : i_color.rgb * (0.25 + 0.2 * d);
        float rim = smoothstep(0.84 - aa, 0.9, d);
        vec3 glow = mix(i_color.rgb, vec3(1.0), 0.35 * (1.0 - smoothstep(0.9, 1.0, d)));
        o_color = vec4(mix(inner, glow, rim), 1.0 - smoothstep(1.0 - aa, 1.0, d));
        return;
    }
    vec4 c = tex(pc.tex, i_uv) * i_color;
    if (pc.fogmix.y > 0.0) {
        // Soft particles: fade as the particle nears the scene behind it.
        // MSAA uses the nearest covered sample (vkQuake fte_particles.inc).
        ivec2 px = ivec2(gl_FragCoord.xy);
        int ns = int(pc.fogmix.z);
        float d;
        if (ns > 1) {
            d = 1.0;
            for (int s = 0; s < ns; s++) d = min(d, texelFetch(scene_depth_ms, px, s).r);
        } else {
            d = texelFetch(scene_depth, px, 0).r;
        }
        float pz = dot(i_wpos - fr.eye.xyz, fr.vfwd.xyz);
        float fade = d < 1.0 ? clamp((linear_depth(d) - pz) * pc.fogmix.y, 0.0, 1.0) : 1.0;
        int bm = int(pc.fogmix.w);
        // Colour-based and premultiplied blends fade their RGB too.
        if (bm == 1 || bm == 3 || bm == 4 || bm == 6 || bm == 7) c.rgb *= fade;
        if (bm == 0 || bm == 2 || bm == 4 || bm == 5 || bm == 7) c.a *= fade;
    }
    // fogmix.x: 1 fogs towards the fog colour (alpha blending),
    // 0 fades towards black (additive / modulating modes).
    if (fr.fog.w > 0.0) {
        float z = length(i_wpos - fr.eye.xyz) * fr.fog.w;
        float f = clamp(exp(-z * z), 0.0, 1.0);
        c.rgb = mix(fr.fog.rgb * pc.fogmix.x, c.rgb, f);
    }
    o_color = c;
}
