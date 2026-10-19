# Functional DNS-name verification (#865)

## Problem and authority

Issue #865 requires a positive DNS-name TLS connection, an observed wrong-name rejection,
reverted fault proofs for both, a comment on the nonconnecting Rust search URL, and a green
`make functional-test`. The frozen authority is WORK:SCOPE q865-3eaeda28 on #865.
The operator approved the exclusions and owners on 2026-10-15 through campaign
83934fcc4ab3-09d204b0-8a21-4b5c-8566-c87f9c89cb5f.

## Design

Extend existing phase 02c and `tls_fixture_start`, retaining their ownership and callers.
The fixture accepts an optional second DNS SAN argument defaulting to `localhost`; its IP SAN,
CA generation, pin calculation, readiness and cleanup stay the existing path. Comparison 06
keeps its one-argument call. No ADR is warranted for this local test parameter.

After the existing CA/IP case, connect credentiallessly to `https://localhost:<fixture-port>`
with `--server-tls-ca-cert`, and require exit zero plus a version payload. The IPv4-bound proxy
is reached through localhost without introducing a resolvable alias or DNS inside cargo tests.
After existing fixture consumers in this phase finish, tear down and start a wrong-name leaf
with DNS SAN `wrong-name.invalid` while keeping its CA trusted and connecting to localhost.
Require nonzero exit and stderr containing `certificate not valid for name` and `localhost`.
A connection refusal, untrusted issuer, or generic transport failure cannot satisfy this check.
Use table error output to avoid JSON escaping the diagnostic. Retain the phase cleanup trap
until the negative fixture is stopped; report restart failure through the existing test helpers.

Add a comment immediately before `configured_url` in the Rust test: the localhost URL is only
compared/imported and never connected to; requests go to the numeric inline mock. Connecting
it would reintroduce resolver/environment mutation races.

## Success and boundaries

The two new stable semantic IDs pass on the real Bugzilla fixture. A deliberately wrong positive
SAN makes the positive case fail for DNS identity; replacing the negative SAN with localhost
makes its rejection assertion fail on exit zero. Both faults are restored before final gates.
Default fixture consumers retain the IP and localhost SANs. No production TLS/pinning behavior,
cargo-test DNS resolution, ENV_LOCK redesign, automated hostname guard, or trust-mode matrix is
included; those exclusions remain owned by the repository maintainer.

## Failure model

- Actors and deployments: local macOS/Linux functional operators with Bash, OpenSSL, Python,
  curl, Docker/podman, and the real Bugzilla container; cargo test runs only the commented test.
- Invariants/assets: CA validation checks DNS identity; negative evidence names identity failure;
  shared fixture defaults and phase cleanup remain usable; fault edits never ship.
- Accepted failure classes: missing TLS tooling retains the existing phase skip, but cannot prove
  this issue; unavailable external containers stop proof. Other trust modes remain untested here
  by operator-approved exclusion. Unnamed deployments have no new coverage guarantee.
- Covered elsewhere: production verifier correctness belongs to existing TLS tests/maintainer;
  semantic-ID/layout/shell checks own harness structure; #857 and ADR0002 own unit DNS avoidance.

## Threat model

- Boundary inventory: added test observations of the existing TLS peer-to-CLI boundary;
  no production boundary is added or widened. Fixture DNS SAN flows from trusted phase literals
  into a private OpenSSL extension file and then the test leaf.
- Actor model: the local functional runner controls fixture names/keys/CA; the peer presents a
  valid chain with a mismatching identity. Repository test code is trusted, not a public SAN API.
- Control per boundary: existing rustls CA/name verification must reject the wrong peer;
  the test requires its name-specific diagnostic. Quoted arguments and the private fixture directory
  retain existing bounded proxy-readiness and cleanup controls. Only synthetic names reach errors.
- Explicitly out of scope: production verifier/pinning changes and other trust modes are approved
  exclusions owned by the maintainer. Compromised test source and arbitrary SAN input are outside
  this trusted local harness; no new command accepts those inputs.

## Validation

The plan has one coupled task. Run the actual sourced phase in an isolated private runner for
scoped green/red/green evidence using a freshly built binary and real container. The positive
controlled fault changes the default fixture DNS SAN; the negative controlled fault changes its
explicit SAN to localhost. Capture actual stderr and status in private logs; publish only synthetic
certificate identities and the observed error. The comment is human-reviewed prose, with no
wording test. Run focused Rust regression, shell/ID/layout/runtime guards, clippy/format, full
`make test` before implementation commit, and `make functional-test` before PR creation.
