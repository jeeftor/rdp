#!/usr/bin/env bash
set -euo pipefail

base="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
export LD_LIBRARY_PATH="$base/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
# Providers are loaded dynamically and must match the bundled libcrypto.
export OPENSSL_MODULES="${OPENSSL_MODULES:-$base/lib/ossl-modules}"
# Authentication and informational commands must work without a desktop renderer.
probe=true
for argument in "$@"; do
  case "$argument" in /version|/buildconfig|/help|--help|+auth-only) probe=false ;; esac
done
if [[ "${RDPCTL_SKIP_RENDER_PROBE:-}" != 1 && -z "${SDL_VIDEO_DRIVER:-}${SDL_VIDEODRIVER:-}" ]] && "$probe"; then
  selection="$("$base/bin/rdp-render-probe")" || exit 1
  export SDL_VIDEO_DRIVER="${selection%%|*}"
  export SDL_VIDEODRIVER="$SDL_VIDEO_DRIVER"
  export SDL_RENDER_DRIVER="${selection#*|}"
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
