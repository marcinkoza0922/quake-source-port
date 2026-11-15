Railgun toy view model, generated 2026-11-14 with Meshy: image-to-3D
meshy-t2.5 Smart Topology (target 3,500 tris, result 3,543 -> 3,527 packed)
+ meshy-7 4K texture from the three-quarter concept view, task
`01a1dcf9-8deb-708b-be03-3651f41a5df2`. These are converted game assets,
not Quake or id Software data.

- `railgun.mdl.zlib`: zlib-compressed Quake MDL v6, single pose, muzzle +X,
  44 units long. Its stored indexed skin is a glow mask (1 = blue coil energy).
- `texture.rgb.zlib`: 512×512 top-down RGB8 atlas preserving the Meshy UV layout
  (blue coil glow boosted while preserving bevel gradients; body darkened at
  load by `rail_model::BODY_GAIN`).
- `import.json`: conversion statistics.

Rebuild from the Meshy GLB + `model_base_color.png`:
`blender -b --factory-startup --python-exit-code 1 --python
tools/import_toy_model.py -- model.glb --out assets/railgun
--name railgun --length 44 --anchor origin --flip --back 14 --gain 1.0
--glow-blue --keep-uv`.
