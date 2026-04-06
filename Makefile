CARGO ?= cargo
RUST_MIN_VERSION := 1.84.0

.PHONY: setup check-rust ensure-components ensure-coverage-prereqs install-hooks \
        build release test coverage fmt clippy lint clean help \
        functional-build functional-start functional-test functional-stop \
        functional-test-bz52 functional-test-bz53 functional-test-all functional-stop-all \
        functional-test-keyring

## Setup & Environment

setup: check-rust ensure-components ensure-coverage-prereqs install-hooks build ## Set up the development environment
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

ensure-components:
	@printf "Checking for rustfmt... "
	@rustup component list --installed 2>/dev/null | grep -q rustfmt || { \
		echo "installing"; \
		rustup component add rustfmt; \
	}
	@printf "Checking for rustfmt... ok\n"
	@printf "Checking for clippy... "
	@rustup component list --installed 2>/dev/null | grep -q clippy || { \
		echo "installing"; \
		rustup component add clippy; \
	}
	@printf "Checking for clippy... ok\n"

ensure-coverage-prereqs:
	@printf "Checking for cargo-llvm-cov... "
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { \
		echo "installing"; \
		$(CARGO) install cargo-llvm-cov; \
	}
	@printf "Checking for cargo-llvm-cov... ok\n"
	@printf "Checking for llvm-tools-preview... "
	@rustup component list --installed 2>/dev/null | grep -q '^llvm-tools-' || { \
		echo "installing"; \
		rustup component add llvm-tools-preview; \
	}
	@printf "Checking for llvm-tools-preview... ok\n"

install-hooks: ## Install git pre-commit and pre-push hooks
	@echo "Installing git hooks..."
	@HOOKS_DIR=$$(git rev-parse --git-path hooks) && \
	mkdir -p "$$HOOKS_DIR" && \
	printf '#!/bin/sh\nset -eu\ncargo fmt -- --check || { echo "Run cargo fmt before committing."; exit 1; }\ncargo clippy -- -D warnings\n' > "$$HOOKS_DIR/pre-commit" && \
	chmod +x "$$HOOKS_DIR/pre-commit" && \
	printf '#!/bin/sh\nset -eu\ncargo test\n' > "$$HOOKS_DIR/pre-push" && \
	chmod +x "$$HOOKS_DIR/pre-push" && \
	echo "Installed pre-commit (fmt + clippy) and pre-push (test) hooks."

## Development

build: ## Build in debug mode
	$(CARGO) build

release: ## Build in release mode
	$(CARGO) build --release

test: ## Run tests
	$(CARGO) test

coverage: ## Run tests with coverage via cargo-llvm-cov
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { echo "cargo-llvm-cov not installed"; echo "  Run: cargo install cargo-llvm-cov"; exit 1; }
	@rustup component list --installed 2>/dev/null | grep -q '^llvm-tools-' || { echo "llvm-tools-preview not installed"; echo "  Run: rustup component add llvm-tools-preview"; exit 1; }
	cargo llvm-cov --locked --workspace --all-features --summary-only

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

functional-test-keyring: ## Run keyring functional test against real OS keychain
	tests/functional/keyring-test.sh

## Help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'
