# ADR 0059: The honoured auth method, not the API mode, governs private-content completeness

## Status

Accepted

## Context

`get_comments_since` and `get_attachments` dispatch through `dispatch_xmlrpc_first`, which
branches only on `api_mode`. `version_to_api_mode` maps Bugzilla `>= 5.1` to `ApiMode::Rest`,
so the XML-RPC arm both methods' doc comments describe as "the only path that returns the full
thread"/"full set" is unreachable on every modern server. Issue #714 asked which half was
wrong: the dispatch or the documentation.

Measurement against the repo's own functional fixtures settled it: on Bugzilla 5.0.6, 5.2 and
5.3.3+, REST returns the private comment and the private attachment whenever the server
honoured the credential, and returns neither when it did not. So the loss is real but the doc
comments name the wrong cause. It is not a REST-versus-XML-RPC property and not a 5.0.x
property — 5.0.6 REST is complete under query-param auth. It is an authentication property,
and XML-RPC's apparent immunity is a side effect of carrying the key in the request body.

Full measurement table, per-version auth behaviour, and protocol-level isolation:
[2026-09-05-issue-714-private-content-dispatch-design.md](../superpowers/specs/2026-09-05-issue-714-private-content-dispatch-design.md).

## Decision

Keep `dispatch_xmlrpc_first` and `version_to_api_mode` exactly as they are. Correct the doc
comments on `get_comments_since`, `get_attachments`, and `get_attachment`, and the matching
user-facing paragraph in `docs/bzr-cli.md`, to state the measured rule. Add default-mode and
forced-`--api rest` private-content assertions to the existing functional phases
`15b-comments-private.sh` and `16b-attachments-private.sh`, which until now exercised only
`--api hybrid` and `--api xmlrpc`.

XML-RPC stays preferred in Hybrid mode, for the reason the measurement supports rather than
the one previously written down: it authenticates in the request body and is therefore immune
to a REST endpoint that ignores the configured auth method.

## Consequences

- The reachability gap the issue reported stays open by design. On `>= 5.1` the XML-RPC arms
  of these two methods remain dead code, and that is now the documented, tested expectation
  rather than an unexplained contradiction.
- The forced-`rest` assertions run on every supported version, so a future server or client
  change that reintroduces REST-side filtering under working auth turns a phase red instead of
  returning a quietly short list.
- **The residual is a default, not an opt-in, and this decision does not close it.** On a server
  `version_to_api_mode` maps to `Rest` whose REST endpoints ignore `X-BUGZILLA-API-KEY` —
  Bugzilla 5.2, measured — `bzr config set-server` with no `--auth-method` leads to `header`
  being selected, and `bzr comment list` then returns the public subset with no error. Bugzilla
  5.0 is not exposed on that path: its `Hybrid` mapping sends both reads XML-RPC-first, and the
  loss appears there only under a forced `--api rest`. So the silent truncation is the
  out-of-the-box outcome on `>= 5.1` servers that ignore the header. It is an auth-selection
  defect (`src/client/auth/valid_login.rs` probes header first, and 5.2's `/rest/valid_login`
  accepts a key its resource endpoints ignore), owned by issue #713. A guard in these two reads
  would patch a shared root cause at one of its callers while every other REST read stayed
  anonymous, and the list endpoints give it nothing to detect — unlike `attachment download`,
  whose `GET /rest/bug/attachment/<id>` returns `401` and so already recovers through the
  transport's auth-method fallback.

## Considered & rejected

- **Force XML-RPC for these two reads regardless of `api_mode`** (the issue's first suggested
  direction). verified: under an auth method the server honours it fixes nothing — `--api rest`
  and `--api hybrid` returned identical counts on 5.0.6, 5.2 and 5.3.3+ under `query_param`.
  Under one it does not honour, it *would* recover the missing content (5.2 + header auth:
  `--api rest` 3 comments / 0 private, `--api hybrid` 5 / 2), and that is the honest reason to
  weigh it. verified: it is not free — `XmlRpcClient::get_attachments` names `data` in
  `ATTACHMENT_LIST_FIELDS` (`src/xmlrpc/resources/attachment.rs`), and every `--api hybrid
  attachment list` in the measurement carried a `data` key where the REST arm did not, so each
  list would pull every attachment's base64 body. judgment: the deciding ground is layer, not
  cost — it would repair the two most visible symptoms of an auth defect that leaves every
  other REST read anonymous, hiding the rest, and it would make these two reads require
  `xmlrpc.cgi` on servers where the current code treats that endpoint as optional.
- **Detect the truncation and surface it to the caller** (the issue's third direction).
  verified: `curl -H 'X-BUGZILLA-API-KEY: <key>' <5.2>/rest/bug/<id>/comment` returns `200`
  carrying only the public comments, with no count, flag, or header distinguishing it from a
  thread that genuinely has no private comment. There is nothing in the reply to detect.
- **Make these two reads fall back when the configured auth method looks unhonoured.**
  judgment: the same root cause reaches every REST read, so the fix belongs in auth-method
  selection (issue #713), not duplicated into the two call sites that happen to expose it most
  visibly.
- **Treat the missing `data` key on `attachment list` as part of the same defect.** verified:
  `get_attachments_rest` sends `exclude_fields=data` deliberately and `ATTACHMENT_FIELDS`
  documents that `data` is populated only by `attachment download`; the measurement shows
  identical entry counts and identical `is_private` counts across modes, differing only in
  that key.
- **Do nothing.** judgment: the contradiction is what makes the reachability gap unreadable —
  a later reader cannot tell whether the dead XML-RPC arm is a bug or a deliberate residue,
  which is the question this issue was filed to ask.
