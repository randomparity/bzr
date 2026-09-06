# ADR 0056: Header-auth verification is a differential probe, not a 2xx check

## Status

Accepted

## Context

`detect_auth_method` (`src/client/auth/mod.rs`) falls back to `rest/valid_login` when
`rest/whoami` is unavailable — the Bugzilla 5.0/5.2 path. When `valid_login` reports that
query-parameter auth works and header auth does not, bzr does not take that answer at face
value. It re-verifies header auth against a real API endpoint, because some servers (the
doc comment names IBM LTC Bugzilla) reject `X-BUGZILLA-API-KEY` at `valid_login` while
honouring it everywhere else. Header auth is preferred when both work: it keeps the API key
out of URLs and therefore out of server access logs.

The verification used `GET /rest/bug?limit=1` and treated **any 2xx** as proof. Stock
Bugzilla answers that endpoint 200 to an anonymous caller, so the probe returned true
whether or not the header was honoured. A verification step that cannot fail for the
condition it verifies provides no signal, and this one actively overrode a correct negative:
bzr computed `AuthMethod::QueryParam` from `valid_login`, then discarded it and returned
`AuthMethod::Header`.

The evidence is issue #713's measurement against a stock Bugzilla 5.2 fixture (image
`bugzilla/bugzilla` at `644c66f4`): a docroot grep for every spelling of the API-key header
returns zero hits while `Bugzilla_api_key` matches `Bugzilla/Auth/Login/APIKey.pm`, and the
`X-BUGZILLA-API-KEY` column of the issue's request table is byte-identical to the anonymous
column. Stock Bugzilla has no API-key-header code path at all, so the probe's positive is
always wrong there. Reads then returned 200 with a narrower body — `estimated_time` and
private comments absent — which is indistinguishable from a legitimately empty result.

The question this record answers is what evidence a client can collect that actually
separates an honoured header from an ignored one, on a server whose version, permission
model and extension set it does not know.

## Decision

**Header auth is preferred over the method `valid_login` already proved only when a
differential probe shows the header response is a success, is distinguishable from the
anonymous response, and matches the response query-parameter auth produces. Every other
outcome — including every inconclusive one — keeps the method `valid_login` proved.**

The probe issues up to three `GET {base}/rest/user?names={login}` requests:

1. **Header** — with `X-BUGZILLA-API-KEY`.
2. **Anonymous** — no credentials. If this response equals the header response, the header
   changed nothing and the probe ends: not confirmed.
3. **Query-parameter** — with `Bugzilla_api_key`, the method `valid_login` proved. Header
   auth is confirmed if and only if this response equals the header response.

**Every leg must complete, return a success status, and not carry a Bugzilla error body.**
A transport failure, an unreadable body, a non-2xx status, or a JSON object with a truthy
top-level `error` key on *any* of the three ends the probe as not confirmed. This is
load-bearing rather than tidiness: without it, a leg that fails for a reason unrelated to
auth is unequal to (or equal to) its peers for that unrelated reason, and on a server whose
`rest/user` record does not discriminate the third leg then matches the first — confirming
header auth on the strength of an error, which is exactly the defect being fixed.

The error-body half is not hypothetical on the server class this fallback exists for.
`Response::check_bugzilla_200_error` (`src/client/response.rs`) exists precisely because
"some servers (e.g. IBM LTC Bugzilla) include error fields alongside valid data" in an HTTP
200. A status check alone would accept two identical 200-error responses on the header and
query-parameter legs as agreement. This probe's test is deliberately broader than that
helper's — any top-level `error` that is neither `false` nor `null` fails the leg, where the
helper additionally requires the absence of real data. The two answer different questions:
the helper must not discard a result the user asked for, while this probe only decides
whether to trust a leg, and the safe answer to an ambiguous leg is no.

Two further properties are part of the decision:

1. **Responses are compared as parsed JSON values, not as bytes.** Content stability is not
   encoding stability: field ordering, whitespace, or a per-response serialisation
   difference would make two identical records unequal, and the probe would then return
   `false` unconditionally — silently shipping the "remove the fallback" alternative this
   record rejects, while claiming not to. Each body is parsed with
   `serde_json::from_str::<serde_json::Value>` and compared structurally; a body that does
   not parse as JSON is compared as raw text, and a parsed body never equals an unparsed
   one. `serde_json`'s own 128-level recursion limit bounds the parse of a
   server-controlled body.

2. **Discriminating power is measured, never assumed.** The probe asserts nothing about
   which fields the server returns, which endpoints require a login, or which Bugzilla
   version it is talking to. It observes whether *this* server distinguishes an
   authenticated caller from an anonymous one at *this* endpoint, and declines to conclude
   anything when it does not. The defect being fixed is exactly an assumption of that kind
   baked into a probe.

The endpoint is `rest/user?names={login}`, with `{login}` the configured email the
`valid_login` probe already required. Issue #713 measured the equivalent path form,
`rest/user/<login>`, and found the anonymous projection (id and real_name) narrower than the
authenticated one (the full record including `groups`) on stock Bugzilla 5.2; the `?names=`
query form is chosen so `reqwest` encodes an email rather than bzr interpolating one into a
path. Its per-request *content* stability is an assumption, not a measurement — see
Consequences.

## Consequences

- On stock Bugzilla the anonymous and header responses agree, the probe stops after two
  requests, and bzr keeps query-parameter auth. REST reads run authenticated, and the
  fields the acting user is entitled to come back. This is the defect closed.
- On a server that genuinely honours the header while `valid_login` denies it, the header
  response matches the query-parameter response and differs from the anonymous one, and bzr
  still prefers header auth.
- **Every failure resolves toward the method `valid_login` proved.** The probe can only
  ever upgrade to header auth on positive evidence; it can never downgrade a working
  configuration to a broken one. False negatives are therefore the accepted direction of
  error, and there are three shapes of them: a server whose `rest/user` record is identical
  for anonymous and authenticated callers, a server that refuses `rest/user` to anonymous
  callers outright (`requirelogin`, where the anonymous leg is non-2xx), and any transient
  failure on any leg. Each keeps query-parameter auth, which works but puts the API key in
  URLs — redacted from bzr's own logs by `safe_url()`, still visible to the server's access
  log.
- **Detection costs one or two extra round trips**, only in the `valid_login` +
  query-parameter branch, and only when auth is not already cached: two requests in the
  common not-confirmed case, three when header auth is confirmed. The old probe cost one.
- **An already-affected user does not get the fix by upgrading.** The detected method is
  persisted per server in `config.toml`, and a server with both `auth_method` and
  `api_mode` cached is never re-detected — `connect` short-circuits to a TLS probe
  (`src/commands/runtime/shared/connection/mod.rs`) and the cached method is read straight
  from config (`.../connection/target.rs`). Anyone whose config the old probe wrote
  `auth_method = "header"` into keeps it, and their REST reads keep silently omitting what
  they are entitled to. The remedy the tool supports today is re-running the server's full
  `bzr config set-server <name> --url <url> --email <email> ...` line, which replaces the
  entry and resets the detected settings; note that it also resets keyring, TLS pin and
  TOFU state, so the whole original command must be re-run rather than a fragment of it.
  Invalidating the stale value automatically would mean a persisted-config migration, which
  is a separate decision and is not taken here.
- **R3 has no functional-tier test.** The confirming branch is covered at the unit tier by a
  synthetic `wiremock` server, which proves the comparison logic. No container the project
  runs is a server that denies the header via `valid_login` and honours it elsewhere — the
  functional tier runs upstream Bugzilla (`bz50`, `bz52`, and `bz53` built from
  `bugzilla/bugzilla` `master`), so it exercises only the negative branch. Against a real
  server of that class, the confirming branch is reasoned, not measured.
- **The endpoint's per-request content stability is assumed, not measured.** If any field of
  a deployment's `rest/user` record varies between two requests — Bugzilla's own
  `last_seen_date` on authenticated access, or an activity field a customised `User.get`
  adds — the header and query-parameter legs are unequal and header auth is never confirmed
  on that server. That is a permanent false negative pinned to exactly the customised,
  BMO-derived class R3 exists for, and it resolves the safe way (query-parameter auth keeps
  working) rather than the dangerous one. Detecting it would mean a fourth request and a
  stability check, which costs more than the preference it protects.
- **Only `bz50` and `bz52` exercise this code at all.** The `bz53` image serves
  `rest/whoami`, so `detect_whoami_auth` resolves the method and `detect_auth_method`
  returns before the `valid_login` fallback is reached. The functional assertion on the
  detected method is therefore version-gated to `bz50`/`bz52` and skipped on `bz53`, rather
  than asserting a value on a version where nothing under test runs.
- **The API key still appears in the query-parameter probe's URL.** Unchanged from the
  existing `whoami` and `valid_login` query-parameter probes and inherent to the method
  being probed.
- **The decision matrix in `src/client/auth/mod.rs` is part of the contract.** That file's
  header states that changing a cell is a behaviour change that must update the table and
  its tests together; this record's rule replaces the "`rest/bug`, any 2xx" row.
- The `whoami` path (Bugzilla 5.3+/BMO-derived) is untouched: `detect_whoami_auth` probes an
  endpoint that returns the caller's own id and treats `id == 0` as anonymous, so it already
  distinguishes what this probe could not.
- This does not address the alternate-auth retry in `src/client/transport.rs`, which judges a
  retry on HTTP status alone (issue #715), nor the absence of an ad-hoc
  `--server-auth-method` override for the inline-server surface. Both are tracked separately.

## Considered & rejected

- **Remove the fallback and always trust `valid_login`.** verified: the doc comment at
  `src/client/auth/valid_login.rs:195-199` names IBM LTC Bugzilla as a server that reports
  header auth unsupported via `valid_login` and accepts it on real endpoints, and issue #713
  states the fallback itself is reasonable. judgment: this would demote every such
  deployment to query-parameter auth permanently, putting the API key in every URL and
  server access log. The defect is the probe's evidence, not the existence of a fallback.
- **Keep `rest/bug?limit=1` and compare the response with and without the header.**
  verified: `GET /rest/bug?limit=1` carries no explicit ordering, and the functional suite
  creates bugs throughout a run (`tests/functional/phases/08-bugs.sh` and later phases), so
  two successive requests can legitimately return different bugs. judgment: a differential
  comparison over a non-deterministic body reintroduces a false positive by a different
  route — the probe would report "the header changed something" because the bug list moved.
- **Probe `rest/whoami` with the header.** verified: this branch is reachable only after
  `detect_whoami_auth` returned `NotFound`, `AuthRejected`, or `MalformedResponse`
  (`src/client/auth/mod.rs` lines 249-261); issue #713 measured `rest/whoami` returning 404
  in all three auth modes on stock Bugzilla 5.2, calling it a Harmony endpoint. judgment: an
  endpoint the server declined to serve cannot verify anything, and the third arm is no
  better — a 200 whose body bzr could not parse is equally unusable as a discriminator.
- **Require a field only an authenticated caller receives — `groups`, `can_login`.**
  verified: issue #713's measured table shows the anonymous `rest/user/<login>` projection
  carrying id and real_name and the query-parameter projection carrying the full record
  including `groups`, on stock Bugzilla 5.2. judgment: which fields a server returns varies
  by version, by the caller's permissions, and by whether the caller is querying itself.
  Hard-coding one name makes a server-specific detail a correctness dependency, which is the
  same class of mistake as the defect being fixed.
- **Compare only the anonymous and header responses, preferring header when they differ.**
  judgment: "differs from anonymous" proves the header changed something, not that it
  authenticated. The query-parameter leg is what pins a positive to the response
  authenticated access actually produces, rather than to any difference at all.
- **Compare only the header and query-parameter responses, dropping the anonymous leg.**
  judgment: two requests always instead of two-or-three, and it gives the right answer on
  stock Bugzilla, where the header response equals the anonymous projection and differs from
  the authenticated one. But on a server whose `rest/user` record does not discriminate at
  all, the header and query-parameter responses are equal for a reason unrelated to auth,
  and the probe confirms header auth on no evidence — the defect being fixed. The anonymous
  leg exists to make non-discrimination observable, and it is skipped in no case because it
  is the second request, not the third.
- **Probe an endpoint anonymous callers cannot read at all.** verified: issue #713's table
  records `rest/group?names=admin` answering 401 (code 410) anonymously and to the header,
  and 200 to query-parameter auth, on stock Bugzilla 5.2. judgment: it needs a group name
  bzr does not have, and whether a given endpoint requires a login is a per-deployment
  configuration fact. Choosing an endpoint on the belief that it always 401s anonymously is
  the same baked-in assumption the current probe is made of, pointed the other way.
- **Do nothing.** verified: the probe's positive is unconditional on stock Bugzilla, where
  no API-key-header code path exists at all. judgment: the failure mode is a correct answer
  being discarded in favour of a wrong one, with the damage appearing as absent fields in a
  200 response. That is worse than having no fallback.
