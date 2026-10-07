# rdpctl desktop

The Rust/Tauri desktop app is the graphical connection manager for the Linux
FreeRDP bundle. Save a host and username, select your displays, and click
**Connect**. FreeRDP opens its own native window and prompts for your password.
The GUI reports process launch and exit; it does not claim that authentication
or the remote connection succeeded.

Profiles use the existing `~/.config/rdpctl/connections/*.json` format, respecting
`XDG_CONFIG_HOME`. The GUI lists existing profiles and creates new ones with
private permissions. Duplicate names/IDs are rejected without overwriting files.
Editing, deletion, and password storage are outside this first version.

## Build and run

Install Rust 1.98.1 through your usual development setup. On Ubuntu 24.04, install
`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev patchelf build-essential pkg-config`.
The lockfile pins all Rust dependencies; no Node.js toolchain is required.
The workspace includes an upstream GLib security fix backported for Tauri's
GTK3 dependencies; see [patches/README.md](patches/README.md). Weekly Dependabot
checks cover Rust, Go and GitHub Actions.

```bash
make gui-check
make gui-build
desktop/target/release/rdpctl-gui
```

The portable launcher uses its bundled FreeRDP client through `APPDIR`.
The separate GUI package uses `/opt/freerdp-portable/freerdp`. For development, set
`RDPCTL_FREERDP` to an absolute path to another FreeRDP wrapper. Display variables
are inherited, allowing the existing wrapper to select Wayland or X11.
The frontend cannot supply an executable path or run arbitrary shell commands.

## Packages and testing

The recommended air-gap package is **rdpctl-portable**: one RPM or DEB installs
FreeRDP, the GUI, GTK3, WebKitGTK 4.1, their helper processes and resources.
It replaces the older `freerdp-portable` and `rdpctl-gui` packages. Open **rdpctl**
from the application menu after installation. Profiles remain in your home directory.

```bash
# RHEL 10: DNF handles replacement of the older packages.
sudo dnf --disablerepo='*' install ./rdpctl-portable-*.x86_64.rpm
# Ubuntu 24.04:
sudo apt install ./rdpctl-portable_*_amd64.deb
```

For installation without root or FUSE, verify `GUI-SHA256SUMS`, extract
`rdpctl-portable-x86_64-VERSION.tar.gz` to a local executable filesystem,
enter its directory, and run `./rdpctl`. An AppImage is also available; use
`chmod +x FILE.AppImage` and `./FILE.AppImage --appimage-extract-and-run` when
FUSE is unavailable. The bundled client is under `app/usr/share/rdpctl/freerdp`.
Host glibc 2.39 or later, desktop graphics drivers, EGL and keyboard data are
still required, along with kernel support and permission for WebKit sandbox
namespaces. Actual RHEL 10 installation, real RDP connections and physical
multi-monitor behavior require target validation.

`make gui-package` retains the smaller separate GUI RPM/DEB for systems that
already have GTK3 and WebKitGTK 4.1. `make gui-portable-package` runs after that
step, requires tauri-cli 2.12.1 and a checksum-verified client release in
`.build/portable-input`, and creates the combined packages, AppImage, tarball,
matching sources, dependency notices and `GUI-SHA256SUMS`.

The GitHub desktop workflow builds natively on Ubuntu 24.04 and tests the
actual webview under X11 and Wayland. Portable smoke tests hide the host GTK
and WebKit libraries and helper processes from the application, then exercise
profile saving, monitor selection and launching through the bundled runtime.
CI also checks upgrades from the older two-package installation.
Tagged releases use the client artifact built from the same tag; development
GUI builds use the pinned `v0.1.0-rc.4` client baseline. Release publication waits
for both client and desktop builds and tests to pass.

To rebuild the source archive offline, extract it, enter `gui-sources`, and run
`cargo build --release --locked --offline -p rdpctl-gui` with the documented Rust
toolchain and system development libraries already installed.

Linux GUI smoke tests use `tauri-driver` 2.0.5, `webkit2gtk-driver`, Xvfb and Weston:

```bash
cargo install tauri-driver --version 2.0.5 --locked
cargo build --manifest-path desktop/Cargo.toml --locked -p rdpctl-gui
bash desktop/tests/display-backends.sh desktop/target/debug/rdpctl-gui
```

These tests drive a debug build, following Tauri's WebDriver setup, with software
rendering on the headless displays. CI separately compiles the production binary
for packaging.
The tests use a temporary profile directory and a fake client to verify the
real GUI-to-Rust-to-process boundary without connecting to a remote host. They
verify profile compatibility, monitor selection, exact launch arguments, safe
text rendering, private files, and protection against duplicate overwrites.
