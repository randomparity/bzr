# Coupled Tokio/open dependency repair (#936)

## Problem
PRs #931 and #932 independently omit one lockfile for the same root pins.
The standalone fuzz package consumes bzr by path but owns its lockfile.

## Scope
Update Tokio =1.53.3 to =1.54.1 and open =5.4.4 to =5.4.5 in Cargo.toml.
Refresh only these packages and necessary transitive changes in Cargo.lock and
fuzz/Cargo.lock. Keep Rust 1.89.0, default features, and current-thread runtime.
Reuse #938 Linux launcher fixtures; extend their existing table to verify that
an unsuccessful xdg-open falls through to a successful gio with intact URL arguments.
No runtime ownership transition or caller migration is necessary.
Unrelated dependency refreshes and prevention issue #907 stay with existing owners.
The operator owns later closure of redundant PRs #931/#932.

### Failure model
Deployments are supported bzr CLI hosts and the standalone fuzz dependency graph.
Protect locked reproducibility, current-thread behavior and browser error/URL output.
Dependency resolution, compiler incompatibility, policy denial, functional failure,
and Linux launcher regression block delivery until explained and resolved in scope.
Coverage measures the configured CI gates, default Bugzilla functional suite and
controlled Linux launchers; it does not promise exhaustive desktop integration.
Upstream release defects outside those observations remain upstream-owned.

## Success
Both graphs resolve locked with the requested pins and no unrelated refreshes.
Required CI passes, the full local suite passes, and real Bugzilla functional
results and Linux opener evidence are recorded on the replacement PR.
Identify #931/#932 as redundant without closing either.

## Validation
- Lockfile contract: focused-test. Reproduce each existing PR omission using
  cargo metadata --locked on its missing graph; expect stale-lockfile failure.
  After synchronized updates, run locked metadata on root and fuzz manifests.
- Opener contract: focused-test. Add failed-first fixture in view_tests.rs.
  On open 5.4.4 it must fail because gio never runs; on 5.4.5 it must pass.
  Linux command: cargo test --locked --features test-helpers --lib
  linux_browser_launchers -- --nocapture (one focused test, not full suite).
- Run make lint, make test, cargo deny check, and fuzz cargo-deny policy.
- Run env -u BZR_BIN make functional-test (default Bugzilla) using Podman;
  force a fresh release build from this tree before live verification.
- Required CI supplies declared platform/coverage gates; report each outcome.
- Rollback is reverting the bounded manifest, lockfile and fixture change together.
