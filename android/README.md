# Android build

The game is built as a `cdylib` (`libmain.so`) that SDL's `SDLActivity` loads
together with `libSDL3.so`. `.github/workflows/release.yml` (job `android`) builds
both with the NDK and stages them alongside that NDK's `libc++_shared.so` in
`app/src/main/jniLibs/arm64-v8a/`. The job copies SDL's Java glue from the SDL
source tag, builds and signs the APK, and checks all three libraries are in it.

Android uses the wgpu renderer. It tries Vulkan first and falls back to
OpenGL ES 3.1 if Vulkan cannot initialize. The OpenGL ES path skips
MSAA, screen-space ambient occlusion, per-object motion blur, and soft particle fading.
The game activity shows an error if the game data or graphics cannot initialize.
Gameplay opens in immersive landscape fullscreen. Swipe in from an edge to
briefly show the Android system bars. Use **Options → Graphics → Render Scale**
to adjust the internal rendering resolution.

## Release signing

Android installs an update only over an APK signed with the same key, and
uninstalling deletes the player's saves. The release job signs with the keystore
in the repository secrets `ANDROID_KEYSTORE_B64` (the keystore file, base64),
`ANDROID_KEYSTORE_PASSWORD` (for the store and the key) and `ANDROID_KEY_ALIAS`:

```sh
keytool -genkeypair -keystore qsp-release.keystore -alias qsp -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 qsp-release.keystore | gh secret set ANDROID_KEYSTORE_B64
gh secret set ANDROID_KEYSTORE_PASSWORD
gh secret set ANDROID_KEY_ALIAS --body qsp
```

Keep the keystore and its password backed up, and never commit them: without
them no update can ever be signed. Without the secret the job warns and builds a
debug APK with a throwaway key, as v0.2.3 and earlier did. Players of those
versions have to uninstall once (copy the saves out of
`/sdcard/Android/data/org.quakesourceport.app/files/` first).

## Select your Quake PAKs

1. Put your Quake files in a folder such as `Documents/Quake` on your phone.
2. Open the app and tap **Choose Quake folder**. Choose either the folder with
   `id1/pak0.pak` or the folder containing `pak0.pak` directly.
3. Tap **Use this folder** in Android's picker, then **Play** after the import.

The app copies numbered PAKs (`pak0.pak`, `pak1.pak`, etc.) into app storage.
It uses that copy on future launches. Choose the folder again to replace the
import or pick up changes. The import covers the base-game PAKs. Loose files,
music folders, and sibling mod directories stay where they are.

For a manual install, open the app once to create its storage folder, then put
the Quake `id1/` directory under `/sdcard/Android/data/org.quakesourceport.app/files/`
over USB or with `adb push id1 /sdcard/Android/data/org.quakesourceport.app/files/`.
The launcher uses this location when there is no imported copy.
