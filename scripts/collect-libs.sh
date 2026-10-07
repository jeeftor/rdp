#!/usr/bin/env bash
set -euo pipefail
bundle="${1:?Bundle directory required}"
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
export LD_LIBRARY_PATH="$bundle/lib:$root/.build/prefix/lib"
mkdir -p "$bundle/LICENSES/system" "$root/.build/system-sources"
: >"$bundle/DEPENDENCIES.txt"
: >"$bundle/LICENSES/SYSTEM-PACKAGES.txt"
declare -A copied=() sources=()

copy_library() {
  local path="$1" real package source_name source_version
  real="$(readlink -f "$path")"
  [[ -n "${copied[$real]:-}" ]] && return
  copied[$real]=1
  case "$(basename "$real")" in
    ld-linux*|libc.so.*|libm.so.*|libdl.so.*|libpthread.so.*|librt.so.*|libresolv.so.*) return ;;
  esac
  case "$real" in "$bundle"/*) return ;; esac
  cp -pL "$real" "$bundle/lib/$(basename "$real")"
  if [[ "$(basename "$path")" != "$(basename "$real")" ]]; then
    ln -sfn "$(basename "$real")" "$bundle/lib/$(basename "$path")"
  fi
  case "$real" in "$root/.build/prefix"/*) return ;; esac
  # dpkg records may use /lib even when the resolved path is under /usr/lib.
  package="$(dpkg-query -S "$real" 2>/dev/null | head -1 | sed 's/: \/.*//')" || true
  if [[ -z "$package" && "$real" == /usr/lib/* ]]; then
    package="$(dpkg-query -S "${real#/usr}" 2>/dev/null | head -1 | sed 's/: \/.*//')" || true
  fi
  [[ -n "$package" ]] || { echo "No package provenance for $real" >&2; exit 1; }
  cp -L "/usr/share/doc/${package%%:*}/copyright" "$bundle/LICENSES/system/${package//:/_}.copyright"
  dpkg-query -W -f='${binary:Package} ${Version} ${source:Package} ${source:Version}\n' "$package" >>"$bundle/LICENSES/SYSTEM-PACKAGES.txt"
  read -r source_name source_version < <(dpkg-query -W -f='${source:Package} ${source:Version}\n' "$package")
  sources["$source_name=$source_version"]=1
}

# Revisit newly copied ELF dependencies until the closure stops growing.
previous=-1
while [[ "$previous" -ne "${#copied[@]}" ]]; do
  previous="${#copied[@]}"
  while IFS= read -r -d '' elf; do
    file -b "$elf" | rg -q '^ELF' || continue
    links="$(ldd "$elf" 2>&1)" || { [[ "$links" == *'not a dynamic executable'* || "$links" == *'statically linked'* ]] && continue; echo "$links" >&2; exit 1; }
    [[ "$links" != *'not found'* ]] || { echo "$links" >&2; exit 1; }
    printf 'ELF: %s\n%s\n' "${elf#"$bundle/"}" "$links" >>"$bundle/DEPENDENCIES.txt"
    while read -r path; do copy_library "$path"; done < <(awk '/=> \// {print $3}' <<<"$links")
  done < <(find "$bundle/bin" "$bundle/lib" -type f -print0)
done
sort -u -o "$bundle/LICENSES/SYSTEM-PACKAGES.txt" "$bundle/LICENSES/SYSTEM-PACKAGES.txt"
# Ship matching source archives alongside redistributed system libraries.
for source_package in "${!sources[@]}"; do
  (cd "$root/.build/system-sources" && apt-get -o Acquire::Retries=2 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 source --download-only "$source_package")
done
