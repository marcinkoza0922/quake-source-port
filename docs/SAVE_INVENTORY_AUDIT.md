# Installed mod save-inventory audit

Read-only audit on 2026-11-11 using `tools/audit_save_inventory.py`, covering all 32 installed game/mod directories. This checks compiled inventory constants, saved entity-field layouts, available HUD pictures and supplied QC; it is not a full gameplay playthrough. Sources and game assets remain outside the repository.

| Family / mods | Findings and preview handling |
| --- | --- |
| id1, bbelief, contract, copper, dmd, dopa, func_mapjam5, honey, mg1, pun, q64, qbj, qbj2, rubicon2, terra, udob | Standard inventory masks; each mounted mod's replacement WAD supplies its own weapon/key pictures. Copper retains standard key bits. MG1 declares the standard weapon inventory. |
| mg3 | Standard inventory plus `upgrade_flag`, `IT_MJOLNIR` and `IT_LASER_CANNON`. Native `gfx/weapons/ui_h_weapon_{mjolnir,laser}_1.lmp` art is preferred for the extra weapon bits. |
| hipnotic | Mjolnir, laser and proximity bits; native WAD icons. |
| rogue | Different axe bit and extra lava/multi/plasma weapons; native WAD icons. |
| ad, xmasjam2020, xmasjam2021 | Separate `moditems`, upgraded axe/SSG/lightning, four custom keys with saved names and HUD skins. Preview uses AD weapon and custom-key LMP art. |
| func_xmasjam2017_v2, xmasjam2018, xmasjam2019 | Earlier AD layouts, custom-key names without saved HUD skin fields and without separate custom-key pictures in their own archives. Names remain visible when art cannot be resolved from mounted assets. |
| mjolnir | Installed content is Keep-based: different axe bit; `items`, `items2`, `moditems`, `perms` inventory banks; AD custom-key fields; many extra weapon icons in its WAD. Preview uses compiled masks and Keep icon mappings. Custom keys without corresponding shipped HUD pictures use saved names. |
| qbj3 | Standard weapon slots with replaced art, wrench icon, themed key strips and key counts encoded in the player's `worldtype`. Preview selects the saved theme and count frame, rather than displaying the entire strip. |
| dwellv2p2 | `items_dwell` adds quad shotgun, railstaff and rifle; no distinct HUD pictures are shipped for these extra types. Preview retains their names. |
| ctf | Standard inventory plus hook bit; no single-player autosaves in multiplayer. |
| Oxyblack_Fortress_by_Heresy, fallen1c, func_mapjam3, retrojam2 | No own progs in the inspected directory; inventory behavior comes from the mounted base game. |
| rrp | Local `pak0.pak` is truncated (directory extends beyond EOF). Its inventory could not be audited; repair/reinstall the local data before verification. |

Additional mods can reuse named item constants without depending on a directory name. Entirely new inventory conventions still require a preview adapter. Missing icons show names rather than substituting unrelated base-game art. Cards do not execute arbitrary CSQC HUD scripts.
