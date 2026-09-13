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

**Verification.** Mode: focused-test. Add a mock-server test that observes a
Bearer header for `https://bugzilla.redhat.com`-shaped REST construction and
standard header/query for all other hosts. Before wiring, the Red Hat case
observes `X-BUGZILLA-API-KEY`; green command: `make test-one T=red_hat` exits 0.

1. Update every REST auth application call site, including probe and strict
   paths, to pass the base URL.
2. Add the client/probe tests and confirm their focused green run.
3. Run `cargo fmt` and commit the implementation and unit tests.

## Task 3 — Functional parity and documentation

**Interfaces.** The existing `r11_bearer_control` remains the python-bugzilla
positive control; bzr uses a Red Hat-shaped host alias and the proxy log must
record one `auth-kind bearer` request. No CLI option is added.

**Verification.** Mode: focused-test. The comparison phase fails before the
change because bzr has only the controlled parser gap. Green command:
`make functional-compare` exits 0 and reports no `expect_gap 678`.

1. Replace only the #678 parser-gap assertion with bzr's positive wire check;
   leave sibling gap checks untouched.
2. Update `docs/bzr-cli.md` and the parity matrix with exact-host automatic
   REST Bearer behavior and the existing comparison test ID.
3. Run `make lint`, `make test`, and `make functional-test`; each exits 0.
   Commit documentation and functional coverage with a `feat(auth)` subject.

## Rollback

Reverting the feature commit restores standard REST transport. No config data,
server state, or XML-RPC behavior is migrated.
