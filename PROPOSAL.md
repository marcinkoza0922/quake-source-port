Write a full fledged Quake source port from scratch. It should be written in Rust using a Vulkan renderer and custom, fit-to-purpose ECS engine.

The repos to ironwail and vkQuake are included as well as a reference, as is a symlink to a directory where I have a bunch of quake maps installed. Exclude these from the git repo. 

It should be compatible with everything ironwail or vkQuake are compatible with, with special attention paid to Arcane Dimensions. It should also be compatible with the two official Machine Game wads included (under mg1 and mg3)

Third party dependencies should be light and be only used for "solved" problems, primitive interactions with the underlying system, or things that are trivial. Anything that is necessarily opinionated, like the engine or gui, needs to be built custom-fit for the project.

The game should have controller support with industry standard aim-assist practices. It should also have a nicer mod-menu.

Performance is key. It should at worst be within spitting distance of ironwail or vkQuake, and ideally even better than those.

Build a testing framework to automatically traverse each level in a way that finds each secret and kills every enemy on the hardest difficulty, though god mode can be enabled during this.

Give discrete tasks that do not interfere with each other to subagents.


## Combat feedback

HUD settings offer optional hit markers alongside crosshair controls. Each
crosshair shape has a distinct hit marker; kills use a different color.
Only confirmed damage to living enemies counts, including projectile and
explosive damage. Audio settings offer hit sounds, disabled by default,
with separate hit/kill cue editors and previews. Include a selection of the
owner's synthesized Padwight cues, with independent volume, time stretch,
speed, pitch, bass, treble, drive, echo and reverb for each cue. Save settings
with the user's config and support each local split-screen player's feedback.

## Save usability

Single-player autosaves rotate independently of manual/quicksaves. Gameplay settings control retention (default five, zero disables) and level-entry (on by default), timer, secret discovery, all enemies killed, and all secrets found triggers. Loading a save does not itself trigger an autosave.

Save/load screens use cards showing level name, save timestamp, health, armor, owned weapons and keys, and kills/secrets against their totals. The Load screen shows only the latest autosave, with a dedicated submenu listing the rotating history newest to oldest and each save's trigger. Inventory art comes from the active mod; adapters cover custom inventory fields and use names when a mod supplies no corresponding icon.

## Enhanced blood

An optional enhanced blood toggle and settings submenu add surface splatters
when enemies or corpses are shot, localized red wounds that follow enemies
and ragdolls, and contact painting from moving giblets. Settings control
splatter amount, size, lifetime, mark limit, wounds, and gib painting.
Blood decals vary in shape, orientation and proportions. Wall and trigger hits
produce no blood; explosive dismemberments throw dense splatter across a room.

## Optional weapon impacts

Enhancements → Impact Effects options independently enable shotgun bullet decals, explosion scorch
marks, and physical nail impacts. Nails embed in wood, enemies and ragdolls;
other solid materials deflect them into cosmetic objects that fall and settle.
A shared lifetime and count limit bound these client-only effects. Railgun
burns are out of scope (Quake II weapon).

Optional visual enhancements live under Options → Enhancements, alongside
Graphics. Water, blood, debris and impact effects have their own settings pages;
ambient occlusion and motion blur also live in Enhancements.


## See-through teleporters

An optional enhancement (off by default, `r_teleportwindows 1`, also available
in the Enhancements menu) makes visible intra-level teleporters show their linked destination with Portal-style
parallax as the player moves. Preserve the original dark look with a black-tinted
shimmer and animated sparkles. Slipgates, silent or activated teleport triggers,
and unrelated uses of teleport textures keep their existing appearance.
