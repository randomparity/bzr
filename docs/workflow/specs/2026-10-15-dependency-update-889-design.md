# Review the Cargo updates from PR #889

## Problem

PR #889 updates five direct dependencies and changes tempfile's resolved entropy
provider from getrandom 0.3.4 to 0.4.3. Automated restock stopped for review of
that newly active dependency; it did not demonstrate a compatibility failure.

## Scope

Carry clap 4.6.7, clap_complete 4.6.11, tokio 1.53.2, toml 1.1.7,
and thiserror 2.0.21 onto current main in one replacement PR. Review the
getrandom 0.4.3 edge against tempfile's published dependency range, the
Rust 1.89 floor, supported platform backends, and existing temporary-file use.
Preserve the upstream-supported edge when verification succeeds; fix only
demonstrated compatibility defects. Document the evidence in dependency-health
notes and correct dependency-policy commentary if its description becomes stale.
No ownership transition or application architecture change is needed.

The operator approved this full-update scope on 2026-10-15. Unrelated upgrades,
toolchain, public API, and architecture changes are excluded; follow-ups remain
unassigned and are reported. Leave #889 open while validating this replacement.

### Failure model

Deployments are the existing Linux, macOS, and Windows release targets, with
the single-threaded Tokio runtime and existing keyring feature configurations.
Protect temporary-file isolation/cleanup, attachment streaming, CLI parsing,
configuration serialization, and the Rust floor. Upstream RNG internals remain
upstream-owned: review compatibility rather than inventing a new entropy API.
The author verifies native macOS and real Bugzilla locally; existing CI owns
Linux/Windows and cross-target build checks. Unrun arms are reported explicitly.
No new supported platform or stronger entropy guarantee is introduced.

## Success

The replacement contains the five updates with an explained dependency graph,
no unrelated lockfile churn, and unchanged Rust floor and dependency-policy gates.
Relevant local checks, default-version functional tests, and PR CI pass.
The PR links #889 and explains the resolution of the review concern.

## Validation

- Dependency selection: task-test-not-applicable; no new bzr behavior or schema
  is introduced, so a test asserting exact upstream versions would mirror the
  manifest. Inspect the lock diff, cargo tree (native and target-all), upstream
  manifests/changelog, then run locked build and existing Rust tests.
- Runtime compatibility: task-test-not-applicable; no implementation contract is
  changed. Existing attachment-stream/config/CLI tests and `make functional-test`
  exercise consumers; do not add upstream-RNG implementation tests.
- Evidence/policy commentary: task-test-not-applicable; human-readable prose has
  no executable consumer. Verify claims against source and `cargo deny check`.
- Run format, both all-targets Clippy feature configurations, applicable Make
  guards, and the mandatory full-test push hook. Require green PR CI before handoff.
