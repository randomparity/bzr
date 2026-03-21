# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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

Pre-commit hooks run `cargo fmt` and `cargo clippy` on commit, `cargo test` on push.

## Architecture

Layered CLI pattern: `main.rs` parses args → `lib.rs::dispatch()` matches `Commands` enum → delegates to `commands/*.rs::execute()` → which loads `Config`, resolves auth, builds `BugzillaClient`, calls API, and formats output.

### Key modules

- **`cli/`** — clap derive structs split into per-resource submodules. `mod.rs` defines `Cli`, `Commands`, and re-exports all `*Action` enums. Per-resource files (`bug.rs`, `comment.rs`, `attachment.rs`, `config.rs`, `product.rs`, `field.rs`, `user.rs`, `group.rs`, `server.rs`, `classification.rs`, `component.rs`) each define one action enum.
- **`client/`** — `BugzillaClient` wraps reqwest for Bugzilla REST API. Split into per-resource submodules (`bug.rs`, `attachment.rs`, `comment.rs`, `product.rs`, `user.rs`, `group.rs`, `component.rs`, `classification.rs`, `field.rs`, `server.rs`). `auth/` submodule handles auth detection (split into `whoami.rs` and `valid_login.rs` probing strategies with `mod.rs` as orchestrator). `version.rs` handles version detection and API mode determination. Public data types live in `types/`.
- **`config.rs`** — `Config` and `ServerConfig` structs. TOML file at `~/.config/bzr/config.toml`. Multiple named servers with a default. Uses `AuthMethod` and `ApiMode` from `types/common.rs`; `AuthMethod` (`Header`/`QueryParam`) persisted per-server.
- **`error.rs`** — `BzrError` enum (thiserror) with 14 variants: `Http`, `Config`, `Api`, `Io`, `TomlParse`, `TomlSerialize`, `XmlRpc`, `NotFound`, `HttpStatus`, `InputValidation`, `Deserialize`, `Auth`, `DataIntegrity`, `Other`. Each variant has a distinct `exit_code()` and `error_type()`. `Result<T>` type alias.
- **`output/`** — `mod.rs` is a re-export facade (decouples commands from output internals). `formatting.rs` holds formatting primitives (`print_formatted`, field helpers). `result_types.rs` holds mutation result types (`ActionResult`, `ResourceKind`, `MembershipResult`, etc.). Per-resource submodules (`bug.rs`, `comment.rs`, `attachment.rs`, `product.rs`, `classification.rs`, `user.rs`, `group.rs`, `field.rs`, `server.rs`, `config.rs`) each handle one domain type. Uses `tabled` for tables, `colored` for status colors.
- **`commands/`** — Each network command submodule has an `execute()` function taking `(action, server, format, api)` and following the pattern: load config → resolve auth → connect client → call API → print output. Two exceptions: `config.rs` is synchronous (local I/O only, no server/api params) and `whoami.rs` has no action enum (no subcommands). `shared.rs` provides `connect_client()` (config load + auth detection + client construction). `flags.rs` handles Bugzilla flag syntax parsing. See `src/commands/mod.rs` for the complete list of command modules.

### Conventions

- `#[expect(clippy::print_stdout)]` is used to allow `println!` in command modules and success messages, since `print_stdout` is denied project-wide.
- Logging uses `tracing` (not println). Verbosity: `-v`=info, `-vv`=debug, `-vvv`=trace. `RUST_LOG` env var overrides.
- URLs are sanitized via `safe_url()` in debug logs to avoid leaking API keys in query params.
- Tests use `wiremock` for HTTP mocking. Unit tests are in `#[cfg(test)] mod tests` within each source file. Integration tests live in `tests/integration.rs` and functional tests in `tests/functional/`. All API tests require `#[tokio::test]` (the runtime is tokio). Test modules use `#[expect(clippy::unwrap_used)]` to allow `.unwrap()` in tests.
- Clippy pedantic is enabled with strict rules (see `[lints.clippy]` in Cargo.toml). `unwrap_used` is denied, `expect_used` and `allow_attributes` are warned.
- CLI reference documentation lives in `docs/bzr-cli.md`. When adding a new command, update that file.
