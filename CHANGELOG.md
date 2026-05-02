# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Documentation

- Top-level (`bzr.1`), per-resource (`bzr-bug.1`, `bzr-config.1`,
  ...), and per-action pages for `bug`, `config`, and `query`
  (`bzr-bug-create.1`, `bzr-config-set-server.1`,
  `bzr-query-save.1`, ...) now carry full descriptive prose instead
  of one-line summaries. Each page describes
  auth/permission expectations, required vs. optional inputs, output
  shape, representative examples, exit-code semantics where
  non-trivial, and cross-references to related pages. Per-action
  detail for the remaining resources and per-flag detail will follow
  in subsequent passes; see
  `docs/plans/2026-05-02-cli-doc-expansion.md`.
- Added `docs/dev/cli-doc-style.md` documenting the conventions for
  clap doc comments (2-space example indent, ASCII-only,
  `verbatim_doc_comment` on items with examples).
- Added a `cli_doc_long_about_coverage` test that asserts every
  doc-expanded subcommand has a multi-paragraph `long_about` distinct
  from its short `about`. Catches regressions where a future edit
  collapses a long doc back to a single line.

## [0.2.0-rc3] - 2026-05-01

### Added

- Manpages: `bzr` and one roff page per subcommand, auto-generated from
  the clap-derive CLI tree by a new `xtask` workspace member. Run
  `make man` locally; release tarballs ship them under `man/man1/`.
- `.deb` packages for amd64, arm64, and ppc64el; `.rpm` packages for
  x86_64, aarch64, ppc64le, and s390x. Built and attached to GitHub
  releases by `release.yml`, with `lintian`/`rpmlint` checks (warn-only)
  and Docker install smoke-tests for the x86_64 packages.
- Homebrew tap support via `randomparity/homebrew-tap`: pre-built
  binaries on macOS arm64 and Linux x86_64/aarch64; Intel Mac falls
  back to a source build with a build-time `rust` dep. The
  `update-homebrew.yml` workflow auto-bumps the formula on each
  stable release.
- `SHA256SUMS` file attached to each GitHub release, covering every
  tarball, zip, `.deb`, and `.rpm` artifact. Verify a download with
  `sha256sum --check --ignore-missing SHA256SUMS`.

### Changed

- MSRV raised to 1.88 (was 1.84). Existing crates.io installs continue
  to work with `cargo install bzr --locked`; users building without
  `--locked` may need to upgrade their toolchain.

### Documentation

- README `Installation` section restructured around the package
  manager that fits each platform (Homebrew, `.deb`, `.rpm`, tarball,
  `cargo install`). Manual page setup promoted from a sub-bullet of the
  keychain section to a top-level subsection.
- `docs/bzr-cli.md` exit-code table now lists exit code 13 (TLS pin
  mismatch / issuer changed). Command tree for `config set-server`
  includes `--tls-ca-cert`, `--tls-pin-sha256`, `--tls-pin-now`, and
  `--tls-pin-clear`. Configuration file format example now shows
  `tls_insecure`, `tls_ca_cert`, and `tls_pin_sha256` per-server fields.
- `docs/skills.md` exit-code list expanded to mention 0–13 with a
  pointer to the full table.
- `CLAUDE.md` updated to reflect 18 `BzrError` variants (was 14),
  rename of `connect_client` to `connect_and_configure`, and addition
  of `template.rs` / `query.rs` to the `cli/` module list.

## [0.2.0-rc2] - 2026-04-28

> Same-day re-spin of rc1 to fix a defect found during smoke testing
> (PR #102: eager TLS probe on the cached connection path). Both rc1
> and rc2 carry the 2026-04-28 date because both were cut on the same
> calendar day.

### Added

- TLS certificate pinning with trust-on-first-use (TOFU) prompt flow.
  New CLI flags on `bzr config set-server`: `--tls-ca-cert <path>` to pin
  a CA certificate, `--tls-pin-sha256 <hex>` to pin a leaf SPKI
  fingerprint, `--tls-pin-now` to probe the server and prompt before
  storing the observed pin, and `--tls-pin-clear` to remove an existing
  pin.
- Per-server config fields `tls_ca_cert`, `tls_pin_sha256`,
  `tls_pin_issuer` persisted in `~/.config/bzr/config.toml`.
- New error variants `PinMismatch` and `IssuerChanged` with distinct
  exit codes and actionable hints (`--tls-pin-now`, `--tls-ca-cert`).
- `bzr config show` displays configured CA cert path and pin fingerprint
  for each server.

### Changed

- Internal: migrated PEM parsing from `rustls-pemfile` to
  `rustls-pki-types` `PemObject` API. No user-visible change.
- Internal: `commands/bug.rs` split into per-action submodules;
  `xmlrpc/mod.rs` split into `call`, `fault`, `parsing`. No public API
  change.
- Internal: test modules moved to sibling `_tests.rs` files for
  SonarCloud copy-paste-detection exclusion.

### Fixed

- HTTP error messages now walk the reqwest error source chain, so TLS
  diagnostics surface even when wrapped in transport errors.
- `bzr bug search --from-url` strips shell-escaped backslashes from URL
  arguments pasted from terminals that quote them.
- TLS verification is now eager at connect time on the fully-cached
  path. Previously, when both `auth_method` and `api_mode` were cached
  for a server, `connect_and_configure` returned a client without
  probing TLS, so untrusted-CA errors and pin rotations only surfaced
  from the first real API call — bypassing the TOFU and rotation
  prompts entirely. Cert-detection probes also no longer follow HTTP
  redirects, so prompts always describe the configured URL itself
  rather than a redirect target.

## [0.1.2] - 2026-04-27

### Added

- `bzr bug search --from-url <buglist.cgi URL>` to import Bugzilla web
  searches into the CLI, with automatic URL parsing and parameter extraction
- `bzr bug search --save-as <name>` to save searches as named queries in one
  step
- `bzr query run --server <name>` to run a saved query against a different
  server than it was saved from
- `source_url`, `server`, and `raw_params` fields displayed in `bzr query show`
  output
- Auto-suggested save name derived from the URL's `known_name` parameter
  when using `--from-url`
- Raw Bugzilla query parameters passthrough (`raw_params`) for search terms
  the CLI doesn't model directly; forces REST API mode when present

### Changed

- Unified field-mapping tables into a single `FIELD_MAPPINGS` constant,
  eliminating duplicated field name/alias definitions
- `to_search_params` now delegates to `into_search_params`, removing duplicate
  conversion logic
- Extracted `into_search_params` (owned version) to avoid unnecessary cloning
  in `query run`
- Reduced cognitive complexity and test duplication for SonarCloud compliance
- Dependency bumps: tokio 1.52.1, clap 4.5.61, actions/checkout v6,
  SonarSource/sonarqube-scan-action v7, rustls-webpki security patch
  (RUSTSEC-2026-0104)

### Fixed

- `bug search --from-url` now uses the parsed server name instead of the
  default server
- Limit override logic corrected so CLI `--limit` takes precedence over the
  saved query limit
- Credentials are sanitized from URLs before storing `source_url`
- Save is deferred until the search succeeds (no orphaned queries on failure)
- Guard ordering and fallthrough issues in field accessor match arms
- CI: SonarQube analysis skipped on Dependabot PRs to avoid token permission
  failures

## [0.1.1] - 2026-04-06

### Added

- OS keychain-backed API key storage via the `keyring` crate (enabled by
  default; opt out with `--no-default-features`)
- `bzr config set-keyring`, `unset-keyring`, and `migrate-to-keyring`
  subcommands for managing credentials in the system keychain
- Environment-variable-backed API keys as a secure alternative to plaintext
  config (`BZR_<SERVER>_API_KEY`)
- Keyring credential source reporting in `bzr config show`
- Troubleshooting guide for keyring and credential issues (`docs/troubleshooting.md`)
- SonarCloud static analysis in CI with coverage reporting
- Dependabot configuration with grouped updates and cooldowns

### Changed

- MSRV raised to 1.84; `resolver = "3"` enabled for MSRV-aware dependency resolution
- `clap` capped below 4.6 to preserve MSRV compatibility
- Expanded test coverage across client transport, auth fallback, dispatch,
  query, and CLI paths; added functional autodetect coverage

### Fixed

- `quick-xml` 0.39 compatibility in the XML-RPC parser (#63)
- `migrate-to-keyring` race condition guarded; test-only hook gated to debug builds
- `clippy` 1.94 lint regressions in tests
- Dependency supply-chain hardening in CI

## [0.1.0] - 2026-04-02

### Added

- Bug management: list, search, view, create, clone, update, batch-update, and view change history
- Personal bug workflows with `bug my`
- Comments: list and add comments, with `$EDITOR` integration for composing
- Comment tags: add, remove, and search comment tags
- Attachments: list, download, upload, and update file attachments with auto-detected MIME types
- Flag support for bugs and attachments (set, request, clear)
- Products: list, view, create, and update
- Components: create and update product components
- Classifications: view classification details
- Fields: look up valid values for bug fields
- Users: search, create, and update users
- Groups: list members, add/remove users, view, create, and update groups
- Server diagnostics: `whoami` and `server info` commands
- Multi-server configuration with named servers and defaults
- Local bug templates for reusable creation defaults
- Saved queries with runtime overrides for `limit`, `fields`, and `exclude-fields`
- JSON and human-readable table output formats
- Header-based and query parameter authentication with auto-detection
