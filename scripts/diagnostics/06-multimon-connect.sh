#!/usr/bin/env bash
# Open one RDP session across the explicitly supplied local FreeRDP monitor IDs.
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

[[ $# -eq 3 ]] || die "Usage: bash $0 BUNDLE_DIRECTORY LOCAL_ARGS_FILE MONITOR_IDS"
client="$(bundle_client "$1")"
args_file="$2"
monitor_ids="$3"
[[ "$monitor_ids" =~ ^[0-9]+(,[0-9]+)+$ ]] || die "Monitor IDs must look like 0,1"
[[ -r "$args_file" ]] || die "Local args file not readable: $args_file"
case "$args_file" in
  /Volumes/*) die "Refusing an args file on removable media; keep it in your home directory." ;;
esac

log_file="$(new_log connect-multimon)"
umask 077
multi_args_file="$(mktemp "${args_file}.multimon.XXXXXX")"
trap 'rm -f -- "$multi_args_file"' EXIT
cat "$args_file" >"$multi_args_file"
{
  printf '/multimon\n'
  printf '/monitors:%s\n' "$monitor_ids"
  printf '/f\n'
} >>"$multi_args_file"
chmod 600 "$multi_args_file"
run_logged "$log_file" "$client" "/args-from:file:$multi_args_file"
