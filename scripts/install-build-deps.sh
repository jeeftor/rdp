#!/usr/bin/env bash
set -euo pipefail
[[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || { echo 'Use Ubuntu 24.04 x86_64.' >&2; exit 2; }
# Source repositories supply matching sources for bundled system libraries.
# The runner's Azure mirror can stall on individual source-package downloads.
sudo sed -i \
  -e 's/^Types: deb$/Types: deb deb-src/' \
  -e 's|mirror+file:/etc/apt/apt-mirrors.txt|https://archive.ubuntu.com/ubuntu/|g' \
  -e 's|mirror+file:/etc/apt/apt-mirrors-security.txt|https://security.ubuntu.com/ubuntu/|g' \
  /etc/apt/sources.list.d/ubuntu.sources
sudo apt-get -o Acquire::Retries=2 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 update
sudo apt-get -o Acquire::Retries=2 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 install -y --no-install-recommends \
  build-essential cmake ninja-build pkg-config curl ca-certificates \
  file ripgrep bison flex dpkg-dev jq gettext-base \
  openssl libssl-dev zlib1g-dev libjpeg-dev libkrb5-dev libicu-dev \
  libxml2-dev libffi-dev libfreetype-dev libharfbuzz-dev \
  libwayland-dev wayland-protocols libxkbcommon-dev libxkbcommon-x11-dev \
  libegl-dev libgl-dev libgles-dev libdbus-1-dev \
  libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxfixes-dev \
  libxi-dev libxss-dev libxtst-dev libxrender-dev \
  shellcheck xvfb xauth weston
