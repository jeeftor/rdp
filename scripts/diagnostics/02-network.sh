#!/usr/bin/env bash
# Test only one user-supplied host and RDP TCP port; it never scans a subnet.
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

[[ $# -ge 1 && $# -le 2 ]] || die "Usage: bash $0 WINDOWS_HOST_OR_IP [PORT]"
host="$1"
port="${2:-3389}"
log_file="$(new_log network)"

{
  printf 'Target: %s:%s\n\n' "$host" "$port"
  printf 'Route:\n'; ip route get "$host" || true
  printf '\nDNS:\n'; getent ahosts "$host" || true
  printf '\nICMP (failure can be normal when ICMP is blocked):\n'; ping -c 2 -W 2 "$host" || true
  printf '\nRDP TCP port:\n'
  if command -v nc >/dev/null 2>&1; then
    nc -vz -w 4 "$host" "$port" || true
  elif command -v nmap >/dev/null 2>&1; then
    nmap -Pn -p "$port" --reason "$host" || true
  else
    printf 'Neither nc nor nmap is installed; port test skipped.\n'
  fi
} >"$log_file" 2>&1
cat "$log_file"
note "Log: $log_file"
