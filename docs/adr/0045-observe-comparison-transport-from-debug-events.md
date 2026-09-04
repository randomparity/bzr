# ADR 0045: Observe comparison transport from debug events

## Status

Accepted

## Context

ADR 0044 requires every bzr/python-bugzilla comparison to record the transport used. The initial
bug-lifecycle implementation copied bzr's requested `--api` value into its evidence and accepted
any non-empty python-bugzilla backend name. Successful semantics could therefore publish parity
without proving which request boundary handled the operation.

The existing bzr request boundaries already emit debug events: REST responses emit `API response`
and XML-RPC sends emit `XML-RPC call`. Python-bugzilla 3.3.0 exposes its selected backend as the
concrete `_BackendREST` or `_BackendXMLRPC` instance.

## Decision

The functional comparison harness will enable `bzr=debug` for observed bzr invocations and derive
transport from the captured request-boundary events. A successful command must have one or more
events from exactly one transport, normalized to `REST` or `XMLRPC`; no recognized event or events
from both transports are infrastructure failures. An invocation rejected before client dispatch
has no transport record because it exercised no client operation. Requested CLI arguments are
assertions only and never supply the observation.

The lifecycle phase keeps observation failures distinct from expected capability gaps. A
successful command with missing or ambiguous evidence remains a test failure and cannot be
converted to GAP by `expect_gap`. A pre-dispatch command rejection may become an expected gap only
without a transport claim.

The python-bugzilla adapter will map exactly `_BackendREST` and `_BackendXMLRPC` to the same closed
vocabulary. Missing or unknown backend classes fail closed. Lifecycle checks compare exact
normalized values.

## Consequences

- Comparison stderr contains debug diagnostics in addition to command errors.
- Existing production tracing is reused; no test-only production flag, environment variable, or
  request behavior is added.
- Expected-gap rows distinguish a command rejected before client dispatch from a successful client
  operation whose transport must be observed.
- A tracing-message change must update the harness and its controlled fixtures together.
- Hybrid operations that emit both transports are intentionally ambiguous until a comparison
  explicitly defines an operation-scoped fallback contract.

## Considered & rejected

- **Copy `--api` into the evidence record.** verified: issue #691 identifies the false-positive
  path at commit `2a46368de76e3565a338c3c96ea5ea2db7303d60`; the wrapper wrote its caller's value
  without observing a request.
- **Read the Bugzilla container's access log.** judgment: it couples comparison assertions to
  image-specific logging and risks retaining credential-bearing request URLs.
- **Add a test-only transport-output feature to the Rust binary.** judgment: it expands production
  build surface when existing sanitized request-boundary events already expose the needed fact.
