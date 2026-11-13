The project owner supplied the Meshy-generated retro portal gun on 2026-11-13
as `Meshy_AI_retro_portal_gun_20261113_image-to-3d-texture_fbx.zip`.
These are its converted game assets, not Quake or Valve game data.

- `projector.mdl.zlib`: zlib-compressed Quake MDL v6, single pose. Its temporary
  indexed skin stores a recoloring mask: 0 metal, 1 primary energy, 2 secondary
  energy, 3 selected muzzle energy, 4 primary paint, 5 secondary paint.
- `texture.rgb.zlib`: zlib-compressed 512×512 top-down RGB8 color atlas.
- `import.json`: conversion statistics and source archive name.

Rebuild using `tools/import_portal_gun.py` under Blender. See
[the add-on design](../../docs/ADDONS.md) for the command and runtime behavior.
The original FBX and 2048×2048 textures remain in the owner's external archive.
