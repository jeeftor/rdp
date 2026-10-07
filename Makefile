.DEFAULT_GOAL := help

.PHONY: help build package check gui-check gui-build gui-package
help:
	@printf '%s\n' 'Targets:' '  make build    Compile on Ubuntu 24.04 x86_64.' '  make package  Create the tarball, RPM, and DEB (requires nFPM).' '  make check    Run Go checks, shell lint, and launcher tests.'
	@printf '%s\n' '  make gui-check    Check the Rust desktop app.' '  make gui-build    Build the Rust/Tauri desktop app.' '  make gui-package  Package the desktop RPM/DEB (requires nFPM).'

build:
	bash scripts/build-freerdp.sh

package:
	bash scripts/package.sh

check:
	go test ./...
	go vet ./...
	shellcheck scripts/*.sh scripts/diagnostics/*.sh tests/*.sh
	bash tests/wrapper.sh

gui-check:
	cargo fmt --manifest-path desktop/Cargo.toml --all -- --check
	cargo test --manifest-path desktop/Cargo.toml -p rdpctl-core --locked
	cargo clippy --manifest-path desktop/Cargo.toml --workspace --locked -- -D warnings

gui-build:
	cargo build --manifest-path desktop/Cargo.toml -p rdpctl-gui --release --locked

gui-package:
	bash scripts/package-gui.sh
