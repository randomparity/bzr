# Deterministic tracing capture design

Issue: [#651](https://github.com/randomparity/bzr/issues/651)

ADR: [0043](../../adr/0043-stabilize-test-tracing-callsite-interest.md)

## Outcome

Make the shared test tracing capture reliably observe scoped events under Rust 1.89 parallel test
execution while preserving the server-capability tests' API-key redaction assertions.

## Requirements and boundaries

- Reproduce deterministically: one thread owns the sole capture, an uncaptured thread first
  registers a shared debug callsite, then the capture thread emits it.
- Fix process-global callsite interest in `TracingCapture`, not each affected assertion.
- Keep `.with_current_subscriber()` where futures may be polled on another thread.
- Keep the `reason=request`, non-secret marker, and raw-key absence assertions unchanged.
- Add no dependency, change no MSRV, and alter no production or user-facing behavior.
- Pass repeated Rust 1.89 focused tests, `make lint`, `make test`, and `make functional-test-all`.

## Design

`TracingCapture::install` initializes a `OnceLock<tracing::Dispatch>` with a no-op subscriber
before constructing the formatter. The sentinel prevents the single-live-dispatch fast path while
a capture is active. Their disagreement records dynamic interest, letting the current subscriber
decide whether to record the event.

The regression test invokes one fresh debug callsite first on an uncaptured spawned thread, then
on the captured thread. Before the sentinel the first call caches `never`; after it, only the
captured call is recorded. Repeated isolated Rust 1.89 execution proves the schedule.

Server-capability tests keep `with_current_subscriber()`, positive `reason` and marker assertions,
and the negative raw-key assertion.

## Failure handling

`OnceLock` initialization cannot fail. The sentinel owns no output. Capture mutex poisoning and
`DefaultGuard` restoration retain their current behavior.

## Threat model

This is test-only infrastructure. It adds no input, network, credential, authorization, or output
boundary. Existing tests still prove that server-controlled error bodies retain a non-secret
marker while API-key material is removed from captured diagnostics.

## Verification

- controlled-fault regression test on Rust 1.89 before the sentinel
- 50 isolated runs of
  `CARGO='rustup run 1.89.0 cargo' make test-one T=tracing_capture` on Rust 1.89
- 50 isolated runs of `CARGO='rustup run 1.89.0 cargo' make test-one
  T=server_capabilities_nulls_attachment_size_on_parameters_error` on Rust 1.89
- `make lint`
- `make test`
- `make functional-test-all`
