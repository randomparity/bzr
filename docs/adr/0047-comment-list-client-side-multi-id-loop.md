# ADR 0047: `comment list` fetches multi-ID sets with a client-side loop

## Status

Accepted

## Context

Issue #699 widens `bzr comment list` from one bug ID to many. Two transports
serve the command. REST fetches one bug per request at
`bug/{id_or_alias}/comment`. XML-RPC's `Bug.comments` takes an `ids` array, and
`src/xmlrpc/resources/comment.rs` already builds that array with exactly one
element, so extending it to N is mechanically trivial and would collapse N
round trips into one on XML-RPC and Hybrid servers.

Taking that batching would make the same command's per-ID failure behavior
depend on which transport the server exposes. `bug view` faced the identical
choice for `Bug.get` in
`docs/superpowers/specs/2026-05-06-bug-view-multi-id-design.md` and rejected
server-side multi-ID batching because it "silently drops inaccessible IDs and
would lose the per-ID error detail `--permissive` is supposed to surface."
`comment list` gains a `--permissive` flag with the same contract, so the same
property is at stake, and this decision either follows that precedent or
knowingly departs from it.

## Decision

Fetch a multi-ID set with a sequential client-side loop over the existing
per-bug `BugzillaClient::get_comments_since`, on every transport. XML-RPC's
`ids` array keeps carrying exactly one element. No new client method is added
and `dispatch_xmlrpc_first`'s Hybrid REST fallback keeps operating at per-bug
granularity, as it does today.

Because neither transport is trusted to attribute a record,
`get_comments_since` backfills `Comment.bug_id` from its own `bug_id` argument
for any record where the server left the field absent, once at the public
client entry point after transport dispatch. A server-supplied value is never
overwritten.

## Consequences

- One failure story: a per-bug error is classified and reported the same way
  whichever transport served it. An XML-RPC fault becomes
  `BzrError::Api { code }` (`src/xmlrpc/protocol/fault.rs:22`) and a REST
  Bugzilla error body becomes the same variant, so
  `is_permissive_bug_view_error` — which admits `NotFound` and `Api` with codes
  100/101/102 — classifies both alike and `--permissive` behaves identically on
  REST, XML-RPC, and Hybrid.
- Per-bug failures are reported on stderr, one line each, and the `--json`
  payload stays a bare `Comment` array. So a consumer reading only stdout
  cannot tell a bug that failed under `--permissive` from a bug with no
  comments; the failed IDs are on stderr and nowhere else. Without
  `--permissive` the ambiguity cannot arise, because the first failure aborts.
- One residual the loop does not remove: on XML-RPC, a server that answers
  `Bug.comments` with a `bugs` map lacking the requested key — rather than
  faulting — yields an empty comment list rather than an error
  (`src/xmlrpc/resources/comment.rs`, `lookup_bug_entry` returning `None`).
  That bug is then indistinguishable from an empty thread with or without
  `--permissive`. This is the existing single-ID behavior, carried forward
  unchanged.
- N bugs cost N requests on every transport. The win the issue asks for — N-1
  process invocations, each of which today re-loads config, resolves
  credentials, detects API mode, and completes a TLS handshake — is captured in
  full; the N-1 HTTP round trips on XML-RPC servers are not.
- The requests share one connection through reqwest's pool, so the per-request
  cost is a round trip, not a new connection.
- Latency grows linearly with the ID count. The 100-ID cap borrowed from `bug
  adjacency` bounds it; a set larger than that is rejected at exit 7 rather
  than fanning out.
- A later change may still add XML-RPC batching as an optimization, but it
  would have to reproduce this per-ID failure contract to be adoptable, and
  that is the cost this decision is declining to pay now rather than
  foreclosing.
- The backfill changes single-ID `--json` output on the deployments that omit
  the field: `bug_id` moves from `null` to the requested ID on the Bugzilla
  5.0.x flat `{"comments": […]}` envelope and on XML-RPC responses that leave
  it out. That is a fix, not a regression — `schemas/comment.json` declares
  `bug_id` as a required `integer`, so the previous `null` was already
  schema-invalid — but it is a visible change to a published payload and is
  documented rather than claimed away.
- No existing consumer of `get_comments_since` reads `bug_id`
  (`attachment upload --comment-private` matches on `attachment_id`,
  `bug history` on creator plus creation time, `bug clone` on `count == 0`), so
  the backfill's effect today is confined to `comment list` output.

## Considered & rejected

- **Batch on XML-RPC via `Bug.comments` `ids`, loop on REST.** verified:
  Bugzilla's REST comment resource documents `id_or_alias` as "a single integer
  bug ID or alias" and `new_since` as its only other parameter, with no `ids`
  request parameter (Bugzilla REST API core v1, `api/core/v1/comment.html`), so
  REST must loop regardless; the batching would therefore apply to one
  transport only and split the command's per-ID error semantics along a line
  the user cannot see from the command line.
- **Batch on both transports.** verified: the REST resource above accepts one
  ID per request, so there is no REST batching to adopt.
- **Batch on XML-RPC and accept losing per-ID detail everywhere.** verified:
  `docs/superpowers/specs/2026-05-06-bug-view-multi-id-design.md` records the
  operator rejecting exactly this for `bug view`, on the ground that a
  multi-ID server call "silently drops inaccessible IDs"; adopting it here
  would make the two sibling read verbs disagree about what `--permissive`
  means.
- **Fetch the bugs concurrently.** judgment: it multiplies load against a
  shared Bugzilla instance for a read a triage operator runs interactively, and
  buys latency the 100-ID cap already bounds. `bug view`'s multi-ID path is
  sequential for the same reason.
- **Report per-bug failures in the payload, via a `{comments, failed}` wrapper
  like `bug view`'s.** verified: `comment list --json` already publishes
  `{"schema_version": "3.0.0", "data": [<Comment>, …]}`
  (`src/output/mod.rs:10`, `src/output/formatting.rs`), so a wrapper replaces
  `.data[]` with `.data.comments[]` — a breaking change to a shipped contract,
  for attribution `Comment.bug_id` already carries. The wrapper exists on
  `bug view` because `Bug` has no field naming the requested ID; `Comment`
  does.
- **Drop `--permissive` and always abort on the first per-bug failure.**
  judgment: it removes the stdout ambiguity above at no code cost, but issue
  #699's proposed approach asks for `--permissive`-style per-bug handling by
  name, and one stale ID would otherwise fail an entire triage read — the cost
  the issue exists to remove.
- **Backfill `bug_id` inside each transport's extractor.** judgment: three
  guards — REST `bugs` envelope, REST flat envelope, XML-RPC — where the entry
  point needs one, and a fourth path added later would silently skip it.
- **Do nothing; leave `comment list` scalar.** judgment: it is the last
  multi-ID-shaped read verb still scalar, and it sits on the bulk-triage hot
  path the issue names, where the per-invocation setup cost is the whole
  expense.
