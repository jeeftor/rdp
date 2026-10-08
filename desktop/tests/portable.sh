#!/usr/bin/env bash
set -euo pipefail
root="$(pwd -P)"
# Ubuntu 24.04 restricts unprivileged namespaces. Allow only the test helper
# paths on this disposable runner; the WebKit sandbox remains enabled.
cat > .build/rdpctl-test.apparmor <<PROFILE
abi <abi/4.0>,
include <tunables/global>
profile rdpctl-test-isolation /usr/bin/bwrap flags=(unconfined) {
  userns,
}
profile rdpctl-test-bundled "$root/.build/**/app/usr/bin/bwrap" flags=(unconfined) {
  userns,
}
PROFILE
sudo apparmor_parser -r .build/rdpctl-test.apparmor
# Fail here before WebDriver waits on a process that cannot start.
bwrap --ro-bind / / --proc /proc --dev-bind /dev /dev /usr/bin/true
version="${PACKAGE_VERSION:-0.1.0-dev}"
portable="$root/.build/rdpctl-portable-x86_64-$version"
client="$portable/app/usr/share/rdpctl/freerdp/freerdp"
"$client" /version
rpm -qp --requires output/rdpctl-portable*.rpm
if rpm -qp --requires output/rdpctl-portable*.rpm | rg 'webkit|gtk3'; then exit 1; fi
if dpkg-deb -f output/rdpctl-portable*.deb Depends | rg 'webkit|libgtk'; then exit 1; fi

# Exercise the exact upgrade from the old two packages in an isolated RPM database.
mkdir -p .build/rpm-upgrade
rpm --root "$root/.build/rpm-upgrade" --initdb
rpm --root "$root/.build/rpm-upgrade" --nodeps -i .build/portable-input/freerdp-portable*.rpm output/rdpctl-gui*.rpm
rpm --root "$root/.build/rpm-upgrade" --nodeps -U output/rdpctl-portable*.rpm
rpm --root "$root/.build/rpm-upgrade" -q rdpctl-portable
if rpm --root "$root/.build/rpm-upgrade" -q freerdp-portable; then exit 1; fi
if rpm --root "$root/.build/rpm-upgrade" -q rdpctl-gui; then exit 1; fi
rpm --root "$root/.build/rpm-upgrade" -qf /usr/bin/freerdp /usr/bin/rdpctl-gui

# WebDriver needs a debug binary. Keep the packaged production binary intact.
mkdir -p .build/portable-test
cp -a "$portable" .build/portable-test/bundle
cp desktop/target/debug/rdpctl-gui .build/portable-test/bundle/app/usr/bin/rdpctl-gui
cat > .build/portable-test/isolated-gui <<'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
export XDG_CACHE_HOME="${XDG_CONFIG_HOME:?}/cache"
export XDG_DATA_HOME="$XDG_CONFIG_HOME/data"
args=(--ro-bind / / --dev-bind /dev /dev --proc /proc --bind /tmp /tmp --bind "$root" "$root")
# Hide the host GUI libraries and WebKit helpers only for the app process.
# WebDriver remains outside this namespace; WebKit's own sandbox stays enabled.
for pattern in /usr/lib/x86_64-linux-gnu/libgtk-3.so* /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so* /usr/lib/x86_64-linux-gnu/libjavascriptcoregtk-4.1.so*; do
  [[ ! -f "$pattern" ]] || args+=(--ro-bind /dev/null "$pattern")
done
args+=(--tmpfs /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1 --ro-bind /dev/null /usr/bin/bwrap --ro-bind /dev/null /usr/bin/xdg-dbus-proxy)
exec bwrap "${args[@]}" "${RDPCTL_PORTABLE_TEST_APP:-$root/bundle/rdpctl}" "$@"
WRAPPER
chmod 0755 .build/portable-test/isolated-gui
bash desktop/tests/display-backends.sh .build/portable-test/isolated-gui
bash desktop/tests/display-backends.sh .build/portable-test/isolated-gui --bundled-monitors
# Production also must create its actual window through the isolated launcher.
RDPCTL_PORTABLE_TEST_APP="$portable/rdpctl" dbus-run-session -- xvfb-run -a env -u WAYLAND_DISPLAY GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 bash desktop/tests/portable-production.sh "$root/.build/portable-test/isolated-gui"

# Install and remove the combined DEB after both legacy packages.
sudo apt-get install -y ./.build/portable-input/freerdp-portable*.deb ./output/rdpctl-gui*.deb
sudo apt-get install -y ./output/rdpctl-portable*.deb
freerdp /version
test -x /opt/rdpctl-portable/rdpctl
test -x /opt/rdpctl-portable/app/AppRun
test -x /opt/rdpctl-portable/app/usr/bin/rdpctl-gui
test "$(dpkg-query -S /usr/bin/rdpctl-gui)" = 'rdpctl-portable: /usr/bin/rdpctl-gui'
sudo apt-get remove -y rdpctl-portable
test ! -e /opt/rdpctl-portable
test ! -e /usr/bin/freerdp
