#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
fixture="$(mktemp -d)"
fixture="$(cd "$fixture" && pwd -P)"
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/bundle/bin" "$fixture/bundle/lib"
cp "$root/scripts/freerdp-wrapper.sh" "$fixture/bundle/freerdp"
cat >"$fixture/bundle/bin/rdp-render-probe" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${PROBE_RESULT:-wayland|opengl}"
exit "${PROBE_STATUS:-0}"
EOF
cat >"$fixture/bundle/bin/sdl-freerdp" <<'EOF'
#!/usr/bin/env bash
printf '%s|%s|%s|%s\n' "${SDL_VIDEODRIVER:-auto}" "$LD_LIBRARY_PATH" "$1" "$OPENSSL_MODULES"
EOF
chmod +x "$fixture/bundle/freerdp" "$fixture/bundle/bin/"*
check() {
  local expected="$1" result
  shift
  result="$(env -u SDL_VIDEO_DRIVER -u SDL_VIDEODRIVER -u WAYLAND_DISPLAY -u DISPLAY -u LD_LIBRARY_PATH -u OPENSSL_MODULES "$@" "$fixture/bundle/freerdp" '/v:example.invalid')"
  [[ "$result" == "$expected|$fixture/bundle/lib|/v:example.invalid|$fixture/bundle/lib/ossl-modules" ]] || { echo "Unexpected result: $result" >&2; exit 1; }
}
check wayland WAYLAND_DISPLAY=wayland-0 DISPLAY=:0
check x11 DISPLAY=:0 PROBE_RESULT='x11|opengl'
check x11 WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 PROBE_RESULT='x11|opengl'
check x11 WAYLAND_DISPLAY=wayland-0 SDL_VIDEODRIVER=x11
check wayland
if env -u SDL_VIDEO_DRIVER -u SDL_VIDEODRIVER PROBE_STATUS=1 "$fixture/bundle/freerdp" /list:monitor; then
  echo 'Failed renderer detection must not launch a desktop client.' >&2; exit 1
fi
result="$(env -u SDL_VIDEO_DRIVER -u SDL_VIDEODRIVER PROBE_STATUS=1 "$fixture/bundle/freerdp" +auth-only)"
[[ "$result" == "auto|"* ]]
result="$(env -u SDL_VIDEO_DRIVER -u SDL_VIDEODRIVER PROBE_STATUS=1 RDPCTL_SKIP_RENDER_PROBE=1 "$fixture/bundle/freerdp" /args-from:stdin)"
[[ "$result" == "auto|"* ]]
# Package commands execute the wrapper by absolute path instead of symlinking it.
printf '#!/usr/bin/env bash\nexec "%s/bundle/freerdp" "$@"\n' "$fixture" >"$fixture/launcher"
chmod +x "$fixture/launcher"
result="$(env -u LD_LIBRARY_PATH -u OPENSSL_MODULES SDL_VIDEODRIVER=x11 "$fixture/launcher" '/v:example.invalid')"
[[ "$result" == "x11|$fixture/bundle/lib|/v:example.invalid|$fixture/bundle/lib/ossl-modules" ]]
result="$(env -u LD_LIBRARY_PATH SDL_VIDEODRIVER=x11 OPENSSL_MODULES=/operator/modules "$fixture/launcher" '/v:example.invalid')"
[[ "$result" == "x11|$fixture/bundle/lib|/v:example.invalid|/operator/modules" ]]
echo 'Launcher tests passed.'
