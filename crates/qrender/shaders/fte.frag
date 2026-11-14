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
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
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
    vec4 c;
    if (pc.fogmix.w == 9.0) {
        // add-on gel: the texture alpha is a height field. The surface
        // frame comes from screen-space derivatives; a parallax shift and a
        // normal from the height gradient make the blob look raised, then it
        // is lit as a glossy liquid (two highlights, fresnel rim, thin dark
        // edges, a brighter thick middle). i_color: lit gel colour, alpha.
        vec3 dpx = dFdx(i_wpos), dpy = dFdy(i_wpos);
        vec2 dux = dFdx(i_uv), duy = dFdy(i_uv);
        vec3 n = normalize(cross(dpx, dpy));
        vec3 v = normalize(fr.eye.xyz - i_wpos);
        if (dot(n, v) < 0.0) n = -n;
        vec3 p2 = cross(dpy, n), p1 = cross(n, dpx);
        vec3 t = p2 * dux.x + p1 * duy.x;
        vec3 b = p2 * dux.y + p1 * duy.y;
        float im = inversesqrt(max(max(dot(t, t), dot(b, b)), 1e-12));
        t *= im;
        b *= im;
        // i_color: light (rgb), alpha; texture rgb: gel colour (white for
        // tinted sprites, which carry their colour in i_color)
        float h0 = tex(pc.tex, i_uv).a;
        vec2 vts = vec2(dot(v, t), dot(v, b)) / max(dot(v, n), 0.3);
        // parallax and gradient steps in world units (uv per unit from the
        // derivatives), so any atlas scale looks the same
        float upw = sqrt(max(dot(dux, dux), dot(duy, duy)) / max(max(dot(dpx, dpx), dot(dpy, dpy)), 1e-12));
        vec2 uv = i_uv - vts * h0 * 4.0 * upw;
        vec2 ts = vec2(max(1.6 * upw, 1.0 / float(textureSize(textures[nonuniformEXT(pc.tex)], 0).x)));
        vec4 g = tex(pc.tex, uv);
        float h = g.a;
        vec3 gc = i_color.rgb * g.rgb;
        float hx = tex(pc.tex, uv + vec2(ts.x, 0.0)).a - tex(pc.tex, uv - vec2(ts.x, 0.0)).a;
        float hy = tex(pc.tex, uv + vec2(0.0, ts.y)).a - tex(pc.tex, uv - vec2(0.0, ts.y)).a;
        vec3 nb = normalize(n - (t * hx + b * hy) * 3.5);
        vec3 l1 = normalize(vec3(0.35, 0.25, 1.0));
        vec3 l2 = normalize(v + fr.vup.xyz * 0.6 - fr.vright.xyz * 0.3);
        float s1 = pow(max(dot(nb, normalize(l1 + v)), 0.0), 70.0);
        float s2 = pow(max(dot(nb, normalize(l2 + v)), 0.0), 260.0);
        float fres = pow(1.0 - max(dot(nb, v), 0.0), 3.0);
        float lit = clamp(max(max(i_color.r, i_color.g), i_color.b) * 1.4, 0.3, 1.2);
        float edge = smoothstep(0.03, 0.18, h) * (1.0 - smoothstep(0.18, 0.4, h));
        vec3 base = gc * (0.6 + 0.4 * max(dot(nb, l1), 0.0)) * mix(0.75, 1.2, h) * (1.0 - 0.35 * edge);
        vec3 col = base + vec3(s1 * 0.7 + s2 * 0.45) * lit + gc * fres * 0.6;
        c = vec4(col, smoothstep(0.0, 0.08, h) * i_color.a);
    } else {
        c = tex(pc.tex, i_uv) * i_color;
    }
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
