CARGO ?= cargo
PRE_COMMIT ?= pre-commit
RUST_MIN_VERSION := 1.70.0

.PHONY: setup check-rust check-components check-pre-commit install-hooks \
        build release test fmt clippy lint clean help \
        functional-build functional-start functional-test functional-stop \
        functional-test-bz52 functional-test-bz53 functional-test-all functional-stop-all

## Setup & Environment

setup: check-rust check-components check-pre-commit install-hooks build ## Set up the development environment
	@echo
	@echo "Setup complete. You're ready to develop bzr."

check-rust:
	@printf "Checking for Rust toolchain... "
	@command -v rustc >/dev/null 2>&1 || { echo "MISSING"; echo "  Install Rust: https://rustup.rs"; exit 1; }
	@command -v cargo >/dev/null 2>&1 || { echo "MISSING"; echo "  Install Rust: https://rustup.rs"; exit 1; }
	@RUST_VER=$$(rustc --version | sed 's/rustc \([^ ]*\).*/\1/'); \
	printf "%s " "$$RUST_VER"; \
	LOWEST=$$(printf '%s\n%s\n' "$(RUST_MIN_VERSION)" "$$RUST_VER" | sort -V | head -n1); \
	if [ "$$LOWEST" != "$(RUST_MIN_VERSION)" ]; then \
		echo "(need >= $(RUST_MIN_VERSION))"; exit 1; \
	fi
	@echo "ok"

check-components:
	@printf "Checking for rustfmt... "
	@rustup component list --installed 2>/dev/null | grep -q rustfmt || { echo "MISSING"; echo "  Run: rustup component add rustfmt"; exit 1; }
	@echo "ok"
	@printf "Checking for clippy... "
	@rustup component list --installed 2>/dev/null | grep -q clippy || { echo "MISSING"; echo "  Run: rustup component add clippy"; exit 1; }
	@echo "ok"

check-pre-commit:
	@printf "Checking for pre-commit... "
	@command -v $(PRE_COMMIT) >/dev/null 2>&1 || { echo "MISSING (optional)"; echo "  Install: pip install pre-commit"; echo "  Skipping hook installation."; exit 0; }
	@echo "ok"

install-hooks:
	@if command -v $(PRE_COMMIT) >/dev/null 2>&1; then \
		echo "Installing pre-commit hooks..."; \
		$(PRE_COMMIT) install; \
		$(PRE_COMMIT) install --hook-type pre-push; \
	fi

## Development

build: ## Build in debug mode
	$(CARGO) build

release: ## Build in release mode
	$(CARGO) build --release

test: ## Run tests
	$(CARGO) test

fmt: ## Format source code
	$(CARGO) fmt

clippy: ## Run clippy lints
	$(CARGO) clippy -- -D warnings

lint: fmt clippy ## Run all linters (fmt + clippy)

clean: ## Remove build artifacts
	$(CARGO) clean

## Functional Tests

functional-build: ## Build the Bugzilla container image
	tests/functional/setup-bugzilla.sh build

functional-start: ## Start the Bugzilla container
	tests/functional/setup-bugzilla.sh start

functional-test: functional-start ## Run functional tests against real Bugzilla
	tests/functional/run-tests.sh

functional-stop: ## Stop and remove the Bugzilla container
	tests/functional/setup-bugzilla.sh stop

functional-test-bz52: ## Run functional tests against Bugzilla 5.2
	BZR_BZ_VERSION=bz52 tests/functional/setup-bugzilla.sh start
	BZR_BZ_VERSION=bz52 tests/functional/run-tests.sh

functional-test-bz53: ## Run functional tests against Bugzilla 5.3 (master)
	BZR_BZ_VERSION=bz53 tests/functional/setup-bugzilla.sh start
	BZR_BZ_VERSION=bz53 tests/functional/run-tests.sh

functional-test-all: ## Run functional tests against all Bugzilla versions
	tests/functional/run-all-versions.sh

functional-stop-all: ## Stop all Bugzilla test containers
	BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh stop
	BZR_BZ_VERSION=bz52 tests/functional/setup-bugzilla.sh stop
	BZR_BZ_VERSION=bz53 tests/functional/setup-bugzilla.sh stop

## Help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'
