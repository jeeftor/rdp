#!/usr/bin/env bash
# Open one minimal RDP session from a protected local args file; no monitor options.
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

[[ $# -eq 2 ]] || die "Usage: bash $0 BUNDLE_DIRECTORY LOCAL_ARGS_FILE"
client="$(bundle_client "$1")"
args_file="$2"
[[ -r "$args_file" ]] || die "Local args file not readable: $args_file"
case "$args_file" in
  /Volumes/*) die "Refusing an args file on removable media; keep it in your home directory." ;;
esac

log_file="$(new_log connect-minimal)"
run_logged "$log_file" "$client" "/args-from:file:$args_file"
