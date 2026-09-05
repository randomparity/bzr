# `bzr comment list`: accept multiple bug IDs

**Date:** 2026-09-04
**Issue:** [#699](https://github.com/randomparity/bzr/issues/699) (`enhancement`, `area:cli`, `area:agent`)
**ADR:** [0047](../../adr/0047-comment-list-client-side-multi-id-loop.md)
**Sibling precedent:** [`2026-05-06-bug-view-multi-id-design.md`](2026-05-06-bug-view-multi-id-design.md)

## Goal

`bzr comment list 1 2 3` reads the comment threads of all three bugs in one
invocation. Records stay attributable to their bug in every output format.
Single-ID behavior — table and JSON — is byte-identical to today.

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
`jq 'group_by(.bug_id)'`.

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
overwritten: the server is the authority on what a record actually belongs to,
and a divergence is evidence worth surfacing rather than erasing.

Doing this at the entry point rather than inside each transport's extractor is
deliberate — one fix covering REST, XML-RPC, and the Hybrid fallback, instead
of one guard per path.

## Transport strategy: loop on both

Both transports fetch one bug per call, via the existing per-bug
`client.get_comments_since(bug_id, since)`. XML-RPC's native `Bug.comments`
`ids` array is **not** used for batching. See
[ADR-0047](../../adr/0047-comment-list-client-side-multi-id-loop.md) for the
decision and its rejected alternatives; the summary is:

- REST cannot batch at all. Bugzilla's REST comment resource is
  `bug/{id_or_alias}/comment` with a single path segment and no `ids` query
  parameter, so a REST loop is forced regardless.
- Batching only on XML-RPC would make the same command's per-ID failure
  behavior depend on which transport the server exposes. `bug view` rejected
  server-side multi-ID batching for exactly this reason: it "silently drops
  inaccessible IDs and would lose per-ID error detail."
- The win the issue actually asks for is N-1 *process invocations* — config
  load, credential resolution, API-mode detection, TLS handshake — not N-1 HTTP
  round trips. A client-side loop captures that win in full, and reqwest's
  connection pool keeps the N requests on one connection.
- Looping keeps `dispatch_xmlrpc_first`'s Hybrid REST fallback at per-bug
  granularity, which is where it already operates today.

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

Checked at handler entry, before any network call, all exit 7
(`BzrError::InputValidation`):

| Condition | Message |
|---|---|
| more than 100 IDs | `comment list accepts at most 100 bug ids` |
| `--permissive` with exactly one ID | `--permissive only meaningful with multiple bug ids` |

The 100-ID cap matches `bug adjacency`'s `MAX_REQUESTS`
(`src/commands/bug/adjacency.rs:13`). Each ID is one round trip, so an
unbounded list is an unbounded fan-out; the cap is the same number the sibling
verb already teaches. Duplicate IDs are not rejected or deduplicated — `bug
view` fetches and prints duplicates as given, and matching that is cheaper than
a rule users would have to learn.

## Per-bug failure handling

Without `--permissive`, the first per-bug failure aborts the whole call with
that error's own exit code. For a single ID this is exactly today's behavior.

With `--permissive`, a failure that `BzrError::is_permissive_bug_view_error()`
classifies as per-resource (`NotFound`, or `Api` with a `Bug.get` per-resource
fault code) is reported on stderr as one line and the loop continues; the
command exits 0 even if every bug failed. Session-wide failures — transport,
auth, TLS, deserialization — still bail immediately, in both modes.

Failure lines go to **stderr in every output format**:

```text
bug 999: not found: 999
```

Failures are not carried in the JSON payload. A flat array of `Comment` records
has nowhere to put a failure entry, and adding one would be the wrapper this
design rejected. Routing them to stderr keeps one mechanism across table and
JSON, keeps stdout a clean comment stream for `jq` and pipelines, and matches
where ADR-0007 already puts diagnostic output.

Reusing `is_permissive_bug_view_error()` rather than adding a comment-specific
predicate is intentional: the per-resource fault codes are `Bug.get`'s and
apply identically to `Bug.comments`, and one predicate keeps `bug view` and
`comment list` classifying the same server fault the same way.

## Output composition

`src/commands/comment/list.rs` branches on ID count and format:

- **One ID** — the current path verbatim: one `get_comments_since`, one
  `write_comments`. No header, no behavior change.
- **Multiple IDs, table** — for each ID in order, write a `Bug #<N>` header,
  then that bug's comments through the existing `write_comments`. Without the
  header a multi-bug table is an undifferentiated wall of `Comment #<n>`
  blocks with no way to tell whose thread is whose.
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
| Multi-ID, `--permissive`, transport/auth failure | abort | that error's code |
| `--permissive` with one ID | rejected before any call | 7 |
| More than 100 IDs | rejected before any call | 7 |
| Unknown `--fields` | rejected before any call, unchanged | 7 |

Partial success does not use `BzrError::BatchPartialFailure` (exit 11): failed
reads change no server state, and `bug view` set the same precedent.

## Testing

**Unit (`src/commands/comment/list_tests.rs`, wiremock):**

- single ID emits a bare array and no `Bug #` header (table and JSON);
- three IDs, JSON: one flat array, records carry each requested `bug_id`, order
  follows argument order;
- three IDs, table: one `Bug #<N>` header per bug, in argument order;
- a bug with zero comments contributes no records to JSON and a header plus
  `No comments.` to the table;
- `--permissive` with a 404 middle bug: exit 0, stderr names the failed bug,
  stdout holds the other two bugs' comments;
- without `--permissive` the same 404 aborts and no partial JSON is written;
- `--permissive` with one ID exits 7 before any request is made;
- 101 IDs exits 7 before any request is made;
- `--since` and `--fields` apply to every bug in a multi-ID call.

**Unit (`src/client/resources/comment_tests.rs`):** a REST response whose
comment records omit `bug_id` comes back with `bug_id` backfilled from the
request; a response that supplies a `bug_id` keeps the server's value.

**Unit (`src/output/resources/comment_tests.rs`):** `write_comment_bug_header`
emits the bug number.

**Unit (`src/cli/comment_tests.rs`):** `comment list 1 2 3` parses to three
IDs; a non-numeric ID is a `ValueValidation` clap error; zero IDs is a clap
error.

**Functional (`tests/functional/phases/15-comments.sh`):** against a real
container, using `$BUG1` plus a second bug the phase creates with `make_bug`
and comments on, so the cases are self-contained —

- multi-ID JSON returns comments for both bugs, with both `bug_id`s present,
  distinct, and matching the requested IDs;
- multi-ID table shows a `Bug #<id>` header for each bug;
- `--permissive` with one real and one nonexistent ID exits 0 and still returns
  the real bug's comments;
- the same pair without `--permissive` exits non-zero;
- `--permissive` with a single ID exits 7;
- 101 IDs exits 7;
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
- `content/skills/bzr-reference/reference/commands.md`: the `comment` section
  gains the multi-ID form, the flat-array grouping idiom, and the
  `--permissive` note.
- `src/cli/comment.rs` doc comment: describes multi-ID behavior. It must not
  reintroduce a `tags` claim — PR #701 removed a false one, and the field does
  not exist yet (issue #700).

## Threat model

Not required. The change adds no trust boundary: `comment list` is an
authenticated read that already accepts a caller-supplied bug ID and already
sends it on the same authenticated connection. The diff widens an existing
entry point's arity without changing who may call it, touches no
authn/authz/tenancy logic, handles no secret, adds no dependency, and parses
only responses the existing extractors already parse. The one new bound — the
100-ID cap — narrows fan-out rather than widening it. `$detect-evil` still runs
against the branch diff at review time.

## AI surfaces

None. The bzr-reference skill edit is documentation content, not an LLM call,
prompt, retrieval path, or agent loop.
