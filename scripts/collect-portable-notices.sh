#!/usr/bin/env bash
set -euo pipefail
app="${1:?Pass the extracted AppDir}"
mkdir -p "$app/usr/share/licenses/system" .build/portable-system-sources
declare -A packages=() sources=()
# linuxdeploy flattens some library paths, so fall back to their installed names.
while IFS= read -r -d '' file; do
  relative="${file#"$app/"}"
  case "$relative" in usr/share/rdpctl/freerdp/*|usr/share/licenses/*) continue ;; esac
  records="$(dpkg-query -S "/$relative" 2>/dev/null || true)"
  if [[ -z "$records" ]] && { [[ "$relative" == *.typelib ]] || file -Lb "$file" | rg -q '^ELF '; }; then
    records="$(dpkg-query -S "*/$(basename "$file")" 2>/dev/null || true)"
  fi
  while IFS= read -r record; do
    [[ "$record" == *': /'* ]] || continue
    package="${record%%: /*}"
    packages["$package"]=1
  done <<< "$records"
done < <(find "$app/usr" -type f ! -path '*/share/licenses/*' -print0)
: > "$app/usr/share/licenses/system/PACKAGES.txt"
for package in "${!packages[@]}"; do
  copyright="/usr/share/doc/${package%%:*}/copyright"
  [[ -f "$copyright" ]] || { echo "Missing notices: $package" >&2; exit 1; }
  cp -L "$copyright" "$app/usr/share/licenses/system/${package//:/_}.copyright"
  dpkg-query -W -f='${binary:Package} ${Version} ${source:Package} ${source:Version}\n' "$package" >> "$app/usr/share/licenses/system/PACKAGES.txt"
  read -r source_name source_version < <(dpkg-query -W -f='${source:Package} ${source:Version}\n' "$package")
  sources["$source_name=$source_version"]=1
done
sort -u -o "$app/usr/share/licenses/system/PACKAGES.txt" "$app/usr/share/licenses/system/PACKAGES.txt"
[[ "${#sources[@]}" -gt 0 ]]
# A single transaction avoids repeatedly refreshing source metadata.
(cd .build/portable-system-sources && apt-get -o Acquire::Retries=2 -o Acquire::https::Timeout=30 source --download-only "${!sources[@]}")
