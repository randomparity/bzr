# Red Hat REST Bearer API-key implementation plan

Implement documented Bearer authentication at the shared REST auth boundary.
The policy is an exact parsed-host check in `bugzilla_auth`; client detection
and request dispatch reuse it. Rust 1.89.0, no new dependencies, current-thread
Tokio, and existing redaction/REST-XML-RPC boundaries are global constraints.

Expected implementation size: 120–190 changed lines (M) — shared auth helper,
focused unit coverage, one comparison assertion, and two documentation rows.

## File map

- `src/bugzilla_auth.rs`, `src/bugzilla_auth_tests.rs`: exact-host policy and
  credential application contracts.
- `src/client/mod.rs`, `src/client/transport.rs`, client auth tests: carry the
  REST base URL into the shared policy without changing XML-RPC.
- `tests/functional/compare/06-auth-config-tls.sh`: turn the controlled Bearer
  absence case into a positive bzr wire check.
- `tests/functional/lib.sh`: stage and invoke the release artifact in the
  already-running comparison sidecar, whose `/etc/hosts` owns the exact-host
  alias used only by this fixture.
- `docs/bzr-cli.md`, `docs/dev/python-bugzilla-parity.md`: published behavior
  and parity result.

## Task 1 — Shared policy and focused tests

**Interfaces.** Add `is_red_hat_bearer_host(base_url: &str) -> bool` and extend
the existing request-auth helper with the base URL. Callers pass the resolved
REST base URL and the existing standard method. The helper either applies
`Authorization: Bearer <key>` or the existing header/query request mutation.

**Verification.** Mode: focused-test. Add tests for exact production host,
case-normalized host, a suffix lookalike, and a URL containing the host text in
its path; before implementation the new test fails because no Bearer header is
present. Green command: `make test-one T=bearer` exits 0.

1. Write the focused request tests and run the focused command, observing the
   missing Bearer assertion fail.
2. Implement parsed-host equality and HeaderValue construction in
   `src/bugzilla_auth.rs`; use `AUTHORIZATION` rather than a literal header
   name and preserve the existing error string for invalid key characters.
3. Extend redaction coverage with a Bearer header/error preview containing the
   active key; rerun `make test-one T=bearer` and expect exit 0.

## Task 2 — Route all REST callers through the policy

**Interfaces.** `BugzillaClient::new` keeps `PreparedAuth` for standard
methods; its REST `apply_auth` supplies `self.base_url`. Pre-client detection
and strict proof likewise supply their resolved base URL. XML-RPC constructors
and protocol calls are unchanged.

**Verification.** Mode: focused-test. Add a request matrix for the exact host
with both persisted/pinned `Header` and `QueryParam` methods across normal
client dispatch, pre-client detection, and strict credential proof; every
exact-host case observes only `Authorization: Bearer`. Pair it with a
non-Red-Hat control for each standard method, which observes its selected
header or query transport. Before wiring, exact-host cases observe the selected
standard method; green command: `make test-one T=red_hat` exits 0.

The detection matrix explicitly proves exact-host detection bypasses the
standard `whoami`-not-found → `valid_login` → REST header-verification sequence
and sends the version probe with Bearer instead. This is the smaller safe
alternative: no standard query/header probe can reach the exact host. The
non-Red-Hat control retains the current query/header sequence.

1. Update every REST auth application call site, including normal dispatch,
   auth detection/version probing, alternate-auth handling, and strict proof,
   to apply the policy. Ensure a Bearer client does not retry a 401 with a
   standard header or query key.
2. Add the complete exact-host and non-Red-Hat request matrix and confirm its
   focused green run, including the exact-host fallback bypass.
3. Run `cargo fmt` and commit the implementation and unit tests.

## Task 3 — Functional parity and documentation

**Interfaces.** The existing `r11_bearer_control` remains the python-bugzilla
positive control. Add a narrow fixture helper in `tests/functional/lib.sh` that
stages the release bzr artifact in the existing sidecar and invokes it with a
mounted temporary config. `pybz_redhat_alias_install` already maps exactly
`bugzilla.redhat.com` to sidecar loopback, so bzr's URL is
`http://bugzilla.redhat.com:18082`; no runtime hostname override or CLI option
is added.

**Verification.** Mode: focused-test. The comparison phase fails before the
change because bzr has only the controlled parser gap. Green command:
`make functional-compare` exits 0 and reports no `expect_gap 678`.

1. Replace only the #678 parser-gap assertion with the sidecar bzr positive
   wire check. Pin its standard method so setup performs no discovery, then
   assert the proxy recorded exactly one total `auth-kind` line, that it is
   `bearer`, and no query/header credential; leave sibling gap checks untouched.
2. Update `docs/bzr-cli.md` and the parity matrix with exact-host automatic
   REST Bearer behavior and the existing comparison test ID.
3. Run `make lint`, `make test`, and `make functional-test`; each exits 0.
   Commit documentation and functional coverage with a `feat(auth)` subject.

## Rollback

Reverting the feature commit restores standard REST transport. No config data,
server state, or XML-RPC behavior is migrated.
