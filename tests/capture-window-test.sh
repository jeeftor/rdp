#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# pkg-config emits separate compiler/linker arguments for system libraries.
# shellcheck disable=SC2046
cc -Wall -Wextra -Werror -DHAVE_XCOMPOSITE "$root/scripts/capture-window.c" -o "$work/capture" $(pkg-config --cflags --libs x11 xcomposite libpng)
# shellcheck disable=SC2046
cc -Wall -Wextra -Werror "$root/tests/capture-window-fixture.c" -o "$work/fixture" $(pkg-config --cflags --libs x11 xcomposite)
xvfb-run -a python3 "$root/tests/capture-window-test.py" "$work/capture" "$work/fixture" "$work"
