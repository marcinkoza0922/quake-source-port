#!/bin/sh
# Build a musl release binary inside an Alpine container (see docs/DESIGN.md, "Releases").
# Usage: docker run --rm -v "$PWD:/src" -w /src alpine:3.22 sh tools/release_musl.sh
# The binary links musl dynamically: a fully static binary could not dlopen the Vulkan
# loader or the X11/Wayland/audio libraries SDL loads at run time.
set -eu
apk add --no-cache bash build-base cmake samurai pkgconf curl git linux-headers shaderc \
  libx11-dev libxext-dev libxcursor-dev libxi-dev libxfixes-dev libxrandr-dev \
  libxscrnsaver-dev libxtst-dev libxkbcommon-dev wayland-dev wayland-protocols libdecor-dev \
  alsa-lib-dev pulseaudio-dev eudev-dev dbus-dev libdrm-dev mesa-dev sndio-dev >/dev/null
if ! command -v cargo >/dev/null; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
fi
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTFLAGS="-C target-feature=-crt-static"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/musl}"
cargo build --release --locked -p quake-source-port --features bundled-sdl
