# ADR 0043: Keep a sentinel tracing dispatch alive in tests

## Status

Accepted

## Context

`TracingCapture` installs a thread-local subscriber. Commit `5abe41ae` propagated it with the
affected futures, but an unchanged parallel MSRV job later lost one expected debug event while
capturing surrounding events.

`tracing-core` keeps process-global callsite interest. Its single-dispatch fast path evaluates a
new callsite against the first caller thread's dispatcher. An uncaptured thread can therefore
cache `never` while the sole live capture belongs to another thread, causing that capture to skip
the event. With two live dispatches, `tracing-core` aggregates registered dispatchers and uses
dynamic per-dispatch interest when they disagree.

## Decision

The shared helper retains one process-lifetime no-op `Dispatch`, initialized before the first
capture subscriber. Every capture then coexists with another registered dispatch, so a callsite
first reached elsewhere retains dynamic interest for the capture's own subscriber to resolve.

Keep subscriber propagation for futures that may move across threads. The sentinel addresses
process-global callsite registration; propagation addresses which dispatcher is current while a
future is polled. Neither substitutes for the other.

## Consequences

Tests retain one no-op dispatch and may dynamically check callsites observed during capture.
Capture API, filters, redaction checks, production logging, dependencies, and MSRV stay unchanged.
The spec and transient plan repeat the required proof so design and execution remain independently
checkable; this bounded documentation overhead is accepted for the concurrency fix.

## Considered & rejected

- **Serialize capture tests.** judgment: uncaptured test threads could still register a callsite.
- **Rebuild interest before assertions.** judgment: a later registration can recreate the race.
- **Rely on `with_current_subscriber`.** verified: `5abe41ae` did so, yet CI run 33664497573 lost
  the event; propagation does not change process-global callsite registration.
- **Set a global formatter.** judgment: a library must not consume the application's global slot,
  and independent buffers would require routing machinery.
