// Shared declarations for all graphics shaders (set 0 is global).
#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 0, std140) uniform Frame {
    mat4 viewproj;
    vec4 eye;       // xyz eye, w time
    vec4 fog;       // rgb colour, w density (already /64)
    vec4 vright;    // view right
    vec4 vup;       // view up
    vec4 vfwd;      // view forward
    vec4 sky;       // x skyfog, y skybox flag, z sky layer alpha
    uvec4 skybox0;  // rt bk lf ft
    uvec4 skybox1;  // up dn solid-layer alpha-layer
    vec4 alphas;    // water lava slime tele
} fr;

layout(set = 0, binding = 1) uniform sampler2D textures[];
layout(set = 0, binding = 2) readonly buffer AliasVerts { uint alias_verts[]; };
layout(set = 0, binding = 3) uniform sampler2DArray lightmap;
layout(set = 0, binding = 4) readonly buffer Materials { uvec4 materials[]; };
layout(set = 0, binding = 5) readonly buffer Palette { uint palette[]; };

const uint NO_TEX = 0xffffffffu;

vec4 tex(uint i, vec2 uv) { return texture(textures[nonuniformEXT(i)], uv); }

vec3 apply_fog(vec3 c, vec3 wpos) {
    if (fr.fog.w <= 0.0) return c;
    float z = length(wpos - fr.eye.xyz) * fr.fog.w;
    float f = clamp(exp(-z * z), 0.0, 1.0);
    return mix(fr.fog.rgb, c, f);
}

vec4 unpack_rgba(uint c) { return unpackUnorm4x8(c); }
