# TLS advisory repair — issue #923

## Outcome and authority

Issue #923 and its approved separate-repair scope require patched root and fuzz
TLS graphs, green dependency policy, preserved compatibility and real-server proof.
The operator approved the bounded repair with an explicitly empty exclusion set.
Scope: Cargo.toml, Cargo.lock, fuzz/Cargo.lock and deny.toml, plus this design.

## Design

Pin webpki-roots to 1.0.10, resolve rustls-webpki to 0.103.16 in both lockfiles,
and retarget the existing CDLA-Permissive-2.0 exception to webpki-roots@1.0.10.
Crates.io on 2026-10-21 lists these as current compatible stable releases; both
require Rust 1.71. The newer webpki 0.104 train is pre-release, not selected.
[RUSTSEC-2026-0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html) and
[RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) identify
these patched minima. Use targeted Cargo updates; review graph changes.
Existing manifests and policy remain the coherent owners. No caller migration,
new interface, application code, ownership move or compatibility shim is needed.
No viable architecture decision is introduced, so no ADR is needed.
A broad dependency refresh adds unrelated risk; an advisory waiver fails #923.

## Global Constraints

Keep Rust 1.89 MSRV, existing default/no-default/vendored feature behavior and
supported target behavior. Do not waive advisories or bump unrelated dependencies.

## Failure model

- Model: an untrusted server may offer a certificate path containing distrusted
  emSign keys; registries and upstream patch provenance are trusted supply inputs.
- Prevent: affected versions remain resolved in either graph, stale fuzz locks,
  widened policy waivers, incompatible versions or broken existing features.
- Accept: intentional insecure mode and explicit pin policy retain their existing
  semantics; this dependency-only repair does not promise new controls for them.
- Recovery: dependency/policy failures block shipping; targeted updates and review
  repair the graph. A regression can be reverted, but restores advisory exposure.

## Threat model

- Boundaries: TLS certificate chains enter from remote servers; crate artifacts
  enter via the existing registry. No new or widened boundary is introduced.
- Actors/trust: remote peers are untrusted; upstream Rustls patches and Cargo's
  checksummed registry artifacts supply the existing verifier implementation.
- Controls: patched webpki rejects distrusted keys; roots removes the affected
  trust anchor. cargo-deny checks both graphs without advisory ignores; lock
  checksums and unchanged source policy constrain supply inputs. Existing TLS
  errors continue to fail the connection and existing redaction remains intact.
- Limits: no new insecure/pinning behavior, OS trust-store administration or
  unrelated advisory remediation is promised by these two upstream fixes.

## Validation and success

Existing cargo-deny policy must fail on the affected baseline and pass for root
and fuzz after the repair. Locked fuzz metadata must succeed. Inspect package
sets/checksums to confirm only the two packages changed. Run make lint, feature
checks, existing TLS-focused tests, the default real-server functional suite,
and mandatory pre-push full tests. CI independently verifies configured targets.
These checks establish integration; upstream patches own malicious-chain coverage.
