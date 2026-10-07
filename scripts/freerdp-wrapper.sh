#!/usr/bin/env bash
set -euo pipefail

base="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
export LD_LIBRARY_PATH="$base/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
# Prefer the native session backend while retaining your explicit override.
if [[ -z "${SDL_VIDEODRIVER:-}" ]]; then
  if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
    export SDL_VIDEODRIVER=wayland
  elif [[ -n "${DISPLAY:-}" ]]; then
    export SDL_VIDEODRIVER=x11
  fi
fi
export XKB_CONFIG_ROOT="${XKB_CONFIG_ROOT:-/usr/share/X11/xkb}"
for plugin_dir in "$base"/lib/freerdp*; do
  if [[ -d "$plugin_dir" ]]; then
    export FREERDP_PLUGIN_PATH="$plugin_dir${FREERDP_PLUGIN_PATH:+:$FREERDP_PLUGIN_PATH}"
    break
  fi
done
for candidate in "$base/bin/sdl-freerdp" "$base/bin/sdl-freerdp3"; do
  [[ -x "$candidate" ]] && exec "$candidate" "$@"
done
echo 'No FreeRDP SDL3 client exists in this bundle.' >&2
exit 127
