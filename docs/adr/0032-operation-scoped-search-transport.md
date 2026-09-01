# ADR 0032: Resolve search transport once per operation

## Status

Accepted

## Context

`BugzillaClient::search_bugs` currently chooses REST whenever a request contains raw URL
parameters and the configured mode is not REST. It also emits the fallback warning at that request
boundary. `--paginate` calls the method once per page, so an invocation-level transport decision
and warning repeat for every page even though only the offset changes.

Issue #611 requires at most one warning per CLI invocation while all pages continue to use REST.
It also requires result, structured-output, progress-event, and error behavior to remain
unchanged, and explicit REST mode to remain silent.

## Decision

Represent one bug-search operation with a crate-private client handle. Constructing the handle
resolves configured dispatch versus forced REST from the initial parameters. When forced REST is
selected, the mutable handle emits the existing fallback warning immediately before its first
request and records that the warning was emitted. Every request in that operation executes through
the resolved transport after applying the existing required-ID field normalization.

The paging runtime constructs one handle before selecting its unbounded, over-fetch, or looped
path. `BugzillaClient::search_bugs` preserves its existing one-request interface by constructing a
handle and executing it once.

## Consequences

Transport selection and its diagnostic share the CLI operation lifetime, while the client remains
immutable and reusable. Pagination cannot drift back to XML-RPC or repeat the warning on later
pages. Lazy emission preserves warning-free input errors raised by paging before its first request.
Keeping required-ID normalization inside the handle preserves projected-result deserialization for
both the compatibility wrapper and paging. Single-request behavior, configured-mode dispatch,
REST request construction, and error propagation remain unchanged.

The client gains a small crate-private type and paging becomes aware of that operation abstraction,
but not of REST request mechanics. A caller that intentionally changes raw parameters between
requests must begin a new operation; pagination only changes offset and therefore satisfies this
invariant.

## Considered & rejected

- **Do nothing.** verified: issue #611 records a four-warning production invocation, and
  `rg -n 'search_bugs|tracing::warn' src/client/resources/bug.rs src/commands/runtime/search/paging.rs`
  at commit `74ac47660bb152014b8acf90f526f2ebd9cc9d80` shows the warning in the per-request method and
  the call inside the page loop.
- **Move transport selection into the paging command.** judgment: this duplicates a client-owned
  dispatch rule and requires exposing REST request mechanics to a higher layer.
- **Suppress repeats with mutable or atomic client state.** judgment: client lifetime is broader
  than one search operation, so state there couples unrelated sequential or concurrent searches
  and makes warning reset semantics implicit.
- **Pass a mutable warning flag through every request.** judgment: a Boolean out-parameter exposes
  diagnostic bookkeeping at each call site instead of representing the operation whose lifetime
  owns the decision.
