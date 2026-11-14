"""Generic Blender importer for Meshy toy models (see docs/ADDONS.md).

blender -b --factory-startup --python tools/import_toy_model.py -- \
    SOURCE(.zip|dir|.fbx|.glb) [OUT] [NAME] [--out DIR] [--name NAME] [--length 36]
    [--anchor bbox|centroid|origin] [--back B] [--forward F] [--flip] [--flat]
    [--size 512] [--gain 0.5] [--glow]

Generalised from tools/import_portal_gun.py. Writes OUT/NAME.mdl.zlib (MDL v6,
one pose; NAME defaults to "model"), OUT/texture.rgb.zlib (top-down RGB8,
palette-converted at load) and OUT/import.json. Placement: the model is scaled
so its longest axis is --length units along +X (muzzle forward), +Z up.
  --flip    source muzzle faces -X: turn half a turn about Z.
  --flat    thinnest axis becomes Z (thrown props such as the boomerang).
  --anchor  bbox: centred on the bounding box, or with --back the rear end
            sits B units behind the origin; centroid: centred on the vertex
            centroid, moved --forward units (default 6) forward; origin: y/z
            kept as in the source, rear end at --back (default 6) behind.
  --gain    texture multiplier (alias overbright); --glow marks bright
            saturated "energy" texels as fullbright (skin index 1) at full
            brightness and applies --gain only to the rest.
No Quake palette or game data is needed.
"""
import argparse
import array
import json
import math
from pathlib import Path
import re
import struct
import sys
import tempfile
import zipfile
import zlib

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
SIZE = 512
TRIANGLES = 4000



def place(obj, opts):
    """Scale and orient the mesh in place (world space)."""
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    lo = [min(p[i] for p in pts) for i in range(3)]
    hi = [max(p[i] for p in pts) for i in range(3)]
    dims = [hi[i] - lo[i] for i in range(3)]
    if opts.flat:
        axes = sorted(range(3), key=lambda i: -dims[i])
    else:
        # longest horizontal axis -> X, Z stays up
        axes = [1, 0, 2] if dims[1] > dims[0] else [0, 1, 2]
    ysign = -1.0 if axes[0] == 1 and not opts.flat else 1.0
    factor = opts.length / dims[axes[0]]
    sign = -1.0 if opts.flip else 1.0
    if opts.anchor == 'centroid':
        centre = [sum(p[i] for p in pts) / len(pts) for i in range(3)]
        shift = opts.forward
    elif opts.anchor == 'origin':
        centre = [0.0, 0.0, 0.0]
        centre[axes[0]] = hi[axes[0]] if sign < 0 else lo[axes[0]]
        shift = -(6.0 if opts.back is None else opts.back)
    else:
        centre = [(hi[i] + lo[i]) / 2 for i in range(3)]
        shift = 0.0 if opts.back is None else opts.length / 2 - opts.back
    for vertex, p in zip(obj.data.vertices, pts):
        x = (p[axes[0]] - centre[axes[0]]) * factor
        y = (p[axes[1]] - centre[axes[1]]) * factor * ysign
        z = (p[axes[2]] - centre[axes[2]]) * factor
        vertex.co = (sign * x + shift, sign * y, z)


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument('archive', type=Path)
    args.add_argument('pos_out', nargs='?')
    args.add_argument('pos_name', nargs='?')
    args.add_argument('--out')
    args.add_argument('--name')
    args.add_argument('--length', type=float, default=36.0)
    args.add_argument('--anchor', choices=('bbox', 'centroid', 'origin'), default='bbox')
    args.add_argument('--back', type=float, help='units behind the origin')
    args.add_argument('--forward', type=float, default=6.0)
    args.add_argument('--flip', action='store_true', help='source muzzle faces -X')
    args.add_argument('--flat', action='store_true', help='thinnest axis becomes Z')
    args.add_argument('--size', type=int, default=512)
    args.add_argument('--gain', type=float, default=0.5)
    args.add_argument('--glow', action='store_true')
    opts = args.parse_args(sys.argv[sys.argv.index('--') + 1:])
    opts.out = opts.out or opts.pos_out
    opts.name = opts.name or opts.pos_name or 'model'
    if not opts.out:
        args.error('an output directory is required (OUT or --out)')
    global SIZE
    SIZE = opts.size
    out = Path(opts.out) if Path(opts.out).is_absolute() else ROOT / opts.out
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='quake-toy-') as directory:
        if opts.archive.suffix.lower() in ('.fbx', '.glb'):
            fbx = opts.archive
        else:
            if opts.archive.is_dir():
                directory = str(opts.archive)
            else:
                with zipfile.ZipFile(opts.archive) as archive:
                    archive.extractall(directory)
            fbx = next(p for ext in ('*.fbx', '*.glb') for p in Path(directory).rglob(ext))
        if fbx.suffix.lower() == '.glb':
            bpy.ops.import_scene.gltf(filepath=str(fbx))
        else:
            bpy.ops.import_scene.fbx(filepath=str(fbx))
        objects = [o for o in bpy.context.selected_objects if o.type == 'MESH']
        if len(objects) != 1:
            raise ValueError('Expected the supplied single-mesh Meshy export')
        obj = objects[0]
        bpy.context.view_layer.objects.active = obj
        place(obj, opts)
        obj.matrix_world.identity()
        obj.data.update()
        # Load the supplied PNG explicitly, avoiding FBX extracted JPEG copies.
        texture = next(p for p in fbx.parent.glob('*.png') if not any(
            suffix in p.stem for suffix in ('_normal', '_metallic', '_roughness')))
        image = bpy.data.images.load(str(texture), check_existing=False)
        image.colorspace_settings.name = 'Non-Color'
        # Keep the original surface for selected-to-active color baking. Meshy
        # packs many tiny UV islands, so interpolating them during decimation
        # damages the texture even when the simplified silhouette is accurate.
        source = obj.copy()
        source.data = obj.data.copy()
        bpy.context.collection.objects.link(source)
        source.name = 'Source high resolution'
        material = bpy.data.materials.new('Source color only')
        material.use_nodes = True
        nodes = material.node_tree.nodes
        nodes.clear()
        tex = nodes.new('ShaderNodeTexImage')
        tex.image = image
        emission = nodes.new('ShaderNodeEmission')
        output = nodes.new('ShaderNodeOutputMaterial')
        material.node_tree.links.new(tex.outputs['Color'], emission.inputs['Color'])
        material.node_tree.links.new(emission.outputs[0], output.inputs['Surface'])
        source.data.materials.clear()
        source.data.materials.append(material)
        bpy.ops.object.select_all(action='DESELECT')
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        mod = obj.modifiers.new('View weapon budget', 'DECIMATE')
        original = sum(len(p.vertices) - 2 for p in obj.data.polygons)
        mod.ratio = min(1.0, TRIANGLES / original)
        mod.use_collapse_triangulate = True
        bpy.ops.object.modifier_apply(modifier=mod.name)
        mod = obj.modifiers.new('Native triangles', 'TRIANGULATE')
        bpy.ops.object.modifier_apply(modifier=mod.name)
        mesh = obj.data
        while mesh.uv_layers:
            mesh.uv_layers.remove(mesh.uv_layers[0])
        mesh.uv_layers.new(name='Game atlas')
        bpy.ops.object.mode_set(mode='EDIT')
        bpy.ops.mesh.select_all(action='SELECT')
        bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.015)
        bpy.ops.object.mode_set(mode='OBJECT')
        baked = bpy.data.images.new('Game color', width=1024, height=1024, alpha=False)
        baked.colorspace_settings.name = 'Non-Color'
        target_material = bpy.data.materials.new('Baked game color')
        target_material.use_nodes = True
        bake_node = target_material.node_tree.nodes.new('ShaderNodeTexImage')
        bake_node.image = baked
        target_material.node_tree.nodes.active = bake_node
        mesh.materials.clear()
        mesh.materials.append(target_material)
        for poly in mesh.polygons:
            poly.material_index = 0
        source.select_set(True)
        bpy.context.view_layer.objects.active = obj
        scene = bpy.context.scene
        scene.render.engine = 'CYCLES'
        scene.cycles.samples = 1
        scene.render.bake.use_selected_to_active = True
        scene.render.bake.cage_extrusion = 0.25
        scene.render.bake.max_ray_distance = 0.5
        scene.render.bake.margin = 8
        bpy.ops.object.bake(type='EMIT')
        baked.scale(SIZE, SIZE)
        pixels = array.array('f', [0.0]) * (SIZE * SIZE * 4)
        baked.pixels.foreach_get(pixels)
        # Blender pixels are bottom-up; MDL texture rows are top-down.
        rgb = bytes(round(max(0, min(1, pixels[(y * SIZE + x) * 4 + c])) * 255)
                    for y in reversed(range(SIZE)) for x in range(SIZE) for c in range(3))
        uv = mesh.uv_layers.active.data
        normals_text = (ROOT / 'crates/qcore/src/anorms.rs').read_text()
        normals = [tuple(map(float, match)) for match in re.findall(
            r'\[\s*(-?\d+\.\d+),\s*(-?\d+\.\d+),\s*(-?\d+\.\d+)\s*\]', normals_text)]
        lo = [min(v.co[i] for v in mesh.vertices) for i in range(3)]
        hi = [max(v.co[i] for v in mesh.vertices) for i in range(3)]
        scale = [(hi[i] - lo[i]) / 255 for i in range(3)]
        verts, st, triangles, remap = [], [], [], {}
        collapsed = 0
        for poly in mesh.polygons:
            corners = []
            for loop_index in poly.loop_indices:
                loop = mesh.loops[loop_index]
                v = mesh.vertices[loop.vertex_index]
                packed = tuple(round((v.co[i] - lo[i]) / scale[i]) for i in range(3))
                normal = max(range(len(normals)), key=lambda n: v.normal.dot(Vector(normals[n])))
                s, t = uv[loop_index].uv
                texel = (max(0, min(SIZE - 1, round(s * SIZE - 0.5))),
                         max(0, min(SIZE - 1, round((1 - t) * SIZE - 0.5))))
                key = (packed, normal, texel)
                if key not in remap:
                    remap[key] = len(verts)
                    verts.append(bytes((*packed, normal)))
                    st.append(struct.pack('<3i', 0, *texel))
                corners.append(remap[key])
            p = [Vector(verts[i][:3]) for i in corners]
            if (p[1] - p[0]).cross(p[2] - p[0]).length_squared == 0:
                collapsed += 1
                continue
            # Native aliases use clockwise winding.
            triangles.append(struct.pack('<4i', 1, corners[0], corners[2], corners[1]))
        mask = bytearray(SIZE * SIZE)
        rgb = bytearray(rgb)
        for i in range(SIZE * SIZE):
            px = rgb[i * 3:i * 3 + 3]
            top = max(px)
            if opts.glow and top > 60 and sorted(px)[1] * 1.4 < top:
                # one dominant channel: energy; brighten it to full glow
                mask[i] = 1
                gain = 255 / top
            else:
                gain = opts.gain
            for c in range(3):
                rgb[i * 3 + c] = min(255, round(px[c] * gain))
        rgb = bytes(rgb)
        bboxmin, bboxmax = bytes(4), bytes((255, 255, 255, 0))
        header = b'IDPO' + struct.pack('<i3f3ff3f8if', 6, *scale, *lo,
            max(Vector(v.co).length for v in mesh.vertices), 0, 0, 0,
            1, SIZE, SIZE, len(verts), len(triangles), 1, 0, 0, 1.0)
        data = header + struct.pack('<i', 0) + mask + b''.join(st) + b''.join(triangles)
        data += struct.pack('<i', 0) + bboxmin + bboxmax + opts.name.encode()[:15].ljust(16, b'\0') + b''.join(verts)
        (out / f'{opts.name}.mdl.zlib').write_bytes(zlib.compress(data, 9))
        (out / 'texture.rgb.zlib').write_bytes(zlib.compress(rgb, 9))
        info = dict(source=opts.archive.name, source_triangles=original,
                    triangles=len(triangles), vertices=len(verts), texture_size=SIZE,
                    removed_packed_degenerates=collapsed, bounds=[lo, hi],
                    glow_texels=mask.count(1), length=opts.length,
                    anchor=opts.anchor, flip=opts.flip, flat=opts.flat, gain=opts.gain)
        (out / 'import.json').write_text(json.dumps(info, indent=2) + '\n')
        print(json.dumps(info))


if __name__ == '__main__':
    main()
