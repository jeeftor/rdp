# Portable FreeRDP

[![Build and release](https://github.com/jeeftor/rdp/actions/workflows/build.yml/badge.svg)](https://github.com/jeeftor/rdp/actions/workflows/build.yml)

A portable Linux x86_64 FreeRDP SDL3 client with **Wayland and X11** support,
plus a password-free `rdpctl` connection manager. GitHub Actions builds directly
on Ubuntu 24.04 and publishes tarball, RPM, and DEB release artifacts. Docker
is not required.

## Download and run

Download an artifact from [Releases](https://github.com/jeeftor/rdp/releases).
Verify it against the accompanying `SHA256SUMS` before installing or extracting.

```bash
sha256sum --ignore-missing --check SHA256SUMS
tar xzf freerdp-portable-x86_64-3.30.0.tar.gz
cd freerdp-portable-x86_64-3.30.0
./freerdp /version
./freerdp /list:monitor
./freerdp /v:windows.example.invalid /u:USERNAME /multimon /monitors:0,1 /f
```

Use the monitor IDs reported by `/list:monitor`. FreeRDP prompts for your
password; do not put it on the command line.

The wrapper selects native Wayland when `WAYLAND_DISPLAY` is set and X11 when
only `DISPLAY` is set. You can choose explicitly:

```bash
SDL_VIDEODRIVER=wayland ./freerdp /list:monitor
SDL_VIDEODRIVER=x11 ./freerdp /list:monitor
```

Both sessions must have a working display server, host graphics drivers, and
keyboard data. The portable archive needs no root access or Internet access
on the target. It requires glibc 2.39 or later. Ubuntu 24.04 is the build and
DEB smoke-test baseline. **RHEL 10 is an intended target; installation, real
RDP connections, and physical multi-monitor behavior require target validation.**
An RPM/DEB extension alone does not establish support for every distribution.

## RPM and DEB installation

```bash
# RHEL 10: install the downloaded RPM (target validation pending).
sudo dnf install ./freerdp-portable*.rpm

# Ubuntu 24.04: install the downloaded DEB.
sudo apt install ./freerdp-portable*.deb

freerdp /version
rdpctl --help
```

Packages install the bundle under `/opt/freerdp-portable` and the `freerdp`
and `rdpctl` commands under `/usr/bin`. Upgrading from `v0.1.0-rc.1` replaces
its `freerdp-portable` command with `freerdp`. They do not replace distribution
FreeRDP libraries. Your profiles
remain in your home directory when you remove the package.

## Saved connections

A Rust/Tauri graphical connection manager is available in
[desktop/](desktop/README.md). It shares these profiles and launches the same
FreeRDP client, with display selection and password prompts through FreeRDP.
Prereleases from `v0.1.0-rc.3` include separate `rdpctl-gui` RPM and DEB packages.
Install the GUI alongside `freerdp-portable`, then open **rdpctl** from your
application menu. See the desktop documentation for runtime dependencies.

Run `./bin/rdpctl` in the archive, or `rdpctl` after package installation, to
manage connections interactively. Profiles contain hosts and usernames but no
passwords. They live in `~/.config/rdpctl/connections`; generated desktop
launchers live in `~/.local/share/applications`.

```bash
rdpctl add --name 'Windows' --host windows.example.invalid --user USERNAME --multimon --monitors 0,1
rdpctl shortcut add windows
rdpctl launch windows
```

Generic troubleshooting helpers are documented in
[scripts/diagnostics/README.md](scripts/diagnostics/README.md). Their logs and
local connection files are private to your machine and excluded from Git.

## Build and release

Build on Ubuntu 24.04 x86_64 with Go matching `go.mod`:

```bash
bash scripts/install-build-deps.sh
GOTOOLCHAIN=auto go install github.com/goreleaser/nfpm/v2/cmd/nfpm@v2.47.0
make check
make build
make package
```

The packager installs with its own required Go toolchain; this does not change
the application's Go requirement in `go.mod`.
The dependency installer uses `sudo`, enables Ubuntu source repositories, and
installs build tools. Compilation runs as your user. Plain `make` prints help.
Source versions and SHA-256 checksums are pinned in
`config/freerdp-version.env`. Generated files stay in `.build/` and `output/`.
Move existing output aside before repeating packaging.

Every push, pull request, and manual workflow run builds and validates the
artifacts. CI checks the packaged SDL library under Xvfb and headless Weston,
runs FreeRDP monitor enumeration on each backend, upgrades/removes the DEB,
and inspects RPM metadata. These checks do not prove actual RDP connectivity
or physical multi-monitor behavior on another workstation.

Pushing a project SemVer tag such as `v0.1.0-rc.1` publishes the validated
client and GUI artifacts to a GitHub prerelease after both builds and their
X11/Wayland checks pass. Project/package versions are independent of
the pinned FreeRDP version in archive names. Only the release job has repository
write permission; build and pull-request jobs have read permission. No private
certificates, proxy settings, or connection profiles are needed in GitHub.

## Licenses and sources

This project's code and scripts use [Apache-2.0](LICENSE), matching FreeRDP.
Bundled dependencies retain their own licenses. Each binary bundle includes
`LICENSES/`, system-library package provenance, source pins, build information,
and per-file checksums. The accompanying `*-sources.tar.gz` contains pinned
upstream archives, Go dependency archives, and matching Ubuntu source packages
for redistributed system libraries. Host glibc and graphics drivers are not
redistributed wholesale.
