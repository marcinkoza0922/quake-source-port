# Projectile shotguns and bullet time

Both appear in the Add-ons checklist and New Game picker. They obey the existing
new-game lock and local-only, non-deathmatch gate. The browser provides separate
settings pages, explicit Reset to Defaults buttons and Bullet Time key/controller
capture. Controls also lists the `+bullettime` action. Bind an unused key, e.g.
`bind b +bullettime`; no existing binding is overwritten automatically.

## Projectile shotguns

`addon_projectile_shotguns` selects the feature (default 1 in add-on games).
`addon_shotgun_speed` defaults to 10000 units/s, clamped to 100–30000. Reset
restores 10000. Standard QC `FireBullets` traceline calls are intercepted in
`qserver::host::trace`, after QC computes spread. This covers the shotgun,
super shotgun and soldier helper. QC still owns ammo, sounds, pellet count,
quad, animation and firing cadence; the original trace reports no immediate hit.

`qserver::addons::shotgun` creates zero-size MOVETYPE_FLY edicts with the
precached nail model, owner, `spike_touch` and `SUB_Remove`. FLY sweeps point
collision without the enlarged monster bounds of FLYMISSILE. Only these edicts
receive a 30000 velocity ceiling in server physics, preserving their direction
and configured speed without raising `sv_maxvelocity` for other entities.
Pellets live for the original 2048-unit range divided by speed, then expire.
The T_Damage hook changes marked pellets' 9 damage to 4 before QC armor/quad,
pain, kills and hit feedback. Nail impacts and blood use the mod's spike effects.
Ordinary edict serialization preserves pellets in flight and their attribution.
Unrecognized mod firing helpers, missing spike callbacks or missing nail models
retain their QC behavior rather than failing. Mod-specific extra parameters,
penetration and nonstandard damage behavior are not replaced generically.

## Bullet time

`src/host/bullet_time.rs` holds a shared resource for local players. Any living
local player's `+bullettime` input may activate it. Toggle mode responds to press
edges; quick taps are retained by qinput. Release, menus, pause, death,
intermission, disconnect and disallowed addon state cannot leave slowdown stuck.
Exhaustion requires releasing a held action before reuse, avoiding automatic
flicker as the meter regenerates. World and all client clocks use a 0.2 multiplier,
composed with accessibility speed and weapon-wheel slowdown; look input remains
responsive. No remote commands or extra usercmd bits are used.

Archived settings:

| Cvar | Default | Meaning |
| --- | --- | --- |
| `addon_bullet_time` | 1 | Select the add-on |
| `addon_bullet_time_toggle` | 0 | 0 Hold, 1 Toggle |
| `addon_bullet_time_meter` | 1 | Limited use with HUD meter; 0 unlimited |
| `addon_bullet_time_duration` | 5 | Charge capacity, real seconds (1–60) |
| `addon_bullet_time_regen` | 1 | Charge seconds restored per real second (0.1–10) |

Only inactive gameplay regenerates; paused/menu time does not charge or drain.
The meter is drawn in each local viewport. Charge resets on map change/load;
active slowdown is transient. `qserver::addons::settings` stores both add-ons'
selection and parameters in allowlisted `setting` lines in `.qsa` headers;
older headers use defaults. Plain `.sav` games cannot activate either feature.

Tests cover real QC pellet spawning/delayed impacts, damage, swept collision,
vanilla fallback, settings persistence, hold/toggle depletion and regeneration,
unlimited use and press/release handling. Tests needing id1 data skip if absent.
