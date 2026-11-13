Railgun toy view model, generated 2026-11-13 with Meshy: nano-banana concept
(task `01a1d66a-7796-72ce-a8a7-4b995f768ae1`, a rusty low-poly Quake-style
railgun with green coil slot) then image-to-3D meshy-t2 Smart Topology
(target 1,300 tris, result 1,365) + meshy-n1 texture, task
`01a1d66b-d386-773c-aea5-10018b935674`. These are converted game assets,
not Quake or id Software data.

- `railgun.mdl.zlib`: zlib-compressed Quake MDL v6, single pose, muzzle +X,
  44 units long. Its stored indexed skin is a glow mask (1 = energy).
- `texture.rgb.zlib`: 512×512 top-down RGB8 atlas (metal darkened ×0.6,
  energy normalised to full brightness).
- `import.json`: conversion statistics.

Rebuild: zip `model.fbx` + `model_base_color.png` from the Meshy task, then
`blender -b --factory-startup --python-exit-code 1 --python
tools/import_toy_model.py -- meshy_t2_railgun.zip --out assets/railgun
--name railgun --length 44 --anchor origin --flip --back 14 --gain 0.6 --glow`.
