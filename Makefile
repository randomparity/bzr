CARGO ?= cargo
RUST_MIN_VERSION := 1.89.0

.PHONY: setup check-rust ensure-components ensure-coverage-prereqs ensure-mutants-prereq install-hooks \
        build release test test-verbose test-fast test-one coverage fmt clippy lint check-build-script check-test-layout check-functional-test-ids \
        check-no-spawn check-release-security-notes check-shell clean help man \
        skills-test \
        mutants mutants-fast mutants-list audit-mutant-skips \
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
		echo "(need >= $(RUST_MIN_VERSION))"; \
		echo "  Upgrade with: rustup update stable && rustup default stable"; \
		exit 1; \
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
	printf '#!/bin/sh\nset -eu\ncargo fmt -- --check || { echo "Run cargo fmt before committing."; exit 1; }\ncargo clippy --all-targets --features test-helpers -- -D warnings\nmake check-test-layout\nmake check-functional-test-ids\n' > "$$HOOKS_DIR/pre-commit" && \
	chmod +x "$$HOOKS_DIR/pre-commit" && \
	printf '#!/bin/sh\nset -eu\nmake test\n' > "$$HOOKS_DIR/pre-push" && \
	chmod +x "$$HOOKS_DIR/pre-push" && \
	echo "Installed pre-commit (fmt + clippy + test-layout + functional IDs) and pre-push (test) hooks."

## Development

build: ## Build in debug mode
	$(CARGO) build

release: ## Build in release mode
	$(CARGO) build --release

# Agent ergonomics: `make test` runs quiet by default so agent loops keep
# their context small; VERBOSE=1 (or `make test-verbose`) restores the full
# cargo output for debugging.
test: ## Run tests (quiet by default; VERBOSE=1 or test-verbose for full output)
	$(CARGO) test $(if $(filter 1,$(VERBOSE)),,--quiet)

test-verbose: ## Run tests with full output (same as VERBOSE=1 make test)
	$(MAKE) --no-print-directory VERBOSE=1 test

test-fast: ## Run unit tests only (--lib; skips the integration suite)
	$(CARGO) test --lib $(if $(filter 1,$(VERBOSE)),,--quiet)

test-one: ## Run tests matching a name substring: make test-one T=bug_list_returns_bugs
ifeq ($(T),)
	$(error T=<name-substring> is required, e.g. make test-one T=bug_list_returns_bugs)
endif
	$(CARGO) test $(T) $(if $(filter 1,$(VERBOSE)),,--quiet)

skills-test: ## Build bzr and run agent-skills checks (package, drift, installer, lint)
	$(CARGO) build --locked
	BZR_BIN="$$PWD/target/debug/bzr" sh agent-skills/tests/run.sh

coverage: ## Run tests with coverage via cargo-llvm-cov
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { echo "cargo-llvm-cov not installed"; echo "  Run: cargo install cargo-llvm-cov"; exit 1; }
	@rustup component list --installed 2>/dev/null | grep -q '^llvm-tools-' || { echo "llvm-tools-preview not installed"; echo "  Run: rustup component add llvm-tools-preview"; exit 1; }
	cargo llvm-cov --locked --workspace --all-features --summary-only

fmt: ## Format source code
	$(CARGO) fmt

clippy: ## Run clippy lints
	$(CARGO) clippy --all-targets --features test-helpers -- -D warnings

lint: fmt clippy check-build-script check-test-layout check-functional-test-ids check-no-spawn check-release-security-notes check-shell ## Run all linters

check-build-script: ## Run dependency-free build-script validation tests
	@mkdir -p target
	rustc --edition=2021 --test build.rs -o target/build-script-tests
	target/build-script-tests

check-test-layout: ## Verify all test code lives in sibling *_tests.rs files
	@command -v rg >/dev/null || { echo "ERROR: ripgrep (rg) is required for this guard"; exit 1; }
	@if rg -l '^mod tests \{' src/ 2>/dev/null; then \
	  echo "ERROR: inline 'mod tests { ... }' blocks found in src/."; \
	  echo "Move tests to a sibling <name>_tests.rs file linked via"; \
	  echo "  #[cfg(test)] #[path = \"<name>_tests.rs\"] mod tests;"; \
	  echo "See docs/superpowers/specs/2026-05-05-test-sibling-migration-design.md"; \
	  exit 1; \
	fi

check-functional-test-ids: ## Validate stable semantic references in functional tests
	@command -v rg >/dev/null || { echo "ERROR: ripgrep (rg) is required for this guard"; exit 1; }
	bash tools/check-functional-test-ids-tests.sh
	bash tools/check-functional-test-ids.sh .

check-no-spawn: ## Guard the single-threaded-runtime assumption (CONC-3)
	@command -v rg >/dev/null || { echo "ERROR: ripgrep (rg) is required for this guard"; exit 1; }
	bash tools/check-no-spawn.sh .
	bash tools/check-no-spawn-tests.sh

check-release-security-notes: ## Validate release-note security assessments
	bash tools/check-release-security-notes-tests.sh

check-shell: ## Lint shell scripts (shellcheck + shfmt, POSIX and bash)
	@command -v shellcheck >/dev/null || { echo "ERROR: shellcheck is required for this guard"; echo "  Install: brew install shellcheck  |  apt-get install shellcheck"; exit 1; }
	@command -v shfmt >/dev/null || { echo "ERROR: shfmt is required for this guard"; echo "  Install: brew install shfmt  |  https://github.com/mvdan/sh/releases"; exit 1; }
	shellcheck -s sh install.sh tests/installer/smoke.sh
	shellcheck -s bash tools/*.sh
	shellcheck -s bash tests/functional/lib.sh tests/functional/run-tests.sh tests/functional/phases/*.sh
	bash -n tests/functional/lib.sh tests/functional/run-tests.sh tests/functional/phases/*.sh
	shfmt -d -ln posix -i 2 install.sh tests/installer/smoke.sh
	shfmt -d -ln bash -i 2 tools/*.sh

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf man

man: ## Generate manpages into man/man1/
	$(CARGO) run -p xtask --no-default-features --release --quiet -- man --out man/man1

## Mutation Testing
#
# MUTANTS_JOBS caps how many cargo build+test pipelines run in parallel.
# Each one can spawn many compile threads, so values >8 risk overwhelming the
# host (cargo-mutants warns above 8). Default 4 leaves headroom for other work
# on the same machine. Override with: MUTANTS_JOBS=N make mutants
MUTANTS_JOBS ?= 4

ensure-mutants-prereq:
	@command -v cargo-mutants >/dev/null 2>&1 || { echo "cargo-mutants not installed"; echo "  Run: cargo install cargo-mutants --locked"; exit 1; }

mutants: ensure-mutants-prereq ## Run cargo-mutants across the whole crate (slow; hours). MUTANTS_JOBS=N to override parallelism (default 4)
	cargo mutants --jobs $(MUTANTS_JOBS)

mutants-fast: ensure-mutants-prereq ## Run cargo-mutants only on lines changed vs. origin/main
	git diff origin/main...HEAD > /tmp/bzr-mutants.diff
	cargo mutants --in-diff /tmp/bzr-mutants.diff --jobs $(MUTANTS_JOBS)

mutants-list: ensure-mutants-prereq ## List all mutants without running tests
	cargo mutants --list

audit-mutant-skips: ## Print every `mutants::skip` site with surrounding context
	@if command -v rg >/dev/null 2>&1; then \
		rg --line-number --context 2 'mutants::skip' src/ || echo "No mutants::skip annotations found."; \
	else \
		grep -rn --include='*.rs' --context=2 'mutants::skip' src/ || echo "No mutants::skip annotations found."; \
	fi

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
