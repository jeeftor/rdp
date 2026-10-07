#!/usr/bin/env bash
# Capture client, session, and bundle facts before attempting a connection.
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

[[ $# -eq 1 ]] || { usage_bundle; exit 2; }
client="$(bundle_client "$1")"
log_file="$(new_log preflight)"

{
  printf 'UTC: '; date -u --iso-8601=seconds
  printf '\nOS:\n'; cat /etc/os-release 2>/dev/null || true
  printf '\nKernel: '; uname -a
  printf '\nSession type: %s\n' "${XDG_SESSION_TYPE:-unset}"
  printf 'Wayland display: %s\n' "${WAYLAND_DISPLAY:-unset}"
  printf 'X display: %s\n' "${DISPLAY:-unset}"
  printf 'Bundle: %s\n\n' "$1"
  "$client" /version
  "$client" /buildconfig
} >"$log_file" 2>&1
cat "$log_file"
note "Log: $log_file"
