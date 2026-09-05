# `bzr comment list`: accept multiple bug IDs

**Date:** 2026-09-04
**Issue:** [#699](https://github.com/randomparity/bzr/issues/699) (`enhancement`, `area:cli`, `area:agent`)
**ADR:** [0049](../../adr/0049-comment-list-client-side-multi-id-loop.md)
**Sibling precedent:** [`2026-05-06-bug-view-multi-id-design.md`](2026-05-06-bug-view-multi-id-design.md)

## Goal

`bzr comment list 1 2 3` reads the comment threads of all three bugs in one
invocation. Records stay attributable to their bug in every output format.

Single-ID output keeps its shape — a bare `Comment` array under `--json`, the
same comment blocks in table mode, no bug header, no wrapper. One value inside
that shape does change, and it is stated here rather than claimed away: on
deployments that omit `bug_id` (the Bugzilla 5.0.x flat `{"comments": […]}`
envelope, and XML-RPC responses lacking the field) the backfill below moves
`bug_id` from `null` to the requested ID. `schemas/comment.json` declares
`bug_id` a required `integer`, so the previous `null` was already
schema-invalid; the change is a conformance fix, and it applies to single-ID
and multi-ID calls alike.

`comment list` is the last multi-ID-shaped read verb still scalar, and it sits
on `bzr-bulk-triage`'s hot path (`content/skills/bzr-bulk-triage/SKILL.md:60`),
where reading N bugs costs N process invocations.

## Non-goals

- No comment tags in `comment list` output — owned by issue #700.
- No comment embedding in `bug view` — issue #698, closed not-planned.
- No change to `bug view`'s `{bugs, failed}` wrapper or `--permissive`
  semantics.
- No concurrent or parallel fetch. The loop is sequential.
- No new client method and no new `BzrError` variant.

## Result shape: flat array

Multi-ID `--json` emits **one flat array of `Comment` records**, in argument
order, each carrying its existing `bug_id`. Consumers group with
`jq '.data | group_by(.bug_id)'`.

This is not a new published shape. `comment list` already emits
`{"schema_version": "3.0.0", "data": [<Comment>, ...]}`; multi-ID produces the
same envelope with the same per-item schema and more items. `schemas/comment.json`
is unchanged, `SCHEMA_VERSION` is unchanged, and the 5-coupled-update schema
process does not apply.

The `{bugs, failed}` wrapper `bug view` uses was forced by a property `Comment`
does not share: `Bug` has no field recording which requested ID produced it, so
without a wrapper a multi-ID `bug view` result is unattributable. `Comment`
carries `bug_id` (`src/types/comment.rs:29`) and self-identifies. Adopting the
wrapper here would publish a new shape and break `.data[]` for every multi-ID
consumer, to buy attribution the record already has.

### Attribution is enforced, not assumed

`Comment.bug_id` is `Option<u64>`, and neither transport is trusted to populate
it:

- The REST envelope extractor `extract_bugs_comment_envelope`
  (`src/client/resources/comment.rs:25-42`) reads the `bugs` map's *values* and
  discards its keys, so the requested ID never reaches the record from the
  envelope.
- The flat envelope variant `{"comments": [...]}` — observed on some Bugzilla
  5.0.x deployments (issue #135) — carries no bug context at all.

`get_comments_since` therefore backfills `bug_id` from its own `bug_id`
parameter for any record where the server left it absent, once, at the public
client entry point after transport dispatch. A server-supplied value is never
overwritten — it is trusted unverified. Nothing compares it against the
requested ID, so attribution is only as good as the server's own labelling: the
backfill closes the absent case, not the wrong one.

Doing this at the entry point rather than inside each transport's extractor is
deliberate — one fix covering REST, XML-RPC, and the Hybrid fallback, instead
of one guard per path.

### `bug_id` survives the projection flags on a multi-ID call

`FieldProjection::apply` recurses into an array and retains only the named keys
per element, so `comment list 1 2 3 --json --fields id,creator` would return a
merged array with no `bug_id` at all — attribution silently gone, exit 0, no
warning. That form is not hypothetical: `docs/bzr-cli.md` ships
`--fields id,creator,creation_time` as the token-saving shape for agents and a
functional test already exercises its single-ID version.

So whenever more than one bug ID is requested, `bug_id` is forced into the
resolved include set and removed from the exclude set after `projection_for`
returns, and one line goes to stderr saying so:

```text
keeping bug_id; it is what attributes each comment to its bug
```

The note carries no flag prefix: it fires for `--exclude-fields bug_id` too,
and naming `--fields` there would report a flag the operator never passed.

This is the operator's decision, and it is the cheapest of the options: it
breaks no currently-working invocation, and it follows `bug view`'s established
habit of doing the sensible thing and warning on stderr. Rejected: rejecting
the flag combination at exit 7, accepting the loss with documentation, and
switching to a `{bugs, failed}` wrapper.

Single-ID calls are untouched — one bug's comments need no attribution field to
stay attributable, so `--fields id` still projects to exactly one key there.

## Transport strategy: loop on both

Both transports fetch one bug per call, via the existing per-bug
`client.get_comments_since(bug_id, since)`. XML-RPC's native `Bug.comments`
`ids` array is **not** used for batching.
[ADR-0049](../../adr/0049-comment-list-client-side-multi-id-loop.md) is the
record: it carries the decision, its grounds, and its five rejected
alternatives, and this spec does not restate them.

The one fact the rest of this spec depends on: every requested bug costs one
call on every transport, so the loop's failure handling below is per bug, and
`dispatch_xmlrpc_first`'s Hybrid REST fallback keeps operating at per-bug
granularity as it does today.

## CLI surface

`CommentAction::List` in `src/cli/comment.rs`:

```rust
List {
    /// Bug ID(s)
    #[arg(required = true, num_args = 1..)]
    bug_ids: Vec<u64>,
    /// Continue past per-bug failures (multi-ID only).
    #[arg(long)]
    permissive: bool,
    /// Only show comments created after this date (ISO 8601)
    #[arg(long)]
    since: Option<String>,
    #[command(flatten)]
    projection: crate::cli::ProjectionArgs,
},
```

`--since`, `--fields`, and `--exclude-fields` apply uniformly to every
requested bug. `bug_ids` stays `u64` — `comment list` has never accepted
aliases, and `Bug.comments` / the REST comment route are ID-keyed.

### Input validation

One condition is checked at handler entry, before any network call, and exits 7
(`BzrError::InputValidation`):

| Condition | Message |
|---|---|
| `--permissive` with exactly one ID | `--permissive only meaningful with multiple bug ids` |

**There is no cap on the number of IDs.** An earlier draft borrowed
`bug adjacency`'s `MAX_REQUESTS = 100`; the operator removed it. `bug view` is
the sibling this design follows, and it loops client-side per ID with no bound
(`src/commands/bug/view.rs`, `fetch_batch` — no `MAX` constant, no length
check). `bug adjacency`'s cap covers a different shape: it fans out several
requests *per* ID. Capping `comment list` would newly introduce a disagreement
between two sibling read verbs — a script that works against a 150-ID list with
`bug view` would hard-fail on `comment list` — and it buys no protection the
sequential loop does not already have.

Duplicate IDs are not rejected or deduplicated — `bug view` fetches and prints
duplicates as given, and matching that is cheaper than a rule users would have
to learn.

## Per-bug failure handling

Without `--permissive`, the first per-bug failure aborts the whole call with
that error's own exit code. For a single ID this is exactly today's behavior.

With `--permissive`, a failure that `BzrError::is_permissive_bug_view_error()`
classifies as per-resource (`NotFound`, or `Api` with a `Bug.get` per-resource
fault code) is reported on stderr as one line and the loop continues; the
command exits 0 even if every bug failed. Session-wide failures — transport,
auth, TLS, deserialization — still bail immediately, in both modes.

Failure lines go to **stderr in every output format**, one per failed bug,
prefixed `bug <id>: ` and carrying that error's own `Display`. For a
nonexistent bug both transports produce `BzrError::Api { code: 101, .. }` — a
REST Bugzilla error body and an XML-RPC fault map to the same variant
(`src/xmlrpc/protocol/fault.rs:22`) — so the line reads:

```text
bug 999: Bugzilla API error: Bug #999 does not exist. (code 101)
```

A trailing line then reports the total, so an all-failed run is distinguishable
from one that found nothing:

```text
2 of 2 bugs could not be read
```

**What this costs, stated rather than assumed:** failures are not carried in
the JSON payload. A flat array of `Comment` records has nowhere to put a
failure entry, and adding one would be the wrapper this design rejected. So a
consumer reading only stdout cannot tell a bug that failed under `--permissive`
from a bug with no comments — the failed IDs are on stderr and nowhere else.
Without `--permissive` the ambiguity cannot arise, because the first failure
aborts. This trade is recorded in ADR-0049's Consequences and stated on the
`--permissive` row in `docs/bzr-cli.md` and in the reference-skill bullet, so
an agent consuming the JSON meets it where it reads the flag.

Routing to stderr keeps one mechanism across table and JSON and keeps stdout a
clean comment stream for `jq` and pipelines. It matches the stdout=data /
stderr=diagnostics split ADR-0014 records (ADR-0007 decides the
`{schema_version, data}` envelope and says nothing about streams).

Reusing `is_permissive_bug_view_error()` rather than adding a comment-specific
predicate is intentional: the per-resource fault codes are `Bug.get`'s and
apply identically to `Bug.comments`, and one predicate keeps `bug view` and
`comment list` classifying the same server fault the same way. An HTTP failure
whose body is not a Bugzilla error object becomes `BzrError::HttpStatus`, which
the predicate rejects — so a bare 404 or a proxy error page aborts even under
`--permissive`, which is the intended behavior for an unclassifiable failure.

### A missing `bugs` key on XML-RPC becomes a per-ID failure

`extract_comments` (`src/xmlrpc/resources/comment.rs`) currently returns
`Ok(Vec::new())` when `lookup_bug_entry` finds no entry for the requested bug.
At N=1 that surfaces as `No comments.` — ambiguous but scoped to the one bug
the operator asked about. At N>1 it is silent data loss: the bug contributes no
records to the flat array, gets no table header, and produces no stderr line,
so a caller cannot tell that bug 43 was dropped from `comment list 42 43 44`.

This change is what introduces N>1, so that path is this feature's error
handling rather than adjacent breakage. A missing key now yields
`BzrError::NotFound { resource: "bug", id }`, which
`is_permissive_bug_view_error()` already admits, so it flows through the loop
exactly as a REST-side per-ID failure does: aborts by default, one stderr line
and continue under `--permissive`.

The fix goes in `extract_comments`, not in the shared `lookup_bug_entry`
mapper — other XML-RPC resources call that helper and their semantics are not
in scope here.

Two consequences, both authorized rather than incidental:

- Single-ID XML-RPC behavior changes: a server answering with a `bugs` map that
  omits the requested key previously printed `No comments.` and now exits with
  a not-found error. Conflating "I have no record of that bug" with "that bug
  has zero comments" is the defect being removed. A bug that exists with an
  empty thread is unaffected — Bugzilla returns
  `{bugs: {"42": {comments: []}}}` for that, key present.
- Three other commands share that call path, and the change lands differently
  on each. `bug clone` propagates the error with `?`
  (`src/commands/bug/clone.rs:137`), so it goes from cloning without a
  description to aborting at **exit 2** (`EXIT_CODE_NOT_FOUND`) — a new
  hard-failure mode on a mutating command, and the consequence most worth
  knowing. `bug history` goes from a silent empty correlation set to a warned
  one, which it already handles. `attachment upload --comment-private` changes
  exit code from 10 (`DataIntegrity`, reached after an empty comment list) to 2,
  with different stderr text — a change, not a no-op, since exit codes are a
  published contract. The shift is not XML-RPC-only: on REST, `{"bugs": {}}`
  moves from `Deserialize` (exit 8) to `NotFound` (exit 2) for the same three
  callers.

### When every bug fails

Under `--permissive` with no bug reachable, the exit code is 0 in both formats
and stdout differs by format, deliberately:

- **JSON** — the accumulated vector is empty and the single trailing
  `write_comments` call emits `{"schema_version": "3.0.0", "data": []}`. That
  is a valid payload a consumer already handles.
- **NDJSON** — stdout is byte-for-byte **empty**. NDJSON carries no envelope
  (`SCHEMA_VERSION` is "Present in `--json` output only",
  `src/output/mod.rs:6-9`) and `write_ndjson` emits one line per element, so an
  empty array emits nothing. An NDJSON consumer therefore cannot distinguish
  "every bug failed" from "every bug has an empty thread" from "the command
  produced nothing" by any means on stdout; it must read stderr, or drop
  `--permissive` and let the first failure abort. This is stated on the
  `--permissive` row in `docs/bzr-cli.md` and in the reference-skill bullet.
- **Table** — the loop wrote nothing, so a single trailing `write_comments`
  call with an empty slice emits `No comments.`, the same thing a single ID
  with an empty thread prints today. The human contract for "nothing to show"
  does not silently stop applying in multi-ID mode.

The table path tracks whether it has written anything. That flag does two
jobs: it suppresses the trailing empty-slice call once any bug has been
written, and it keys the blank-line separator between bugs — so a bug skipped
at index 0 leaves no stray leading blank line.

## Output composition

`src/commands/comment/list.rs` branches on ID count and format:

- **One ID** — the current path verbatim: one `get_comments_since`, one
  `write_comments`. No header, no behavior change.
- **Multiple IDs, table** — for each ID in order, write a `Bug #<N>` header,
  then that bug's comments through the existing `write_comments`. Without the
  header a multi-bug table is an undifferentiated wall of `Comment #<n>`
  blocks with no way to tell whose thread is whose. If the loop wrote nothing
  at all, one trailing empty-slice `write_comments` supplies `No comments.`
  (see *When every bug fails*).
- **Multiple IDs, JSON/NDJSON** — accumulate every bug's comments into one
  `Vec<Comment>` in argument order and call `write_comments` once, so the
  payload is a single valid array (and, for NDJSON, one record per line across
  the whole set).

The header writer is a new `write_comment_bug_header` in
`src/output/resources/comment.rs` — user-facing output belongs in the output
layer per the repo's `Writers` convention, and putting it there makes it
directly testable.

## Error handling summary

| Situation | Behavior | Exit |
|---|---|---|
| Single ID, bug missing | error, unchanged from today | that error's code |
| Multi-ID, no `--permissive`, one bug fails | abort at that bug | that error's code |
| Multi-ID, `--permissive`, per-resource failure | stderr line, continue | 0 |
| Multi-ID, `--permissive`, every bug fails | stderr lines; `data: []` (JSON), `No comments.` (table), **empty stdout** (ndjson) | 0 |
| Multi-ID, `--permissive`, unclassifiable HTTP failure | abort | that error's code |
| Multi-ID, `--permissive`, transport/auth failure | abort | that error's code |
| `--permissive` with one ID | rejected before any call | 7 |
| XML-RPC `bugs` map omits the requested key | per-ID failure (`NotFound`), same as any other | that error's code, or 0 under `--permissive` |
| Unknown `--fields` | rejected before any call, unchanged | 7 |

Partial success does not use `BzrError::BatchPartialFailure` (exit 11): failed
reads change no server state. `bug view` sets the precedent for the **exit
code** only — it reports its failures in the payload (a `failed` array under
`--json`, `Bug #N — UNAVAILABLE` rows in table mode), which is the one place
this design deliberately diverges from it. See ADR-0049.

## Testing

**Unit (`src/commands/comment/list_tests.rs`, wiremock):**

- single ID emits a bare array and no `Bug #` header (table and JSON);
- three IDs, JSON: one flat array, records carry each requested `bug_id`, order
  follows argument order;
- three IDs, table: one `Bug #<N>` header per bug, in argument order;
- a bug with zero comments contributes no records to JSON and a header plus
  `No comments.` to the table;
- `--permissive` with a middle bug returning a Bugzilla error object
  (`{"error": true, "code": 101}`): exit 0, stderr names the failed bug,
  stdout holds the other two bugs' comments;
- without `--permissive` the same error aborts and no partial JSON is written;
- `--permissive` with a bug returning a **plain-text** 404 aborts anyway — that
  is `BzrError::HttpStatus`, which the predicate rejects;
- every bug failing under `--permissive`: exit 0, `data: []` in JSON and
  `No comments.` in table, with one stderr line per bug;
- `--permissive` with one ID exits 7 before any request is made;
- `--since` is threaded into every iteration, not just the first — mount each
  bug's path with a `new_since` query-param matcher and `.expect(1)`, so an
  unthreaded value fails the mock expectation;
- `--fields` applies to every bug, **and `bug_id` survives it** on a multi-ID
  call: `--fields id` yields rows with `id` and `bug_id`, and stderr carries
  the retention note. `--exclude-fields bug_id` on a multi-ID call keeps it
  too;
- `--fields id` on a **single**-ID call still yields exactly one key, with no
  retention note — the override is multi-ID only.

**Unit (`src/xmlrpc/resources/comment_tests.rs`):** `Bug.comments` answering
with a `bugs` map that omits the requested key yields
`BzrError::NotFound`, not an empty vector; a map carrying the key with an
empty `comments` array still yields `Ok(vec![])`.

**Unit (multi-ID over XML-RPC, `src/commands/comment/list_tests.rs`):** a
two-bug call where the second bug's key is missing aborts by default and, under
`--permissive`, exits 0 with a stderr line naming that bug and the first bug's
comments on stdout.

The transport is selected through the context, not through a client:
`CommandContext::new(None, format, Some(ApiMode::XmlRpc))`
(`src/commands/runtime/invocation/context.rs:27` takes `api` as its third
argument), against a wiremock `POST` on `/xmlrpc.cgi`. `test_client_xmlrpc`
cannot serve these tests — it returns a `BugzillaClient`, and the handler builds
its own through `connect_and_configure`, which has no injection point.
`setup_test_env()` pins `api_mode = "rest"` in the config it writes, so the
context override is what reaches the XML-RPC path. This is the same selection
`bzr --api hybrid comment list` uses functionally at `15-comments.sh:106`.

No unit test in the design's earlier drafts exercised XML-RPC at all; every one
ran through `setup_test_env`, which is `ApiMode::Rest`.

**Unit (`src/client/resources/comment_tests.rs`):** a REST response whose
comment records omit `bug_id` comes back with `bug_id` backfilled from the
request; a response that supplies a `bug_id` keeps the server's value. A
single-ID flat-envelope call is pinned at the command level too, so the
`null` → requested-ID change named in *Goal* is a tested contract rather than
a side effect.

**Unit (`src/output/resources/comment_tests.rs`):** `write_comment_bug_header`
emits the bug number.

**Unit (`src/cli/comment_tests.rs`):** `comment list 1 2 3` parses to three
IDs and `--permissive` binds. The file's existing
`parse_comment_list_requires_bug_id` and
`parse_comment_list_rejects_non_numeric_bug_id` already pin the arity floor and
the value-parse failure; they must keep passing unchanged rather than being
duplicated.

**Functional (`tests/functional/phases/15-comments.sh`):** against a real
container, using `$BUG1` plus a second bug the phase creates with `make_bug`
and comments on, so the cases are self-contained —

- multi-ID JSON returns comments for both bugs, with both `bug_id`s present,
  distinct, and matching the requested IDs;
- multi-ID table shows a `Bug #<id>` header for each bug;
- `--permissive` with one real and one nonexistent ID exits 0 and still returns
  the real bug's comments — asserted as **both** "every returned record belongs
  to the real bug" **and** "at least one record came back", since a `jq all`
  over an empty array is vacuously true;
- the same pair without `--permissive` exits non-zero;
- `--permissive` with a single ID exits 7;
- multi-ID with `--fields id` keeps `bug_id` in every record;
- the credentialless path runs the multi-ID JSON case through
  `run_bzr_raw --json --server public`, the anonymous-server pattern
  `08-bugs.sh:108` already uses.

## Documentation

- `docs/bzr-cli.md`: Command Tree node becomes
  `list <BUG_ID>... [--permissive] [--since <DATE>] [--fields <F>] [--exclude-fields <F>]`
  — the flag-drift check compares that block against the binary's `--help`, so
  `--permissive` must appear there. The `### bzr comment list` section gains the
  multi-ID examples, the `--permissive` row, and the arity change on the
  `<BUG_ID>` row.
- Both documents state the `--permissive` stdout ambiguity on the flag's own
  row or bullet: under `--json`, a bug that failed is indistinguishable from a
  bug with no comments, and the failed IDs are on stderr only. An agent
  consuming this output meets the caveat where it reads the flag.
- Both documents state, on the `--fields` row or bullet, that `bug_id` is
  retained on a multi-ID call regardless of the projection flags, and why.
- `content/skills/bzr-reference/reference/commands.md`: the `comment` section
  gains the multi-ID form, the flat-array grouping idiom
  (`jq '.data | group_by(.bug_id)'` — the payload is enveloped, so the array
  lives at `.data`, matching how `bug view` is already documented), the
  `--permissive` note, and the `bug_id` retention rule.
- `src/cli/comment.rs` doc comment: describes multi-ID behavior. It must not
  reintroduce a `tags` claim — PR #701 removed a false one, and the field does
  not exist yet (issue #700).

## Threat model

Not required. The change adds no trust boundary: `comment list` is an
authenticated read that already accepts a caller-supplied bug ID and already
sends it on the same authenticated connection. The diff widens an existing
entry point's arity without changing who may call it, touches no
authn/authz/tenancy logic, handles no secret, adds no dependency, and parses
only responses the existing extractors already parse. Fan-out is unbounded and
sequential — one round trip per requested ID on a pooled connection — matching
`bug view`, so this change adds no bound and removes none. `$detect-evil` still runs
against the branch diff at review time.

## AI surfaces

None. The bzr-reference skill edit is documentation content, not an LLM call,
prompt, retrieval path, or agent loop.
