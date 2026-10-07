#!/usr/bin/env bash
# Record the monitor IDs that FreeRDP can use with /monitors:ID,ID.
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/lib.sh"

[[ $# -eq 1 ]] || { usage_bundle; exit 2; }
client="$(bundle_client "$1")"
log_file="$(new_log monitors)"

run_logged "$log_file" "$client" /list:monitor || true
note 'Use the IDs reported above, for example: /multimon /monitors:0,1 /f'
