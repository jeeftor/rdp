#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck disable=SC1091
source "$root/config/freerdp-version.env"
[[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || { echo 'Build on Linux x86_64 (Ubuntu 24.04).' >&2; exit 2; }
work="$root/.build"
prefix="$work/prefix"
mkdir -p "$work/download" "$work/source" "$work/build" "$prefix"
download_extract() {
  local label="$1" url="$2" expected="$3" destination="$4" archive
  [[ "$expected" =~ ^[[:xdigit:]]{64}$ ]] || { echo "Invalid source checksum for $label" >&2; exit 2; }
  archive="$work/download/$label"
  curl --fail --location --proto '=https' --tlsv1.2 --output "$archive" "$url"
  printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status
  mkdir -p "$destination"
  tar -xaf "$archive" --strip-components=1 -C "$destination"
}
download_extract "SDL3-${SDL_VERSION}.tar.gz" "$SDL_SOURCE_URL" "$SDL_SHA256" "$work/source/SDL3"
download_extract "SDL3_ttf-${SDL_TTF_VERSION}.tar.gz" "$SDL_TTF_SOURCE_URL" "$SDL_TTF_SHA256" "$work/source/SDL3_ttf"
download_extract "freerdp-${FREERDP_VERSION}.tar.gz" "$FREERDP_SOURCE_URL" "$FREERDP_SHA256" "$work/source/freerdp"
export CMAKE_PREFIX_PATH="$prefix"
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
cmake -S "$work/source/SDL3" -B "$work/build/SDL3" -GNinja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_INSTALL_LIBDIR=lib \
  -DSDL_SHARED=ON \
  -DSDL_STATIC=OFF \
  -DSDL_TEST_LIBRARY=OFF \
  -DSDL_TESTS=OFF \
  -DSDL_X11=ON \
  -DSDL_DEPS_SHARED=OFF \
  -DSDL_WAYLAND=ON \
  -DSDL_WAYLAND_LIBDECOR=OFF \
  -DSDL_KMSDRM=OFF \
  -DSDL_PIPEWIRE=OFF \
  -DSDL_PULSEAUDIO=OFF
ninja -C "$work/build/SDL3" install

cmake -S "$work/source/SDL3_ttf" -B "$work/build/SDL3_ttf" -GNinja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_INSTALL_LIBDIR=lib \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DSDLTTF_VENDORED=OFF \
  -DSDLTTF_SAMPLES=OFF
ninja -C "$work/build/SDL3_ttf" install

# SDL3 provides both display backends; separate FreeRDP X11/Wayland clients are unnecessary.
# shellcheck disable=SC2016
cmake -S "$work/source/freerdp" -B "$work/build/freerdp" -GNinja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_INSTALL_LIBDIR=lib \
  -DCMAKE_BUILD_WITH_INSTALL_RPATH=ON \
  '-DCMAKE_INSTALL_RPATH=$ORIGIN/../lib;$ORIGIN/..' \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DBUILD_SHARED_LIBS=ON \
  -DBUILD_TESTING=OFF \
  -DWITH_SERVER=OFF \
  -DWITH_SHADOW=OFF \
  -DWITH_PROXY=OFF \
  -DWITH_PLATFORM_SERVER=OFF \
  -DWITH_SAMPLE=OFF \
  -DWITH_WEBVIEW=OFF \
  -DWITH_MANPAGES=OFF \
  -DWITH_X11=OFF \
  -DWITH_WAYLAND=OFF \
  -DWITH_CLIENT_SDL=ON \
  -DWITH_CLIENT_SDL2=OFF \
  -DWITH_CLIENT_SDL3=ON \
  -DWITH_FFMPEG=OFF \
  -DWITH_SWSCALE=OFF \
  -DWITH_OPENH264=OFF \
  -DCHANNEL_URBDRC=OFF \
  -DWITH_FUSE=OFF \
  -DWITH_SMARTCARD=OFF \
  -DWITH_PCSC=OFF \
  -DWITH_SMARTCARD_EMULATE=OFF \
  -DWITH_SMARTCARD_PCSC=OFF \
  -DWITH_PKCS11=OFF \
  -DWITH_WINPR_TOOLS=OFF \
  -DWITH_WINPR_TOOLS_CLI=OFF \
  -DWITH_ALSA=OFF \
  -DWITH_OSS=OFF \
  -DWITH_CUPS=OFF
ninja -C "$work/build/freerdp"
ninja -C "$work/build/freerdp" install

# Require both compiled backends; CMake can otherwise silently omit one.
for backend in X11 WAYLAND; do
  rg -q "^#define SDL_VIDEO_DRIVER_${backend} 1" "$work/build/SDL3/include-config-release/build_config/SDL_build_config.h"
done
CGO_ENABLED=0 go build -C "$root" -trimpath -ldflags='-s -w' -o "$prefix/bin/rdpctl" ./cmd/rdpctl
cc "$root/tests/sdl-backends.c" -o "$work/sdl-backends" -I"$prefix/include" -L"$prefix/lib" -lSDL3
cc -Wall -Wextra -Werror "$root/scripts/render-probe.c" -o "$prefix/bin/rdp-render-probe" -I"$prefix/include" -L"$prefix/lib" -lSDL3
