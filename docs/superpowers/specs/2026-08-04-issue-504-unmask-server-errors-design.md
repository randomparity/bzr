# Issue #504 — `bug view` reports "bug not found" for restricted bugs

Design and test plan. Decision record: [ADR 0015](../../adr/0015-server-errors-are-never-masked.md).

## Problem

`bzr bug view <id>` intermittently prints `error: bug not found: <id>` (exit 2)
for a bug in an access-restricted product the caller can see. Retrying succeeds.

`NotFound` for a bug is reachable only from "HTTP 200, valid JSON, empty `bugs`
array". Two code paths convert a server-supplied reason into that state:

| # | Path | Trigger |
|---|---|---|
| 1 | `client/response.rs::has_data_fields` treats a present-but-empty data key as data, so `check_bugzilla_200_error` downgrades a real error to a `warn!` | server emits `{"error":true,"code":102,…,"bugs":[]}` |
| 2 | `client/resources/bug.rs::get_bug_via_search` returns `NotFound` when the 100500 search retry finds nothing, discarding the original error | extension crashes on the direct lookup (load-dependent) and the caller's access is not expressible through the search path |

Both triggers are load-dependent server-side, hence the intermittency.

## Production changes

### P1 — `has_data_fields` requires content

```
key counts as data  ⟺  value is  non-empty array
                                 ∨ non-empty object
                                 ∨ non-null scalar
```

`[]`, `{}` and `null` do not count. When no `DATA_KEYS` entry carries content, an
`error: true` payload is fatal (`BzrError::Api { code, message }`), identical to
the key being absent.

Two envelope shapes must keep working, and both are covered by the rule as
stated:

- `{"error":true,…,"bugs":[{…}]}` — IBM LTC error-beside-data. Non-empty array →
  lenient, unchanged (`response_tests.rs::api_error_with_200_and_data_returns_data`).
- `{"bugs":{"42":{"comments":[]}}}` — bug acknowledged, zero comments. The
  top-level `bugs` **map** has one key → non-empty → unchanged. Only `bugs: {}`
  becomes empty, which `comment.rs::extract_bugs_comment_envelope` already treats
  as a non-match.

`DATA_KEYS` is unchanged.

### P2 — the 100500 fallback preserves its cause

`get_bug_rest` passes the original `BzrError::Api { code: 100500, .. }` into
`get_bug_via_search`. When the search retry yields no rows, that error is
returned — annotated to record that the fallback ran and found nothing — instead
of `NotFound`.

`NotFound` remains correct for the direct path returning an empty result with no
error payload: the only case where "no such bug" is what the server said.

## Test plan

### Unit (wiremock, `#[tokio::test]`)

Neither path is reachable from a stock container, so these are the primary guard.

| Test | Fixture | Assertion |
|---|---|---|
| `error_with_empty_data_key_is_fatal` | 200 `{"error":true,"code":102,"message":"You are not authorized to access bug #216593","bugs":[]}` | `BzrError::Api{code:102}`, message relayed; **not** `NotFound` |
| `error_with_empty_object_data_key_is_fatal` | 200 `{"error":true,"code":102,…,"bugs":{}}` | as above |
| `error_with_null_data_key_is_fatal` | 200 `{"error":true,"code":102,…,"bugs":null}` | as above |
| `error_beside_populated_data_is_still_lenient` | existing `api_error_with_200_and_data_returns_data` | unchanged — regression guard for the IBM LTC path |
| `comment_envelope_with_zero_comments_is_not_an_error` | 200 `{"error":true,…,"bugs":{"42":{"comments":[]}}}` | non-empty outer map → lenient |
| `search_fallback_empty_preserves_100500` | direct → 200 `{"error":true,"code":100500,…}`; search → 200 `{"bugs":[]}` | `Api{code:100500}`; **not** `NotFound` |
| `search_fallback_success_still_returns_bug` | direct → 100500; search → 200 with a bug | `Ok(bug)` — regression guard for the existing fallback |
| `direct_empty_without_error_is_still_not_found` | 200 `{"bugs":[]}` | `NotFound` — the reserved case |

### Functional (`tests/functional/`)

The suite exercises restricted content but **cannot detect this defect**:

- `08c-bugs-create-fields.sh:85` test 146c asserts `assert_failure`
  (`lib.sh:162` — exit `!= 0` only), which passes for exit 2, 4, 5 and 9 alike.
- Every version entrypoint seeds exactly **one** `user_api_keys` row, always
  `admin@test.bzr` (`bz50:45`, `bz52:56`, `bz53:72`), and both credentialed
  server configs (`test`, `auto`) use it. 146c's positive case is therefore the
  bug's own reporter-and-admin — visible via reporter/assignee/admin paths, never
  purely via group membership. The reporter's scenario is untested.
- `07-groups.sh:27a` seeds `group_control_map` with `entry=0`, so `FuncTestProd`
  is not entry-restricted; only individual bugs get grouped. A group-restricted
  *product* is a different Bugzilla path with a different envelope.

Changes:

- **F1** — 146c: replace `assert_failure` with `assert_exit_code` plus a stderr
  assertion, so a restricted bug misreported as not-found reddens the suite.
- **F2** — seed a second, non-admin `user_api_keys` row across bz50/bz52/bz53 and
  add a server config for it; cover the authenticated-group-member direction
  (member sees the restricted bug; non-member gets an access error, not
  not-found).
- **F3** — add an entry-restricted product fixture (`group_control_map` with
  `entry=1`) and cover the product-level restriction path.

## Out of scope

- The server-side intermittency itself (Bugzilla under load); bzr cannot fix it,
  and `--retry` already covers the 5xx arm.
- Any change to auth resolution, credential storage, TLS, config persistence, or
  the XML-RPC protocol.
- Adding a `BzrError` variant — `Api` already carries `api_code` (ADR-0014).

## Open

A `-vvv` trace from the reporter (requested on the issue) would confirm which
path fired in their deployment. Both are defects independently and both are
fixed here, so it does not gate the work.
