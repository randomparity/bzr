# CLAUDE.md

This file provides guidance to programming agents when working with code in this repository.

## What is bzr?

A Rust CLI for interacting with Bugzilla REST API servers. Supports bugs, comments, attachments, and multi-server configuration. Inspired by the GitHub CLI (`gh`).

## Build & Development Commands

```bash
cargo build                        # Debug build
cargo build --release              # Release build
cargo test                         # Run all tests
cargo test <test_name>             # Run a single test
cargo fmt                          # Format code
cargo clippy -- -D warnings        # Lint (warnings are errors)
make lint                          # Format + clippy in one step
make setup                         # Full dev environment setup
make functional-test-all           # Run functional tests against real Bugzilla containers
cargo install --path .             # Install locally
```

Git hooks: `make install-hooks` installs a pre-commit hook (`cargo fmt --check` + `cargo clippy`) and a pre-push hook (`cargo test`). Run `make setup` to install everything, including hooks.

## Architecture

Layered CLI pattern: `main.rs` parses args → `lib.rs::dispatch()` matches `Commands` enum → delegates to `commands/*.rs::execute()` → which loads `Config`, resolves auth, builds `BugzillaClient`, calls API, and formats output.

### Key modules

- **`cli/`** — clap derive structs split into per-resource submodules. `mod.rs` defines `Cli`, `Commands`, and re-exports all `*Action` enums. Per-resource files (`bug.rs`, `comment.rs`, `attachment.rs`, `config.rs`, `product.rs`, `field.rs`, `user.rs`, `group.rs`, `server.rs`, `classification.rs`, `component.rs`, `template.rs`, `query.rs`) each define one action enum.
- **`client/`** — `BugzillaClient` wraps reqwest for Bugzilla REST API. Split into per-resource submodules (`bug.rs`, `attachment.rs`, `comment.rs`, `product.rs`, `user.rs`, `group.rs`, `component.rs`, `classification.rs`, `field.rs`, `server.rs`). `auth/` submodule handles auth detection (split into `whoami.rs` and `valid_login.rs` probing strategies with `mod.rs` as orchestrator). `version.rs` handles version detection and API mode determination. Public data types live in `types/`.
- **`config.rs`** — `Config` and `ServerConfig` structs. TOML file at `~/.config/bzr/config.toml`. Multiple named servers with a default. Uses `AuthMethod` and `ApiMode` from `types/common.rs`; `AuthMethod` (`Header`/`QueryParam`) persisted per-server.
- **`error.rs`** — `BzrError` enum (thiserror) with 18 variants: `Http`, `Config`, `Api`, `Io`, `TomlParse`, `TomlSerialize`, `XmlRpc`, `NotFound`, `HttpStatus`, `InputValidation`, `Deserialize`, `Auth`, `DataIntegrity`, `BatchPartialFailure`, `Keyring`, `PinMismatch`, `IssuerChanged`, `MidAirCollision`. Each variant has a distinct `exit_code()` and `error_type()`. `Result<T>` type alias.
- **`output/`** — `mod.rs` is a re-export facade (decouples commands from output internals). `formatting.rs` holds formatting primitives (`print_formatted`, field helpers). `result_types.rs` holds mutation result types (`ActionResult`, `ResourceKind`, `MembershipResult`, etc.). Per-resource submodules (`bug.rs`, `comment.rs`, `attachment.rs`, `product.rs`, `classification.rs`, `user.rs`, `group.rs`, `field.rs`, `server.rs`, `config.rs`) each handle one domain type. Uses `tabled` for tables, `colored` for status colors.
- **`commands/`** — Each network command submodule has an async `execute()` function taking `(action, server, format, api, w)` (where `w: &mut Writers` bundles the stdout/stderr sinks) and following the pattern: load config → resolve auth → connect client → call API → print output. Two exceptions: `config.rs` shares the same async signature but does local I/O only (it ignores the `server`/`api` params) and `whoami.rs` has no action enum (no subcommands). Cross-cutting command infrastructure lives under `commands/runtime/`: `runtime::shared` provides `connect_and_configure()` (config load + auth detection + client construction) and the body-source helpers; `runtime::flags` handles Bugzilla flag syntax parsing; `runtime::{dry_run,confirm,inline_server}` hold process-global flags set in dispatch; `runtime::paging` drives search pagination; `runtime::editor` invokes `$EDITOR`. See `src/commands/mod.rs` for the complete list of command modules.

### Conventions

- User-facing output should use `writeln!(io::stdout(), …)` / `writeln!(io::stderr(), …)` rather than `println!`/`eprintln!`. Two reasons: (a) `print_stdout`/`print_stderr` are denied project-wide, so every `println!` site needs an `#[expect(clippy::print_stdout)]` escape hatch; (b) `test_helpers::capture_stdout` redirects fd 1 via `dup2`, which captures `writeln!(io::stdout(), …)` but **not** `println!` — `println!` goes through cargo test's per-test stdout-capture (a thread-local installed by `set_output_capture`) and bypasses fd 1 entirely. Mixing the two in the same function silently breaks `capture_stdout` tests. The helpers in `src/output/formatting.rs` already follow this convention; new output code should match. Discard the `Result` with `let _ = writeln!(…)` if the function isn't already in a context that allows `.expect()`.
- `#[expect(clippy::print_stdout)]` is used to allow `println!` in the few remaining sites that haven't been migrated to `writeln!`, since `print_stdout` is denied project-wide.
- Logging uses `tracing` (not println). Verbosity: `-v`=info, `-vv`=debug, `-vvv`=trace. `RUST_LOG` env var overrides.
- URLs are sanitized via `safe_url()` in debug logs to avoid leaking API keys in query params.
- Tests use `wiremock` for HTTP mocking. Unit tests live in sibling `<name>_tests.rs` files linked from each source file via `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;` — inline `mod tests { ... }` blocks are **not permitted** in `src/` and `make check-test-layout` enforces this in CI. The reasons are twofold: (1) SonarCloud's CPD scanner is configured to exclude `**/*_tests.rs` (see `sonar-project.properties`), so test-fixture boilerplate stays out of the duplication metric without forcing bad abstractions; (2) it keeps production source files focused. There is no size threshold — even tiny test mods get their own sibling. Test-helpers modules used only by tests follow the same separation (e.g. `src/client/test_helpers.rs`). Sibling files start with the appropriate file-level inner attribute (`#![expect(clippy::unwrap_used)]`, or whatever the original outer attribute was — including combined forms like `#![expect(clippy::unwrap_used, clippy::panic)]`); siblings whose tests don't trigger the lint omit it. Integration tests live in `tests/integration.rs` and functional tests in `tests/functional/`. All API tests require `#[tokio::test]` (the runtime is tokio). See `docs/superpowers/specs/2026-05-05-test-sibling-migration-design.md` for full rationale.
- Clippy pedantic is enabled with strict rules (see `[lints.clippy]` in Cargo.toml). `unwrap_used` is denied, `expect_used` and `allow_attributes` are warned.
- CLI reference documentation lives in `docs/bzr-cli.md`. When adding a new command, update that file.
- `CHANGELOG.md` entries are written **as the work lands**, not deferred to a release-prep commit. When a feature PR ships user-visible behavior, add (or extend) the relevant `## [X.Y.Z]` section in the same PR. The release-prep commit then only needs to confirm the date and version. `release.yml` reads this file via awk to source the GitHub release notes; it does not generate or modify entries. (Earlier releases used dedicated `release: add CHANGELOG entry` commits — that convention is superseded.)
- System package dependencies (e.g. `libdbus-1-dev` for the `keyring` default feature) must be installed in **every** place the crate is built, not just one workflow. When adding or changing a native dependency, audit all of:
  - `.github/workflows/ci.yml` (native host jobs)
  - `.github/workflows/release.yml` (native host jobs **and** the QEMU container for `powerpc64le`)
  - `.github/workflows/publish-crates.yml` (native host job)
  - `Cross.toml` `pre-build` hooks for every `cross`-driven target (adds the target-arch dev package via Debian multiarch: `apt-get install libdbus-1-dev:$CROSS_DEB_ARCH`)
  Do **not** work around a missing system dep by disabling default features on exotic targets — that silently ships a degraded binary. Make the build environment correct on every target instead.
