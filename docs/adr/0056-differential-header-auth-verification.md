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

The consequence is silent. Issue #713 measured it against a stock Bugzilla 5.2 fixture
(image `bugzilla/bugzilla` at `644c66f4`, head of branch `5.2`), with one actor and one key:
the `X-BUGZILLA-API-KEY` column of every request was byte-identical to the anonymous column.
A grep of the docroot for `x.bugzilla.api.key`, `X_BUGZILLA_API_KEY`, `HTTP_X_BUGZILLA` and
`API_AUTH_HEADERS` returned zero hits, while `Bugzilla_api_key` matched
`Bugzilla/Auth/Login/APIKey.pm` and two others — stock Bugzilla has no API-key-header code
path at all, so the probe's positive is always wrong there. Reads then returned 200 with a
narrower body: `estimated_time`/`remaining_time` absent, private comments absent. Nothing
distinguishes that from a legitimately empty result.

The question this record answers is what evidence a client can collect that actually
separates an honoured header from an ignored one, on a server whose version, permission
model and extension set it does not know.

## Decision

**Header auth is preferred over the method `valid_login` already proved only when a
differential probe shows the header response is a success, is distinguishable from the
anonymous response, and matches the response query-parameter auth produces. Every other
outcome — including every inconclusive one — keeps the method `valid_login` proved.**

The probe issues up to three `GET {base}/rest/user?names={login}` requests and compares
`(status, body)` pairs:

1. **Header** — with `X-BUGZILLA-API-KEY`. A transport failure or a non-2xx status ends the
   probe: not confirmed.
2. **Anonymous** — no credentials at all. A transport failure ends the probe: not confirmed.
   If this response equals the header response, the header changed nothing and the probe
   ends: not confirmed.
3. **Query-parameter** — with `Bugzilla_api_key`, the method `valid_login` proved. A
   transport failure ends the probe: not confirmed. Header auth is confirmed if and only if
   this response equals the header response.

Three properties of that rule are the decision, not implementation detail:

1. **The endpoint is `rest/user?names={login}`, and `{login}` is the configured email the
   `valid_login` probe already required.** It is a deterministic record — no clock, no
   counter, no server-side ordering — so two identical requests return identical bytes. That
   is what makes a byte comparison meaningful. It is also the endpoint whose anonymous and
   authenticated projections issue #713 measured as differing on stock Bugzilla.

2. **Discriminating power is measured, never assumed.** The probe asserts nothing about
   which fields the server returns, which endpoints require a login, or which Bugzilla
   version it is talking to. It observes whether *this* server distinguishes an
   authenticated caller from an anonymous one at *this* endpoint, and declines to conclude
   anything when it does not. The defect being fixed is exactly an assumption of that kind
   baked into a probe.

3. **Every failure resolves toward the proven method.** Transport error, non-2xx, an
   indistinguishable pair, an unexpected third response — all of them return
   `AuthMethod::QueryParam`, which `valid_login` established works. The probe can only ever
   *upgrade* to header auth on positive evidence; it can never downgrade a working
   configuration to a broken one.

## Consequences

- On stock Bugzilla the anonymous and header responses are byte-identical, the probe stops
  after two requests, and bzr keeps query-parameter auth. REST reads run authenticated, and
  the fields the acting user is entitled to — private comments, time tracking — come back.
  This is the defect closed.
- On a server that genuinely honours the header while `valid_login` denies it, the header
  response matches the query-parameter response and differs from the anonymous one, and bzr
  still prefers header auth. The fallback the doc comment was written for survives.
- **Detection costs one or two extra round trips**, only in the `valid_login` +
  query-parameter branch, and only when auth is not already cached. Two requests in the
  common (not-confirmed) case, three when header auth is confirmed. The old probe cost one.
  The result is persisted per server in `config.toml` beside `api_mode` and `server_version`,
  so this is a one-time cost per server, not a per-invocation one.
- **A false negative is possible and is the accepted direction of error.** A server whose
  `rest/user` record is identical for anonymous and authenticated callers — a fully public
  deployment, or one whose user endpoint is disabled — cannot demonstrate discriminating
  power, so bzr keeps query-parameter auth even if header auth would also have worked. The
  cost is the API key travelling in URLs, where `safe_url()` redacts it from bzr's own logs
  but the server's access log still sees it. That is strictly better than the inverse error,
  which returns wrong data silently.
- **Auth detection now issues one deliberately unauthenticated request** carrying the
  configured login as a query parameter. It reveals to the server only a value that server
  already holds, and it is sent to the configured URL over the same TLS policy as every
  other probe. It is not a new trust boundary; it is the same one, crossed once more.
- **The API key still appears in the query-parameter probe's URL.** That is unchanged from
  the existing `whoami` and `valid_login` query-parameter probes and is inherent to the
  method being probed.
- **The decision matrix in `src/client/auth/mod.rs` is part of the contract.** That file's
  header states that changing a cell is a behaviour change that must update the table and
  its tests together; this record's rule replaces the "`rest/bug`, any 2xx" row.
- The `whoami` path (Bugzilla 5.3+/BMO-derived) is untouched. `detect_whoami_auth` probes an
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
  `detect_whoami_auth` returned `NotFound` or `AuthRejected` (`src/client/auth/mod.rs`
  lines 248-261), and issue #713 measured `rest/whoami` returning 404 in all three auth
  modes on stock Bugzilla 5.2, calling it a Harmony endpoint. judgment: an endpoint the
  server has already declined to serve cannot verify anything.
- **Require a field only an authenticated caller receives — `groups`, `can_login`.**
  verified: issue #713's measured table shows the anonymous `rest/user/<login>` projection
  carrying id and real_name, and the query-parameter projection carrying the full record
  including `groups`, on stock Bugzilla 5.2. judgment: which fields a server returns varies
  by version, by the caller's permissions, and by whether the caller is querying itself.
  Hard-coding one name makes a server-specific detail a correctness dependency, which is the
  same class of mistake as the defect being fixed. The measured comparison needs no such
  assumption.
- **Compare only the anonymous and header responses, preferring header when they differ.**
  judgment: "differs from anonymous" proves the header changed something, not that it
  authenticated. A transient 5xx or a rate-limit response on either leg is enough to make
  the pair differ, and the probe would then confirm header auth on the strength of an error.
  Comparing against the response query-parameter auth actually produces costs one additional
  request, and only in the branch that ends up confirming header auth.
- **Probe an endpoint anonymous callers cannot read at all.** verified: issue #713's table
  records `rest/group?names=admin` answering 401 (code 410) anonymously and to the header,
  and 200 to query-parameter auth, on stock Bugzilla 5.2. judgment: it needs a group name
  bzr does not have, and whether a given endpoint requires a login is a per-deployment
  configuration fact (`requirelogin`, group visibility). Choosing an endpoint on the belief
  that it always 401s anonymously is the same baked-in assumption the current probe is made
  of, pointed the other way.
- **Do nothing.** verified: the probe's positive is unconditional on stock Bugzilla, where
  no API-key-header code path exists at all. judgment: the failure mode is a correct answer
  being discarded in favour of a wrong one, with the damage appearing as absent fields in a
  200 response. That is worse than having no fallback.
