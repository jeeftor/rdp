#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PACKAGE_VERSION="${PACKAGE_VERSION:-0.1.0-dev}"
export GUI_BINARY="${CARGO_TARGET_DIR:-desktop/target}/release/rdpctl-gui"
export GUI_LICENSES='.build/gui-licenses'
test -f "$GUI_BINARY"
mkdir -p output .build/gui-sources/.cargo "$GUI_LICENSES"
cargo vendor --manifest-path desktop/Cargo.toml --locked .build/gui-sources/vendor > .build/gui-vendor-config.toml
sed 's|directory = ".build/gui-sources/vendor"|directory = "vendor"|' .build/gui-vendor-config.toml > .build/gui-sources/.cargo/config.toml
cp -R desktop/core desktop/src-tauri desktop/ui desktop/patches .build/gui-sources/
rm -rf .build/gui-sources/src-tauri/gen
cp desktop/Cargo.toml desktop/Cargo.lock desktop/rust-toolchain.toml desktop/README.md LICENSE .build/gui-sources/
mkdir -p .build/gui-sources/recipes
cp scripts/capture-window.c scripts/render-probe.c scripts/build-freerdp.sh scripts/install-build-deps.sh .build/gui-sources/recipes/
while IFS= read -r -d '' file; do
  relative="${file#.build/gui-sources/vendor/}"
  mkdir -p "$GUI_LICENSES/$(dirname "$relative")"
  cp "$file" "$GUI_LICENSES/$relative"
done < <(find .build/gui-sources/vendor -type f \( -iname '*license*' -o -iname '*copying*' -o -iname '*notice*' -o -iname '*copyright*' -o -name Cargo.toml \) -print0)
mkdir -p "$GUI_LICENSES/glib-0.18.5-patched"
cp desktop/patches/glib/LICENSE desktop/patches/glib/COPYRIGHT desktop/patches/README.md "$GUI_LICENSES/glib-0.18.5-patched/"
# Preserve licenses for frontend modules embedded in the native GUI.
while IFS= read -r -d '' file; do
  relative="${file#desktop/ui/node_modules/}"
  mkdir -p "$GUI_LICENSES/frontend/$(dirname "$relative")"
  cp "$file" "$GUI_LICENSES/frontend/$relative"
done < <(find desktop/ui/node_modules -type f \( -iname '*license*' -o -iname '*copying*' -o -iname '*notice*' -o -iname '*copyright*' -o -name package.json \) -print0)
# envsubst needs literal variable names, rather than their values.
# shellcheck disable=SC2016
envsubst '${PACKAGE_VERSION} ${GUI_BINARY} ${GUI_LICENSES}' < packaging/gui-nfpm.yaml > .build/gui-nfpm.yaml
nfpm package --config .build/gui-nfpm.yaml --packager deb --target "output/rdpctl-gui_${PACKAGE_VERSION}_amd64.deb"
tar -I pigz -cf "output/rdpctl-gui-${PACKAGE_VERSION}-sources.tar.gz" -C .build gui-sources
(cd output && sha256sum rdpctl-gui*.deb rdpctl-gui*-sources.tar.gz > GUI-SHA256SUMS)
