#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck disable=SC1091
source "$root/config/freerdp-version.env"
work="$root/.build"
prefix="$work/prefix"
name="freerdp-portable-x86_64-${FREERDP_VERSION}"
bundle="$work/$name"
output="$root/output"
[[ ! -e "$bundle" && ! -e "$output" ]] || { echo 'Move existing build artifacts before packaging again.' >&2; exit 2; }
mkdir -p "$bundle/bin" "$bundle/lib" "$bundle/LICENSES" "$output" "$work/launchers"
client="$(find "$prefix/bin" -maxdepth 1 -type f \( -name sdl-freerdp -o -name sdl-freerdp3 \) -print -quit)"
[[ -n "$client" ]] || { echo 'FreeRDP SDL3 client missing.' >&2; exit 1; }
cp "$client" "$prefix/bin/rdpctl" "$bundle/bin/"
cp -a "$prefix/lib/." "$bundle/lib/"
rm -rf "$bundle/lib/cmake" "$bundle/lib/pkgconfig"
find "$bundle/lib" -type f -name '*.a' -delete
cp "$root/scripts/freerdp-wrapper.sh" "$bundle/freerdp"
chmod 0755 "$bundle/freerdp"
cp "$root/LICENSE" "$bundle/LICENSES/rdp-APACHE-2.0.txt"
cp "$work/source/freerdp/LICENSE" "$bundle/LICENSES/FreeRDP-APACHE-2.0.txt"
cp "$work/source/SDL3/LICENSE.txt" "$bundle/LICENSES/SDL3-zlib.txt"
cp "$work/source/SDL3_ttf/LICENSE.txt" "$bundle/LICENSES/SDL3_ttf-zlib.txt"
mkdir -p "$bundle/LICENSES/go" "$work/go-sources"
cp "$(go env GOROOT)/LICENSE" "$bundle/LICENSES/go/Go-BSD-3-Clause.txt"
# Compiled Go dependencies retain their own notices and source archives.
go -C "$root" mod download
while read -r module version directory; do
  [[ -n "$module" ]] || continue
  destination="$bundle/LICENSES/go/${module//\//_}@$version"
  mkdir -p "$destination"
  while IFS= read -r -d '' notice; do cp "$notice" "$destination/"; done < <(find "$directory" -maxdepth 1 -type f \( -iname 'license*' -o -iname 'copying*' -o -iname 'notice*' \) -print0)
  archive="$(go -C "$root" mod download -json "$module@$version" | jq -r .Zip)"
  cp "$archive" "$work/go-sources/${module//\//_}@$version.zip"
done < <(go list -C "$root" -m -f '{{if .Version}}{{.Path}} {{.Version}} {{.Dir}}{{end}}' all)
cp "$root/config/freerdp-version.env" "$bundle/SOURCES.txt"
cp "$root/README.md" "$bundle/README.md"
cmake -LA -N "$work/build/freerdp" >"$bundle/BUILD-CONFIG.txt"
{
  printf 'Client: SDL3 Wayland and X11\nFreeRDP: %s\nCommit: %s\n' "$FREERDP_VERSION" "$(git -C "$root" rev-parse HEAD)"
  cat /etc/os-release
  gcc --version | head -1
  go version
} >"$bundle/BUILD-INFO.txt"
bash "$root/scripts/collect-libs.sh" "$bundle"
bash "$root/scripts/validate-bundle.sh" "$bundle"
(cd "$bundle" && find . -type f -print0 | sort -z | xargs -0 sha256sum) >"$bundle/SHA256SUMS"
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct)}"
tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner -C "$work" -czf "$output/$name.tar.gz" "$name"
tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner -C "$work" -czf "$output/$name-sources.tar.gz" download system-sources go-sources
for command in freerdp rdpctl; do
  target="$command"
  [[ "$command" != rdpctl ]] || target=bin/rdpctl
  printf '#!/usr/bin/env bash\nexec /opt/freerdp-portable/%s "$@"\n' "$target" >"$work/launchers/$command"
  chmod 0755 "$work/launchers/$command"
done
export BUNDLE_DIR="$bundle" LAUNCHER_DIR="$work/launchers" PACKAGE_VERSION="${PACKAGE_VERSION:-$FREERDP_VERSION}"
# nFPM expands version fields but not source paths; render only these variables.
# shellcheck disable=SC2016
envsubst '${BUNDLE_DIR} ${LAUNCHER_DIR} ${PACKAGE_VERSION}' <"$root/packaging/nfpm.yaml" >"$work/nfpm.yaml"
nfpm package --config "$work/nfpm.yaml" --packager rpm --target "$output/"
nfpm package --config "$work/nfpm.yaml" --packager deb --target "$output/"
(cd "$output" && sha256sum ./*.tar.gz ./*.rpm ./*.deb > SHA256SUMS)
