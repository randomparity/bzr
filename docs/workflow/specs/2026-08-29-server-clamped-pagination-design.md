# Server-clamped pagination design

Issue: [#581](https://github.com/randomparity/bzr/issues/581)

## Goal

`--paginate` must retrieve every accessible matching bug even when Bugzilla
returns fewer rows than the requested limit because the server enforces a lower
per-response maximum.

## Design

For each non-empty response, append its bugs and advance the next offset by the
number of bugs actually received. A short response is not evidence of
completion because it may be a server clamp. An empty response is the terminal
signal. The existing 10,000-request cap remains the bounded failure for servers
that ignore offsets or otherwise never return an empty page.

An absent or zero limit retains the current single-request behavior. Manual
`--offset` and non-paginated over-fetch behavior are unchanged.

Both REST and XML-RPC requests must carry the current offset. This is required
for hybrid mode, where an empty REST result with structured filters may be
retried through XML-RPC; omitting the offset there would restart at page zero
instead of preserving the terminal result.

No ADR is needed: the issue's completeness criterion rules out requested-limit
offsets and short-page termination, leaving no viable contract alternative.

## Verification

- A focused regression simulates a requested limit of 100 with batches of two,
  then one, then zero, and asserts contiguous unique IDs.
- Existing safety-cap and offset-overflow tests remain green under observed-size
  stepping.
- The XML-RPC request test proves that an offset reaches the wire.
- The real-Bugzilla functional phase asserts that pagination performs the empty
  terminal request and still returns the complete fixture set.
