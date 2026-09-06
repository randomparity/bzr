# Issue #714 — private-content reads: what actually governs completeness

Design record for issue #714. Decision: [ADR 0059](../../adr/0059-auth-method-governs-private-content-completeness.md).

## Problem

`get_comments_since` (`src/client/resources/comment.rs`) and `get_attachments`
(`src/client/resources/attachment.rs`) dispatch through `dispatch_xmlrpc_first`, which
branches only on `api_mode`. `version_to_api_mode` (`src/client/version.rs`) maps Bugzilla
`>= 5.1` to `ApiMode::Rest`, so on a modern server the XML-RPC arm is unreachable. Both
methods carry a doc comment claiming XML-RPC is "the only path that returns the full
thread"/"full set", attributing the loss to "Bugzilla 5.0.x REST". Code and documentation
cannot both be right.

## Measurement

Taken on this branch against the repo's own functional fixtures (`tests/functional/`),
one bug per run carrying one public and one private comment and one public and one private
attachment, admin credentials in `insidergroup`. `private` counts entries with
`is_private: true`.

| Server | auth method | `--api rest` comments / attachments | `--api hybrid` |
|---|---|---|---|
| 5.0.6 | `query_param` | 5 / 2 (private 2 / 1) | 5 / 2 (private 2 / 1) |
| 5.0.6 | `header` | 3 / 1 (private **0 / 0**) | 5 / 2 (private 2 / 1) |
| 5.2 | `query_param` | 5 / 2 (private 2 / 1) | 5 / 2 (private 2 / 1) |
| 5.2 | `header` | 3 / 1 (private **0 / 0**) | 5 / 2 (private 2 / 1) |
| 5.3.3+ | `query_param` | 5 / 2 (private 2 / 1) | 5 / 2 (private 2 / 1) |
| 5.3.3+ | `header` | 5 / 2 (private 2 / 1) | 5 / 2 (private 2 / 1) |

Isolated at the protocol level: `curl -H 'X-BUGZILLA-API-KEY: <key>' <5.2>/rest/bug/<id>/comment`
returns `200` with only the public comments; the same URL with `?api_key=<key>` returns the
private ones too. Bugzilla 5.0 and 5.2 ignore the header and answer anonymously; 5.3 honours it.
XML-RPC is unaffected in every row because `XmlRpcClient` always carries the key in the request
body (`src/client/mod.rs`, the log line about overriding configured header auth).

**REST is not lossy by version.** The variable is whether the server honoured the credential
at all. The doc comments name the wrong variable, and they name it in the one direction that
cannot be reproduced: 5.0.6 REST returns private content fine under query-param auth.

Two further observations, both already intended behaviour and not part of this defect:

- `attachment list` omits `data` under REST because `get_attachments_rest` sends
  `exclude_fields=data`; `ATTACHMENT_FIELDS` documents this. That is a payload
  optimisation, not filtering.
- `attachment download` succeeds even against a header-auth 5.2 server: `GET
  /rest/bug/attachment/<id>` answers `401` when anonymous, and the transport's
  401 auth-method fallback retries and succeeds. The **list** endpoints answer `200`
  with entries removed, so no fallback can fire — which is exactly the
  "not reliably detectable" property the doc comments assert.

## Change

Two parts, both prose-and-tests. Every claim that names the cause is rewritten to the measured
rule — the doc comments on `get_comments_since`, `get_attachments`, `get_attachment` and the
`BugzillaClient` struct, `docs/bzr-cli.md`'s hybrid-transport paragraph, and both phase header
comments. Then `15b-comments-private.sh` and `16b-attachments-private.sh` gain default-mode and
forced `--api rest` private-content assertions; today they assert only `--api hybrid` and
`--api xmlrpc`, which is why nothing held the claim to account.

No dispatch, version-mapping, or auth behaviour changes: `dispatch_xmlrpc_first` and
`version_to_api_mode` are untouched, and nothing about the compiled artifact changes. Exact
edits and per-case verification live in the plan.

## Residual, not fixed here — corrected after #713 merged

**The original framing of this section is no longer true, and the correction matters.** It said
the loss was the out-of-the-box outcome on `>= 5.1` servers that ignore the header, because a
default `config set-server` selected `header` there. Issue #713 merged while this change was in
review and replaced the any-2xx `/rest/bug?limit=1` probe that caused that selection.

Re-measured against merged `main` rather than reasoned about, same three containers:

| Server | default `set-server` persists | default dispatch | `--api rest` |
|---|---|---|---|
| 5.0.6 | `query_param` | 5 (2) / 2 (1) | 5 (2) / 2 (1) |
| 5.2 | `query_param` | 5 (2) / 2 (1) | 5 (2) / 2 (1) |
| 5.3.3+ | `header` | 5 (2) / 2 (1) | 5 (2) / 2 (1) |

So the default path is complete on every supported version. What remains is reachable only by
pinning `--auth-method header` against a server whose REST ignores it: on 5.2 that loses private
content in both default dispatch and `--api rest` (3 of 5 comments, 1 of 2 attachments, exit 0);
on 5.0.6 only under forced `--api rest`, since 5.0.x maps to `Hybrid`; 5.3.3+ is unaffected.

That is a user who overrode detection with a value the server does not support — a materially
weaker condition than a default-path harm. ADR 0059's Consequences carry it; nothing here fixes
it, and #713 owns the detection that made it reachable by default.

## A second finding: the existing assertions cannot fail

Both phases test private visibility with a bare conjunct:

```bash
if assert_success &&
    assert_json_array_min_length '.' 4 &&
    [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
    test_pass
fi
```

Only `test_pass` / `test_fail` / `test_skip` move the counters (`tests/functional/lib.sh`).
The two `assert_*` helpers call `test_fail` themselves when they fail; the bare `[[ ]]` does
not, and there is no `else`. So when *only* the private-content clause is false — the exact
#125 / #133 regression these phases exist to catch — nothing increments, the case renders no
result, and the suite exits 0 having reported it as neither passed nor failed.

A private-content phase whose private clause cannot fail is worse than no phase: it reports
green over the regression it exists to catch. Same class as the defect PR #728 fixed in
`08c-bugs-create-fields.sh`. Fixed here, before the new cases are added, by reusing
`assert_json_array_min_length` with a jq expression rather than adding a helper.

## Testing

Functional only. The contract is "a real server returns private content over REST when
authenticated", which no wiremock fixture can prove — a fixture only replays a shape someone
assumed. Each new assertion carries its own red recipe in the plan, chosen per case: the
obvious inversion does not work, because `is_private` is present and `false` on public
entries, so flipping the predicate merely counts those instead.
