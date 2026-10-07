#!/usr/bin/env bash
# Show the effective local connection settings without exposing its password.
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

[[ $# -eq 1 ]] || die "Usage: bash $0 LOCAL_ARGS_FILE"
args_file="$1"
[[ -r "$args_file" ]] || die "Local args file not readable: $args_file"
case "$args_file" in
  /Volumes/*) die "Refusing an args file on removable media; keep it in your home directory." ;;
esac

log_file="$(new_log profile-audit)"
{
  printf 'Args file: %s\n' "$args_file"
  printf 'Mode: '; stat -c '%a' "$args_file"
  printf '\nEffective arguments (password redacted):\n'
  while IFS= read -r argument || [[ -n "$argument" ]]; do
    case "$argument" in
      /p:*) printf '/p:[REDACTED] (present)\n' ;;
      *) printf '%s\n' "$argument" ;;
    esac
  done <"$args_file"
} >"$log_file"

cat "$log_file"
printf 'Log: %s\n' "$log_file"
