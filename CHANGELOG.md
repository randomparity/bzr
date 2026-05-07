# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `bzr bug list --summary <substring>` filters bugs by a substring
  match against the Summary field across all bug states. This is
  the structured counterpart to `bzr bug search`, which uses
  Bugzilla's quicksearch syntax and defaults to OPEN bugs only.
  Useful when the matching bug may be CLOSED or RESOLVED — a
  scenario where quicksearch silently returns no results.
- `bzr bug view` now accepts multiple IDs and a `--permissive` flag
  for partial results when some bugs are inaccessible. Single-ID
  invocation behavior — table and JSON output — is unchanged. With
  `--permissive`, per-bug failures (NotFound, Bug.get fault codes
  100/101/102) are surfaced as inline `Bug #N — UNAVAILABLE` blocks
  in table output or entries in the `failed` array in JSON output;
  session-wide failures (transport, auth, security, server internal,
  unrecognized API codes) still bail. JSON output for multi-ID is a
  wrapped `{"bugs": [...], "failed": [...]}` object regardless of
  `--permissive`. Closes #156.
- `bzr bug create` now reads the description from `$EDITOR` when
  `--description`, `--description-file`, and piped stdin are all
  absent and stdin is a TTY. The buffer uses a
  `git commit -v`-style sentinel divider; the first non-empty line
  above the divider becomes the summary and the rest becomes the
  description. `--summary` is optional when the editor flow is
  active. Closes #159.
- `bzr bug create --description-file <PATH>` reads the description
  from a UTF-8 file (mutually exclusive with `--description`).
  Missing or non-UTF-8 paths exit with code 7. Closes #160.
- `bzr attachment upload --comment <BODY>` posts a comment alongside the
  attachment in a single API call. Folded into the underlying
  `Bug.add_attachment` request so the attachment and comment share a
  creation timestamp. Closes #165.
- `bzr attachment upload --is-patch` marks the attachment as a patch
  at upload time, removing the previous two-call pattern (upload then
  `bzr attachment update --is-patch true`). When `--content-type` is
  not supplied, `--is-patch` defaults the type to `text/plain`,
  matching `bzl-attachment-add`. The read-side `Attachment` struct
  also exposes `is_patch` so `bzr attachment list --json` includes the
  field. Closes #166.
- `bzr bug list`, `bzr query save`, and `bzr query run` accept
  `--created-since <DATE>` and `--changed-since <DATE>` filters
  for Bugzilla's `creation_time` and `last_change_time` fields.
  Inputs are ISO 8601 (`YYYY-MM-DDTHH:MM:SS[Z|±HH:MM]`) or a bare
  `YYYY-MM-DD` (canonicalized to `T00:00:00Z`); malformed values
  exit 7 before any network call. `bzr query run` accepts the same
  flags as per-invocation overrides matching the existing
  `--limit` / `--fields` convention. `bzr query show` and the
  one-line `bzr query list` summary surface both filters when set.
  Closes #157.
- `bzr bug list`, `bzr query save`, and `bzr query run` accept
  eight new field filters: `--whiteboard`, `--target-milestone`,
  `--version`, `--op-sys`, `--platform`, `--resolution`,
  `--qa-contact`, and `--url`. All eight are repeatable for OR
  within a field, AND across fields, and accept `!`-prefix to
  invert. Substring fields (`--whiteboard`, `--url`) use
  `notsubstring` for negation; the other six use `notequals`.
  `bzr query show` lists each set field in its detail view.
  Legacy `buglist.cgi` URL parameter names (`status_whiteboard`,
  `rep_platform`, `bug_file_loc`) are recognized by `--from-url`.
  Closes #158.
- `bzr bug update` gains list-mutation flags for four
  string-typed fields, mirroring the existing
  `--blocks-add` / `--depends-on-add` convention:
  `--keywords-add` / `--keywords-remove`,
  `--cc-add` / `--cc-remove`,
  `--groups-add` / `--groups-remove`,
  `--see-also-add` / `--see-also-remove`. Comma-separated values
  for the first three; `--see-also-*` accepts one URL per flag
  instance (URLs may legitimately contain commas). Closes #163.
- `bzr bug update --comment <BODY>` (or `--comment-file <PATH>`)
  posts a comment atomically with the field changes — a single
  `Bug.update` REST call instead of a separate `bzr comment add`.
  `--comment-private` marks the comment private. Mutually exclusive
  with each other; `--comment-private` requires one of the body
  flags. Empty / whitespace-only bodies are rejected (exit 7).
  Closes #161.
- `bzr attachment upload --comment-private` marks the comment posted
  alongside the attachment as private. Bugzilla's `Bug.add_attachment`
  endpoint does not accept a privacy flag on the embedded comment, so
  the upload is followed by a targeted `Bug.update` that flips the
  newly created comment's `is_private` to `true`. Requires `--comment`
  or `--comment-file`. Closes #170.

### Fixed

- `bzr bug search` no longer falls back to XML-RPC when the REST
  search returns an empty result for a free-text query
  (quicksearch or summary). Previously, an empty REST result with
  any "filter" set — including a quicksearch term — would trigger
  an opportunistic XML-RPC retry, which on servers with slow or
  unresponsive XML-RPC could hang for the full 30s request
  timeout before erroring. Free-text predicates are evaluated by
  the same server-side parser regardless of transport, so empty
  results are authoritative; the retry now fires only when
  structured filters (product, component, status, etc.) are
  present, which is the original asymmetry-papering use case.
  Fixes #152.
- The opportunistic XML-RPC fallback for empty REST results is
  now capped at 8s independently of the per-request timeout.
  When the cap fires, the empty REST result is returned with a
  warning suggesting `--api rest` or `api_mode = "rest"` for
  servers where XML-RPC is consistently slow.

### Changed

- The `<query>` argument on `bzr bug search` and the help text
  for `--api` have been clarified to call out quicksearch's
  "open bugs only" default. Prepend the bare token `ALL` to a
  quicksearch query to include closed/resolved bugs (broader
  scope: summary + description + comments); for a Summary-
  field-only match across all states, use the new
  `bzr bug list --summary <text>` instead.
- `bzr bug create --template <NAME>` no longer auto-applies the
  template's `description` field as a fallback when no explicit
  description source is supplied outside the editor flow. The
  template description is now used as the editor buffer's pre-fill
  only. Pass `--description`, `--description-file`, or pipe a
  description via stdin to use a non-template body. Refs: #159, #160.
- `bzr bug history --since` and `bzr comment list --since` now
  validate their input client-side via the same shared validator
  introduced for `--created-since` / `--changed-since`. Malformed
  dates exit 7 instead of being forwarded to the server. Bare
  dates (`YYYY-MM-DD`) are now canonicalized to `T00:00:00Z` on
  the wire; previously the bare form was passed through verbatim.
  Refs: #157.
- `bzr bug update` list-mutation validation errors now identify
  the offending flag. An empty or whitespace-only value supplied
  to any of `--keywords-add/-remove`, `--cc-add/-remove`,
  `--groups-add/-remove`, or `--see-also-add/-remove` produces
  `<flag>: list value cannot be empty or whitespace-only` instead
  of a bare message. Closes #174.
- `Comment` JSON output now includes `attachment_id` (set when the
  comment was created alongside an attachment, otherwise `null`).
  Existing fields are unchanged; the field is also populated by the
  XML-RPC fallback path. Refs: #170.

## [0.3.0] - 2026-05-05

### Fixed

- `bzr attachment list` and `bzr comment list` now accept
  alternate response envelopes returned by some Bugzilla 5.0.x
  deployments (e.g. IBM LTC) that omit the `bugs` key and return
  `attachments` / `comments` at the root. Previously these
  commands hard-failed with `missing field 'bugs'`. Affects #135.
- `BzrError::Deserialize` errors now include a redacted ~512-char
  preview of the response body, so envelope mismatches and
  unexpected response shapes can be diagnosed without re-running
  with `-vv`. Any `Bugzilla_api_key=` value echoed back by the
  server is redacted in the preview.
- `bzr comment list` now returns private comments on Bugzilla
  deployments where the REST endpoint silently truncates them
  (observed on Bugzilla 5.0.x). In Hybrid API mode (the default
  for newly-detected servers), `bzr` now uses XML-RPC
  `Bug.comments` for comment listing, which returns the full set
  including private entries; it falls back to REST only when the
  server doesn't expose `xmlrpc.cgi`. No configuration change
  required. Affects #125.
- `bzr attachment list` and `bzr attachment download` now return
  private attachments on the same Bugzilla 5.0.x deployments —
  REST silently filters them under non-admin API-key auth, while
  XML-RPC `Bug.attachments` returns the full set. Hybrid mode now
  routes both `attachment list` and `attachment download` through
  XML-RPC with REST fallback only on transport failure, mirroring
  the comment-list fix from #125. Affects #133.
- `make setup` now requires Rust 1.88.0 (matching `Cargo.toml`'s
  `rust-version`) and prints a `rustup update stable && rustup
  default stable` upgrade hint when the local toolchain is older.
  Previously the threshold was 1.84.0, so `make setup` would pass
  the version check on rustc 1.85-1.87 and then fail later when
  `cargo install cargo-llvm-cov` rejected the toolchain. Fixes #138.
- The XML-RPC parser now accepts both `<boolean>1</boolean>` and
  `<int>1</int>` wire shapes for the `is_private` field on
  `Bug.comments` responses and the `is_active` field on `Group.get`
  responses. Bugzilla 5.0.x deployments encode the same flag using
  either shape depending on the field; previously the parser only
  recognized `<boolean>`, so int-shaped values were silently
  classified as `false`. Attachment fields already accepted both
  shapes via the same helper. Fixes #140.

### Added

- XML-RPC `Bug.comments` support in the embedded XML-RPC client,
  used by Hybrid-mode comment fallback and directly when a server
  is configured with `api_mode = "xmlrpc"`.
- XML-RPC `Bug.attachments` support in the embedded XML-RPC
  client, covering both bug-scoped (`ids: [bug_id]`) and
  attachment-by-ID (`attachment_ids: [id]`) lookups. Used by
  Hybrid-mode `attachment list` and `attachment download`
  fallback (#133) and directly when `api_mode = "xmlrpc"`.
- `bzr comment add --private` flag, sets `is_private: true` on the
  posted comment.
- `bzr attachment upload --private` flag, sets `is_private: true`
  on the uploaded attachment.

## [0.2.0] - 2026-05-04

### Added

- TLS certificate pinning with trust-on-first-use (TOFU) prompt flow.
  New CLI flags on `bzr config set-server`: `--tls-ca-cert <path>` to
  pin a CA certificate, `--tls-pin-sha256 <hex>` to pin a leaf SPKI
  fingerprint, `--tls-pin-now` to probe the server and prompt before
  storing the observed pin, and `--tls-pin-clear` to remove an existing
  pin.
- Per-server config fields `tls_ca_cert`, `tls_pin_sha256`, and
  `tls_pin_issuer` persisted in `~/.config/bzr/config.toml`.
- New error variants `PinMismatch` and `IssuerChanged` with distinct
  exit codes and actionable hints (`--tls-pin-now`, `--tls-ca-cert`).
- `bzr config show` displays configured CA cert path and pin
  fingerprint for each server.
- Manpages: `bzr` and one roff page per subcommand, auto-generated
  from the clap-derive CLI tree by a new `xtask` workspace member.
  Run `make man` locally; release tarballs ship them under
  `man/man1/`. The `.deb`, `.rpm`, and Homebrew install paths place
  manpages on `MANPATH` automatically.
- `.deb` packages for `amd64`, `arm64`, and `ppc64el`; `.rpm`
  packages for `x86_64`, `aarch64`, `ppc64le`, and `s390x`. Built
  and attached to GitHub releases by `release.yml`, with
  `lintian`/`rpmlint` checks (warn-only) and Docker install
  smoke-tests on the `x86_64` packages.
- Homebrew tap support via
  [`randomparity/homebrew-tap`](https://github.com/randomparity/homebrew-tap):
  pre-built binaries on macOS arm64 and Linux x86_64/aarch64; Intel
  Mac falls back to a source build with a build-time `rust` dep. The
  tap is auto-bumped on each stable release.
- `SHA256SUMS` file attached to each GitHub release, covering every
  tarball, zip, `.deb`, and `.rpm` artifact. Verify a download with
  `sha256sum --check --ignore-missing SHA256SUMS`.
- Installer scripts (`install.sh`, `install.ps1`) for one-line
  installation from GitHub Releases, with SHA-256 verification
  against the published `SHA256SUMS` file. Hosted at the `main`
  branch URL for always-current installs, and as release assets
  pinned to each tag for reproducibility.

### Changed

- MSRV raised to 1.88 (was 1.84). Existing crates.io installs
  continue to work with `cargo install bzr --locked`; users
  building without `--locked` may need to upgrade their toolchain.

### Fixed

- HTTP error messages now walk the reqwest error source chain, so
  TLS diagnostics surface even when wrapped in transport errors.
- `bzr bug search --from-url` strips shell-escaped backslashes from
  URL arguments pasted from terminals that quote them.
- TLS verification is now eager at connect time on the fully-cached
  path. Previously, when both `auth_method` and `api_mode` were
  cached for a server, `connect_and_configure` returned a client
  without probing TLS, so untrusted-CA errors and pin rotations only
  surfaced from the first real API call -- bypassing the TOFU and
  rotation prompts entirely. Cert-detection probes also no longer
  follow HTTP redirects, so prompts always describe the configured
  URL itself rather than a redirect target.
- Release page no longer attaches stray manpage `.1` files. The
  `release` job's artifact download filters with `pattern: bzr-*-*`
  so the internal `bzr-manpages` artifact (used by the build matrix
  to bundle pages into tarballs and packages) is not pulled into the
  release upload set. `SHA256SUMS` correspondingly lists only the
  published archives and packages.
- Windows release zip layout now wraps contents in a top-level
  `bzr-<tag>-<target>/` directory, matching the Unix tarball.

### Documentation

- Every man page and every `--help` long-form output across `bzr`
  and its subcommands carries full descriptive prose: command-level
  pages describe auth/permission expectations, required vs. optional
  inputs, output shape, representative examples, exit-code semantics
  where non-trivial, and cross-references to related pages; per-flag
  detail covers every option that conflicts with another flag, gates
  behavior elsewhere, accepts a structured value, has a non-obvious
  default, or supports env-var/stdin fallback.
- README `Installation` section restructured around the package
  manager that fits each platform (Homebrew, `.deb`, `.rpm`, the
  one-line installer, manual tarball, `cargo install`).
- `docs/bzr-cli.md` exit-code table lists exit code 13 (TLS pin
  mismatch / issuer changed). Command tree for `config set-server`
  includes the new `--tls-*` flags. Configuration file example shows
  `tls_insecure`, `tls_ca_cert`, and `tls_pin_sha256` per-server
  fields.
- New `docs/dev/cli-doc-style.md` documenting clap doc-comment
  conventions (2-space example indent, ASCII-only,
  `verbatim_doc_comment` on items with examples).
- New `cli_doc_long_about_coverage` test that asserts every
  doc-expanded subcommand has a multi-paragraph `long_about`
  distinct from its short `about`.

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
