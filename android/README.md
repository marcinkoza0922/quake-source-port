# Android build

The game is built as a `cdylib` (`libmain.so`) that SDL's `SDLActivity` loads, together
with a shared `libSDL3.so`. `.github/workflows/release.yml` (job `android`) builds both
with the NDK, copies SDL's Java glue from the SDL source tag, and runs `gradle assembleDebug`.

Copy your Quake data so that `id1/pak0.pak` ends up in
`/sdcard/Android/data/org.quakesourceport.app/files/id1/` (USB file transfer or
`adb push id1 /sdcard/Android/data/org.quakesourceport.app/files/`). Run the app once
first so Android creates the folder. Untested on real hardware.
