#!/usr/bin/env bash
# Shared helpers for the portable FreeRDP diagnostics scripts.
set -euo pipefail

log_root="${RDP_DIAGNOSTICS_LOG_DIR:-$PWD/rdp-diagnostics-logs}"

note() {
  printf '%s\n' "$*"
}

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 2
}

ensure_log_root() {
  mkdir -p "$log_root"
}

timestamp() {
  date -u +%Y%m%dT%H%M%SZ
}

new_log() {
  ensure_log_root
  printf '%s/%s-%s.log\n' "$log_root" "$1" "$(timestamp)"
}

run_logged() {
  local log_file="$1"
  shift
  note "==> $*"
  set +e
  "$@" >"$log_file" 2>&1
  local status=$?
  set -e
  cat "$log_file"
  note "Exit status: $status"
  note "Log: $log_file"
  return "$status"
}

bundle_client() {
  local bundle="${1:?bundle directory required}"
  local client="$bundle/freerdp"
  [[ -x "$client" ]] || die "FreeRDP wrapper not executable: $client"
  printf '%s\n' "$client"
}

usage_bundle() {
  cat <<'EOF'
Usage: SCRIPT BUNDLE_DIRECTORY

Example:
  bash SCRIPT $HOME/freerdp-portable-x86_64-3.30.0
EOF
}
