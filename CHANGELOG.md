# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.8.2] - 2026-08-23

### Security

Security assessment: No publicly identified runtime vulnerabilities in bzr were fixed in this release.

### Added

- bzr skills install installs every bundled bzr agent skill from the offline,
  release-matched payload embedded in the running binary. Callers choose an explicit
  --global or --project PATH scope (--project . selects the current repository)
  and an agent layout with --agent standard|bob|codex|claude|all.
  Re-running updates managed skills, while foreign folders and symlinked destination
  components are refused without modification. JSON and NDJSON expose the selected
  scope, canonical project path, destinations, and sorted installed skills. The
  standalone installers remain available for no-binary use and fetch main by
  default, with BZR_SKILL_REF for a pinned payload. (#523)
- New agent skill bzr-dry-run-confirm (under agent-skills/) teaching the
  preview-then-write pattern for mutations: run any supported mutation with the
  global --dry-run flag, parse and verify the printed payload with jq, then
  re-run the same command without --dry-run to commit. Covers bug
  create/update/clone/resolve/close/reopen/dup, product/component/user/group
  create+update, batch previews via --from-json, exit-code triage, and
  --yes batch-confirm semantics. No new CLI verbs; it codifies an
  existing-capability workflow and cross-references the mutation skills.
  The installers and skills table list it. (#460)
- New agent skill bzr-release-tracking (under agent-skills/) teaching the
  milestone/release workflow: enumerate what is open on a target milestone with
  repeatable/negatable filters and --paginate, detect bugs that drifted into
  or out of the release via bzr bug history, rebalance assignees and CC,
  publish a release-ready digest, then close the milestone out. No new CLI
  verbs; it composes bzr bug list, bzr bug history, and bzr bug update.
  The installers and skills table list it. (#463)

## [0.8.1] - 2026-08-05

### Security

- Server-supplied error text is now API-key redacted before it is displayed.
  A deployment that quotes the request URL back in an error message echoes the
  credential with it, and `bzr` printed that verbatim to stderr and into the
  `--json` / `--output ndjson` error object — from where it travels into CI
  logs, terminal transcripts, and pasted bug reports. `Bugzilla_api_key=<key>`
  in a Bugzilla API error message or in an HTTP error-status body now renders
  as `Bugzilla_api_key=[REDACTED]`, matching what transport errors already did.
  The redaction applies at the single point every output format renders
  through, so the human and JSON paths cannot diverge. Error text without the
  marker is unchanged, and codes, statuses, and exit codes are untouched. Every
  occurrence in a message is masked, not just the first — an error page echoing
  the request URI more than once was the shape most likely to leak.

  Configured keys echoed without a parameter marker, such as `invalid api key
  <key>`, are now masked too when the key is at least eight bytes long. The
  floor avoids replacing short, common strings throughout otherwise useful
  server messages. Percent-encoded markers (`Bugzilla_api_key%3D<key>`) are
  also recognized. (#505, #509)

- Diagnostic logging now applies the same API-key redaction before recording
  REST and XML-RPC response bodies, body previews, or server error messages.
  Verbose output (`-vv` / `-vvv` / `RUST_LOG`) no longer bypasses the error
  display seam when a server echoes `Bugzilla_api_key=<key>`. (#511)

### Fixed

- HTTP error-status bodies are now capped to a 512-byte, UTF-8-safe preview
  when the error is constructed. Oversized proxy and portal responses no
  longer remain attached to the error or flood stderr and structured error
  output; truncated bodies end with an ellipsis. REST and XML-RPC use the same
  limit, while status codes, error types, exit codes, and API-key redaction are
  unchanged. (#512)

- `bzr bug view` no longer reports `bug not found` for a bug the server
  declined to return for another reason. Two paths discarded the server's own
  explanation and substituted an empty result, which surfaced as exit 2 —
  intermittently, because both triggers are server-load dependent, so retrying
  the same command eventually succeeded. (#504)

  - A Bugzilla error payload arriving with HTTP 200 was downgraded to a warning
    whenever the response merely *contained* a known data key, even an empty
    one. A body such as `{"error":true,"code":102,"message":"You are not
    authorized…","bugs":[]}` was swallowed, leaving nothing to return. A data
    key must now hold content — an empty array, an empty object and `null` no
    longer count — so the error is surfaced. An error alongside genuinely
    populated data is still used as data and logged as a warning, unchanged.
  - The retry through the search endpoint (used when a direct lookup fails with
    Bugzilla's internal error 100500) dropped the original error when the
    search came back empty. Bugzilla's search path omits rows the caller may
    not see rather than faulting, so an empty retry is "the fallback could not
    answer", not "no such bug". The original error is now re-surfaced. In
    Hybrid mode this also repairs the XML-RPC fallback, which keys off the
    100500 code and could not previously be reached from this path — an
    affected lookup can now succeed instead of failing.

  The same tightening fixes a silent mutation failure: `bzr bug update` checks
  the response body because some deployments report a *rejected* mutation with
  HTTP 200, and an error payload carrying an empty `bugs: []` defeated that
  check — an update the server refused was reported as success. Rejections of
  that shape now fail with the server's code and message.

  **Behaviour change:** a restricted bug that previously exited 2 (`not_found`)
  now exits 4 (`api`) carrying the server's `api_code` and message. Scripts
  branching on exit 2 to mean "absent or restricted" should branch on 2 or 4.
  Exit 2 now means the server itself returned an empty result with no error.
  See ADR-0015.

  **Known trade-off:** on a deployment that attaches `error: true` to every
  response as a standing warning, a query that legitimately matches nothing now
  fails rather than returning an empty list — such a response is
  indistinguishable from "an error, and no data". ADR-0015 accepts this
  knowingly; the fix is server-side.

### Documentation

- The bundled agent skills now document the comment privacy flags, which they
  had never named. `bzr comment add --private` and `bzr bug update --comment` /
  `--comment-file` / `--comment-private` are now in the command reference, and
  `bzr-triage-bug` shows both forms. An agent working from the skills had no way
  to learn the flag name and guessed; the reported guess was
  `comment add --is-private true`, which has never existed (the real flag is a
  bare `--private`, and the value-taking `--is-private` on `attachment update`
  was removed in #312). The attachment privacy flags were already documented —
  only the comment paths were stale. The wording also states that omitting the
  flag posts publicly, because the reported fallback was to drop it and post
  anyway, which discloses the body. (#503)

- Agent-skill docs are now checked against the binary by a new
  `agent-skills/tests/skills-flag-check.sh`, run by `make skills-test`. Every
  long flag the skills show on a `bzr <group> <verb>` invocation must be a real
  flag of that command; it currently covers 356 flag mentions across 37
  commands. Unlike `flag-drift-check.sh` — which is bidirectional against the
  exhaustive command tree in `docs/bzr-cli.md` — this check is one-directional,
  because the skills are a curated cheat-sheet and an omitted flag is not an
  error. A documented flag that does not exist is, since it makes an agent emit
  a command that cannot parse. (#503)

- The bundled agent skills are refreshed for the 0.8.x CLI surface. They had
  shipped through 0.7.0 and 0.8.0 still declaring they were authored against
  0.6.1-dev, so the surface added in those releases was missing from the
  reference an agent reads: `bug links` (with `--recursive`/`--depth`/
  `--relation`), `server capabilities`, the compound `bug create` flags
  (`--with-comment`, `--with-comment-file`, `--with-attachment`,
  `--attachment-description`), the flattened `bug history --json` records, and
  the `server_name`/`auth_mode` keys on `whoami --json`. The published schema
  list, the `schema_version` literal in the `jq` recipes (`0.6.0` → `0.6.1`),
  and the `bug clone` exit-11 case are corrected too. `bzr-reference` now
  carries the whole error surface — every `type`, its exit code, and the keys
  it adds — where previously only `bzr-triage-bug` had a partial list and the
  canonical reference had none. Both skills state that a restricted bug exits 4
  (`api`), not 2, so triage code no longer reports a bug it cannot see as one
  that does not exist, and both distinguish a clap *usage* error (exit 2, plain
  text, no `error` object even under `--json`) from the `not_found` exit 2.
  `agent-skills/VERSION` and the four "authored against" strings move to
  0.8.1-dev in the same change. (#507)

- A new `agent-skills/tests/version-check.sh`, run by `make skills-test`,
  enforces the version contract stated in `agent-skills/README.md`. It checks
  all five version sites against the `[package]` version in `Cargo.toml` —
  `VERSION` plus every "authored against `bzr` X.Y.Z" claim in the README and
  the skills — rather than `VERSION` alone, which would leave the four prose
  claims free to drift on their own. Each of the four sites the contract names
  is required to carry a claim, so rewording a heading cannot silently drop a
  version stamp; the rest of the payload is scanned as well, so a claim written
  into a new skill is covered the day it is written. (#507)

## [0.8.0] - 2026-07-28

### Added

- `--json` / `--output ndjson` error objects (on stderr) now carry optional,
  variant-specific keys beyond `type`/`message`/`exit_code`, so an agent can
  branch on structured data instead of parsing the message: `field`/`value` for
  input validation, `bug_id`/`last_change_time`/`if_match_token` for a mid-air
  collision, `resource`/`identifier` for not-found, `status` for HTTP errors,
  `api_code` for API faults, `succeeded`/`failed` for a partial batch, and
  `server`/`expected`/`actual` for TLS trust changes. The published
  `bzr schema error` is extended and `SCHEMA_VERSION` bumps to `0.6.1` (additive).
  The error stays on stderr; stdout and exit codes are unchanged. See ADR-0014.
  (#482)

- Agent-skills installers (`agent-skills/install.sh`, `install.ps1`) can install
  without a repo clone: piped via `curl … | sh` / a PowerShell scriptblock, they
  download the skill payload from GitHub when no local `skills/` directory is
  found. `BZR_SKILL_REF` (branch/tag/commit) and `BZR_SKILL_TARBALL_URL` (full
  URL or local path) override the source; default is `main`. See ADR-0013. (#480)

### Fixed

- `bzr config unset-keyring` no longer fails when an *unrelated* server in the
  config is structurally invalid (for example a hand-edited entry defining both
  `api_key` and `api_key_env`). It took a validating read of the whole config
  before doing its work, so one bad entry blocked unsetting a credential on any
  other server. It now reads an unvalidated snapshot, matching `config
  remove-server` and `config rename-server`, so the CLI can still repair a
  config it cannot fully load. Because that write skips validation, the command
  now prints a `warning:` on stderr when the file it just rewrote is still not
  loadable, instead of reporting plain success. (#278)

- `bzr config unset-keyring` and `config remove-server` now resolve the OS
  keychain service/account from the config read under the write lock, not from
  the advisory snapshot taken before it. A `config set-keyring` landing between
  the two could otherwise point the delete at the previous entry, destroying the
  superseded secret while the config recorded the new one.

- `bzr config unset-keyring` and `config remove-server` now delete the OS
  keychain entry only *after* the config write commits. Previously the secret
  was destroyed first, so a failed write (read-only config directory, full disk,
  lock contention) left the config still referencing a credential that no longer
  existed — `unset-keyring` on a server that was still keyring-backed, or a
  still-configured server with no retrievable key. `config rename-server`
  already ordered its keychain work this way.

- Config writes now warn when the config file was world-readable *before* the
  write. The permission check ran after the atomic rename, which always leaves
  the file `0o600`, so an exposed config was silently hardened with no notice
  that its credentials had been readable. Affects every command that writes the
  config; hardening behavior itself is unchanged.

## [0.7.0] - 2026-06-27

### Added

- Compound `bug create`: file a bug together with its first comment and one or
  more attachments in a single invocation. Flag form adds `--with-comment` /
  `--with-comment-file`, `--with-attachment` (repeatable) with index-paired
  `--attachment-description`; `--from-json` gains `comment` and `attachments`
  keys on both the object and array forms. Comment/attachment inputs are
  validated before the bug is created (no half-created batch). On any
  post-create sub-step failure the created bug ID is printed and the command
  exits 11 (`BatchPartialFailure`) rather than rolling back — the ID is the
  recovery handle. New `compound-create-result` published schema. See ADR-0012.
  (#458)

- `--progress ndjson` global flag streams structured progress events on stderr
  (one NDJSON object per line) during long operations: `page`/`done` for
  paginated fetches (`bug list`/`search --paginate`, `query run --paginate`) and
  `batch`/`done` for `bug create`/`update --from-json` array form, plus a
  terminal `error` event on dispatch failure (`done` is suppressed on a partial
  batch failure). stdout is unchanged; absent the flag behavior is identical.
  Opt-in, `ndjson`-only. See ADR-0011. (#462)

- `--fields` / `--exclude-fields` JSON projection now works on `comment list`,
  `attachment list`, `product list`/`view`, `component list`/`view`,
  `user search`, `group list-users`/`view`, `classification list`/`view`, and
  `field list`, matching the bug verbs' flag spelling. Field names are each
  verb's `--json` keys; the selection trims the `--json` / `--output ndjson`
  object (or each array element) to those keys, so agents can fetch only what
  they need. Table output is unaffected (the flags warn and are ignored there).
  An unknown field name exits 7. A new `validation/fields` helper centralizes the
  parsing/validation, guarded by per-type drift tests. See ADR-0010. (#455)

- New agent skill `bzr-bulk-triage` (under `agent-skills/`) teaching the
  streaming batch-triage pattern: stream a saved or ad-hoc query with
  `--paginate --output ndjson`, apply the read-before-write rule per row, and
  guard each mutation with a `--dry-run` preview before the real write (plus
  `--expect-unchanged-since` and bounded concurrency). No new CLI verbs; it
  codifies an existing-capability workflow and cross-references `bzr-triage-bug`
  and `bzr-search-report`. The installers and skills table list it. (#454)

- `bzr whoami --json` / `--output ndjson` now report two connection-metadata
  fields alongside the identity fields: `server_name` (the configured/inline
  server the identity resolved against — a named server's config key, or the
  literal `(inline)` for an inline `--server-url` connection) and `auth_mode`
  (`api_key` or `anonymous`). One `whoami --json` call now answers "which server
  am I, and how am I authenticated?". The existing identity fields (`id`, `name`,
  `real_name`, `login`) are unchanged (additive). Adds the `whoami` JSON Schema
  (`bzr schema whoami`). (#470)

- `bzr bug history <id> --json` / `--output ndjson` now emit flattened change
  records — one record per changed field (`when`, `who`, `field`, `old_value`,
  `new_value`, `comment_id`) instead of grouped entries with a nested `changes`
  array. A multi-field history entry expands to one record per field. `comment_id`
  is correlated from the bug's comments by author and timestamp (best-effort,
  null when no comment matches). The table output is unchanged. A `history`
  schema is published via `bzr schema history`. (#456)

- `bzr bug links <id>` prints a bug's relationship graph — one record per related
  bug across all six Bugzilla link types (`depends_on`, `blocks`, `dupe_of`,
  `duplicates`, `regressed_by`, `regressions`) as flat JSON/ndjson/table records
  with `relation`, `direction`, and `depth` fields. `--recursive --depth N`
  performs a bounded, cycle-safe breadth-first walk; `--relation <type>` filters
  to one relation. Read-only and works without an API key. (#453)

- `bzr server capabilities` dumps the connected server's capability surface as
  structured JSON (`--json`/`--output ndjson`): supported API transports and auth
  modes, status-transition summaries, custom field definitions, the
  attachment-size limit (bytes), and `supports_*` feature flags. Works without a
  saved config or API key; fields a stock server does not expose anonymously
  (`max_attachment_size`, `flag_types`) are reported as `null`. Adds the
  `server-capabilities` JSON Schema. (#457)

- A stable, versioned JSON output contract: every pretty `--json` response now
  carries a top-level `schema_version` string, and `bzr schema --json` reports
  the current contract version. Adds the `envelope` JSON Schema and a documented
  "JSON Output Stability" policy (patch = additive, minor = aliased rename,
  major = breaking) in `docs/bzr-cli.md`. (#464)

### Changed

- `bug clone` now exits 11 (`BatchPartialFailure`) when the "Cloned from bug #N"
  back-reference comment fails to post, instead of warning and exiting 0
  (TD-006). The bug is still created and its ID is printed; the non-zero exit
  signals the partial failure so scripted callers are not misled. (#458)

- `bug create --from-json` array results: `created` and `failed` are no longer
  disjoint. A created bug whose compound sub-step (comment/attachment) fails now
  appears in both — its ID in `created` and a `{index, bug_id, step, file,
  error}` entry in `failed`. Count filed bugs with `created`; detect any problem
  with `failed`. (#458)

- The bundled `bzr-setup` agent skill now teaches `bzr whoami --json` as the
  canonical one-call health check ("am I configured?") — config, auth, and
  server reachability in a single probe — instead of chaining `config show`,
  `server info`, and a probe-bug fetch. The new "Health check" section documents
  the failure modes (TLS, auth, connection), the public read-only fallback
  (`server info --json`), and cross-references `bzr-reference` for the TLS-trust
  flag surface and error envelope. No CLI changes. (#461)

- **BREAKING (`--json`):** pretty `--json` output is now wrapped in a versioned
  envelope `{"schema_version": "0.6.0", "data": <previous output>}`. Move `jq`
  recipes and deserializers from the old top level to `.data` (e.g. `.[].id` →
  `.data[].id`, `.assigned_to` → `.data.assigned_to`, `.count` → `.data.count`,
  multi-id `bug view` `.bugs[]` → `.data.bugs[]`). `--json` errors gain a
  top-level `schema_version` beside `error`. **`--output ndjson` is unaffected**
  — records stay bare (no `schema_version`), so it is the recommended stable
  shape for pinned line-oriented automation; read the version out of band via
  `bzr schema --json` or `bzr --version`. `bzr schema <name>` still prints the
  raw schema document verbatim (unenveloped). (#464)

- Internal: pin the local toolchain to the MSRV (1.89) via `rust-toolchain.toml`
  so contributor `cargo`/`make lint` runs match CI, which builds and lints at
  1.89. Catches MSRV-only failures before push. Use `cargo +stable` / a
  one-off override for newer-toolchain work.

- Internal: refreshed the `cargo-mutants` baseline after the recent
  restructure. Re-ran the whole-crate sweep, added behavior tests that kill
  the high-value missed mutants (parsers, command/CLI field-wiring, dispatch
  guards), refreshed `.cargo/mutants.toml` exclusions for equivalent or
  unreachable code, and updated `docs/dev/mutation-baseline.md`. No runtime
  behavior change. (#417)

- Internal: the clap-derived CLI action and argument types are now
  crate-internal (`pub(crate)`). The `bzr` library crate's public surface is
  reduced to `cli::Cli` and `dispatch`; CLI behavior, flags, and output are
  unchanged. (#419)

- Internal: the admin `product`, `component`, `group`, and `user` create/update
  handlers now share a single `runtime::mutation::run` driver for the common
  format/dry-run/connect/call/write flow. Request payloads, dry-run output,
  mutation output, and error behavior are unchanged. (#420)

## [0.6.0] - 2026-06-22

### Security

- Bump the transitive `openssl` dependency (via the `keyring` feature's
  `dbus-secret-service` on Linux) to 0.10.81, addressing CVE-2026-45784
  (GHSA-phqj-4mhp-q6mq): a potential out-of-bounds write in
  `CipherCtxRef::cipher_update_inplace` for AES-KW-PAD ciphers.

### Added

- Named and ad-hoc Bugzilla servers can be used without credentials for
  read-only commands. Writes and identity-derived commands now fail fast when
  no credential source is available. (#380)

- Ad-hoc `--server-url` runs now accept stateless TLS trust controls:
  `--server-tls-insecure`, `--server-tls-ca-cert`,
  `--server-tls-pin-sha256`, and session-only `--server-tls-pin-now`.
  These settings are never persisted to config. (#381)

- `bug my` now accepts the shared practical bug filters for product,
  component, priority, severity, created/changed dates, whiteboard, target
  milestone, version, operating system, platform, resolution, QA contact, and
  URL. (#382)

- `query update` now accepts `--from-url <URL>` to refresh an existing saved
  query from an updated Bugzilla `buglist.cgi` URL without delete-and-recreate.
  (#383)

- Bug templates now support the create metadata fields accepted by
  `bug create`: URL, whiteboard, target milestone, deadline, CC, keywords,
  groups, and flags. (#384)

- `bug clone` now copies URL, whiteboard, target milestone, and deadline from
  the source bug, and accepts create-time metadata overrides for URL,
  whiteboard, target milestone, deadline, CC, keywords, groups, and flags.
  (#385)

- `attachment upload` now accepts `--comment-file <PATH>` and the shared `-`
  stdin convention for `--comment` and `--comment-file`, while rejecting empty
  attachment comments consistently. (#386)

- `attachment download <ID> --out -` now streams a single attachment's raw bytes
  to stdout without writing a file or emitting formatted result output. (#387)

- `component update` now accepts `--product <PRODUCT> --component <COMPONENT>`
  and JSON `product`/`component` targets, resolving exact component names
  without a separate ID lookup. (#388)

- `bzr schema error` now publishes the JSON error envelope emitted on stderr
  under `--json` and `--output ndjson`. (#390)

- CLI docs, source help comments, schema descriptions, and bundled agent-skill
  docs are guarded against stale long flag names such as `--is-patch`,
  `--format json`, and `--ndjson`. (#391)

- Bundled bzr agent skills now teach the new CLI UX follow-ups: public
  read-only servers, stateless TLS trust flags, richer `bug my` filters,
  `query update --from-url`, richer templates and clone overrides, attachment
  stdin comments, single-attachment stdout downloads, and the error schema.
  (#392)

- `bug update` now accepts `--url` and `--target-milestone`, matching fields
  that `bug create` can already set. (#363)

- `bug resolve`, `bug close`, `bug reopen`, and `bug dup` now accept
  `--expect-unchanged-since`, matching `bug update`'s optimistic-concurrency
  guard. (#364)

- `bug update` now accepts `--from-json <PATH|->` structured input for object
  and array update payloads. (#365)

- `schema` now publishes `bug-create-input` and `bug-update-input` JSON
  Schemas for the structured payloads accepted by `--from-json`. (#366)

- `--dry-run` now previews product, component, user, and group create/update
  requests without writing. (#367)

- Product, component, user, and group create/update now accept object-shaped
  `--from-json <PATH|->` structured input with published schemas. (#369)

- `query run` now accepts `--count`, matching the count-only output shape used
  by bug search-backed commands. (#368)

### Changed

- `attachment update <ID>` and `comment tag <COMMENT_ID>` now reject no-op
  invocations before contacting Bugzilla and suggest the flags needed to make a
  change. (#389)

- Documented that `bug create` has no Bugzilla-backed idempotency-key support
  and that agents should inspect before retrying ambiguous create failures.
  (#370)

- Release artifacts now ship a SLSA provenance bundle (`*.intoto.jsonl`) as a
  release asset alongside each binary, in addition to the existing GitHub
  attestations API publication. The installer smoke tests now download and
  verify installers against the published `SHA256SUMS` before executing them
  instead of piping the download straight into a shell.

## [0.5.0] - 2026-06-21

### Changed

- **Breaking:** `bug close` and `bug reopen` now target stock Bugzilla 5.x
  statuses by default — `close` sets `VERIFIED` (was `CLOSED`) and `reopen` sets
  `CONFIRMED` (was `REOPENED`). Neither old status is part of the default
  Bugzilla workflow, so both verbs previously failed against a stock install
  with API error 51. A new `--status <STATUS>` flag on each verb overrides the
  default for installs that define custom statuses (e.g. `--status CLOSED`). The
  target status is validated against the server's status list before writing; an
  unknown status now exits 7 (input validation) listing the valid statuses,
  rather than reaching the server as the opaque API error. Matching is exact and
  case-sensitive. Scripts relying on the old `CLOSED`/`REOPENED` targets must
  pass `--status CLOSED` / `--status REOPENED`. (#349)

- **Breaking:** attachment boolean flags now use a uniform `--x` / `--no-x`
  presence grammar across `attachment upload` and `attachment update`, so the
  same concept uses the same flag everywhere. `attachment upload` keeps
  `--private` and renames `--is-patch` to `--patch` (each gaining a `--no-*`
  form). `attachment update` replaces the value-taking `--obsolete <BOOL>` /
  `--is-patch <BOOL>` / `--is-private <BOOL>` with `--obsolete` / `--no-obsolete`,
  `--patch` / `--no-patch`, and `--private` / `--no-private`; passing neither
  leaves the property unchanged. The old value-boolean forms (e.g.
  `--obsolete true`) and the `--is-*` names no longer parse. Per the project's
  replace-don't-deprecate policy no compatibility aliases are kept; update any
  scripts to the new flags. (#312)

- XML-RPC response object identifiers are extracted with checked conversions
  instead of `as u64` sign-loss casts: a primary-key `id` (bug, comment,
  attachment, user, group) that is missing or negative is now reported as a
  malformed response rather than silently wrapping or becoming `0`. Secondary
  fields stay tolerant — `comment.bug_id`/`attachment.bug_id` and the counters
  `comment.count`/`attachment.size` still default to `0` when absent, so the
  off-spec flat-comment envelope returned by some Bugzilla 5.0.x servers (which
  may omit `bug_id`) keeps parsing.

- Auth detection now surfaces transport/TLS failures instead of silently
  defaulting to header auth. When the `rest/whoami` or `rest/valid_login`
  probes cannot reach the server, the underlying error is propagated so the
  connection layer can classify it — so a TLS certificate error on the *first*
  connection to a server now triggers the trust-on-first-use (TOFU) and
  pin-rotation prompts rather than being masked as a successful detection.

### Removed

- The redundant `bzr whoami show` subcommand. `show` was the only action and
  was exactly equivalent to bare `bzr whoami`, which invited "what's the
  difference?" confusion. Use `bzr whoami` (the form the docs already
  promote). `whoami` now takes no subcommand. (#323)

### Documentation

- Refreshed the bundled agent skills (`agent-skills/skills/`) for the 0.5.0 CLI
  surface: corrected the stale "there is no `bzr component list`" claims (the
  `component list`/`component view` and `classification list` read verbs now
  exist), documented the global flags (`--output ndjson`, `--dry-run`,
  `-y`/`--yes`, `--timeout`/`--retry`, `--config`, and the stateless
  `--server-url`/`--server-api-key-env`/`--server-email` trio), the new
  `completion` and `schema` top-level commands, `bug` sort/pagination/count and
  extended filters, `create` field-parity flags and `--from-json`, `update
  --expect-unchanged-since`, the convenience verbs `resolve`/`close`/`reopen`/
  `dup`, `comment add --body-file`, `attachment view` and the `--x`/`--no-x`
  boolean grammar, `config remove-server`/`rename-server`, and `template
  update`/`query update`. Bumped the skill-set `VERSION` to track the tool
  version (0.5.0). (#355)

- The command tree in `docs/bzr-cli.md` is now drift-checked against the binary
  in CI. A new `agent-skills/tests/flag-drift-check.sh` (run alongside the
  existing verb-level `drift-check.sh`) compares every command-specific long
  flag the binary exposes against that command's block in the tree, both
  directions, and fails the build on a mismatch. Fixed the existing drift it
  surfaced — the tree was missing flags such as `bug view --web`/`--permissive`,
  `bug update --dupe-of`/`--comment`/`--cc-add`/`--keywords-add`/`--groups-add`/
  `--see-also-add` (and `*-remove`), `bug list`/`query` search filters
  (`--resolution`, `--version`, `--op-sys`, `--platform`, `--whiteboard`,
  `--target-milestone`, `--qa-contact`, `--url`), `bug create --description-file`,
  `comment add --private`, `attachment download --bug`/`--out`/`--out-dir`,
  `attachment upload --comment`/`--comment-private`, and `config set-keyring`/
  `migrate-to-keyring --service`/`--account`. (#306)

- Recorded the decision to keep `bzr` as the command name despite the
  historical collision with GNU Bazaar (retired in 2025; its successor Breezy
  uses `brz`), with rationale in `docs/decisions/0001-bzr-command-name.md` and
  a README note plus shell-alias workaround. (#322)

### Added

- `bug view` and `attachment view` now surface Bugzilla `flags`, and `bug view`
  also surfaces `target_milestone` — fields the REST API returns but which were
  previously dropped. Each flag renders as `name` + status token with the
  requestee in parentheses (e.g. `review+`, `needinfo?(qa@example.com)`). Both
  fields are selectable via `--fields` (e.g. `bug view <id> --fields id,flags`)
  and appear in `--json` (flags as an array, always present; `target_milestone`
  as a string). The unset target-milestone sentinel `---` and empty flag lists
  are suppressed in table detail but kept verbatim in JSON. (#351)

- NDJSON output via `--output ndjson` (or `BZR_OUTPUT=ndjson`): list/array
  results print one compact JSON value per line and single objects print as one
  compact line — the streaming shape for agents and `jq -c`. An empty list emits
  no lines; the `bug list`/`search` truncation note goes to stderr so stdout
  stays one clean record per line. Existing `table`/`json` shapes are unchanged.
  (#305)

- `bzr schema [NAME]` publishes checked-in JSON Schemas (draft 2020-12) for the
  `--json` output of each command family — the read-resource objects
  (`bug`, `comment`, `attachment`, `product`, `component`, `classification`,
  `user`, `group`, `field-value`) and the mutation/result envelopes
  (`action-result`, `batch-result`, `batch-create-result`, `multi-bug-view`,
  `tag-result`, `membership-result`, `count-result`, `download-result`,
  `upload-result`, `config-result`, `search-result`, `dry-run-result`). Agents
  can validate output against a contract instead of branching over the per-
  command shape differences. Local command (no network); run without a name to
  list the schemas. A drift test validates each schema against the real
  serialized output so the published contract stays honest. (#305)

- Pagination for the search-backed commands (`bug list`, `bug search`, `bug my`,
  `query run`): `--offset <N>` skips leading matches for manual paging, and
  `--paginate` loops internally past `--limit` to retrieve every match (the path
  for "process all matching bugs" workflows). Because Bugzilla's search API
  returns no total, truncation is detected by over-fetching one row past
  `--limit`: a truncated table window prints a "more available" footer, and the
  JSON window writes the same note to stderr (the stdout array shape is
  unchanged). `--offset`/`--paginate` are mutually exclusive and cannot be
  combined with `--count` (exit 7). (#302)

- `bug create --from-json <PATH|->` structured input. Files one or more bugs
  from a JSON object (one bug) or array (one bug per element, with the
  partial-failure model and exit 11), so an agent that already models a bug as
  an object can submit it directly instead of flattening it into shell flags.
  Keys match the create flag names; unknown keys are rejected (exit 7) so a typo
  fails fast, and undesigned `cf_*` custom-field writes (#283) stay out of this
  path. Explicit CLI flags override the corresponding JSON field, applied to
  every array element. Mutually exclusive with `--template`; bypasses the
  `$EDITOR` flow. `bug update` and other resources are tracked as follow-up.
  (#307)

- Inline / ad-hoc server via global `--server-url`, `--server-api-key-env`, and
  optional `--server-email`. Defines an ephemeral server for a single
  invocation, so a one-off query or a fully stateless agent run can target a
  Bugzilla instance that was never written to any config file — nothing is read
  from or written to `config.toml`. The API key is sourced from the named
  environment variable (never a literal flag, keeping the secret out of the
  process argument list). `--server-url` requires `--server-api-key-env` and
  conflicts with `--server`; it pairs with `--config` for sandboxed runs. (#314)

- `bug update --expect-unchanged-since <TIMESTAMP>` optimistic-concurrency guard.
  Pass the `last_change_time` from a preceding `bug view`; before writing, bzr
  re-reads each target bug and refuses the update (exit 14, a new distinct
  collision code) if its `last_change_time` no longer matches, so a
  read-modify-write agent will not silently clobber a concurrent edit. The check
  is client-side — Bugzilla's REST `Bug.update` exposes no atomic
  compare-and-set (the `check_collision` proposal was never merged; the web UI's
  `delta_ts` guard is not in the API) — so a narrow check-then-write window
  remains. With multiple IDs, all are checked first and any mismatch aborts the
  whole batch before any write. (#320)

- Confirmation gate for large batch bug mutations, with a global `-y`/`--yes`
  bypass. A `bug update`/`resolve`/`close`/`reopen` targeting more than 10 bugs
  now prompts for confirmation at an interactive terminal before writing, so a
  mistyped ID list cannot mass-mutate bugs unnoticed. Non-interactive runs
  (piped stdin, agents) auto-bypass and are never blocked; `--yes` skips the
  prompt explicitly. (#313)

- Global `--dry-run` flag for bug mutations. Previews `bug create`, `update`,
  `clone`, `resolve`, `close`, `reopen`, and `dup` without writing: it resolves
  and validates the request, then prints the would-be payload and affected bug
  IDs as `{"resource":"bug","action":"dry-run","ids":[...],"changes":{...}}`
  instead of calling the write API, exiting 0 on a valid request. Useful as a
  safety net for humans and a pre-flight validation step for agents before an
  irreversible batch `update`. `clone` still reads the source bug to build the
  preview. Using `--dry-run` on any non-mutation command exits 7. (#308)

- `bzr query update <NAME>` and `bzr template update <NAME>` edit a saved query
  or template in place: a supplied flag replaces that field, an omitted flag
  leaves it unchanged, and `--clear <FIELD>` (repeatable, names matching the long
  flags) resets a field. Previously the only way to change one field was delete +
  re-create (re-specifying everything). A no-op call is rejected, as is an update
  that would leave a template with no fields or a query with no filters (exit 7).
  (#315)

- Global `--timeout <SECS>` (and `BZR_TIMEOUT`) overrides the per-request
  timeout (default 30s) for slow or distant servers; the 10s connect timeout is
  unchanged. Global `--retry <N>` (default 0, max 10) retries transient failures
  with exponential backoff that honors a `Retry-After` header. To avoid
  duplicate writes, 429 and connect failures are retried for any operation while
  5xx responses and read timeouts are retried only for safe reads (GET/HEAD),
  never for writes (create, update, comment). Retries are off by default;
  exhausted retries surface the usual network error (exit code 5). (#311)

- `--count` on `bzr bug list`, `bzr bug search`, and `bzr bug my` prints only the
  number of matching bugs — an integer (table) or `{"count": N}` (JSON) — for
  dashboards, triage gates, and agent branching. It fetches ids only and lifts
  the row limit (`limit=0`), so the count reflects all matches (bounded by the
  server's max-results setting) rather than a single page; `bug my` reports the
  distinct total deduped across the active categories. (#317)

- Convenience verbs over `bzr bug update` for the common state transitions:
  `bzr bug resolve <ID...> [--as <RESOLUTION>]` (defaults to `FIXED`),
  `bzr bug close <ID...> [--as <RESOLUTION>]`, `bzr bug reopen <ID...>`, and
  `bzr bug dup <ID> <TARGET>`. Each is thin sugar that builds the equivalent
  `Bug.update` and inherits its batch (multi-ID, partial-failure exit 11) and
  `--comment` / `--comment-file` / `--comment-private` behavior. (#309)

- `bzr bug create` gains field flags shared with `bug update`: `--alias`,
  `--url`, `--whiteboard`, `--target-milestone`, `--deadline`, `--cc`,
  `--keywords`, `--groups`, and `--flag`. They are sent in the same
  `Bug.create` call, so a bug and its metadata are filed in one API round-trip
  instead of a create-then-update two-step. The list flags accept
  comma-separated, repeatable values; `--flag` uses Bugzilla flag syntax;
  `--deadline` takes `YYYY-MM-DD` and rejects malformed input with exit 7. (#301)

- `bzr bug view <ID> --web` opens the bug's page (`show_bug.cgi?id=<ID>`) on the
  active server in the default browser, the `gh issue view --web` affordance.
  Resolves the URL from local config with no network call or authentication, so
  it works even before credentials are set. When stdout is not a terminal or no
  display is available (headless / SSH without X), it prints the URL and exits 0
  instead of opening a browser, which keeps it safe for scripts, pipes, and
  agents. Multiple IDs open (or print) one page each. (#310)

- `--sort <FIELD>` / `--order asc|desc` on `bug list`, `bug search`, `bug my`,
  and `query run` control result ordering (mapped to Bugzilla's `order`), with
  a `bug_id` tiebreaker for deterministic ties. Absent `--sort`, results now
  default to a stable `bug_id` order so identical runs are reproducible — note
  this means `bug search` no longer defaults to relevance ranking; pass `--sort`
  to choose another order. `query save --sort` persists an order into the saved
  query; `query run --sort` overrides it. (#303)

- Global `--config <PATH>` flag and `BZR_CONFIG` environment variable select an
  alternate `config.toml` for reads and writes. Precedence: `--config` >
  `BZR_CONFIG` > `$XDG_CONFIG_HOME/bzr/config.toml` (or the platform config
  dir). Sandboxes CI, throwaway agent runs, and per-profile configs without
  manipulating `$HOME`/`$XDG_CONFIG_HOME`. (#304)

- `bzr component list --product <P>` and `bzr component view <product> <component>`
  make components directly discoverable (ID, name, description, default
  assignee, active flag) instead of only via `product view`. Both read the
  product's component set; `--json` emits the component array / object. (#316)

- `bzr attachment view <ATTACHMENT_ID>` shows a single attachment's metadata
  (summary, bug, file name, content type, size, flags, creator, timestamps)
  without downloading its bytes. On REST the `data` field is excluded
  server-side via `exclude_fields`, so inspecting a large attachment is cheap;
  `data` is also omitted from all attachment `--json` output. (#318)

- `bzr classification list` enumerates the server's classifications (ID, name,
  description, product count; full objects under `--json`). Bugzilla has no
  bulk classification endpoint, so names come from the `classification` field's
  legal values and each is fetched for detail. When classifications are
  disabled (only `Unclassified` exists) bzr prints a note to stderr. (#319)

- `bzr config remove-server <NAME>` deletes a server alias (and its OS-keychain
  entry, if any). Removing the current default is refused while other servers
  remain; removing the only server clears the default. `bzr config
  rename-server <OLD> <NEW>` renames an alias, preserving credentials —
  including moving the keychain secret when it is stored under the default
  (server-name) account — and updates `default_server` if it pointed at the old
  name. Both emit the standard mutation JSON (`removed` / `renamed`). (#300)

- `bzr completion <bash|zsh|fish|powershell|elvish>` prints a shell completion
  script to stdout, generated from bzr's live clap command tree so it always
  matches the installed binary's subcommands and flags. README and
  `docs/bzr-cli.md` document a one-line install per shell. (#299)

- `bzr comment add` now accepts `--body-file <PATH>` to read the comment body
  from a UTF-8 file, matching `bug create --description-file` and
  `bug update --comment-file`. A path of `-` reads from stdin. `--body` and
  `--body-file` are mutually exclusive.

- Bundled agent skills for driving `bzr` from AI coding agents, with a
  runtime-free installer. Five skills (`bzr-reference`, `bzr-setup`,
  `bzr-file-bug`, `bzr-triage-bug`, `bzr-search-report`) live under
  `agent-skills/`; `agent-skills/install.sh` (POSIX) and `install.ps1` (Windows)
  copy selected skills into agent skill directories (`~/.agents/skills`,
  `~/.claude/skills`) with a `.bzr-skill-managed` ownership sentinel. CI runs a
  command-surface drift check against the built binary. Replaces the previous
  `docs/skills.md` and `docs/bob-skills.md` guides.

### Fixed

- `--body`, `--description`, and `--comment` (and their `--*-file` companions)
  now honour the `-` convention, reading the body from stdin instead of posting
  or sending a literal `-`. Previously `bzr comment add <id> --body -` posted a
  literal `-`. (#295)

## [0.4.4] - 2026-06-12

### Packaging

- The Homebrew formula now ships real bottles (precompiled, poured binaries)
  for Apple Silicon macOS, x86_64 Linux, and arm64 Linux. Homebrew pours a
  bottle straight into the Cellar instead of running the formula's build
  phase, so `brew install`/`brew upgrade` no longer enters Homebrew's build
  sandbox. This sidesteps a recent Homebrew change (build-phase
  `deny_read_home`, which raises on any `$HOME` entry containing characters
  like `(`/`)`) that broke source installs for some users, and makes installs
  faster. `release.yml` builds the bottles on per-OS runners after the formula
  is published, uploads them to the GitHub release, and adds the `bottle do`
  block to the tap formula. Intel macOS still builds from source (no prebuilt
  Intel binary is produced).
- The x86_64 and arm64 Linux release binaries now statically link libdbus
  (new opt-in `vendored-keyring` Cargo feature, enabled only for these targets
  in CI). A Homebrew bottle is poured as-is and cannot declare a system-library
  dependency, so the binary that feeds the Linux bottles must carry no runtime
  `libdbus-1.so` requirement. Local builds and the `.deb`/`.rpm` packages are
  unchanged — they keep dynamic linking, with the system dependency declared in
  package metadata as before.

## [0.4.3] - 2026-06-10

### Security

- API client redirects are now confined to the configured host. The client
  attaches the API key as the custom header `X-BUGZILLA-API-KEY`, which reqwest
  does not strip on a cross-host redirect (it only strips its own known-sensitive
  headers). A malicious or compromised server could therefore return a redirect
  to an attacker-controlled host and receive the forwarded API key. The client
  now refuses to follow any cross-host redirect; same-host redirects are still
  followed. The query-parameter auth variant was already unaffected.
- Config writes are now atomic. The previous in-place truncate-then-write
  could leave the config file empty or partial if a concurrent `bzr` process
  read it mid-write, or if the process crashed between truncate and write —
  silently dropping all configured servers. The config is now written to a
  sibling temp file and atomically renamed into place (with a directory fsync
  on Unix for crash durability), and crash-orphaned temp files are reaped on
  the next save. This fixes torn reads, not lost updates: two `bzr` processes
  editing the config simultaneously still resolve last-writer-wins, so one
  process's change can be silently overwritten until file locking lands.

### Changed

- Minimum supported Rust version raised from 1.88 to 1.89 (uses the standard
  library's native file locking, stabilized in 1.89, for config write
  serialization — no third-party locking dependency).

### Fixed

- `bzr bug list`, `bug search`, `bug my`, `bug view`, and `query run` now
  preserve Bugzilla custom fields named `cf_*` when requested through
  `--fields`. Custom field values are emitted as top-level JSON keys and as
  dynamic table columns/detail rows instead of being dropped during bug
  deserialization.
- Concurrent `bzr` processes editing different config fields no longer clobber
  each other. All config writes now take an exclusive advisory lock on
  `config.lock` and reload the latest config from disk before applying their
  change, so a write that touches one field preserves a concurrent write to a
  different field. (Two edits to the *same* field still resolve
  last-writer-wins, which is expected.)

## [0.4.2] - 2026-05-29

### Changed

- TLS issuer pinning: removed the legacy issuer-DN string-comparison fallback in
  certificate-change detection. Issuer-change detection (`ISSUER_CHANGED`) now
  relies solely on tamper-proof raw-DER comparison. Pins created before raw-DER
  storage existed keep full certificate-fingerprint pinning but no longer emit
  the secondary `ISSUER_CHANGED` signal (they fall back to `PIN_MISMATCH`) until
  re-pinned. The human-readable issuer is still stored and shown by
  `bzr config show`.

### Security

- `bzr config unset-keyring`: the config file is now rewritten through the same
  `0o600` (file) / `0o700` (directory) hardening as every other save. Previously
  this path wrote the file directly, so a config recreated by `unset-keyring`
  (e.g. after the file had been deleted) could be left world-readable.
- `bzr attachment download`: the server-supplied attachment file name is now
  reduced to its final path component before being used as a write
  destination, so a malicious `file_name` such as `../../etc/foo` or an
  absolute path can no longer escape the target directory. An explicit
  `--out` path is still honored verbatim, as that is the user's own choice.

### Fixed

- Auth detection against a server with an untrusted or expired TLS certificate
  now reports the actionable TLS hint instead of an opaque "network error". The
  `valid_login` probe previously logged send failures generically and skipped
  the certificate detection that the `whoami` probe performs; both probes now
  share the same network-error handling.
- HTTP error responses whose body cannot be read no longer report an empty
  body: the read failure is surfaced in the error message while the HTTP status
  is still reported, instead of being silently swallowed.
- `bzr bug update --deadline`: the deadline is now validated client-side and a
  malformed value fails fast with exit 7, instead of being forwarded to the
  server. Valid `YYYY-MM-DD` deadlines are unchanged.
- `bzr bug search --from-url`: a non-numeric or out-of-range `limit=` in the URL
  is now rejected with exit 7 instead of being silently dropped; `limit=0`
  (Bugzilla "no limit") and an empty `limit=` are documented.
- `bzr bug clone`: if the "Cloned from #N" comment fails to post after the bug
  is created, the new bug ID is now reported with a warning instead of being
  lost, so the clone is not accidentally repeated.
- `bzr query save`: `--search` is now rejected when combined with a structured
  filter flag (`--product`, `--component`, `--whiteboard`, etc.), matching the
  documented mutual exclusivity. Previously only `--search` + `--from-url` was
  caught, so a `--search` plus filter-flag combination was silently accepted and
  both were stored.
- `bzr bug update` / `bzr attachment update`: a Bugzilla error returned with an
  HTTP 200 status (`{"error":true,...}`, as some deployments do) is now
  surfaced as an error instead of being reported as a successful mutation.
- `bzr bug update <id>` with no change flags now fails fast with exit 7 and a
  clear message instead of issuing an empty PUT that touches the bug's
  last-change time while reporting success.
- XML-RPC responses that use self-closing tags for empty or null fields
  (`<value/>`, `<struct/>`, `<array/>`) are now parsed correctly instead of
  failing with an "unexpected EOF" or wrong-type error.
- `bzr config show`: masking an API key that contains a multi-byte character
  near the truncation point no longer panics; key masking and text truncation
  now count characters rather than bytes.

## [0.4.1] - 2026-05-27

### Fixed

- `bzr bug list`, `bug search`, `bug my`, and `query run`: `--fields` now
  selects which columns appear in table output (in the order given) and
  `--exclude-fields` removes columns, instead of always rendering a fixed
  five-column table. `bug view` honors an explicit field set for its detail
  rows. (#206)
- Under `--json`, `--fields` / `--exclude-fields` now trim the output object
  to the selected fields, gh-style: `bzr bug list --fields summary --json`
  returns `[{"summary": ...}]` rather than the full object with every other
  field nulled. The selection is honored literally — `id` appears only when
  requested, and `--exclude-fields id` drops it — while the client still
  fetches `id` internally so every bug deserializes. With no field selection
  the full object is returned, unchanged. Trimming happens client-side after
  the fetch, so it applies on REST and XML-RPC alike. Single-ID `bug view`
  emits a trimmed bare object; multi-ID `bug view` trims each entry in `bugs`
  while leaving the `{"bugs": [...], "failed": [...]}` wrapper and the per-bug
  failure metadata intact. (#206)
- An id-less `--fields` (e.g. `--fields summary,status`) no longer fails to
  deserialize: `id` is always requested from the server so every bug parses,
  regardless of the field selection. (#206)
- A field selection that leaves nothing to emit now exits 7 with a clear
  message, before any network I/O, for every bug subcommand (`bug list`,
  `bug my`, `bug search`, `query run`) — an all-unknown `--fields` value, or
  an `--exclude-fields` that removes every field. In table mode emptiness is
  measured against the five default columns; under `--json` against the full
  field set, so `--exclude-fields` of the five table defaults still succeeds
  and keeps the other fields. `bug view` is exempt in both modes, since a
  sparse or empty single-bug object is a coherent result. Unknown `--fields`
  tokens (typos, custom `cf_*` fields) are reported once on stderr. (#206)
- `bzr bug view --json` stays lenient where the list-style commands now exit
  7: a selection that resolves to no known fields (an unknown/mistyped
  `--fields`, or an `--exclude-fields` covering everything) emits an empty
  `{}` object and exits 0 with a one-line stderr warning, rather than failing.
  A `{}` plus a zero exit can therefore signal a misspelled field name —
  consistent with `bug view` being exempt from the zero-field error in table
  mode too. (#206)

### Changed

- Enabling `serde_json`'s `preserve_order` feature (needed for #206 bug-field
  trimming) is crate-wide: JSON values built via the `json!` macro now
  serialize keys in insertion order rather than alphabetically. The
  user-visible effect is cosmetic key ordering — the error envelope
  (`{"error":{"type",…,"message",…,"exit_code"}}`) and the `query`/`template`
  `--json` result objects emit their keys in a different order. JSON key order
  is not a contract for spec-compliant consumers; typed result structs (e.g.
  `ActionResult`) are unaffected, since they already serialize in declaration
  order. (#206)

## [0.4.0] - 2026-05-09

### Added

- `bzr bug update --dupe-of <ID>` marks a bug as a duplicate by
  forwarding Bugzilla's `dupe_of` field. Bugzilla performs the
  RESOLVED/DUPLICATE transition, and `bzr bug view --json` now
  includes `dupe_of` for verification. Closes #162.
- `bzr bug update` now supports `--alias`, `--deadline`,
  `--estimated-time`, `--remaining-time`, `--work-time`,
  `--reset-assigned-to`, and `--reset-qa-contact`, forwarding the
  corresponding Bugzilla `Bug.update` fields. Bug JSON output also
  includes `deadline` for verification. Closes #164.
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
- `bzr attachment download` accepts multiple attachment IDs and
  `--bug <ID>` (repeatable) for bulk downloads into per-bug
  subdirectories at `<out-dir>/<bug-id>/<att-id>.<file_name>`. New
  `--out-dir` flag (default `./attachments`). Legacy single-ID `--out`
  shape unchanged. (#167)
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

- CI flake in stdout-capture tests caused by process-wide `dup2`
  fd-1 redirection racing concurrent writers (#192). Tests now own
  per-test buffers via `CapturedIo`; production output flows through
  explicit `Writers` from `main` to the formatters. The fd
  redirection construct (`capture_stdout`, `extract_json`, the
  `dup`/`dup2`/`close` extern block) has been removed from
  `test_helpers.rs`, so the race class is structurally impossible.
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
- `bzr attachment download --bug <ID>` no longer performs one extra
  per-attachment fetch in Hybrid/XmlRpc mode when the XML-RPC
  attachment listing can return inline data. Closes #190.

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
