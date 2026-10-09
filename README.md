# Portable FreeRDP

[![Build and release](https://github.com/jeeftor/rdp/actions/workflows/build.yml/badge.svg)](https://github.com/jeeftor/rdp/actions/workflows/build.yml)

A portable Linux x86_64 FreeRDP SDL3 client with **Wayland and X11** support,
plus terminal and graphical `rdpctl` connection managers. GitHub Actions builds directly
on Ubuntu 24.04 and publishes one combined RPM, matching sources and checksums. Docker
is not required.

## Download and run

Download the combined `rdp` RPM and `SHA256SUMS` from
[Releases](https://github.com/jeeftor/rdp/releases), then follow the desktop
installation below. Releases from rc.7 publish just one installer. Other package
formats are still built and tested in CI and can be built locally.

For a locally built client-only tarball, verify its `SHA256SUMS` before extracting:

```bash
sha256sum --ignore-missing --check SHA256SUMS
tar xzf freerdp-portable-x86_64-3.30.0.tar.gz
cd freerdp-portable-x86_64-3.30.0
./freerdp /version
./freerdp /list:monitor
./freerdp /v:windows.example.invalid /u:USERNAME /multimon /monitors:0,1 /f
```

Use the monitor IDs reported by `/list:monitor`. FreeRDP prompts for your
password when you invoke it without credentials. The desktop manager can save
plaintext passwords locally and shows full copyable commands and live FreeRDP
output in the GUI log and terminal, including passwords.

The wrapper probes actual SDL frame presentation before opening a desktop or
detecting monitors. It tries Wayland OpenGL/OpenGL ES, then X11 when available,
and selects the first working path. Software rendering remains an explicit
option. Each candidate is bounded to three seconds. Authentication-only tests,
version and help commands do not require a graphics probe. You can still choose
a backend explicitly:

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

## Combined desktop installation

For an air-gapped RHEL 10 desktop, download the single **rdp** RPM with
FreeRDP, the Rust GUI and GTK/WebKit runtime. Verify `SHA256SUMS` first.

```bash
sha256sum --ignore-missing --check SHA256SUMS
sudo dnf --disablerepo='*' install ./rdp-*.x86_64.rpm
```

The RPM also replaces the previous `rdpctl-portable` RPM. The combined
package replaces the older two packages and installs under
`/opt/rdpctl-portable`. Open **rdpctl** from your application menu, or run
`rdpctl-gui`. The `freerdp` command remains available. For installation without
root, build and extract the `rdpctl-portable-x86_64-VERSION.tar.gz` archive and run
`./rdpctl` inside it. See [desktop documentation](desktop/README.md) for the
AppImage alternative, building, and remaining host requirements.

## Locally built client-only DEB installation

```bash
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
FreeRDP client, with display selection, optional saved plaintext passwords,
NLA authentication tests, per-connection certificate policies, and full command
logging. Existing connections can be edited, including an X11 software-rendering
option for hosts where GPU renderers fail. See the desktop documentation for certificate trust and backup controls.
Older prereleases include separate `rdpctl-gui` packages requiring host
GTK/WebKit. Current releases publish the combined `rdp` RPM described above.

Run `./bin/rdpctl` in the client-only archive, or `rdpctl` after installing the
client-only package, to
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
The installer selects official Ubuntu HTTPS mirrors in place of the runner's
mirror lists; package and matching-source downloads use bounded retries and
connection timeouts.
Source versions and SHA-256 checksums are pinned in
`config/freerdp-version.env`. Generated files stay in `.build/` and `output/`.
Move existing output aside before repeating packaging.

Every push, pull request, and manual workflow run builds and validates the
artifacts. CI checks the packaged SDL library under Xvfb and headless Weston,
runs FreeRDP monitor enumeration on each backend, upgrades/removes the DEB,
and inspects the single combined RPM. Other RPM installers are no longer built;
upgrade tests use old published packages. These checks do not prove actual RDP connectivity
or physical multi-monitor behavior on another workstation.

Pushing a project SemVer tag such as `v0.1.0-rc.1` publishes the validated
combined `rdp` RPM, one consolidated source archive and `SHA256SUMS` to a
GitHub prerelease after both builds and their
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

The desktop GUI can confirm and remember a working graphics mode using **Test
video options**. See [desktop video trials](desktop/README.md#confirm-and-remember-desktop-video-modes)
for the preference order and monitor detection workflow.
