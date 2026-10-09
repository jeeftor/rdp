# rdpctl desktop

The Rust/Tauri desktop app is the graphical connection manager for the Linux
FreeRDP bundle. Save a host and username, select your displays, and click
**Connect**. FreeRDP opens its own native window. Under **Password and certificate
settings**, enter a password and optionally save it in plaintext on this machine.
Leave the field empty to use your saved password; without one, FreeRDP prompts
when connecting. **Forget saved password** removes only that connection's password.

Profiles use the existing `~/.config/rdpctl/connections/*.json` format, respecting
`XDG_CONFIG_HOME`. The GUI lists existing profiles and creates new ones with
private permissions. Duplicate names/IDs are rejected without overwriting files.
Certificate settings can be updated without recreating a profile. Plaintext
passwords are stored separately in `connections/passwords/<id>` with directory
mode 700 and file mode 600. Profile JSON still contains no password, preserving
compatibility with the terminal app. The terminal app does not use saved GUI
passwords or GUI certificate settings.

## Test a connection and handle certificates

**Test connection** checks NLA authentication using `+auth-only /sec:nla`, your
current certificate setting and entered/saved password. It does not open a
remote desktop. The test has a 20-second deadline and distinguishes network,
certificate, changed-certificate, credential, and crypto-runtime failures. A
successful test proves NLA authentication, not desktop or monitor behavior.

Choose a policy for each connection:

- **Verify certificate**: require trusted certificate/name verification or
  an already remembered matching certificate; reject unresolved trust.
- **Trust on first use**: accept and remember the first certificate, including
  during a test; reject a changed certificate.
- **Pin SHA-256 fingerprint**: accept the specified fingerprint. A failed test
  can populate this field; compare it with your server before saving/testing.
- **Ignore certificate checks**: accept any certificate for this connection.

If a remembered certificate needs replacing, **Back up remembered certificate**
followed by **Confirm certificate backup** moves only the selected host/port's
`freerdp/server/HOST_PORT.pem` to a timestamped `.pem.previous-*` backup. It
does not automatically trust the new certificate or change your policy.

Every launch and test prints a shell-quoted, copyable FreeRDP command to standard
error, including the password. The actual launch passes arguments through
standard input when using a password. To keep terminal output locally:

```bash
rdpctl-gui 2>&1 | tee rdpctl.log
```

The portable client bundles OpenSSL's matching `legacy.so` and selects it through
`OPENSSL_MODULES`. Packaging checks provider loading and MD4 for NTLM. Host
graphics drivers/EGL remain system requirements; renderer warnings are separate
from certificate or authentication failures. Missing `canberra-gtk-module` or
`pk-gtk-module` is an optional desktop-integration warning, not an RDP failure.

To isolate GPU/EGL warnings, try software rendering for one launch:

```bash
SDL_RENDER_DRIVER=software rdpctl-gui
```

This uses SDL's [renderer selection](https://wiki.libsdl.org/SDL3/SDL_HINT_RENDER_DRIVER)
for the inherited client process and does not change your system graphics setup.
The `WITH_VERBOSE_WINPR_ASSERT` build warning is diagnostic information; the
certificate rejection and unavailable MD4 are the connection blockers in the
reported log.

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

The published air-gap installer is the single **rdp** RPM for RHEL 10 x86_64.
The release also includes `SHA256SUMS` and one consolidated matching-source
archive; other formats are available through local builds and CI artifacts.
One package installs
FreeRDP, the GUI, GTK3, WebKitGTK 4.1, their helper processes and resources.
It replaces the older `freerdp-portable` and `rdpctl-gui` packages; the RPM
also replaces the previous `rdpctl-portable` RPM. Open **rdpctl**
from the application menu after installation. Profiles remain in your home directory.

```bash
sha256sum --ignore-missing --check SHA256SUMS
# DNF handles replacement of the older packages and updates an existing install.
sudo dnf --disablerepo='*' install ./rdp-*.x86_64.rpm
```

For a locally built installation without root or FUSE, verify `GUI-SHA256SUMS`, extract
`rdpctl-portable-x86_64-VERSION.tar.gz` to a local executable filesystem,
enter its directory, and run `./rdpctl`. Local packaging also creates an AppImage; use
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
CI also checks upgrades from the older two-package installation. The Ubuntu
runner loads a temporary AppArmor rule for the namespace test helpers; the
application does not disable the WebKit sandbox.
Tagged releases use the client artifact built from the same tag; development
GUI builds use the pinned `v0.1.0-rc.4` client baseline. Release publication waits
for both client and desktop builds and tests to pass.

To rebuild the GUI offline, extract the consolidated source archive, then its
embedded `rdpctl-gui-VERSION-sources.tar.gz`, enter `gui-sources`, and run
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
text rendering, private files, plaintext password storage, authentication-test
results, explicit certificate backup and protection against duplicate overwrites.
