"""Blender offline importer for the owner's Meshy FBX (see docs/ADDONS.md).

blender -b --factory-startup --python tools/import_portal_gun.py -- SOURCE.zip
Outputs embedded native MDL geometry, RGB texture and energy mask to assets/portal-gun.
The source archive stays external. No Quake palette or game data is needed.
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
TRIANGLES = 16000


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument('archive', type=Path)
    opts = args.parse_args(sys.argv[sys.argv.index('--') + 1:])
    out = ROOT / 'assets/portal-gun'
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='quake-portal-') as directory:
        with zipfile.ZipFile(opts.archive) as archive:
            archive.extractall(directory)
        fbx = next(Path(directory).rglob('*.fbx'))
        bpy.ops.import_scene.fbx(filepath=str(fbx))
        objects = [o for o in bpy.context.selected_objects if o.type == 'MESH']
        if len(objects) != 1:
            raise ValueError('Expected the supplied single-mesh Meshy export')
        obj = objects[0]
        bpy.context.view_layer.objects.active = obj
        # Source muzzle faces -X; rotate half a turn, keeping +Z upright.
        length = obj.dimensions.x
        factor = 36.0 / length
        for vertex in obj.data.vertices:
            p = obj.matrix_world @ vertex.co
            vertex.co = (-p.x * factor + 6.0, -p.y * factor, p.z * factor)
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
        # Classify texture color; metal stays non-fullbright. Mask distinguishes
        # bright chamber glass from the muted split-color painted emblem.
        mask = bytearray(SIZE * SIZE)
        for i in range(SIZE * SIZE):
            r, g, b = rgb[i * 3:i * 3 + 3]
            blue = b > r * 1.5 and b > g * 1.15 and b > 65
            orange = r > b * 2.2 and r > g * 1.5 and g > b * 1.5 and r > 85
            if blue or orange:
                mask[i] = (1 if blue else 2) if max(r, g, b) > 175 else (4 if blue else 5)
        # Emission tint at the muzzle follows the selected slot. Rasterize UVs
        # from forward-facing geometry so chamber and emblem colors stay paired.
        for poly in mesh.polygons:
            if poly.center.x < 10.0:
                continue
            points = [(uv[i].uv.x * SIZE - .5, (1 - uv[i].uv.y) * SIZE - .5) for i in poly.loop_indices]
            (ax, ay), (bx, by), (cx, cy) = points
            den = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
            if abs(den) < 1e-8:
                continue
            for y in range(max(0, math.floor(min(p[1] for p in points))), min(SIZE, math.ceil(max(p[1] for p in points)) + 1)):
                for x in range(max(0, math.floor(min(p[0] for p in points))), min(SIZE, math.ceil(max(p[0] for p in points)) + 1)):
                    a = ((by - cy) * (x - cx) + (cx - bx) * (y - cy)) / den
                    b = ((cy - ay) * (x - cx) + (ax - cx) * (y - cy)) / den
                    if a >= -.01 and b >= -.01 and a + b <= 1.01 and mask[y * SIZE + x] in (1, 2):
                        mask[y * SIZE + x] = 3
        bboxmin, bboxmax = bytes(4), bytes((255, 255, 255, 0))
        header = b'IDPO' + struct.pack('<i3f3ff3f8if', 6, *scale, *lo,
            max(Vector(v.co).length for v in mesh.vertices), 0, 0, 0,
            1, SIZE, SIZE, len(verts), len(triangles), 1, 0, 0, 1.0)
        data = header + struct.pack('<i', 0) + mask + b''.join(st) + b''.join(triangles)
        data += struct.pack('<i', 0) + bboxmin + bboxmax + b'meshy_projector'.ljust(16, b'\0') + b''.join(verts)
        (out / 'projector.mdl.zlib').write_bytes(zlib.compress(data, 9))
        (out / 'texture.rgb.zlib').write_bytes(zlib.compress(rgb, 9))
        info = dict(source=opts.archive.name, source_triangles=original,
                    triangles=len(triangles), vertices=len(verts), texture_size=SIZE,
                    removed_packed_degenerates=collapsed, bounds=[lo, hi],
                    mask_counts={str(i): mask.count(i) for i in range(6)})
        (out / 'import.json').write_text(json.dumps(info, indent=2) + '\n')
        print(json.dumps(info))


if __name__ == '__main__':
    main()
