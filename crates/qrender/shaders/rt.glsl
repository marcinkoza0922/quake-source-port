// Ray-traced reflections (r_water_reflections 2, src/rt): set 1 holds the
// TLAS (world, brush entities, this frame's posed alias models) and what a
// hit needs to be shaded like the raster path: world vertices and triangle
// textures, the texture animation table and the alias triangle attributes.
// Included by water.glsl when RT is defined (water_rt*.frag).
#extension GL_EXT_ray_query : require
#include "rt_types.glsl"
// hits have no screen derivatives: textures are sampled at a LOD from the
// ray footprint (sky_common.glsl reads it through SKY_LOD)
float rt_lod = 0.0;
#define SKY_LOD rt_lod
#include "sky_common.glsl"

layout(set = 1, binding = 0) uniform accelerationStructureEXT tlas;
layout(set = 1, binding = 1, std430) readonly buffer RtVerts { float rt_verts[]; };
layout(set = 1, binding = 2, std430) readonly buffer RtIdx { uint rt_idx[]; };
layout(set = 1, binding = 3, std430) readonly buffer RtTex { uint rt_tex[]; };
layout(set = 1, binding = 4, std430) readonly buffer RtGeom { uint rt_geom[]; };
layout(set = 1, binding = 5, std430) readonly buffer RtAnim { uint rt_anim[]; };
layout(set = 1, binding = 6, std430) readonly buffer RtDraws { RtDraw rt_draws[]; };
layout(set = 1, binding = 7, std430) readonly buffer RtAttr { vec4 rt_attr[]; };

const uint RT_WV = 13u;        // floats per WorldVertex
const uint RT_SKY = 0x80000000u; // rt_tex[]: sky surface

vec3 rt_bary(vec2 b) { return vec3(1.0 - b.x - b.y, b.x, b.y); }

// Triangle (vertex indices, texture | RT_SKY) of a brush instance's geometry.
uvec4 rt_brush_tri(uint custom, uint geom, uint prim) {
    uint i = rt_geom[(custom & 0x3fffffu) * 2u + geom] + prim;
    return uvec4(rt_idx[i * 3u], rt_idx[i * 3u + 1u], rt_idx[i * 3u + 2u], rt_tex[i]);
}

vec3 rt_vattr(uvec4 t, uint ofs, uint n, vec3 b) {
    vec3 r = vec3(0.0);
    for (uint k = 0u; k < n; k++) {
        r[k] = rt_verts[t.x * RT_WV + ofs + k] * b.x + rt_verts[t.y * RT_WV + ofs + k] * b.y
             + rt_verts[t.z * RT_WV + ofs + k] * b.z;
    }
    return r;
}

vec3 rt_vpos(uint v) { return vec3(rt_verts[v * RT_WV], rt_verts[v * RT_WV + 1u], rt_verts[v * RT_WV + 2u]); }

uvec4 rt_material(uvec4 t, uint custom) {
    uint tex = t.w & 0xffffu;
    return materials[rt_anim[tex + ((custom & RT_ALT) != 0u ? RT_ANIM_ALT : 0u)]];
}

// Texture uv of a brush hit (st in miptex texels; external replacements
// may differ in size); sets rt_lod for a footprint of `foot` world units.
vec2 rt_brush_uv(uvec4 m, vec2 st, float foot) {
    vec2 ts = vec2(textureSize(textures[nonuniformEXT(m.x)], 0));
    vec2 size = m.w != 0u ? vec2(m.w & 0xffffu, m.w >> 16) : ts;
    rt_lod = log2(max(foot * ts.x / size.x, 1e-4));
    return st / size;
}

// Alias hit attributes: texture coordinates and Quake's shadedot.
vec3 rt_alias_st_shade(uint prim, vec3 b, out uint draw) {
    vec4 a0 = rt_attr[prim * 3u], a1 = rt_attr[prim * 3u + 1u], a2 = rt_attr[prim * 3u + 2u];
    draw = floatBitsToUint(a2.y);
    vec2 st = a0.xy * b.x + a0.zw * b.y + a1.xy * b.z;
    return vec3(st, a1.z * b.x + a1.w * b.y + a2.x * b.z);
}

float rt_alias_lod(uint skin, float foot) {
    // skins cover a model a few times their texel count per unit
    return log2(max(foot * float(textureSize(textures[nonuniformEXT(skin)], 0).x) / 192.0, 1e-4));
}

// Alpha-tested candidates: fence textures and holey alias skins (geometry 1).
bool rt_opaque_at(uint custom, uint geom, uint prim, vec3 b, float foot) {
    if (custom == RT_ALIAS) {
        uint alias_prim = (geom == 1u ? floatBitsToUint(fr.refl.y) : 0u) + prim;
        uint di;
        vec3 sts = rt_alias_st_shade(alias_prim, b, di);
        RtDraw d = rt_draws[di];
        if ((d.texs.w & 1u) == 0u) return true;
        return textureLod(textures[nonuniformEXT(d.texs.x)], sts.xy, rt_alias_lod(d.texs.x, foot)).a >= 0.666;
    }
    uvec4 t = rt_brush_tri(custom, geom, prim);
    uvec4 m = rt_material(t, custom);
    vec2 uv = rt_brush_uv(m, rt_vattr(t, 3u, 2u, b).xy, foot);
    return textureLod(textures[nonuniformEXT(m.x)], uv, rt_lod).a >= 0.666;
}

vec3 rt_shade_alias(uint prim, vec3 b, float foot) {
    uint di;
    vec3 sts = rt_alias_st_shade(prim, b, di);
    RtDraw d = rt_draws[di];
    float lod = rt_alias_lod(d.texs.x, foot);
    vec3 rgb = textureLod(textures[nonuniformEXT(d.texs.x)], sts.xy, lod).rgb;
    if (d.texs.z != NO_TEX && (d.texs.w & 4u) != 0u) {
        vec2 m = textureLod(textures[nonuniformEXT(d.texs.z)], sts.xy, 0.0).rg * 255.0 + 0.5;
        uint top = uint(m.x), bot = uint(m.y);
        if (top > 0u) {
            uint k = top - 1u, row = d.poses.z;
            uint i = row < 8u ? row * 16u + k : row * 16u + 15u - k;
            rgb += unpack_rgba(palette[i]).rgb - unpack_rgba(palette[16u + k]).rgb;
        }
        if (bot > 0u) {
            uint k = bot - 1u, row = d.poses.w;
            uint i = row < 8u ? row * 16u + k : row * 16u + 15u - k;
            rgb += unpack_rgba(palette[i]).rgb - unpack_rgba(palette[6u * 16u + k]).rgb;
        }
    }
    vec3 shade = (d.texs.w & 2u) != 0u ? vec3(2.0) : d.light.rgb * (sts.z * 2.0);
    vec3 fb = d.texs.y != NO_TEX ? textureLod(textures[nonuniformEXT(d.texs.y)], sts.xy, lod).rgb : vec3(0.0);
    return clamp(rgb * shade + fb, 0.0, 1.0);
}

// A world or brush entity hit at `p` (normal `n` facing the ray).
vec3 rt_shade_brush(uvec4 t, uint custom, vec3 b, vec3 p, vec3 n, vec3 dir, float foot) {
    if ((t.w & RT_SKY) != 0u) {
        rt_lod = 0.0;
        vec3 sky = sky_color(dir);
        return fr.fog.w > 0.0 ? mix(sky, fr.fog.rgb, fr.sky.x) : sky;
    }
    uvec4 m = rt_material(t, custom);
    vec2 st = rt_vattr(t, 3u, 2u, b).xy;
    uint kind = m.z & 0xfu;
    if (kind != 0u) {
        // liquids: the classic warp, unlit
        st += 8.0 * sin(st.yx * 0.125 + fr.eye.w);
        rt_lod = log2(max(foot, 1e-4));
        vec2 ts = vec2(textureSize(textures[nonuniformEXT(m.x)], 0));
        return textureLod(textures[nonuniformEXT(m.x)], st / 64.0, rt_lod + log2(ts.x / 64.0)).rgb;
    }
    vec2 uv = rt_brush_uv(m, st, foot);
    vec4 c = textureLod(textures[nonuniformEXT(m.x)], uv, rt_lod);
    bool fullbright = (m.z & 0x20u) != 0u || (fr.softf.x & 16u) != 0u;
    vec3 light = vec3(1.0);
    if (!fullbright) {
        light = textureLod(lightmap, rt_vattr(t, 5u, 3u, b), 0.0).rgb;
        if (light_count.x > 0u) light = min(light + dyn_lights(p, n), vec3(2.0));
    }
    vec3 fb = m.y != NO_TEX ? textureLod(textures[nonuniformEXT(m.y)], uv, rt_lod).rgb : vec3(0.0);
    return clamp(c.rgb * light + fb, 0.0, 1.0);
}

vec3 rt_fog(vec3 c, float dist) {
    if (fr.fog.w <= 0.0) return c;
    float z = dist * fr.fog.w;
    return mix(fr.fog.rgb, c, clamp(exp(-z * z), 0.0, 1.0));
}

// Colour seen from `o` along `dir`, fogged over the whole path (`dist0`
// from the eye to `o`, plus the ray). `cone`: pixel footprint per unit of
// distance.
vec3 rt_reflect(vec3 o, vec3 dir, float dist0, float cone) {
    rayQueryEXT rq;
    rayQueryInitializeEXT(rq, tlas, gl_RayFlagsNoneEXT, 0xffu, o, 0.0, dir, 65536.0);
    while (rayQueryProceedEXT(rq)) {
        if (rayQueryGetIntersectionTypeEXT(rq, false) != gl_RayQueryCandidateIntersectionTriangleEXT) continue;
        float foot = (dist0 + rayQueryGetIntersectionTEXT(rq, false)) * cone;
        if (rt_opaque_at(rayQueryGetIntersectionInstanceCustomIndexEXT(rq, false),
                         uint(rayQueryGetIntersectionGeometryIndexEXT(rq, false)),
                         uint(rayQueryGetIntersectionPrimitiveIndexEXT(rq, false)),
                         rt_bary(rayQueryGetIntersectionBarycentricsEXT(rq, false)), foot))
            rayQueryConfirmIntersectionEXT(rq);
    }
    if (rayQueryGetIntersectionTypeEXT(rq, true) == gl_RayQueryCommittedIntersectionNoneEXT) {
        rt_lod = 0.0;
        vec3 sky = sky_color(dir);
        return fr.fog.w > 0.0 ? mix(sky, fr.fog.rgb, fr.sky.x) : sky;
    }
    float t = rayQueryGetIntersectionTEXT(rq, true);
    float foot = (dist0 + t) * cone;
    uint custom = rayQueryGetIntersectionInstanceCustomIndexEXT(rq, true);
    uint geom = uint(rayQueryGetIntersectionGeometryIndexEXT(rq, true));
    uint prim = uint(rayQueryGetIntersectionPrimitiveIndexEXT(rq, true));
    vec3 b = rt_bary(rayQueryGetIntersectionBarycentricsEXT(rq, true));
    vec3 c;
    if (custom == RT_ALIAS) {
        uint alias_prim = (geom == 1u ? floatBitsToUint(fr.refl.y) : 0u) + prim;
        c = rt_shade_alias(alias_prim, b, foot);
    } else {
        uvec4 tri = rt_brush_tri(custom, geom, prim);
        if ((tri.w & RT_SKY) != 0u) return rt_shade_brush(tri, custom, b, o, vec3(0.0), dir, foot);
        mat4x3 o2w = rayQueryGetIntersectionObjectToWorldEXT(rq, true);
        vec3 vn = rt_vattr(tri, 10u, 3u, b);
        vec3 n = dot(vn, vn) > 1e-6
            ? normalize(mat3(o2w) * vn)
            : normalize(mat3(o2w) * cross(rt_vpos(tri.y) - rt_vpos(tri.x), rt_vpos(tri.z) - rt_vpos(tri.x)));
        if (dot(n, dir) > 0.0) n = -n;
        c = rt_shade_brush(tri, custom, b, o + dir * t, n, dir, foot);
    }
    return rt_fog(c, dist0 + t);
}
