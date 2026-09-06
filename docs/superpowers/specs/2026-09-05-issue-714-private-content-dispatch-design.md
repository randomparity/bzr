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

1. Rewrite the three doc comments that misattribute the cause — `get_comments_since`,
   `get_attachments`, `get_attachment` — to state the measured rule: private content is
   complete whenever the server honoured the credential; XML-RPC is preferred in Hybrid
   mode because it authenticates in the request body and so is immune to a REST endpoint
   that ignores the configured auth method. Correct the same claim in the header comment of
   `tests/functional/phases/16b-attachments-private.sh`.
2. Extend `tests/functional/phases/15b-comments-private.sh` and
   `16b-attachments-private.sh` with default-mode and forced `--api rest` private-content
   assertions. Today both phases assert only `--api hybrid` and `--api xmlrpc`, which is why
   nothing held the claim to account. The forced-`rest` assertion runs on every supported
   version including 5.0, so it refutes the old wording where the old wording made its claim.

No dispatch, version-mapping, or auth behaviour changes. `dispatch_xmlrpc_first` and
`version_to_api_mode` are untouched.

## Residual, not fixed here

A client configured for `header` auth against Bugzilla <= 5.2 still reads `comment list` and
`attachment list` anonymously and gets a silently incomplete answer. The root cause is
auth-method selection accepting a method the REST endpoint does not honour, in
`src/client/auth/` — outside this issue's surface and owned by issue #713. A detection guard
added to these two reads would be a caller-side patch over a shared root cause; reported as a
follow-up with the measurement above instead.

## Testing

Functional only. The contract is "a real server returns private content over REST when
authenticated", which no wiremock fixture can prove — a fixture only replays a shape someone
assumed. Each new assertion is verified to bite by inverting its expectation once and
observing red.
