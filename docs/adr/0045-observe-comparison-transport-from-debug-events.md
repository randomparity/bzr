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

The functional comparison harness will enable `bzr=debug` for bzr invocations expected to exercise
a client request boundary and derive transport from the captured boundary events. A successful
client operation must have one or more events from exactly one transport, normalized to `REST` or
`XMLRPC`; no recognized event or events from both transports are infrastructure failures. A
dedicated local-only wrapper runs the #672 `--dry-run` request-shape control without making a
transport claim because that successful invocation deliberately performs no client operation.
Requested CLI arguments are assertions only and never supply the observation.

The lifecycle phase keeps observation and infrastructure failures distinct from expected
capability gaps. A successful client operation with missing or ambiguous evidence remains a test
failure and cannot be converted to GAP by `expect_gap`. A non-zero invocation may become an
expected gap only when exit 2 and its captured clap diagnostic positively identify the exact
unsupported option or subcommand exercised by that probe. Every other non-zero outcome, including
connection, timeout, TLS, authentication, server, and harness failures, remains FAIL. A recognized
parser rejection makes no transport claim.

A probe becomes eligible for `expect_gap` only through a terminal classifier that has positively
validated one complete outcome: the recognized parser rejection; successful client operations
with required transport observations and structurally valid response evidence; or the dedicated
successful no-dispatch dry-run with a structurally valid request payload. Semantic,
request-shape, and valid-transport mismatches inside those outcomes remain gap evidence, while
malformed evidence and harness failures do not.

The python-bugzilla adapter will map exactly `_BackendREST` and `_BackendXMLRPC` to the same closed
vocabulary. Missing or unknown backend classes fail closed. Lifecycle checks compare exact
normalized values.

## Consequences

- Comparison stderr contains debug diagnostics in addition to command errors.
- Existing production tracing is reused; no test-only production flag, environment variable, or
  request behavior is added.
- Expected-gap rows distinguish a positively recognized parser rejection, a successful local-only
  control, and a successful client operation whose transport must be observed.
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
