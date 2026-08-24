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
make test                          # Run tests (quiet output; failures still print details)
make test-verbose                  # Run tests with full cargo output (or: VERBOSE=1 make test)
cargo fmt                          # Format code
cargo clippy -- -D warnings        # Lint (warnings are errors)
make lint                          # Format + clippy in one step
make setup                         # Full dev environment setup
make functional-test-all           # Run functional tests against real Bugzilla containers
cargo install --path .             # Install locally
```

Git hooks: `make install-hooks` installs a pre-commit hook (`cargo fmt --check` + `cargo clippy`) and a pre-push hook (`make test`, the quiet suite). Run `make setup` to install everything, including hooks.

Test output selection for agent loops: `make test` runs quiet by default —
a summary line per suite, with failing tests still printing their captured
output and a full failure summary — which is the right default for routine
verification. Use `make test-verbose` (or `VERBOSE=1 make test`) only when
diagnosing a failure that needs the full per-test listing or compilation
detail; only the exact value `VERBOSE=1` enables it. Note that `VERBOSE` is
a generic name: a shell where `VERBOSE=1` is exported for some other tool
will make `make test` verbose unexpectedly, so prefer the unambiguous
`make test-verbose`.

Iteration loop for agent runs: while writing or fixing tests, scope the run —
`make test-one T=<name-substring>` runs only the matching tests, and
`make test-fast` runs unit tests only (`--lib`, skipping the integration
suite). Reserve full `make test` for pre-commit verification. Never invoke
bare `cargo test`: it prints hundreds of per-test lines that pollute agent
context and bypasses the quiet default.

## Architecture

Layered CLI pattern: `main.rs` parses args → `lib.rs::dispatch()` matches the
`Commands` enum → delegates to resource modules under `commands/` → those load
`Config`, resolve auth, build `BugzillaClient`, call the API, and format output.

### Key modules

- **`cli/`** — clap derive structs split into per-resource submodules. `mod.rs` defines `Cli`, `Commands`, and re-exports all `*Action` enums. Per-resource files (`bug.rs`, `comment.rs`, `attachment.rs`, `config.rs`, `product.rs`, `field.rs`, `user.rs`, `group.rs`, `server.rs`, `classification.rs`, `component.rs`, `template.rs`, `query.rs`) each define one action enum.
- **`client/`** — `mod.rs` defines `BugzillaClient`,
  `BugzillaClientConfig`, client construction, shared dispatch helpers, and
  cross-resource helpers. `request.rs`, `response.rs`, and `transport.rs` own
  the shared HTTP request/response pipeline. `resources/` is the REST resource
  layer (`bug.rs`, `attachment.rs`, `comment.rs`, `product.rs`, `user.rs`,
  `group.rs`, `component.rs`, `classification.rs`, `field.rs`, `server.rs`).
  `auth/` handles auth detection (split into `whoami.rs` and `valid_login.rs`
  probing strategies with `mod.rs` as orchestrator). `version.rs` handles
  version detection and API mode determination. Public data types live in
  `types/`.
- **`tls/`** — TLS client construction and trust policy. `mod.rs` builds the
  reqwest clients used by the connection layer, `fingerprint.rs` formats
  SHA-256 pins, `verifier.rs` enforces pin and issuer checks, and `tofu.rs`
  handles first-use pin capture. Config syntax validation stays in
  `validation/`; TLS modules consume already-validated policy.
- **`credentials/`** — Credential-source resolution for inline API keys,
  environment-backed keys, and keyring-backed keys. Command/config code decides
  whether credentials are required; this module turns a `ServerConfig`
  credential source into the secret material used by connection setup.
- **`validation/`** — Shared input validators that are not owned by one command
  or runtime subsystem, including date parsing, sort order construction, and TLS
  pin string parsing. Prefer adding cross-command value-shape validation here
  rather than making persisted config or CLI parsing depend on implementation
  modules.
- **`bugzilla_auth.rs` / `http.rs`** — Low-level transport helpers. Auth header
  and query parameter names, request auth application, API-key redaction, and
  request timeout constants live here so REST, XML-RPC, auth detection, and logs
  do not duplicate security-sensitive strings.
- **`xmlrpc/`** — XML-RPC protocol and resource adapter used by `BugzillaClient`
  when `ApiMode` is `XmlRpc` or Hybrid fallback selects XML-RPC. The command
  layer does not talk to XML-RPC directly; it stays behind the client boundary.
- **`config/`** — configuration subsystem for `~/.config/bzr/config.toml`.
  `model.rs` owns the persisted domain model (`Config`, `ServerConfig`,
  credential source types, saved templates, and saved queries). `store.rs`
  owns path resolution (`--config`/`BZR_CONFIG`/XDG default), advisory locking,
  unvalidated reads, validation-on-load, atomic persistence, stale temp cleanup,
  and permissions hardening. Multiple named servers share one default; per-server
  auth/API/TLS detection state is persisted here.
- **`error.rs`** — `BzrError` enum (thiserror) with 18 variants: `Http`, `Config`, `Api`, `Io`, `TomlParse`, `TomlSerialize`, `XmlRpc`, `NotFound`, `HttpStatus`, `InputValidation`, `Deserialize`, `Auth`, `DataIntegrity`, `BatchPartialFailure`, `Keyring`, `PinMismatch`, `IssuerChanged`, `MidAirCollision`. Each variant has a distinct `exit_code()` and `error_type()`. `Result<T>` type alias.
- **`output/`** — `mod.rs` is deliberately *not* a re-export facade: command modules import each writer from its owning leaf module so unused facade exports don't accumulate as output formats change. `formatting.rs` holds formatting primitives (`write_formatted`, `write_json_family`, field helpers). `result_types.rs` holds mutation result types (`ActionResult`, `ResourceKind`, `MembershipResult`, etc.) and the shared `write_result`/`write_saved`/`write_count` helpers. `writers.rs` defines the `Writers` stdout/stderr bundle. Per-resource writers live under `output/resources/` (`bug.rs`, `comment.rs`, `attachment.rs`, `product.rs`, `classification.rs`, `component.rs`, `user.rs`, `group.rs`, `field.rs`, `server.rs`, `config.rs`, `template.rs`, `query.rs`), each handling one domain type. Uses `tabled` for tables, `colored` for status colors.
- **`commands/`** — Resource command submodules expose async
  `execute(action, &CommandContext, &mut Writers)` entry points (where `Writers`
  bundles stdout/stderr). `lib.rs::dispatch()` builds one `CommandContext` for
  the invocation and passes it across command boundaries; the context carries
  server, output format, API mode, dry-run, confirmation, inline-server,
  config-path, timeout, and retry settings. Network commands follow the
  pattern: load config → resolve auth → connect client → call API → print
  output. Local-only commands under `commands/config/` use the same context
  boundary but do local I/O, while `whoami.rs` has no action enum. Cross-cutting command
  infrastructure lives under `commands/runtime/`: `runtime::invocation` owns
  per-command state, capability policy, and inline server configuration;
  `runtime::input` owns payload loading, Bugzilla URL import, attachment input,
  and flag parsing; `runtime::interaction` owns confirmation prompts and
  `$EDITOR`; `runtime::search` owns query execution, field projection, and
  paging; `runtime::shared` owns connection and body-source helpers; and
  `runtime::mutation` owns the shared admin create/update driver. See
  `src/commands/mod.rs` for the complete list of command modules.

### Conventions

- User-facing CLI output should go through `Writers` (`w.out`/`w.err`) and the
  output helpers, which ultimately use `writeln!(io::stdout(), …)` /
  `writeln!(io::stderr(), …)` rather than `println!`/`eprintln!`. This keeps
  command output testable with `test_helpers::capture_stdout`, which redirects
  fd 1 via `dup2`; `println!` goes through cargo test's per-test stdout capture
  and bypasses fd 1. Discard the `Result` with `let _ = writeln!(…)` if the
  function isn't already in a context that allows `.expect()`.
- Direct `println!`/`eprintln!` is reserved for non-CLI process integration:
  Cargo directives in `build.rs` and `xtask` status output. Do not add
  `#[expect(clippy::print_stdout)]` or `#[expect(clippy::print_stderr)]` in
  `src/`; use `Writers` for user output and `tracing` for diagnostics.
- Logging uses `tracing` (not println). Verbosity: `-v`=info, `-vv`=debug, `-vvv`=trace. `RUST_LOG` env var overrides.
- URLs are sanitized via `safe_url()` in debug logs to avoid leaking API keys in query params.
- Tests use `wiremock` for HTTP mocking. Unit tests live in sibling `<name>_tests.rs` files linked from each source file via `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;` — inline `mod tests { ... }` blocks are **not permitted** in `src/` and `make check-test-layout` enforces this in CI. The reasons are twofold: (1) SonarCloud's CPD scanner is configured to exclude `**/*_tests.rs` (see `sonar-project.properties`), so test-fixture boilerplate stays out of the duplication metric without forcing bad abstractions; (2) it keeps production source files focused. There is no size threshold — even tiny test mods get their own sibling. Test-helpers modules used only by tests follow the same separation (e.g. `src/client/test_helpers.rs`). Sibling files start with the appropriate file-level inner attribute (`#![expect(clippy::unwrap_used)]`, or whatever the original outer attribute was — including combined forms like `#![expect(clippy::unwrap_used, clippy::panic)]`); siblings whose tests don't trigger the lint omit it. Integration tests live in `tests/integration.rs` and functional tests in `tests/functional/`. All API tests require `#[tokio::test]` (the runtime is tokio). See `docs/superpowers/specs/2026-05-05-test-sibling-migration-design.md` for full rationale.
- **Functional tests are mandatory, not optional.** The harness under
  `tests/functional/` runs the real `bzr` binary against real Bugzilla containers
  (Docker or podman, auto-detected) and is the **only** tier that catches REST
  response-shape and server-behavior mismatches that `wiremock` fixtures cannot —
  fixtures only prove "given this shape, we parse it right", never that the shape
  matches a live server. Two rules, no exceptions:
  - **Any user-facing change** — a new or changed command, subcommand, flag,
    output shape, exit code, or published schema — **must add or extend a phase
    script** under `tests/functional/phases/` exercising it against a real
    container (cover the credentialless path too, when the command supports it).
    A feature without a functional test is incomplete; do not open the PR.
  - **Any other change** — refactors, internal-only changes, dependency bumps —
    **must get a full functional run green before the PR is opened**:
    `make functional-test-all` (all supported Bugzilla versions) or, at minimum,
    `make functional-test` (default version). If Docker/podman is genuinely
    unavailable in your environment, say so explicitly in the PR body and state
    which tier you could not run — never silently skip it. CI does not gate these
    (they need containers), so the discipline is on the author, not the pipeline.
- Clippy pedantic is enabled with strict rules (see `[lints.clippy]` in Cargo.toml). `unwrap_used` is denied, `expect_used` and `allow_attributes` are warned.
- CLI reference documentation lives in `docs/bzr-cli.md`. When adding a new command, update that file.
- `CHANGELOG.md` is **generated from conventional commits at release time**
  (`git cliff` via `cliff.toml` + `tools/generate-changelog-section.sh`; do not
  hand-edit release sections). The contract: an entry documents a change to the
  compiled `bzr` artifact — commands, flags, output shapes, exit codes,
  config/TLS/auth semantics, or the embedded skills payload. Repo plumbing
  (website, CI, README, dev tooling) never qualifies. To land in the notes, at
  least one commit in the PR must carry a conventional type of `feat`, `fix`,
  `perf`, or `refactor` with an explicit non-infra scope (infra scopes:
  `site`, `release`, `ci`, `adr`, `plan`, `spec`, `changelog`, `build`,
  `test`, `docs`, `deps`). Unscoped product commits are excluded by design —
  scope them. Commit bodies are ignored; put anything the notes must say into
  the subject line.
- System package dependencies (e.g. `libdbus-1-dev` for the `keyring` default feature) must be installed in **every** place the crate is built, not just one workflow. When adding or changing a native dependency, audit all of:
  - `.github/workflows/ci.yml` (native host jobs)
  - `.github/workflows/release.yml` (native host jobs **and** the QEMU container for `powerpc64le`)
  - `.github/workflows/publish-crates.yml` (native host job)
  - `Cross.toml` `pre-build` hooks for every `cross`-driven target (adds the target-arch dev package via Debian multiarch: `apt-get install libdbus-1-dev:$CROSS_DEB_ARCH`)
  Do **not** work around a missing system dep by disabling default features on exotic targets — that silently ships a degraded binary. Make the build environment correct on every target instead.
