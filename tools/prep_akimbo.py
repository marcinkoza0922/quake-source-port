"""Rebuild Meshy's pistol into the akimbo pistol (Blender, offline).

blender -b --factory-startup --python tools/prep_akimbo.py -- \
    ~/Assets/Meshy/akimbo/model.glb OUT_DIR

Widens the gun, and bolts on parts Meshy did not make: a slotted octagonal
muzzle brake, a bronze recoil weight under the frame and fins along the
slide's top. Writes OUT_DIR/gun.glb and its base colour PNG for
tools/import_toy_model.py (see docs/ADDONS.md for the whole pipeline).
"""
import math
from pathlib import Path
import shutil
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

WIDE = 1.35


def main():
    src, out = (Path(p) for p in sys.argv[sys.argv.index('--') + 1:])
    out.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(src))
    obj = next(o for o in bpy.context.selected_objects if o.type == 'MESH')
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    # model centimetres: muzzle -X, up +Z
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-6)
    bm.transform(Matrix.Diagonal((100, 100 * WIDE, 100, 1)) @ obj.matrix_world)
    uv = bm.loops.layers.uv.active

    def patch(x0, x1, z0, z1):
        """Mean UV of the source faces on the gun's side there."""
        pts = [l[uv].uv for f in bm.faces for l in f.loops
               if x0 < f.calc_center_median().x < x1 and z0 < f.calc_center_median().z < z1 and abs(f.normal.y) > 0.8]
        pts.sort(key=lambda p: (p.x, p.y))
        return pts[len(pts) // 2].copy()

    steel = patch(0, 15, 19, 27)
    bronze = patch(-12, -2, 13, 17)

    # hidden: the barrel inside the slide and stubs inside the grip
    seen, drop = set(), []
    for f in bm.faces:
        if f in seen:
            continue
        part, todo = [], [f]
        seen.add(f)
        while todo:
            g = todo.pop()
            part.append(g)
            for h in (h for e in g.edges for h in e.link_faces if h not in seen):
                seen.add(h)
                todo.append(h)
        xs = [v.co.x for g in part for v in g.verts]
        size = max(xs) - min(xs)
        if size > 78 and max(v.co.z for g in part for v in g.verts) < 29:
            drop += [g for g in part if -44 < g.calc_center_median().x < -2 or g.calc_center_median().x > 18]
        elif size < 2 and min(xs) > 30 and len(part) < 36 and min(v.co.z for g in part for v in g.verts) > 13:
            drop += part
    bmesh.ops.delete(bm, geom=drop, context='FACES')

    def add(verts, faces, where):
        vs = [bm.verts.new(v) for v in verts]
        for f in faces:
            face = bm.faces.new([vs[i] for i in f])
            for k, loop in enumerate(face.loops):
                loop[uv].uv = where + Vector((0.0004 * (k & 1), 0.0004 * (k >> 1)))

    def prism(x0, x1, profile, where, inset=0.0):
        """A solid along X from a (y, z) outline (counter-clockwise seen
        from +X), its ends chamfered by `inset`."""
        n = len(profile)
        cy = sum(p[0] for p in profile) / n
        cz = sum(p[1] for p in profile) / n
        hy = max(abs(p[0] - cy) for p in profile)
        k = 1 - inset / hy if inset else 1.0
        rings = [(x0, k), (x0 + inset, 1.0), (x1 - inset, 1.0), (x1, k)] if inset else [(x0, 1.0), (x1, 1.0)]
        verts = [(x, cy + (y - cy) * s, cz + (z - cz) * s) for x, s in rings for y, z in profile]
        faces = [[r * n + i, r * n + (i + 1) % n, (r + 1) * n + (i + 1) % n, (r + 1) * n + i]
                 for r in range(len(rings) - 1) for i in range(n)]
        faces = [f[::-1] for f in faces]
        last = (len(rings) - 1) * n
        faces += [list(range(n)), [last + i for i in reversed(range(n))]]
        add(verts, [f[::-1] for f in faces], where)

    def octagon(ry, rz, cz):
        return [(ry * math.cos(a), cz + rz * math.sin(a)) for a in (math.radians(22.5 + 45 * i) for i in range(8))]

    def tube(x0, x1, outer, inner, cz, where):
        """An octagonal tube with chamfered lips, muzzle at x0."""
        ry, rz = outer
        prof = [(x0 + 4, inner), (x0, inner), (x0, 0.86), (x0 + 1.2, 1.0), (x1 - 1.2, 1.0), (x1, 0.8)]
        verts = [(x, y, z) for x, s in prof for y, z in octagon(ry * s, rz * s, cz)]
        faces = [[r * 8 + i, (r + 1) * 8 + i, (r + 1) * 8 + (i + 1) % 8, r * 8 + (i + 1) % 8]
                 for r in range(len(prof) - 1) for i in range(8)]
        faces.append([i for i in range(8)])
        add(verts, faces, where)
        # the face across the back end
        n = (len(prof) - 1) * 8
        add([verts[n + i] for i in range(8)], [list(range(8))[::-1]], where)

    # muzzle brake on the barrel, two collars, with raised bars between
    tube(-61, -47.2, (10.2, 9.4), 0.47, 22.5, steel)
    for x in (-58.6, -55.4, -52.2):
        for s in (-1, 1):
            prism(x, x + 1.7, [(s * 8.8 - 1.7, 18.8), (s * 8.8 + 1.7, 18.8), (s * 8.8 + 1.7, 26.2), (s * 8.8 - 1.7, 26.2)], bronze)
    # recoil weight under the frame, with a sloped nose
    prism(-40, -13, [(-6.6, 3.5), (6.6, 3.5), (8.0, 6.0), (8.0, 12.0), (6.4, 14.5), (-6.4, 14.5), (-8.0, 12.0), (-8.0, 6.0)], bronze, 1.5)
    prism(-43.5, -40, [(-5.2, 5.2), (5.2, 5.2), (5.2, 12.5), (-5.2, 12.5)], steel, 0.8)
    for x in (-36, -27, -18):
        for s in (-1, 1):
            verts = [(x + 1.9 * math.cos(a), s * y, 9.0 + 1.9 * math.sin(a)) for y in (7.6, 8.9) for a in (math.radians(60 * i) for i in range(6))]
            faces = [[i, (i + 1) % 6, 6 + (i + 1) % 6, 6 + i] for i in range(6)] + [[6 + i for i in range(6)]]
            add(verts, faces if s < 0 else [f[::-1] for f in faces], steel)
    # cooling fins on the slide's top
    for i in range(4):
        x = -24 + i * 5.2
        prism(x, x + 2.2, [(-4.2, 28.6), (4.2, 28.6), (3.0, 31.6), (-3.0, 31.6)], steel, 0.4)
    # Meshy left the slide's back open: a plate, and the firing pin's cap
    prism(31.0, 36.4, octagon(7.1, 5.9, 21.6), steel, 0.6)
    prism(36.2, 37.6, octagon(2.9, 2.9, 22.2), bronze, 0.4)
    bm.transform(Matrix.Diagonal((0.01, 0.01, 0.01, 1)))
    bm.to_mesh(obj.data)
    obj.matrix_world = Matrix.Identity(4)
    obj.data.update()
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.ops.export_scene.gltf(filepath=str(out / 'gun.glb'), use_selection=True, export_image_format='NONE')
    shutil.copy(src.with_name(src.stem + '_base_color.png'), out / 'gun_base_color.png')
    print('TRIS', sum(len(p.vertices) - 2 for p in obj.data.polygons))


main()
