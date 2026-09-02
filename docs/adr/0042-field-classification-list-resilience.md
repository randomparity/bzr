# ADR 0042: Keep response leniency and disabled-classification policy at their owners

## Status

Accepted

## Context

Bugzilla omits `values` from `/rest/field/bug` rows that do not have legal values. bzr models
that endpoint twice: the field-list model requires `values`, while the server-capabilities model
defaults it. The same server may therefore be accepted by one command and rejected by another.

On a stock server with classifications disabled, an unprivileged caller can read the
`classification` field but receives API error 900 when bzr follows its value with
`GET /classification/{name}`. The command's existing disabled-server note is consequently
unreachable for that caller. Machine-readable output must remain valid JSON or NDJSON while the
human table form reports the disabled state directly.

## Decision

Define one internal field-definition response model in the field resource and reuse it for both
single-field lookup and server capabilities. Its `values`, `type`, and `is_custom` members retain
the server-capabilities defaults. Percent-encode the resolved field name before placing it in the
request path.

Keep API error 900 distinguishable at the command boundary. `classification list` treats that
specific response as a disabled-classification state: table output prints the existing note on
stdout and stops, while JSON-family output writes an empty collection and sends the note to
stderr so structured stdout stays parseable. Other API errors propagate unchanged. A lone fetched
`Unclassified` row uses the same presentation policy.

Keep `Classification.sort_key` unsigned. ADR 0028 intentionally excludes unrelated sort keys,
and issue #629 permits explicitly deferring the separate major-schema retype.

## Consequences

All consumers of `/rest/field/bug` share omission behavior. Non-select field lookup reaches the
existing empty-values message, and arbitrary field names cannot add path structure. Disabled
classification listing succeeds for unprivileged callers without hiding unrelated failures.
JSON-family callers receive an empty list rather than human prose on stdout.

The classification sort-key wire domain remains unchanged and retains the fork/direct-database
negative-value limitation recorded by issue #629.

## Considered & rejected

- **Add `#[serde(default)]` only to the field-list duplicate.** judgment: it preserves two models
  of one endpoint and lets their next wire-shape change diverge again.
- **Return an empty classification list from the client for error 900.** judgment: it erases the
  distinction between a genuinely empty enabled server and a permission-gated disabled server,
  leaving presentation policy to inference.
- **Print the disabled note on stdout for every format.** verified: `tests/functional/lib.sh`
  invokes commands with `--json` and parses stdout with `jq`; prose on that stream invalidates the
  documented envelope.
- **Retype `Classification.sort_key` in this change.** verified: ADR 0028 classifies the analogous
  published payload-domain retype as a major schema change and explicitly leaves unrelated sort
  keys unchanged; issue #629 permits deferral instead of carrying that cascade.

