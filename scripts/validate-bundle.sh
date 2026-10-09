#!/usr/bin/env bash
set -euo pipefail
bundle="${1:?Bundle directory required}"
export LD_LIBRARY_PATH="$bundle/lib"
export OPENSSL_MODULES="$bundle/lib/ossl-modules"
[[ -f "$OPENSSL_MODULES/legacy.so" ]] || { echo 'Bundled OpenSSL legacy provider missing.' >&2; exit 1; }
# Exercise dlopen and MD4, not just ELF linkage; MD4 is required by NTLM.
openssl list -providers -provider default -provider legacy
printf '' | openssl dgst -provider default -provider legacy -md4
"$bundle/freerdp" /version
"$bundle/freerdp" /buildconfig
"$bundle/bin/rdpctl" --help >/dev/null
"$bundle/freerdp" /help >"$bundle/.help.txt" 2>&1 || true
rg -qi '/(multimon|monitors|list:monitor)' "$bundle/.help.txt"
rm "$bundle/.help.txt"
while IFS= read -r -d '' elf; do
  file -b "$elf" | rg -q '^ELF' || continue
  file -b "$elf" | rg -q 'x86-64' || { echo "Unexpected architecture: $elf" >&2; exit 1; }
  links="$(ldd "$elf" 2>&1)" || { [[ "$links" == *'not a dynamic executable'* || "$links" == *'statically linked'* ]] && continue; echo "$links" >&2; exit 1; }
  [[ "$links" != *'not found'* ]] || { echo "$links" >&2; exit 1; }
done < <(find "$bundle/bin" "$bundle/lib" -type f -print0)
if find "$bundle" -type f \( -iname '*.pem' -o -iname '*.key' -o -iname '*.args' \) -print -quit | rg -q .; then
  echo 'Unexpected certificate, key, or connection arguments in bundle.' >&2
  exit 1
fi
