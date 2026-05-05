# Hybrid-mode XML-RPC fallback for `comment list` (issue #125)

**Status:** Approved (brainstorming complete)
**Target release:** v0.2.1
**Tracking issue:** [#125](https://github.com/randomparity/bzr/issues/125)
**Related, deferred:** [#133](https://github.com/randomparity/bzr/issues/133) (verify whether attachments need the same treatment)

## Background

`bzr comment list <bug>` calls `GET /rest/bug/<id>/comment` and prints whatever
the server returns. On the Bugzilla 5.0.6 deployment named in #125, REST
silently drops private comments under API-key auth (returning only the public
subset), while the XML-RPC `Bug.comments` method on the same server returns the
full set. `bzr` already has an embedded XML-RPC client and a Hybrid API mode
(REST primary, XML-RPC fallback) used today for `Group.get` (on error 32610)
and bug search (on empty-but-filtered results) — but the comment-list path
goes straight to REST with no fallback.

This spec extends Hybrid mode to cover comment listing.

## Goals

1. On healthy Bugzilla deployments (any version): no behavior change. REST is
   used, no extra round-trips, no XML-RPC traffic.
2. On deployments where REST truncates private comments (the #125 scenario):
   `bzr comment list` returns the full comment set transparently when
   `api_mode = "hybrid"` (the default for newly-detected servers).
3. Users on `api_mode = "rest"` keep the legacy behavior — explicit choice is
   respected; no second-guessing.
4. Users on `api_mode = "xmlrpc"` get a direct XML-RPC path (no wasted REST
   call) using the new `Bug.comments` method.
5. `bzr comment add --private` lets users create private comments directly
   via `bzr` (closes a CLI gap and makes functional testing of this feature
   straightforward without dropping to `curl`).

## Non-goals

- Attachments. The same private-resource issue *may* affect
  `bzr attachment list`; tracked separately as #133. Unlike comments,
  attachments have no sequential `count` field, so a different detection
  strategy would be required.
- A general-purpose "REST truncation detector" framework. We add one bespoke
  gap check for comments. If a third resource needs it, that is the time to
  extract a shared abstraction (rule of three).
- `bzr comment get <comment_id>` — the single-comment REST endpoint is not
  currently wired in `bzr`; nothing to fix and nothing to test.
- New CLI flags, config schema changes, or migration. Existing
  `~/.config/bzr/config.toml` files keep working unchanged.

## Architecture

The change spans the comment list path (the core of the issue), and a small
additive change to the comment add path (the `--private` flag):

```
src/xmlrpc/client.rs       (+) get_comments_since(bug_id, since)
src/client/comment.rs      (~) get_comments_since: hybrid orchestrator
                           (+) has_count_gaps: pure helper
                           (~) get_comments_since_rest: extracted REST body
                           (~) add_comment: accept is_private parameter
                           (~) AddCommentBody: include is_private field
src/cli/comment.rs         (~) Add::is_private flag (--private)
src/commands/comment.rs    (~) plumb --private into add_comment call
```

One small CLI surface change (`--private` on `bzr comment add`). No config
schema changes. No new error variants (transport errors propagate via
existing `BzrError::Http`; XML-RPC faults via `BzrError::XmlRpc`).

### Mode behavior matrix

| `api_mode` | Path |
|---|---|
| `Rest` | REST only. No gap detection, no fallback. Explicit user choice respected. |
| `XmlRpc` | XML-RPC `Bug.comments` direct. No REST call. |
| `Hybrid` | REST → if count gaps OR transport failure → XML-RPC `Bug.comments`. |

This mirrors the existing patterns in `client/group.rs::get_group` and
`client/bug.rs::get_bug` / `search_bugs`.

## Detection: count-sequence gaps

Bugzilla comments carry a sequential `count` field (`0` for the bug
description, then `1`, `2`, ... in chronological order). The REST response
preserves this field. A truncated REST response is detectable as a gap in
this sequence.

```rust
fn has_count_gaps(comments: &[Comment], since_provided: bool) -> bool {
    if comments.is_empty() {
        return false;
    }
    let first = comments[0].count;
    if !since_provided && first != 0 {
        return true;
    }
    comments.windows(2).any(|w| w[1].count != w[0].count + 1)
}
```

| Counts | `since`? | Gap? | Reasoning |
|---|---|---|---|
| `[]` | any | no | empty response, not truncation |
| `[0,1,2,3,4,5]` | none | no | healthy full set |
| `[0,4]` | none | yes | original issue — counts 1,2,3 missing |
| `[4]` | none | yes | only public; 0..3 missing |
| `[5,6,7]` | yes | no | filtered start, contiguous |
| `[5,7]` | yes | yes | gap inside response |

### False-positive: deleted comments

Some Bugzilla deployments support comment deletion, which would create a
legitimate gap in the count sequence. In that case the heuristic fires, the
fallback calls XML-RPC, XML-RPC sees the same gap, and we return whatever
XML-RPC has. Cost: one extra XML-RPC call. Outcome: identical or better data
than REST alone. The bounded false-positive cost is acceptable; we do not add
a more sophisticated detector.

## Hybrid orchestration

```rust
pub async fn get_comments_since(
    &self,
    bug_id: u64,
    since: Option<&str>,
) -> Result<Vec<Comment>> {
    match self.api_mode {
        ApiMode::XmlRpc => return self.xmlrpc_client()?
            .get_comments_since(bug_id, since).await,
        ApiMode::Rest | ApiMode::Hybrid => {}
    }

    match self.get_comments_since_rest(bug_id, since).await {
        Ok(comments) if self.api_mode == ApiMode::Hybrid
            && has_count_gaps(&comments, since.is_some()) =>
        {
            tracing::info!(
                bug_id,
                rest_count = comments.len(),
                "REST comment list has count gaps, retrying via XML-RPC"
            );
            self.xmlrpc_client()?.get_comments_since(bug_id, since).await
        }
        Ok(comments) => Ok(comments),
        Err(e) if self.api_mode == ApiMode::Hybrid && e.is_transport_failure() => {
            tracing::info!(
                bug_id,
                "REST comment list failed ({e}), retrying via XML-RPC"
            );
            self.xmlrpc_client()?.get_comments_since(bug_id, since).await
        }
        Err(e) => Err(e),
    }
}
```

`get_comments_since_rest` is the current REST-only body, extracted as a
private helper — same refactor pattern as `get_group_rest`.

If XML-RPC also fails, the XML-RPC error propagates. We do not merge results,
do not retry REST again, and do not invent a new error type.

## XML-RPC `Bug.comments` mapping

`Bug.comments` returns a structurally identical response to the REST endpoint:

```
bugs: { "<bug_id>": { comments: [<Comment>, ...] } }
```

Each comment carries `id`, `bug_id`, `text`, `creator`, `creation_time`,
`count`, `is_private`, `tags` — all fields already present on
`types::Comment`. No new type is introduced.

Implementation in `src/xmlrpc/client.rs`, following the existing pattern of
`search_bugs` / `get_bug`:

```rust
pub async fn get_comments_since(
    &self,
    bug_id: u64,
    since: Option<&str>,
) -> Result<Vec<Comment>> {
    let mut params = HashMap::new();
    params.insert("ids", XmlValue::Array(vec![XmlValue::Int(bug_id as i64)]));
    if let Some(s) = since {
        params.insert("new_since", XmlValue::String(s.to_string()));
    }
    let response: BugCommentsResponse =
        self.call("Bug.comments", &[XmlValue::Struct(params)]).await?;
    Ok(response.bugs.into_values().next().map_or_else(Vec::new, |e| e.comments))
}
```

The `new_since` parameter accepts the same ISO-8601 timestamp format as REST,
so the existing CLI input is passed through unchanged. If the existing
parsing helpers in `xmlrpc/parsing.rs` don't already cover every field on
`Comment` (notably the `tags` array), the implementation extends them — a
small mechanical change, not architecturally significant.

## Tests

### Unit: `has_count_gaps` (in `client/comment.rs`)

- `[]` → false
- `[0,1,2,3]` no since → false
- `[0,4]` no since → true (issue scenario)
- `[4]` no since → true
- `[5,6,7]` with since → false
- `[5,7]` with since → true

### Mock integration: hybrid orchestrator (in `client/comment.rs`, wiremock)

- REST returns `[0..5]`, `Hybrid` → no fallback fires; XML-RPC mock receives 0 calls.
- REST returns `[4]`, `Hybrid` → fallback fires; XML-RPC mock returns full set; that set is returned.
- REST returns 502, `Hybrid` → fallback fires; XML-RPC result returned.
- REST returns `[4]`, `Rest` → no fallback; `[4]` returned (explicit choice respected).
- `XmlRpc` mode → REST mock receives 0 calls; XML-RPC called directly.

### XML-RPC unit (in `xmlrpc/client.rs`)

- Canned `methodResponse` body parsed into `Vec<Comment>`, including
  `is_private = true` entries.
- `since` parameter correctly serialized into the `methodCall` body.

### Functional: `tests/functional/run-tests.sh`

Run on bz50, bz52, bz53.

**Reproduction probe (manual, during implementation):** Against the running
bz50 container, create a bug, add ≥2 comments where one is private (using
`curl` against the REST API since 0.2.0 has no `--private` flag), then run
current `bzr` 0.2.0 with `bzr comment list` using the API-key-authenticated
test user. Compare what REST returns to what was created. This determines
whether bz50 reproduces the #125 truncation behavior.

**Phase 14b: private-comment visibility (Hybrid mode)** — Create a bug with
one public and one private comment, then `bzr --api-mode hybrid comment list`,
assert both comments returned. Runs on all three containers:

- Healthy fixtures (bz52/bz53, and bz50 if it does not reproduce): passes via
  the REST path with no fallback needed.
- bz50 if it reproduces: passes via the count-gap fallback. Live proof the
  fallback works against a real Bugzilla, not just a mock.

Same assertion either way: all created comments come back.

**Phase 14c: explicit XML-RPC mode** — Same setup, run with
`--api-mode xmlrpc`, assert both comments returned. Exercises the new
XML-RPC `Bug.comments` method on every supported Bugzilla version.

No version-conditional skips. Both phases run unconditionally on all three
containers.

## Versioning, changelog, docs

### `Cargo.toml`

Bump `version = "0.2.0"` → `version = "0.2.1"`. Additive change; semver patch
appropriate at 0.x.

### `CHANGELOG.md`

Add a new section above `## [0.2.0] - 2026-05-04`. The date is filled in at
release-prep time, per project convention (CHANGELOG entries land *with* the
work; the release-prep commit confirms the date).

```markdown
## [0.2.1] - YYYY-MM-DD

### Fixed

- `bzr comment list` now returns private comments on Bugzilla
  deployments where the REST endpoint silently truncates them
  (observed on Bugzilla 5.0.x). In Hybrid API mode (the default
  for newly-detected servers), `bzr` detects gaps in the comment
  count sequence returned by REST and transparently retries the
  request via XML-RPC `Bug.comments`. No configuration change
  required. Affects #125.

### Added

- XML-RPC `Bug.comments` support in the embedded XML-RPC client,
  used by Hybrid-mode comment fallback and directly when a server
  is configured with `api_mode = "xmlrpc"`.
- `bzr comment add --private` flag, sets `is_private: true` on the
  posted comment.
```

### `docs/bzr-cli.md`

The existing `bzr comment list` description does not mention API mode
behavior; nothing to change there. If a higher-level "API mode" doc section
exists, add one bullet noting that comment list participates in Hybrid
fallback. Verified during implementation.

## Implementation outline (handed to writing-plans)

1. Add `has_count_gaps` helper + unit tests in `client/comment.rs`.
2. Add `get_comments_since` to `xmlrpc/client.rs` + unit test.
3. Refactor REST body in `client/comment.rs` into `get_comments_since_rest`.
4. Replace `get_comments_since` body with the hybrid orchestrator.
5. Add wiremock orchestrator tests.
6. Add functional Phase 14b/14c blocks to `tests/functional/run-tests.sh`.
   Run reproduction probe against bz50 as part of this step.
7. Bump `Cargo.toml` to `0.2.1`.
8. Add `## [0.2.1]` section to `CHANGELOG.md`.
9. Verify `make lint` and `cargo test` pass.
10. Run `make functional-test-all` and confirm Phase 14b/14c pass on all
    three containers.
