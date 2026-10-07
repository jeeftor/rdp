#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"
export PACKAGE_VERSION="${PACKAGE_VERSION:-0.1.0-dev}"
# shellcheck disable=SC1091
source config/freerdp-version.env
client_archive="freerdp-portable-x86_64-${FREERDP_VERSION}.tar.gz"
client_sources="freerdp-portable-x86_64-${FREERDP_VERSION}-sources.tar.gz"
test -f ".build/portable-input/$client_archive"
test -f ".build/portable-input/$client_sources"
(cd .build/portable-input && sha256sum --ignore-missing --check SHA256SUMS)
mkdir -p .build/portable-client .build/portable-sources .build/portable-notices output
cp packaging/AppImageKit-LICENSE .build/portable-notices/
cp LICENSE .build/portable-notices/PROJECT-LICENSE
tar -xzf ".build/portable-input/$client_archive" --strip-components=1 -C .build/portable-client
test -x .build/portable-client/freerdp
python3 - <<'PY'
import json
from pathlib import Path
root = Path.cwd()
config = {"bundle": {"linux": {"appimage": {"files": {
    "/usr/bin/bwrap": "/usr/bin/bwrap",
    "/usr/bin/xdg-dbus-proxy": "/usr/bin/xdg-dbus-proxy",
    "/usr/share/rdpctl/freerdp": str(root / ".build/portable-client"),
    "/usr/share/licenses/rdpctl-gui": str(root / ".build/gui-licenses"),
    "/usr/share/fonts/truetype/dejavu": "/usr/share/fonts/truetype/dejavu",
    "/usr/share/licenses/portable": str(root / ".build/portable-notices"),
    "/usr/share/rdpctl/fonts.conf": str(root / "packaging/portable-fonts.conf"),
}}}}}
Path('.build/portable-tauri.json').write_text(json.dumps(config))
PY
(cd desktop && cargo tauri build --bundles appimage --config "$root/.build/portable-tauri.json" -- --locked)
target="$(realpath "${CARGO_TARGET_DIR:-$root/desktop/target}")/release/bundle/appimage"
images=("$target"/*.AppImage)
[[ "${#images[@]}" -eq 1 && -f "${images[0]}" ]]
mkdir -p .build/portable-extract
(cd .build/portable-extract && "${images[0]}" --appimage-extract > extraction.log)
name="rdpctl-portable-x86_64-${PACKAGE_VERSION}"
export PORTABLE_DIR="$root/.build/$name"
[[ ! -e "$PORTABLE_DIR" ]] || { echo 'Move the previous portable staging directory aside before repackaging.' >&2; exit 2; }
mkdir -p "$PORTABLE_DIR"
mv .build/portable-extract/squashfs-root "$PORTABLE_DIR/app"
cp scripts/portable-gui-wrapper.sh "$PORTABLE_DIR/rdpctl"
chmod 0755 "$PORTABLE_DIR/rdpctl"
cp desktop/README.md LICENSE "$PORTABLE_DIR/"
bash scripts/collect-portable-notices.sh "$PORTABLE_DIR/app"
cp -R "$PORTABLE_DIR/app/usr/share/licenses/system" .build/portable-notices/system
# Rebundle with the notices discovered from the actual deployed runtime.
(cd desktop && cargo tauri build --bundles appimage --config "$root/.build/portable-tauri.json" -- --locked)
rm -rf .build/portable-extract/squashfs-root
(cd .build/portable-extract && "${images[0]}" --appimage-extract > extraction.log)
rm -rf "$PORTABLE_DIR/app"
mv .build/portable-extract/squashfs-root "$PORTABLE_DIR/app"
cp "${images[0]}" "output/$name.AppImage"
cp -R "$PORTABLE_DIR/app/usr/share/licenses" .build/portable-sources/licenses
cp -R .build/portable-system-sources .build/portable-sources/system-sources
cp "output/rdpctl-gui-${PACKAGE_VERSION}-sources.tar.gz" ".build/portable-input/$client_sources" .build/portable-sources/
mkdir -p .build/portable-sources/recipes
cp scripts/package-portable-gui.sh scripts/collect-portable-notices.sh scripts/portable-gui-wrapper.sh packaging/portable-fonts.conf packaging/portable-nfpm.yaml packaging/AppImageKit-LICENSE .build/portable-sources/recipes/
mkdir -p .build/portable-launchers
printf '#!/usr/bin/env bash\nexec /opt/rdpctl-portable/rdpctl "$@"\n' > .build/portable-launchers/rdpctl-gui
printf '#!/usr/bin/env bash\nexec /opt/rdpctl-portable/app/usr/share/rdpctl/freerdp/freerdp "$@"\n' > .build/portable-launchers/freerdp
chmod 0755 .build/portable-launchers/*
# envsubst needs literal variable names.
# shellcheck disable=SC2016
envsubst '${PACKAGE_VERSION} ${PORTABLE_DIR}' < packaging/portable-nfpm.yaml > .build/portable-nfpm.yaml
nfpm package --config .build/portable-nfpm.yaml --packager rpm --target "output/rdpctl-portable-${PACKAGE_VERSION}.x86_64.rpm"
nfpm package --config .build/portable-nfpm.yaml --packager deb --target "output/rdpctl-portable_${PACKAGE_VERSION}_amd64.deb"
tar -czf "output/$name.tar.gz" -C .build "$name"
tar -czf "output/rdpctl-portable-${PACKAGE_VERSION}-sources.tar.gz" -C .build portable-sources
(cd output && sha256sum rdpctl-gui*.rpm rdpctl-gui*.deb rdpctl-gui*-sources.tar.gz rdpctl-portable*.rpm rdpctl-portable*.deb rdpctl-portable*.tar.gz rdpctl-portable*.AppImage > GUI-SHA256SUMS)
