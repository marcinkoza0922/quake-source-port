# Dodge / dive

`addon_dodge` is archived, enabled by default within add-on games and saved in
the add-on settings header. The browser and Controls expose `+dodge` for keyboard
or controller binding. No default binding replaces existing controls.

Input preserves brief presses until a server tick. Usercmd bit 7 carries the
press independently of fire/jump and toy IDs (now masked to bits 4–6). Each local
player has independent input. The server consumes the bit once, including when
the launch is rejected. `addons::dodge` checks add-on authorization, living walk
players, water depth and a 0.8 second recovery. Ground contact is not required;
a fresh press can launch during a jump or fall using the same directional impulse. It normalizes
movement directions in yaw space and launches at 550 units/s horizontally,
190 vertically for forward dives or 250 for other leaps. Standard Quake physics
handles gravity and swept collision; collision hulls do not change.

Per-level, per-player animation state uses simulation time, so bullet time
slows the motion naturally. A 0.65 second sine-squared envelope pitches the body
into a forward dive or backward leap, rolls side jumps and lowers the body
slightly, then recovers. The renderer applies this procedural pose only to the
local player body, including portal views, leaving camera and weapon unchanged.
It works with mod alias player models without assuming vanilla frame numbers.
Map changes and save loads reset transient animation and recovery.
