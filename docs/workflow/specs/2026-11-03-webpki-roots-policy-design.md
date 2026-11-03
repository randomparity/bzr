# Synchronize webpki-roots 1.0.11 and its license exception

## Problem
Dependabot PRs #940 and #941 split the same root pin across separate lockfiles.
Their combined update fails dependency policy because the exact license exception
still names webpki-roots@1.0.10.

## Scope
The operator approved the repair steps and requested a branch and implementation.
Pin webpki-roots to =1.0.11 in Cargo.toml, update only that package in Cargo.lock
and fuzz/Cargo.lock, and retarget the existing CDLA-Permissive-2.0 exception in
deny.toml to webpki-roots@1.0.11. Preserve the exact-version policy and Rust 1.89.0.
Existing manifests and policy retain ownership; no caller migration is needed.
This is one small independently testable dependency repair with no new interface.
No architecture decision or ADR is needed. The approved exclusion set is empty.

### Failure model
- Model: Cargo resolves the root and standalone fuzz graphs independently;
  dependency policy permits this license only for the named package version.
- Prevent: stale locks, unrelated package changes, a widened license allowance,
  MSRV incompatibility, and build or functional regressions.
- Accept: local checks cover macOS arm64 and default Bugzilla 5.0; configured CI
  owns other platforms, feature combinations, and timed fuzz execution.
- Recovery: failed gates block delivery; revert the bounded update together.

## Success
Both graphs resolve webpki-roots 1.0.11 with unchanged transitive packages.
Root and fuzz dependency policy, lint, tests and default functional tests pass.
The policy remains restricted to the exact package version and existing license.

## Validation
- Policy: focused-test. Observe cargo deny check licenses fail after the package
  update and pass after retargeting the exception; run full root and fuzz policy.
- Reproducibility: focused-test. Run cargo metadata --locked for root and fuzz;
  compare lockfile package sets to confirm the requested version is the only delta.
- Integration: run release build, make lint, make test, configured fuzz compile,
  and make functional-test against a fresh release binary from the candidate.
- New unit test: task-test-not-applicable; existing Cargo and cargo-deny checks
  directly reproduce the defects and enforce the repaired contracts.
