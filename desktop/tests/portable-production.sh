#!/usr/bin/env bash
set -euo pipefail
runtime="$(mktemp -d)"
export XDG_CONFIG_HOME="$runtime"
"$1" > output/desktop-tests/portable-production.log 2>&1 &
pid=$!
# Invoked by the EXIT trap.
# shellcheck disable=SC2329
cleanup() {
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  rm -rf "$runtime"
}
trap cleanup EXIT
for _ in {1..100}; do
  kill -0 "$pid" || { cat output/desktop-tests/portable-production.log; exit 1; }
  if xwininfo -root -tree | rg 'rdpctl.*Connections'; then exit 0; fi
  sleep 0.1
done
cat output/desktop-tests/portable-production.log
exit 1
