# Hit and kill feedback

HUD → Crosshair groups crosshair visibility, shape, size, outline and hitmarker
visibility (`hud_hitmarkers`, default 0), independent shape (`hitmarker_shape`)
and size (`hitmarker_size`). `reticle` shares the gameplay drawing paths with a
static side-by-side previews of the crosshair alone and with a full hitmarker,
with rows for normal, target and kill colors. Marker names describe their geometry:
diagonal streaks, short diagonals, diamond, double diagonals, double chevrons,
and broken square. Gameplay confirmations retain their brief fade and drift. Shared palette cvars
`crosshair_color` (grey), `crosshair_targetcolor` (white), `crosshair_killcolor`
(red) apply to both layers. `crosshair_ontarget` and `crosshair_onkill` toggle
state colors; kills take priority for the confirmation lifetime. Target detection
uses a normal weapon collision trace against the local server, including solid
occluders and all entities with `takedamage`, per split-screen player. Remote
servers without local QC state cannot supply target highlighting.
Audio → Hit Sounds (`s_hitfeedback`, default 0) enables synthesized feedback;
Hit Sound and Kill Sound submenus independently edit and preview nine cues,
volume, time stretch, playback speed, pitch, bass/treble, drive, echo and reverb.
All settings are archived (`s_hit_*`, `s_kill_*`); preview works while disabled.
Time stretch changes envelopes/timing without transposing; speed changes both
timing and pitch. Master SFX volume applies to previews and gameplay.

`qvm::Host::{before_damage,after_damage}` bracket QC `T_Damage`, including
nested calls. `qserver::host::feedback` captures live monster/player health,
attacker and entity generation, then confirms an actual health reduction.
World/self damage, scenery, corpses and invulnerability produce no feedback.
Only the attacking client receives reliable `svc_stufftext` comments
`//qhit 0` / `//qhit 1`; qclient consumes these exact lines as `HitFeedback`
events without executing commands. Classic clients can ignore the comments.
Servers/mods without this hook do not provide confirmations.
`host::feedback` coalesces events per frame (kills win), throttles rapid hit
audio, preserves kill visibility, and tracks split-screen players separately.
`qsound::feedback` adapts the owner's original Padwight synthesis/effects with
permission under this project's license; no recorded assets are copied.
Two bounded caches regenerate samples on settings/device-rate changes; the
audio thread only mixes PCM. State clears across maps/disconnects.
`hitmarker_test [kill]` previews the marker for screenshot diagnostics;
`hitfeedback_preview hit|kill` previews the configured sound.

