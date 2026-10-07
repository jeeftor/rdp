#!/usr/bin/env bash
# Capture AMD/Mesa/Wayland evidence without changing graphics drivers or settings.
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

[[ $# -eq 1 ]] || { usage_bundle; exit 2; }
client="$(bundle_client "$1")"
log_file="$(new_log gpu-backend)"

{
  printf 'Session type: %s\n' "${XDG_SESSION_TYPE:-unset}"
  printf 'Wayland display: %s\n\n' "${WAYLAND_DISPLAY:-unset}"
  printf 'GPU driver:\n'; lspci -nnk 2>/dev/null | sed -n '/VGA\|3D\|Display/,+3p' || true
  printf '\nDRM nodes:\n'; ls -l /dev/dri 2>/dev/null || true
  printf '\nEGL renderer:\n'
  if command -v eglinfo >/dev/null 2>&1; then eglinfo -B; else printf 'eglinfo unavailable (install mesa-demos only if approved).\n'; fi
  printf '\nVulkan summary:\n'
  if command -v vulkaninfo >/dev/null 2>&1; then vulkaninfo --summary; else printf 'vulkaninfo unavailable (install vulkan-tools only if approved).\n'; fi
  printf '\nFreeRDP Wayland monitor probe:\n'; "$client" /list:monitor
} >"$log_file" 2>&1
cat "$log_file"
note "Log: $log_file"
