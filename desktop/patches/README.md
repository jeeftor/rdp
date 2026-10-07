# GLib 0.18 security backport

`glib/` is the complete crates.io `glib` 0.18.5 source, with its original MIT
license and copyright notices. Cargo uses this local copy through the desktop
workspace's `[patch.crates-io]` override.

The only upstream code changes are the two-line mutability fix in
`src/variant_iter.rs`, backported from gtk-rs/gtk-rs-core PR 1343:
https://github.com/gtk-rs/gtk-rs-core/pull/1343
Upstream merge commit: `05dff0ee696f9bcd8617cd48c4b812d046d440cb`.

This fixes GHSA-wrw7-89jp-8q8g / RUSTSEC-2024-0429. The Linux Tauri/GTK3 chain
requires GLib 0.18; GLib 0.20 cannot replace it without upgrading that chain.
The version remains 0.18.5 to retain API compatibility and accurately identify
the upstream base. Version-only scanners may still report the advisory; this
copy contains the actual upstream fix, rather than a renamed version.

CI tests forward and reverse string-variant iteration with release optimization,
then builds and drives the Linux GUI under X11 and Wayland. Source packages
include this patched crate; binary packages include its notices and this record.
Remove the override and local source when the GTK dependency chain supports a
fixed upstream GLib release.
