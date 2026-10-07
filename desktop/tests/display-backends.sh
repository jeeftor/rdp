#!/usr/bin/env bash
set -euo pipefail
app="${1:?Pass the built rdpctl-gui binary}"
mkdir -p output/desktop-tests
xvfb-run -a env -u WAYLAND_DISPLAY GDK_BACKEND=x11 python3 desktop/tests/gui_smoke.py "$app" output/desktop-tests/x11.png

runtime="$(mktemp -d)"
weston_pid=''
cleanup() {
  if [[ -n "$weston_pid" ]]; then kill "$weston_pid" 2>/dev/null || true; wait "$weston_pid" 2>/dev/null || true; fi
  rm -rf "$runtime"
}
trap cleanup EXIT
chmod 700 "$runtime"
XDG_RUNTIME_DIR="$runtime" weston --backend=headless-backend.so --use-pixman --socket=rdpctl-test --idle-time=0 > output/desktop-tests/weston.log 2>&1 &
weston_pid=$!
for _ in {1..100}; do
  [[ -S "$runtime/rdpctl-test" ]] && break
  kill -0 "$weston_pid"
  sleep 0.1
done
test -S "$runtime/rdpctl-test"
env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=rdpctl-test GDK_BACKEND=wayland python3 desktop/tests/gui_smoke.py "$app" output/desktop-tests/wayland.png
