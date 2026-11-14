# Hitstop add-on

Freezes the game for a split second when the player damages an enemy. Off by
default like every add-on; local single player started with add-ons only (no
deathmatch, split-screen, demos or network clients).

## How it works

- Server (`crates/qserver/src/host/feedback.rs`): the same damage hook as hit
  sounds. When `addon_hitstop` is on, a confirmed hit also sends
  `//qsp_hitstop <kill> <damage> <class> <target health before>` as a comment
  stufftext. Classes: 0 ranged, 1 melee, 2 explosive splash (damage dealt by
  `T_RadiusDamage`). Melee is a direct hit (inflictor is the player) within 96
  units while the player holds the axe (`weapon == IT_AXE`, which mods reuse)
  or a weapon with no ammo, so mod melee weapons count.
- Client (`qclient`): `ClientEvent::HitStop`; malformed lines are dropped.
- Host (`src/host/hitstop.rs`): `HitStop::trigger` applies the filters and
  timing in real seconds; `Host::hitstop_hold` feeds the `hold` flag of
  `Server::frame_held` (the menu uses the same freeze). The client keeps
  running, so sound and looking around continue, and the weapon model stays
  on its server frame.

## Settings

Add-ons → Hitstop (`menu_options hitstop`); all `CVAR_ARCHIVE`.

| Cvar | Default | Meaning |
|---|---|---|
| `addon_hitstop` | 0 | Master toggle |
| `addon_hitstop_weapons` | 0 | 0 all, 1 melee, 2 ranged, 3 explosives |
| `addon_hitstop_duration` | 60 | Freeze in ms (10–250) |
| `addon_hitstop_kill_bonus` | 2 | Multiplier on killing blows (1–4) |
| `addon_hitstop_shake` | 0 | Camera jolt 0–1; off with `a11y_reducemotion` or `gunkick 0` |

Advanced sub-page:

| Cvar | Default | Meaning |
|---|---|---|
| `addon_hitstop_targets` | 0 | 0 all enemies, 1 only tough ones |
| `addon_hitstop_tough` | 100 | Health before the hit that counts as tough |
| `addon_hitstop_scale_damage` | 3 | Extra ms per 10 damage |
| `addon_hitstop_min_damage` | 10 | Ignore hits below this (nails 9, super nails 18, pellets 4; axe 20 still counts) |
| `addon_hitstop_cooldown` | 150 | ms between freezes; pellets count once, kills ignore it |
| `addon_hitstop_max` | 250 | Cap on any one freeze in ms |
