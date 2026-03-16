# AGENTS.md

This file provides guidance to AI coding agents when working with code in this repository.

## What is bzr?

A Rust CLI for interacting with Bugzilla REST API servers. Supports bugs, comments, attachments, and multi-server configuration. Inspired by the GitHub CLI (`gh`).

## Build & Development Commands

```bash
cargo build                        # Debug build
cargo build --release              # Release build
cargo test                         # Run all tests
cargo fmt                          # Format code
cargo clippy -- -D warnings        # Lint (warnings are errors)
make lint                          # Format + clippy in one step
make setup                         # Full dev environment setup
cargo install --path .             # Install locally
```

Pre-commit hooks run `cargo fmt` and `cargo clippy` on commit, `cargo test` on push.

## Architecture

The codebase follows a layered CLI pattern with clear separation:

```
src/
├── main.rs          # Entry point: parses CLI, dispatches to commands
├── cli.rs           # CLI structure via clap derive (Cli, Commands, *Action enums)
├── client.rs        # BugzillaClient: HTTP client wrapping the Bugzilla REST API
├── config.rs        # Config: TOML file at ~/.config/bzr/config.toml
├── error.rs         # BzrError enum (thiserror) + Result type alias
├── output.rs        # OutputFormat (Table/Json) + print_* display functions
└── commands/
    ├── bug.rs       # Bug subcommands (list, view, search, create, update)
    ├── comment.rs   # Comment subcommands (list, add)
    ├── attachment.rs # Attachment subcommands (list, download, upload)
    └── config_cmd.rs # Config subcommands (set-server, set-default, show)
```

**Request flow:** `main.rs` parses args into `Cli` struct → matches on `Commands` enum → calls `commands/*.rs::execute()` → which loads `Config`, builds `BugzillaClient`, calls API methods, and formats output via `output.rs`.

**Key design decisions:**
- `BugzillaClient` authenticates via `X-BUGZILLA-API-KEY` header (set once at client construction)
- All API methods are async (tokio runtime)
- Attachments are base64-encoded/decoded in `client.rs`
- `output.rs` handles both table (using `tabled` + `colored`) and JSON output formats
- Config supports multiple named servers with a default; first server added auto-becomes default

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` (derive) | CLI argument parsing |
| `reqwest` (rustls-tls) | HTTP client |
| `serde` / `serde_json` | Serialization |
| `tokio` | Async runtime |
| `toml` | Config file format |
| `tabled` | Table output formatting |
| `colored` | Terminal color output |
| `thiserror` | Error type derivation |
| `base64` | Attachment encoding |
| `dirs` | Platform config directory |
