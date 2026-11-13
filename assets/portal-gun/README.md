The portal gun was regenerated on 2026-11-13 with Meshy image-to-3D from the owner's
concept render: meshy-t2 Smart Topology (target 1,500 tris, result 1,354)
plus the meshy-n1 texture, task `01a1d654-2976-739b-adcb-b14454690097`.
`model.fbx` and `model_base_color.png` were zipped as `meshy_t2_portal_gun.zip`
before import, which leaves the low-poly faceted shape untouched.
These are its converted game assets, not Quake or Valve game data.

- `projector.mdl.zlib`: zlib-compressed Quake MDL v6, single pose. Its temporary
  indexed skin stores a recoloring mask: 0 metal, 1 primary energy, 2 secondary
  energy, 3 selected muzzle energy, 4 primary paint, 5 secondary paint.
- `texture.rgb.zlib`: zlib-compressed 512×512 top-down RGB8 color atlas.
- `import.json`: conversion statistics and source archive name.

Rebuild using `tools/import_portal_gun.py` under Blender. See
[the add-on design](../../docs/ADDONS.md) for the command and runtime behavior.
The original FBX and 2048×2048 textures remain in the owner's external archive.
