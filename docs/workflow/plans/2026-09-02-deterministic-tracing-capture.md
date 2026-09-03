# Implement deterministic tracing capture

Issue: [#651](https://github.com/randomparity/bzr/issues/651)

Spec: [Deterministic tracing capture design](../specs/2026-09-02-deterministic-tracing-capture-design.md)

Decision: [ADR 0043](../../adr/0043-stabilize-test-tracing-callsite-interest.md)

## Goal

Prevent an uncaptured test thread from poisoning global callsite interest while another thread
owns the sole tracing capture.

Expected implementation size: 25–45 changed lines.

## Task 1: Prove the callsite-registration failure

Add a debug-event helper and test in `src/test_helpers_tests.rs`. Invoke its callsite first on an
uncaptured thread, then on the capture thread. Require the marker and record the pre-fix failure.

## Task 2: Stabilize shared capture registration

In `src/test_helpers.rs`, retain a process-lifetime no-op `tracing::Dispatch` with `OnceLock`,
initialized before capture construction. Re-run the regression repeatedly plus the existing
cross-thread propagation and server-capability redaction tests without changing their assertions.

## Task 3: Verify and ship

Run `make lint`, `make test`, and `make functional-test-all`. Review the complete branch under the
frozen issue charter, simplify without changing behavior, then deliver a green mergeable PR and
stop without merging.
