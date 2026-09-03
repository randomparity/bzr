# Implement deterministic tracing capture

Issue [#651](https://github.com/randomparity/bzr/issues/651); governed by the
[spec](../specs/2026-09-02-deterministic-tracing-capture-design.md) and
[ADR 0043](../../adr/0043-stabilize-test-tracing-callsite-interest.md).

Expected implementation size: 25–45 changed lines.

## Task 1: Prove the callsite-registration failure

Add a debug-event helper and test in `src/test_helpers_tests.rs`. Invoke its callsite first on an
uncaptured thread, then on the capture thread. Require the marker and record the pre-fix failure.

## Task 2: Stabilize shared capture registration

In `src/test_helpers.rs`, retain a process-lifetime registered no-op dispatch with `OnceLock`,
constructed as `tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default())` before the
capture. Do not use unregistered `Dispatch::none()`. Run the spec's focused proof without changing
the existing cross-thread propagation or server-capability redaction assertions.

## Task 3: Index, verify, and ship

Add ADR 0043's Accepted row to `docs/adr/README.md` and verify its link. Run the spec's full
verification, review the branch, simplify, and deliver a green mergeable PR without merging.
