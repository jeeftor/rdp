#!/usr/bin/env bash
# Create or replace the protected, local-only FreeRDP connection arguments.
set -euo pipefail

args_file="${1:-$HOME/.config/freerdp/rdp-test.args}"
host="${RDP_HOST:?Set RDP_HOST to your Windows host}"
user="${RDP_USER:?Set RDP_USER to your Windows username}"
security_mode="${RDP_SECURITY_MODE:-nla}"

case "$security_mode" in
  nla|tls) ;;
  *) printf 'ERROR: RDP_SECURITY_MODE must be nla or tls.\n' >&2; exit 2 ;;
esac

case "$args_file" in
  /Volumes/*) printf 'ERROR: keep the password file in your home directory, not /Volumes.\n' >&2; exit 2 ;;
esac

parent="$(dirname -- "$args_file")"
mkdir -p "$parent"
chmod 700 "$parent"

read -r -s -p "RDP password for $user@$host: " rdp_password
printf '\n'
[[ -n "$rdp_password" ]] || { printf 'ERROR: no password entered; configuration unchanged.\n' >&2; exit 2; }
escaped_password="${rdp_password//\\/\\\\}"
escaped_password="${escaped_password//\"/\\\"}"

temporary_file="$args_file.new.$$"
umask 077
{
  printf '/v:%s\n' "$host"
  printf '/u:%s\n' "$user"
  printf '/p:"%s"\n' "$escaped_password"
  printf '/cert:ignore\n'
  printf '/sec:%s\n' "$security_mode"
  printf '/log-level:trace\n'
} >"$temporary_file"
unset rdp_password
unset escaped_password
chmod 600 "$temporary_file"
mv -f -- "$temporary_file" "$args_file"

printf 'Configured %s for %s@%s (mode 600).\n' "$args_file" "$user" "$host"
printf 'Diagnostic mode uses /sec:%s and temporarily ignores the certificate.\n' "$security_mode"
