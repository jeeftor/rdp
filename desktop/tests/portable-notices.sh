#!/usr/bin/env bash
set -euo pipefail
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
export NOTICE_FIXTURE="$fixture"
mkdir -p "$fixture/app/usr/bin" "$fixture/app/usr/share/doc/rdpctl" "$fixture/tools"
cp /usr/bin/true "$fixture/app/usr/bin/true"
printf 'Project copyright notice\n' > "$fixture/app/usr/share/doc/rdpctl/copyright"
cat > "$fixture/tools/dpkg-query" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == -S ]]; then
  case "$2" in
    /usr/bin/true) echo 'coreutils: /usr/bin/true'; exit 0 ;;
    '*/copyright') echo 'temurin-17-jdk: /usr/share/doc/temurin-17-jdk/copyright'; exit 0 ;;
    *) exit 1 ;;
  esac
fi
exec /usr/bin/dpkg-query "$@"
MOCK
cat > "$fixture/tools/apt-get" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > "$NOTICE_FIXTURE/source-arguments"
MOCK
chmod 0755 "$fixture/tools/"*
PATH="$fixture/tools:$PATH" bash scripts/collect-portable-notices.sh "$fixture/app"
rg '^coreutils=' "$fixture/source-arguments"
if rg temurin "$fixture/source-arguments"; then exit 1; fi
[[ "$(wc -l < "$fixture/app/usr/share/licenses/system/PACKAGES.txt")" -eq 1 ]]
echo 'Portable notices regression passed: custom documentation does not pull unrelated packages.'
