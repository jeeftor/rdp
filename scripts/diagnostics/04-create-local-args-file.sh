#!/usr/bin/env bash
# Compatibility entry point. Use configure.sh for the repeatable workflow.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
exec bash "$script_dir/configure.sh" "$@"
