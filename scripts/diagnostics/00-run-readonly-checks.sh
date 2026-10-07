#!/usr/bin/env bash
# Run safe local diagnostics; this script never opens an RDP session.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"

if [[ $# -ne 2 ]]; then
  cat <<EOF
Usage: bash $0 BUNDLE_DIRECTORY WINDOWS_HOST_OR_IP
EOF
  exit 2
fi

bash "$script_dir/01-preflight.sh" "$1"
bash "$script_dir/02-network.sh" "$2"
bash "$script_dir/03-monitors.sh" "$1"
bash "$script_dir/07-gpu-backend.sh" "$1"
