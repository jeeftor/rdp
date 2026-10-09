#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck disable=SC1091
source "$root/config/freerdp-version.env"
bundle="$root/.build/freerdp-portable-x86_64-$FREERDP_VERSION"
export LD_LIBRARY_PATH="$bundle/lib"
check_monitors() {
  local output status=0
  output="$("$@" "$bundle/freerdp" /list:monitor 2>&1)" || status=$?
  printf '%s\n' "$output"
  # FreeRDP uses 255 for this successful informational command.
  [[ "$status" -eq 0 || "$status" -eq 255 ]]
  rg -q '^listing [1-9][0-9]* monitors:' <<<"$output"
  rg -q '\[[0-9]+\].*[0-9]+x[0-9]+' <<<"$output"
}
xvfb-run -a env -u WAYLAND_DISPLAY SDL_VIDEODRIVER=x11 "$root/.build/sdl-backends" x11
xvfb-run -a env -u WAYLAND_DISPLAY SDL_VIDEODRIVER=x11 SDL_RENDER_DRIVER=software SDL_FRAMEBUFFER_ACCELERATION=0 "$root/.build/sdl-backends" x11 software
set -x
check_monitors xvfb-run -a env -u WAYLAND_DISPLAY SDL_VIDEODRIVER=x11
# A stale/unusable Wayland socket must fall back to X11/OpenGL, never software.
selection="$(xvfb-run -a env -u SDL_VIDEO_DRIVER -u SDL_VIDEODRIVER -u SDL_RENDER_DRIVER WAYLAND_DISPLAY=rdp-missing "$bundle/bin/rdp-render-probe")"
printf 'Automatic graphics selection: <%s>\n' "$selection"
[[ "$selection" == 'x11|opengl' || "$selection" == 'x11|opengles2' ]]
set +x
export XDG_RUNTIME_DIR
XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
weston_pid=''
cleanup() {
  if [[ -n "$weston_pid" ]]; then kill "$weston_pid" 2>/dev/null || true; wait "$weston_pid" 2>/dev/null || true; fi
  rm -rf -- "$XDG_RUNTIME_DIR"
}
trap cleanup EXIT
weston --backend=headless-backend.so --use-pixman --socket=rdp-test --idle-time=0 --log="$XDG_RUNTIME_DIR/weston.log" &
weston_pid=$!
for _ in {1..100}; do
  [[ ! -S "$XDG_RUNTIME_DIR/rdp-test" ]] || break
  kill -0 "$weston_pid" 2>/dev/null || { cat "$XDG_RUNTIME_DIR/weston.log"; exit 1; }
  sleep 0.1
done
[[ -S "$XDG_RUNTIME_DIR/rdp-test" ]] || { cat "$XDG_RUNTIME_DIR/weston.log"; exit 1; }
env -u DISPLAY WAYLAND_DISPLAY=rdp-test SDL_VIDEODRIVER=wayland "$root/.build/sdl-backends" wayland
check_monitors env -u DISPLAY WAYLAND_DISPLAY=rdp-test SDL_VIDEODRIVER=wayland
