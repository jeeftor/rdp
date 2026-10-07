.DEFAULT_GOAL := help

.PHONY: help build package check
help:
	@printf '%s\n' 'Targets:' '  make build    Compile on Ubuntu 24.04 x86_64.' '  make package  Create the tarball, RPM, and DEB (requires nFPM).' '  make check    Run Go checks, shell lint, and launcher tests.'

build:
	bash scripts/build-freerdp.sh

package:
	bash scripts/package.sh

check:
	go test ./...
	go vet ./...
	shellcheck scripts/*.sh scripts/diagnostics/*.sh tests/*.sh
	bash tests/wrapper.sh
