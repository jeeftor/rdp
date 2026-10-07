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

```bash
make gui-check
make gui-build
desktop/target/release/rdpctl-gui
```

The launcher uses `/opt/freerdp-portable/freerdp`. For development, set
`RDPCTL_FREERDP` to an absolute path to another FreeRDP wrapper. Display variables
are inherited, allowing the existing wrapper to select Wayland or X11.
The frontend cannot supply an executable path or run arbitrary shell commands.

## Packages and testing

`make gui-package` uses the existing pinned nFPM packager to create a separate
`rdpctl-gui` RPM and DEB, checksums, dependency licenses, and a source archive
including the locked Rust dependencies. Install the matching `freerdp-portable`
package first or alongside it. The GUI adds an application-menu entry named
**rdpctl**. The original terminal command remains available.

```bash
sudo dnf install ./freerdp-portable*.rpm ./rdpctl-gui*.rpm
# Ubuntu 24.04:
sudo apt install ./freerdp-portable*.deb ./rdpctl-gui*.deb
```

The GitHub desktop workflow builds natively on Ubuntu 24.04, runs Rust tests and
Clippy, drives the actual Tauri webview under X11 and Wayland, and creates both
packages. Download the `rdpctl-desktop-linux-x86_64` Actions artifact from the
feature branch while this prototype is under review. Existing published
`v0.1.0-rc.2` assets do not contain this new GUI.

The GUI needs host GTK3 and WebKitGTK 4.1 at runtime. RHEL 10 requires the latter
from EPEL; offline installations must include those dependencies. The current
Ubuntu-built RPM requires glibc 2.39 or later. Actual RHEL 10 installation,
graphics drivers, and real RDP connections still require target validation.

To rebuild the source archive offline, extract it, enter `gui-sources`, and run
`cargo build --release --locked --offline -p rdpctl-gui` with the documented Rust
toolchain and system development libraries already installed.

Linux GUI smoke tests use `tauri-driver` 2.0.5, `webkit2gtk-driver`, Xvfb and Weston:

```bash
cargo install tauri-driver --version 2.0.5 --locked
bash desktop/tests/display-backends.sh desktop/target/release/rdpctl-gui
```

These tests use a temporary profile directory and a fake client to verify the
real GUI-to-Rust-to-process boundary without connecting to a remote host. They
verify profile compatibility, monitor selection, exact launch arguments, safe
text rendering, private files, and protection against duplicate overwrites.
