# Gun sounds

Made by `tools/gun_sounds.py` (cut, equalised, compressed) from recordings
published under CC0 1.0 (checked on their pages 2026-11-24; no attribution
required, given anyway):

| Source file | Page | Authors |
|---|---|---|
| `Prepared SFX Library.7z` (1911: `A_34P`, `A_42P`; AR-15/M4: `D_24P`, `D_32P`): every shot | https://opengameart.org/content/the-free-firearm-sound-library | The Free Firearm Sound Library: Ben Jaszczak, Brian Nelson, Kevin Heras, Matthew Nanney |
| `reload.wav`: pistol dry click, magazine in, slide | https://opengameart.org/content/handgun-reload-sound-effect | see page |
| `gunreload1.wav`, `assaultriflereload1_0.wav`: pistol magazine out; rifle dry click, magazine out/in, bolt | https://opengameart.org/content/gun-reload-sounds | SpringySpringo |

Rebuild: `python3 tools/gun_sounds.py SRC_DIR`, with the three WAVs and the
extracted archive (those two folders are enough) anywhere below SRC_DIR. The
raw files are archived outside the repository in `~/Assets/Sounds/guns/`.
The rustle of raising and lowering the rifle's sight is synthesised
(`crates/qsound/src/rifle.rs`).
