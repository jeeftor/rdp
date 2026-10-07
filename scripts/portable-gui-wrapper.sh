#!/usr/bin/env bash
set -euo pipefail
base="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
export APPDIR="$base/app"
export FONTCONFIG_FILE="$APPDIR/usr/share/rdpctl/fonts.conf"
exec "$APPDIR/AppRun" "$@"
