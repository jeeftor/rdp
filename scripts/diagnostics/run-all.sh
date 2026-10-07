#!/usr/bin/env bash
# Run every diagnostic stage and continue after a failing stage for evidence.
set -u -o pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
bundle="${1:?Supply the bundle directory}"
host="${2:-${RDP_HOST:?Supply the Windows host or set RDP_HOST}}"
args_file="${3:-$HOME/.config/freerdp/rdp-test.args}"
monitor_ids="${4:-${RDP_MONITORS:?Supply monitor IDs or set RDP_MONITORS}}"
failures=0

run_step() {
  local label="$1"
  shift
  printf '\n===== %s =====\n' "$label"
  if bash "$@"; then
    printf 'PASS: %s\n' "$label"
  else
    printf 'FAIL: %s (continuing)\n' "$label" >&2
    failures=$((failures + 1))
  fi
}

run_step '01 preflight' "$script_dir/01-preflight.sh" "$bundle"
run_step '02 network' "$script_dir/02-network.sh" "$host"
run_step '03 monitors' "$script_dir/03-monitors.sh" "$bundle"
run_step '07 GPU backend facts' "$script_dir/07-gpu-backend.sh" "$bundle"

if [[ -r "$args_file" ]]; then
  run_step '08 redacted profile audit' "$script_dir/08-redacted-profile-audit.sh" "$args_file"
  run_step '05 minimal connection' "$script_dir/05-connect-minimal.sh" "$bundle" "$args_file"
  run_step '06 multi-monitor connection' "$script_dir/06-multimon-connect.sh" "$bundle" "$args_file" "$monitor_ids"
else
  printf '\nSKIP: connection tests; configure first with: bash %s/configure.sh\n' "$script_dir" >&2
  failures=$((failures + 1))
fi

printf '\n===== SUMMARY =====\n'
printf 'Completed with %d failed or skipped stage(s). Logs are in ./rdp-diagnostics-logs/.\n' "$failures"
exit "$failures"
