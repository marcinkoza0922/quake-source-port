"""Re-unwrap an imported toy MDL into a densely packed atlas (Blender, offline).

blender -b --factory-startup --python tools/relayout_toy_uv.py -- \
    IN.mdl.zlib OUT.mdl.zlib [--angle 60] [--margin 0.004]

Meshy smart-topology meshes come out of tools/import_toy_model.py split into
one tiny UV island per triangle, using as little as 5% of the 512² atlas.
This welds the MDL's vertices by position, runs Smart UV Project and packs
the islands (rotation allowed, concave shapes), then writes the same model
with only the skin coordinates changed: positions, normals and the triangle
order (and corner order) are kept, so tools/clean_toy_texture.py --from OLD
can resample the old texture triangle by triangle into the new layout.
The skin must be a plain (all-zero) mask; the old texture is not touched.
"""
import argparse
import math
from pathlib import Path
import struct
import sys
import zlib

import bpy


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('src', type=Path)
    ap.add_argument('dst', type=Path)
    ap.add_argument('--angle', type=float, default=60.0, help='smart project angle limit, degrees')
    ap.add_argument('--margin', type=float, default=0.004, help='island margin (atlas fraction)')
    a = ap.parse_args(sys.argv[sys.argv.index('--') + 1:])

    data = zlib.decompress(a.src.read_bytes())
    assert data[:4] == b'IDPO'
    header = struct.unpack_from('<i3f3ff3f8if', data, 4)
    nskins, w, h, nverts, ntris, nframes = header[11:17]
    assert nskins == 1 and nframes == 1, 'one skin, one frame expected'
    off = 84 + 4
    skin = data[off:off + w * h]
    assert not any(skin), 'skin mask is not empty; this tool would drop it'
    off += w * h
    off += nverts * 12  # old stverts are replaced
    tris = [struct.unpack_from('<4i', data, off + k * 16)[1:] for k in range(ntris)]
    off += ntris * 16
    frame_head = data[off:off + 4 + 8 + 16]
    off += len(frame_head)
    pverts = [data[off + k * 4:off + k * 4 + 4] for k in range(nverts)]

    # weld by packed position (the MDL grid), keeping the triangle order
    weld, coords = {}, []
    for v in pverts:
        if v[:3] not in weld:
            weld[v[:3]] = len(coords)
            coords.append(tuple(float(c) for c in v[:3]))
    # a triangle that welding would make degenerate or a duplicate of an
    # earlier one keeps its own vertices, so no face is lost
    faces, seen, corner_vert = [], set(), []
    for t in tris:
        f = tuple(weld[pverts[i][:3]] for i in t)
        if len(set(f)) < 3 or tuple(sorted(f)) in seen:
            f = tuple(range(len(coords), len(coords) + 3))
            coords.extend(tuple(float(c) for c in pverts[i][:3]) for i in t)
        seen.add(tuple(sorted(f)))
        faces.append(f)
        corner_vert.append(f)
    mesh = bpy.data.meshes.new('toy')
    mesh.from_pydata(coords, [], faces)
    assert len(mesh.polygons) == ntris, 'welding merged or dropped triangles'
    obj = bpy.data.objects.new('toy', mesh)
    bpy.context.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    mesh.uv_layers.new(name='atlas')
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.smart_project(angle_limit=math.radians(a.angle), island_margin=a.margin)
    bpy.ops.uv.pack_islands(rotate=True, margin=a.margin, shape_method='CONCAVE')
    bpy.ops.object.mode_set(mode='OBJECT')

    uv = mesh.uv_layers['atlas'].data
    remap, st, verts, out_tris = {}, [], [], []
    for k, poly in enumerate(mesh.polygons):
        # from_pydata keeps each face's corner order
        corners = []
        for c, li in enumerate(poly.loop_indices):
            old = tris[k][c]
            assert mesh.loops[li].vertex_index == corner_vert[k][c]
            u, v = uv[li].uv
            texel = (max(0, min(w - 1, round(u * w - 0.5))), max(0, min(h - 1, round((1 - v) * h - 0.5))))
            key = (old, texel)
            if key not in remap:
                remap[key] = len(verts)
                verts.append(pverts[old])
                st.append(struct.pack('<3i', 0, *texel))
            corners.append(remap[key])
        out_tris.append(struct.pack('<4i', 1, *corners))
    fields = list(header)
    fields[14] = len(verts)
    out = b'IDPO' + struct.pack('<i3f3ff3f8if', *fields) + struct.pack('<i', 0) + skin
    out += b''.join(st) + b''.join(out_tris) + frame_head + b''.join(verts)
    a.dst.write_bytes(zlib.compress(out, 9))
    print(f'{a.dst}: {len(verts)} vertices ({nverts} before), {ntris} triangles')


if __name__ == '__main__':
    main()
