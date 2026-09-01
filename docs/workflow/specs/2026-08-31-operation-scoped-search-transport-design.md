# Operation-scoped search transport design

Issue: [#611](https://github.com/randomparity/bzr/issues/611)

Decision: [ADR 0032](../../adr/0032-operation-scoped-search-transport.md)

## Scope and outcome

A custom bug search containing raw URL parameters must select its effective transport once per
CLI search operation. When the configured mode is XML-RPC or Hybrid, the operation emits the
existing REST-fallback warning once and every request in that operation uses REST. When the
configured mode is REST, the operation uses REST without that warning.

The change preserves result ordering and completeness, structured stdout, page and terminal
progress events, validation and transport errors, the warning text, and the existing behavior of
single-request callers. It adds no schema, configuration, authentication, dependency,
persistence, or migration surface.

## Considered designs

1. **Operation-scoped search handle (selected).** The client resolves configured versus forced
   REST transport once, emits the warning during resolution, and returns a handle used for every
   page. The type makes the chosen transport reusable without mutable global or client state.
2. **Command-layer transport branching.** Paging could inspect raw parameters and call a newly
   exposed REST method, but that would duplicate a client-owned transport rule in the command
   layer and expose lower-level transport details.
3. **Client-level warning suppression.** An atomic or mutable “already warned” flag would be small,
   but its lifetime would be the client rather than one invocation and concurrent or sequential
   searches could suppress warnings incorrectly.

## Architecture and interfaces

`src/client/resources/bug.rs` owns a crate-private `BugSearch<'a>` operation handle. Its factory
has this interface:

```rust
pub(crate) fn begin_bug_search(&self, params: &SearchParams) -> BugSearch<'_>
```

Construction examines the initial parameters. Non-empty `raw_params` with any configured mode
other than `ApiMode::Rest` records forced REST with a pending warning. All other searches record
configured dispatch. The handle exposes:

```rust
pub(crate) async fn execute(&mut self, params: &SearchParams) -> Result<Vec<Bug>>
```

On the first forced-REST execution, the handle emits the current warning immediately before the
request and clears its pending-warning state; later executions remain silent. Every execution
first applies the existing `force_id_fields` normalization, so an include projection that omits
`id` still fetches it and an explicit `exclude_fields=id` cannot remove the identifier required to
deserialize `Bug`. Forced REST then calls the existing REST request path directly. Configured
dispatch preserves the existing REST/XML-RPC/Hybrid match, including Hybrid empty-result fallback. Public
`BugzillaClient::search_bugs` remains source-compatible by creating a handle and executing one
request.

`src/commands/runtime/search/paging.rs` creates one handle at the start of `fetch_page` and passes
it through the unbounded, over-fetch, and paginated paths. The pagination loop changes only the
structured offset on clones of the same parameter set, so the operation-level raw-parameter
decision remains valid for every page. No writer or progress interface changes.

## Data and failure flow

1. The command completes its existing URL import and parameter normalization.
2. `fetch_page` begins one bug-search operation from those parameters without emitting output.
3. Existing pre-request limit and offset validation runs unchanged.
4. The first requested page executes through the operation, which emits at most one fallback
   warning immediately before forced REST; later pages execute through the same silent handle.
5. Existing paging code appends results, advances by rows received, emits page events, stops on an
   empty page, or returns its existing overflow/safety-cap/transport error.
6. The caller writes the same result document and terminal progress event as before.

The warning still precedes a REST validation or transport error, matching current single-request
ordering. Errors raised by paging before a request remain warning-free. No error is intercepted,
translated, retried differently, or converted into output.

## Verification

- A focused multi-page wiremock test uses a Hybrid client plus raw Boolean-chart parameters,
  an include projection that omits `id`, and `exclude_fields=id`; it proves every offset request
  reaches REST with `id` forced into the effective field selection, proves the complete ordered
  result set is returned, and counts exactly one warning.
- A focused REST-mode variant proves the same raw-parameter paging emits no fallback warning.
- A focused raw-parameter over-fetch validation case proves an error raised before the first
  request remains warning-free.
- Existing paging tests continue to prove structured stdout isolation, page-event order,
  terminal-event ownership, offset overflow, safety-cap failure, and server-clamped completeness.
- Existing client tests continue to prove raw parameters bypass XML-RPC fallback and preserve REST
  validation.
- The real-container project-manager Custom Search runs with configured XML-RPC, a one-row page
  limit, an output projection that omits `id`, raw Boolean-chart parameters, and `--paginate`; it
  proves all rows deserialize and are returned with the requested structured output while the
  warning text occurs exactly once on stderr.
- `make lint`, `make test`, and `make functional-test-all` are the final guardrails.

## Global constraints

- Preserve compatibility with all declared release targets:
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `powerpc64le-unknown-linux-gnu`, `s390x-unknown-linux-gnu`,
  `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, and
  `aarch64-pc-windows-msvc`.
- Add no dependency and no new public CLI or configuration contract.
- Keep CLI output paths unchanged: result data through existing writers, diagnostics through
  tracing, and progress through existing progress helpers.
- Keep test modules in sibling `*_tests.rs` files.
- Functional tests use semantic phase-local IDs and the existing runner helpers.

## Security boundary assessment

Raw URL parameters already cross the existing URL-import and REST query-encoding boundaries. This
change neither adds nor widens that input path: it reuses the current validated `SearchParams` and
the current reqwest query builder, and changes only the lifetime of the transport decision and its
warning. Authentication, authorization, secret handling, URL construction, and request encoding
remain unchanged, so no new threat model is required for this design.

## Rollback

Revert the implementation and focused tests. No persisted data or external state requires repair;
the prior per-request warning behavior returns immediately.
