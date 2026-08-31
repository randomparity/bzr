# Inline URL Server Warning Design

Issue: [#593](https://github.com/randomparity/bzr/issues/593)  
Decision: [ADR 0027](../../adr/0027-inline-server-aware-url-import.md)

## Scope charter

- **Interaction:** interactive
- **Scope identity:** issue #593, `q593-8c64ca6a`
- **Outcome:** suppress the false server-mismatch warning when a `bug search --from-url`
  hostname matches the active `--server-url`, while retaining that inline server as the request
  destination.
- **Completion criteria:** matching inline hostnames are silent; genuine inline mismatches get
  accurate guidance; configured-server matches and default fallback retain their current behavior;
  credential stripping and URL sanitization are unchanged; unit tests cover each resolution path;
  a real-container functional test covers the credentialless stateless inline path; the functional
  suite better represents production by not relying on a matching persisted server.
- **Provenance:** issue #593 supplies the warning and resolution behavior; the campaign request
  supplies the production-fidelity and functional-test requirements.
- **Exclusions:** credential stripping, URL sanitization, named/default server semantics, and merge
  are unchanged. The campaign orchestrator owns the ADR index and campaign manifest.
- **Surface:** URL parser, bug-search caller, sibling unit tests, functional tests, ADR 0027, and
  these direct design records.
- **Ambiguities:** none. Equality means URL hostname equality as normalized by `url::Url`; schemes
  and ports do not participate.

## Problem

`parse_bugzilla_url` knows only persisted `Config` servers. `bug search --from-url` calls it before
connection resolution, even though `CommandContext` already carries an inline server that later
wins connection selection. A matching inline URL can therefore warn that the command will use the
default server, or fail when no default exists, while the actual request would use the inline
server.

Existing unit tests use `setup_test_env`, whose persisted mock server has the same hostname as the
imported URL. Existing functional `--from-url` coverage likewise runs after phase 01 configures the
local container as a named default. Neither tier exercises the stateless combination of
`--server-url`, `--from-url`, and an otherwise empty config. The container is real, but the client
state is less production-like than the stateless demos where the defect appears.

## Design

Add an optional active-server URL argument to URL parsing. The bug-search caller passes the inline
server URL from `CommandContext`; query save/update and fuzz callers pass no active URL and retain
their behavior.

Resolution remains ordered around the existing persisted match:

1. Find a configured server whose hostname matches the imported URL and preserve its name in the
   saved query.
2. Parse the optional active inline URL and compare only its normalized hostname.
3. If the inline hostname matches, emit no warning and allow an empty/default-less config.
4. If an inline server exists but its hostname differs, continue with the inline destination and
   warn that the imported hostname differs from the inline hostname and that the inline server is
   being used.
5. With no inline server, preserve the existing configured/default match, warning, and error paths.

An inline URL already passes global invocation validation before command execution. The parser
will nevertheless treat an unparseable optional active URL as non-matching and emit guidance using
only its hostname when available; connection setup remains the owner of actionable inline URL
validation. No inline URL is copied into `SavedQuery`.

## Error and output behavior

The matching path removes stderr output only. A genuine mismatch says that the imported hostname
does not match the inline server hostname and that the inline server will be used. Without an
inline server, existing messages remain byte-for-byte unchanged. Errors for malformed imported
URLs, non-`buglist.cgi` paths, missing imported hostnames, and default-less unmatched configured
state remain unchanged.

## Security model

### Boundaries

- **Existing boundary used, not widened:** the operator-controlled `--from-url` string is parsed by
  `url::Url`, restricted to a `buglist.cgi` path, and sanitized before persistence.
- **Existing boundary used, not widened:** the operator-controlled `--server-url` has already been
  accepted into `CommandContext`; connection setup remains responsible for validating and using it.
- **Added comparison:** the parser reads only normalized hostnames from both URLs. It does not join
  paths, forward imported credentials, or derive the connection destination from the imported URL.

### Actors and controls

The local operator controls both CLI values; a copied Bugzilla URL may contain untrusted query
parameters. `url::Url` parsing, the existing `buglist.cgi` path check, credential-name filtering,
and `sanitize_url` remain the controls. Host comparison changes diagnostics only; connection
selection continues to trust the explicit inline server from `CommandContext`.

### Out of scope

This change does not establish origin equivalence across schemes, ports, DNS aliases, redirects,
or internationalized hostname variants beyond `url::Url` normalization. It does not make a copied
URL authoritative for routing and does not change TLS or credential handling.

## Tests

- Unit tests prove a matching inline hostname succeeds without a configured/default server and
  leaves `query.server` unset.
- Unit tests prove a mismatched inline hostname still succeeds with the inline destination
  available; the command-level test captures the accurate warning where practical.
- Existing configured match, default fallback, no-default error, and credential sanitization tests
  remain green.
- A search command unit test uses `CommandContext::with_inline_server` with an empty config to prove
  the request reaches the explicit inline mock.
- A functional test passes both `--server-url "$BZ_URL"` and a matching `--from-url` while pointing
  `--config` at a new empty path. It asserts success, real Bugzilla data, and absence of the old
  mismatch warning. The empty config is the production-fidelity correction: it prevents earlier
  setup from accidentally satisfying hostname resolution through a persisted server.

## Durable workflow context

- Branch: `feat/inline-url-server-warning-593`
- Base: `main`
- Guardrails: `make test-one T=<name-substring>`, `make test-fast`, `make lint`, `make test`,
  `make functional-test-all`
- Host: arm64 macOS/BSD userland; targets include x86_64, arm64, powerpc64le, s390x, Windows, and
  macOS release targets. Host architecture is included in declared targets.

