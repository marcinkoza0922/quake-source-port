Male-to-male cable add-on view models, generated 2026-11-16 with Meshy:
nano-banana concept images (hands `01a1e752-40e3-76d3-925e-ce8dbd6fd1eb`,
plug `01a1e752-414b-73ca-b60b-4996d85b3565`), then image-to-3D meshy-t2
Smart Topology + meshy-n1 texture (hands task
`01a1e752-bcbf-76a4-84c7-a727a4a858bc`, 2,678 tris for the pair; plug task
`01a1e752-c4b3-75e2-87e2-5cca2d6c26ba`, 889 tris). These are converted game
assets, not Quake or id Software data.

- `hand_left/`, `hand_right/`: one gloved fist each (the pair split at the
  bounding box centre), the owner's view direction along +X, 7.5 units deep.
  The fist's grip hole runs along Y (thumb side inwards).
- `plug/`: the connector, pins along +X, 12 units long.
- Each holds `model.mdl.zlib` (zlib MDL v6, one pose), `texture.rgb.zlib`
  (512×512 RGB8 keeping the Meshy UV layout) and `import.json`.

Rebuild from the Meshy GLBs (+ their `*_base_color.png` next to them):
```
blender -b --factory-startup --python tools/import_toy_model.py -- hands.glb assets/cable/hand_left --side left --fixed --length 7.5 --anchor bbox --keep-uv
blender -b --factory-startup --python tools/import_toy_model.py -- hands.glb assets/cable/hand_right --side right --fixed --length 7.5 --anchor bbox --keep-uv
blender -b --factory-startup --python tools/import_toy_model.py -- plug.glb assets/cable/plug --length 12 --anchor bbox --keep-uv
```
