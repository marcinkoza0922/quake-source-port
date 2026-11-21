#!/usr/bin/env bash
# Android (arm64-v8a) build of the game library, as the release workflow's
# android job does it.
#   tools/android_build.sh         release build; stages libmain.so, libSDL3.so
#                                  and the NDK's libc++_shared.so in
#                                  android/app/src/main/jniLibs/arm64-v8a/
#                                  (gradle assembleDebug packages them)
#   tools/android_build.sh check   compile check only (cargo check for Android)
#   tools/android_build.sh apk     the build, then SDL's Java glue and gradle
#                                  assembleDebug: android/app/build/outputs/apk/debug/app-debug.apk
# Needs cargo-ndk (cargo install cargo-ndk --locked), cmake, ninja and an NDK:
# ANDROID_NDK_HOME, ANDROID_NDK_ROOT, or the newest under
# $ANDROID_HOME/ndk (default ~/Android/Sdk/ndk). `apk` also needs Gradle 8.9
# ($GRADLE, else the newest ~/Android/gradle/gradle-*/bin/gradle, else gradle
# on PATH) running on JDK 17 or 21 ($JAVA_HOME, else found in /usr/lib/jvm),
# and network access the first time (SDL's sources, gradle's Android plugin and
# the SDK platform it installs into $ANDROID_HOME).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [ -z "$ndk" ]; then
  ndk="$(ls -d "$sdk"/ndk/*/ 2>/dev/null | sort -V | tail -n 1)"
  ndk="${ndk%/}"
fi
if [ -z "$ndk" ] || [ ! -d "$ndk" ]; then
  echo "no Android NDK: set ANDROID_NDK_HOME, or install one with sdkmanager \"ndk;<version>\"" >&2
  exit 1
fi
if ! command -v cargo-ndk >/dev/null; then
  echo "cargo-ndk is missing: cargo install cargo-ndk --locked" >&2
  exit 1
fi
export ANDROID_NDK_HOME="$ndk"
# Without the NDK toolchain file cmake-rs passes CMAKE_SYSTEM_PROCESSOR=arm64,
# which CMake's Android support rejects (SDL3 is built from source).
export CMAKE_TOOLCHAIN_FILE_aarch64_linux_android="$ndk/build/cmake/android.toolchain.cmake"
export CMAKE_GENERATOR="${CMAKE_GENERATOR:-Ninja}"
ndk_args=(ndk -t arm64-v8a --platform 24)

mode="${1:-build}"
case "$mode" in
  check)
    cargo "${ndk_args[@]}" check --locked -p quake-source-port --lib --features bundled-sdl-shared
    exit
    ;;
  build | apk) ;;
  *)
    echo "usage: $0 [build|check|apk]" >&2
    exit 2
    ;;
esac

cargo "${ndk_args[@]}" rustc --release --locked -p quake-source-port --lib \
  --features bundled-sdl-shared --crate-type cdylib
rel="${CARGO_TARGET_DIR:-$root/target}/aarch64-linux-android/release"
libs="$root/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$libs"
cp "$rel/libquake_source_port.so" "$libs/libmain.so"
cp "$(find "$rel/build" -name libSDL3.so | head -n 1)" "$libs/"
# libmain.so links the NDK's shared C++ runtime; gradle only packages staged libraries
host="$(ls "$ndk/toolchains/llvm/prebuilt" | head -n 1)"
cp "$ndk/toolchains/llvm/prebuilt/$host/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$libs/"
echo "android libraries staged in $libs"
[ "$mode" = apk ] || exit 0

# Gradle 8.9 doesn't run on Java newer than 22: prefer a JDK 17 or 21
if [ -z "${JAVA_HOME:-}" ]; then
  for j in /usr/lib/jvm/temurin-17-jdk /usr/lib/jvm/java-17* /usr/lib/jvm/java-21*; do
    if [ -x "$j/bin/java" ]; then
      export JAVA_HOME="$j"
      break
    fi
  done
fi
gradle="${GRADLE:-$(ls -d "$HOME"/Android/gradle/gradle-*/bin/gradle 2>/dev/null | sort -V | tail -n 1)}"
gradle="${gradle:-$(command -v gradle || true)}"
if [ -z "$gradle" ]; then
  echo "no gradle: unpack Gradle 8.9 into ~/Android/gradle, or set GRADLE" >&2
  exit 1
fi
# SDL's Java glue (SDLActivity and friends), from the tag the release workflow uses
glue="$root/android/app/src/main/java/org/libsdl/app"
if [ ! -f "$glue/SDLActivity.java" ]; then
  tag="$(grep -o 'release-3[.0-9]*' .github/workflows/release.yml | head -n 1)"
  src="${CARGO_TARGET_DIR:-$root/target}/android/SDL-$tag"
  if [ ! -d "$src" ]; then
    git clone --quiet --depth 1 --branch "$tag" https://github.com/libsdl-org/SDL "$src"
  fi
  mkdir -p "$(dirname "$glue")"
  cp -r "$src/android-project/app/src/main/java/org/libsdl/app" "$(dirname "$glue")/"
fi
export ANDROID_HOME="$sdk"
"$gradle" --quiet -p android assembleDebug
apk="$root/android/app/build/outputs/apk/debug/app-debug.apk"
for lib in libmain.so libSDL3.so libc++_shared.so; do
  unzip -tq "$apk" "lib/arm64-v8a/$lib" >/dev/null
done
echo "apk: $apk"
