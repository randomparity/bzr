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
cargo install --path .             # Install locally
```

Pre-commit hooks run `cargo fmt` and `cargo clippy` on commit, `cargo test` on push.

## Architecture

Layered CLI pattern: `main.rs` parses args → matches `Commands` enum → delegates to `commands/*.rs::execute()` → which loads `Config`, resolves auth, builds `BugzillaClient`, calls API, and formats output.

### Key modules

- **`cli.rs`** — clap derive structs (`Cli`, `Commands`, `BugAction`, `CommentAction`, `AttachmentAction`, `ConfigAction`). All CLI arguments defined here.
- **`client.rs`** — `BugzillaClient` wraps reqwest for Bugzilla REST API. Contains all API data types (`Bug`, `Comment`, `Attachment`, `SearchParams`, etc.) and response wrappers. Handles base64 encoding/decoding for attachments.
- **`auth.rs`** — Auto-detects whether a server supports header auth (`X-BUGZILLA-API-KEY`) or query param auth (`Bugzilla_api_key`). Probes `rest/whoami` first (Bugzilla 5.1+), falls back to `rest/valid_login` (requires `--email`). Caches detected method in config.
- **`config.rs`** — `Config` and `ServerConfig` structs. TOML file at `~/.config/bzr/config.toml`. Multiple named servers with a default. `AuthMethod` enum (`Header`/`QueryParam`) persisted per-server.
- **`error.rs`** — `BzrError` enum (thiserror) with `Http`, `Config`, `Api`, `Io`, `TomlParse`, `TomlSerialize`, `Other` variants. `Result<T>` type alias.
- **`output.rs`** — `OutputFormat` (Table/Json). `print_*` functions for bugs, comments, attachments. Uses `tabled` for tables, `colored` for status colors.
- **`commands/`** — Each submodule has an `execute()` function that takes the action enum, optional server name, and output format. All follow the same pattern: load config → resolve auth → build client → call API → print output.

### Conventions

- `#[expect(clippy::print_stdout)]` is used to allow `println!` in output.rs and command success messages, since `print_stdout` is denied project-wide.
- Logging uses `tracing` (not println). Verbosity: `-v`=info, `-vv`=debug, `-vvv`=trace. `RUST_LOG` env var overrides.
- URLs are sanitized via `safe_url()` in debug logs to avoid leaking API keys in query params.
- Tests use `wiremock` for HTTP mocking. Tests are in `#[cfg(test)] mod tests` within each source file (no separate `tests/` directory).
- Clippy pedantic is enabled with strict deny rules (see `[lints.clippy]` in Cargo.toml). `unwrap_used` is denied, `expect_used` is warned.
