# rdpctl desktop

The Rust/Tauri desktop app is the graphical connection manager for the Linux
FreeRDP bundle. Save a host and username, select your displays, and click
**Connect**. FreeRDP opens its own native window. Under **Edit**, enter a password and optionally save it in plaintext on this machine.
While a desktop is open, **Disconnect** closes all desktops launched by this
GUI for that connection. This disconnects the client; it does not sign you out
of Windows. Clients launched separately from a terminal are not controlled.
Leave the field empty to use your saved password; without one, FreeRDP prompts
when connecting. **Forget password** removes only that connection's password.

Profiles use the existing `~/.config/rdpctl/connections/*.json` format, respecting
`XDG_CONFIG_HOME`. The GUI lists existing profiles and creates new ones with
private permissions. Duplicate names/IDs are rejected without overwriting files.
Edit the name, host, username, monitor IDs, fullscreen and multi-monitor options
without recreating the connection. Its stable ID and saved password are retained.
Profiles remain JSON under `~/.config/rdpctl/connections` (or `XDG_CONFIG_HOME`). Plaintext
passwords are stored separately in `connections/passwords/<id>` with directory
mode 700 and file mode 600. Profile JSON still contains no password, preserving
compatibility with the terminal app. The terminal app does not use saved GUI
passwords or GUI certificate settings.

## Guided troubleshooting and deletion

Choose **Troubleshoot** on a saved connection to work through the connection
in order: server TCP reachability, certificate policy and visible credentials,
NLA sign-in, then video modes. A reachable TCP port does not prove it is an RDP
server or that authentication will work. You explicitly review trust and test
sign-in before opening graphics trials; certificates are never ignored
without your choice.

A rejected sign-in opens **Review sign-in / trust** with the available server
reason and diagnostic details. Correct the visible username/password or
certificate policy and choose **Save and retry**. Cancel saves nothing and
retries nothing. Unchecking password storage uses the entered password for this
wizard without replacing the saved password. Generic `LOGON_FAILURE` does not
identify the exact cause; explicit locked/expired/disabled-account or password
errors are reported when present in the log. A locked account uses **Save
connection**, without retrying: unlock it on the server or wait for its policy
before testing again.

In **Edit**, choose **Delete** and confirm the named connection. This removes
only that profile and its saved password. Other connections, remembered server
certificates and existing diagnostic files stay unchanged. Cancel does not
remove anything.

Use **Export diagnostics** before closing the GUI to save this connection's
commands, logs, graphics results and available screenshots under
`~/.local/state/rdpctl/diagnostics` (respecting `XDG_STATE_HOME`). Retention is the
latest 32 attempts in this GUI process, with the last 1 MiB per desktop attempt
and 64 KiB per sign-in result. Exports include passwords. For terminal output
across a complete run, use `rdpctl-gui 2>&1 | tee "$HOME/rdpctl-debug.log"`.

## Test a connection and handle certificates

**Test sign-in** checks NLA authentication using `+auth-only /sec:nla`, your
current certificate setting and entered/saved password. It does not open a
remote desktop. The test has a 20-second deadline and distinguishes network,
certificate, changed-certificate, credential, and crypto-runtime failures. A
successful test proves NLA authentication, not desktop or monitor behavior.
FreeRDP 3.30's SDL client can explicitly report successful authentication and
then return process status 1 during cleanup. The GUI accepts that specific
symbolic `SUCCESS [0]` result, displays a cleanup warning, and retains the actual
exit code and log. A timeout, conflicting rejection, or signal termination
still fails the test; a numeric `exit status 0` log line alone is insufficient.
See the upstream [SDL event loop and exit handling](https://github.com/FreeRDP/FreeRDP/blob/3.30.0/client/SDL/SDL3/sdl_freerdp.cpp)
and [authentication cleanup](https://github.com/FreeRDP/FreeRDP/blob/3.30.0/client/SDL/SDL3/sdl_context.cpp).

Choose a policy for each connection:

- **Verify certificate**: require trusted certificate/name verification or
  an already remembered matching certificate; reject unresolved trust.
- **Trust on first use**: accept and remember the first certificate, including
  during a test; reject a changed certificate.
- **Pin SHA-256 fingerprint**: accept the specified fingerprint. A failed test
  can populate this field; compare it with your server before saving/testing.
- **Ignore certificate checks**: accept any certificate for this connection.

If a remembered certificate needs replacing, **Back up remembered certificate**
followed by **Confirm backup** moves only the selected host/port's
`freerdp/server/HOST_PORT.pem` to a timestamped `.pem.previous-*` backup. It
does not automatically trust the new certificate or change your policy.

Every launch and test prints a shell-quoted, copyable FreeRDP command to standard
error and the GUI connection log, including the password. The GUI also streams
FreeRDP stdout/stderr into that log; **Copy command** copies the full shell-quoted
command, including environment overrides for software rendering. Sign-in commands
appear under **Details / command**; `+auth-only` checks
credentials and exits, while desktop commands are shown after **Connect**. The actual launch passes arguments through
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
The lockfiles pin Rust and frontend dependencies. Building the interface requires
Node 20 or newer; the installed RPM does not need Node or npm.
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

## Display and rendering failures

The client uses `DISPLAY` for X11 and `WAYLAND_DISPLAY` plus `XDG_RUNTIME_DIR`
for Wayland. These normally come from your logged-in desktop session; do not
point them at arbitrary directories. `XKB_CONFIG_ROOT` refers to host keyboard
data and defaults to `/usr/share/X11/xkb`.

In automatic graphics mode, the launcher probes the bundled SDL library before
a desktop connection or monitor detection. It tests window creation, a target texture and frame
presentation, trying Wayland OpenGL/OpenGL ES first and then X11 when available.
It selects the first working path and includes the selected backend and renderer
in the GUI's copyable desktop command. Probe diagnostics include the OpenGL device
name when available. Each candidate has a three-second timeout. This detects a
working rendering path; Mesa may still provide CPU rendering if hardware is
unavailable. Explicit SDL backend overrides are respected. Authentication tests
and version/help commands do not require the graphics probe.

There is no automatic fallback to SDL software rendering. Existing monitor IDs
can differ after a backend change; use **Detect displays**
to select current IDs. Bundles installed before this feature keep their existing
behavior until updated.

If GPU renderer creation fails and you see `Window framebuffer support not
available`, choose **X11 · Software fallback** under **Graphics mode** in **Edit**. It uses
`SDL_VIDEODRIVER=x11 SDL_RENDER_DRIVER=software SDL_FRAMEBUFFER_ACCELERATION=0`,
requires a working `DISPLAY` (Xorg or XWayland), and applies to that connection.
On Wayland without XWayland, repair the host EGL/graphics driver setup instead.
Monitor IDs can differ between Wayland and X11. **Detect displays for this
connection** uses its chosen backend; select the reported IDs or clear Monitor
IDs to use defaults. The GUI flags invalid IDs in connection output.
Authentication tests do not test desktop rendering. A Kerberos default-realm
warning can precede a successful NTLM fallback; inspect the final outcome.

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
# Install/update the combined RPM offline, including the same-version reinstall.
sudo rpm -Uvh --replacepkgs ./rdp-*.x86_64.rpm
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

`make gui-package` creates GUI sources, notices and a test DEB for systems that
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
for both client and desktop builds and tests to pass. Packaging creates only one
new RPM: the combined `rdp` installer. Upgrade tests download old published RPMs
instead of building additional RPMs.

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

The connection cards show the selected graphics mode. Edit and log disclosure
controls have bordered button styling and keyboard focus indicators. New and
saved passwords are visible as plaintext in their fields, copyable commands,
and diagnostic output.

## Confirm and remember desktop video modes

Use **Test** on a saved connection to try X11 or Wayland with
OpenGL/OpenGL ES, or X11 software rendering. **Open desktop** opens a real
1280×720 NLA desktop using your credentials and certificate policy. Saved monitor
IDs, fullscreen and multi-monitor settings are omitted only for the trial.
Close its desktop before starting the next trial. Full commands and output are
available in the connection log.

Mark **I saw the desktop** only when you saw a usable desktop. The GUI saves
confirmed modes in the JSON profile and selects the first confirmed mode in this
order: X11 OpenGL, Wayland OpenGL, X11 OpenGL ES, Wayland OpenGL ES, X11 software.
Confirmations survive restarting the GUI. This is a preference order, not a GPU
benchmark; OpenGL can still use a CPU driver. Confirmation clears saved monitor
IDs: detect displays again for the chosen mode. Normal Connect and display
detection use the saved choice. **Graphics mode** also lets you select a mode
manually or return to automatic detection. Existing software-override profiles
remain readable. Renderer trials do not change your certificate policy.


### Compact interface and one-click testing

The interface uses Vue and PrimeVue controls. Your saved connections appear in a
compact table with **Connect**, **Edit**, and **Test** actions. New and Edit open
modal forms; passwords remain visible. Test opens five short result rows with
expandable details, full commands, and logs.

**Test all video modes** authenticates once, then opens each available real RDP
desktop sequentially for up to 15 seconds. The active mode is highlighted. Trials
use NLA, your saved password/certificate policy, and a simple 1280×720 window
without saved monitor IDs. Rejected sign-in stops further connection attempts.
**Test sign-in** checks credentials only and does not prove rendering.

X11 trials capture only the launched FreeRDP process's client window after five
seconds, retrying at ten seconds if needed. The bundled capture helper uses an
XComposite backing pixmap when available, with a mapped-window fallback. It
saves private PNGs and reports **Image has content**, **Blank image**, or
**Not captured** with pixel metrics. A nearly black or uniform image is a
warning, not proof of bad credentials. Window captures do not prove that your
physical monitor compositor or keyboard input works. Native Wayland capture is
unavailable through this X11 helper; you can still view and confirm the desktop.

**I saw the desktop** is your explicit confirmation of a usable picture. The
app never automatically treats authentication, API frame presentation,
framebuffer initialization, or nonblack pixels as visual success. Confirmed
available OpenGL modes are preferred to OpenGL ES, then software. Monitor IDs
are cleared when remembering a choice so you can detect them for its backend.

**Compare CLI sign-in** compares stdin arguments with direct CLI arguments using
the same profile and password. The actual exit codes and outputs remain visible.
FreeRDP SDL exit 134 means logon failure; signal termination is recorded
separately from an ordinary numeric exit code.

**Export diagnostics** writes private JSON to
`${XDG_STATE_HOME:-~/.local/state}/rdpctl/diagnostics/`. It includes your profile,
relevant environment, full commands and passwords, logs, process results,
screenshot evidence, and a batch identifier. The latest 32 attempts are retained
in the GUI process, with the last 1 MiB per desktop attempt and 64 KiB per
sign-in attempt. Export before closing the app.

The frontend is built from locked dependencies with `npm ci --prefix desktop/ui`
and `npm run build --prefix desktop/ui` (Node 20 or newer on the build machine).
`make gui-check` and `make gui-build` build the frontend before Rust embeds it.
The installed RHEL RPM contains the compiled interface and capture helper;
it does not require Node, npm, a CDN, or online resources to run. Sources include
vendored frontend modules and their license notices.

CI restores compatible Rust dependencies and stores refreshed build artifacts
per commit. Pinned packaging/test tools are cached, and already-compressed
artifacts are uploaded without redundant compression. Reruns replace their
own generated CI artifacts. Read-only GUI checks tolerate brief WebDriver
connection resets within their existing deadline; mutation actions are not replayed. It runs the frontend checks/build once before the Rust builds.
Packaging uses `pigz` (installed by `scripts/install-build-deps.sh`) to create
standard gzip-compressed tar archives in parallel; package formats and native
GUI checks are unchanged.
