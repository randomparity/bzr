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

The defect was independently reproduced from the other side while this record was being
written, by the worker on issue #714 measuring private-content visibility on live
containers — not by anyone looking for this bug. On Bugzilla 5.0.6 and 5.2,
`/rest/valid_login` *correctly rejects* the header key with `{"result":false}`, and the
any-2xx probe then overrides that correct rejection. Their measurement also shows the cost
is larger than #713 itself reported, and lands on bzr's **default** path rather than an
opt-in one: on Bugzilla 5.2, `config set-server` with no `--auth-method` selects `header`,
after which `comment list` returns 3 of 5 comments and `attachment list` 1 of 2 — **exit 0,
no diagnostic**. Bugzilla 5.0 escapes only because its `Hybrid` mapping happens to route
those two reads XML-RPC-first. Silently returning less than the caller is entitled to, on a
default configuration, with a success exit code, is the harm this record removes.

The question this record answers is what evidence a client can collect that actually
separates an honoured header from an ignored one, on a server whose version, permission
model and extension set it does not know.

## Decision

**Header auth is preferred over the method `valid_login` already proved only when a
differential probe shows the header response is a success, is distinguishable from the
anonymous response, and matches the response query-parameter auth produces. Every other
outcome — including every inconclusive one — keeps the method `valid_login` proved.**

The probe issues up to three `GET {base}/rest/user?names={login}` requests:

1. **Header** — with `X-BUGZILLA-API-KEY`. Must succeed.
2. **Anonymous** — no credentials. If its body equals the header body, the header changed
   nothing and the probe ends: not confirmed.
3. **Query-parameter** — with `Bugzilla_api_key`, the method `valid_login` proved. Must
   succeed. Header auth is confirmed if and only if its body equals the header body.

Three properties of that rule are the decision, not implementation detail.

**1. A leg is evidence only when its outcome means something about auth, and the two
credentialed legs are held to a stricter test than the anonymous one.**

*Every* leg is discarded on a transport failure, an unreadable body, or a status that is
neither a success nor `401`/`403` — a 5xx, a 404, a redirect. Each of those is a response
the server would have given whatever credential it was shown, so an equality or inequality
it produces says nothing.

The **header and query-parameter legs** must additionally show the credential was
*accepted*: a 2xx, a body that parsed as JSON, **and** no truthy top-level `error` key. A
credential-bearing request that was refused cannot stand in for the authenticated response,
and on this server class the refusal often arrives as an HTTP 200. The JSON requirement
closes the mirror of the case the anonymous re-check guards: a middlebox that challenges any
credential-bearing request with a stable `200` HTML interstitial, while passing anonymous
requests to the origin, answers both credentialed legs identically and differently from the
anonymous leg — satisfying every comparison on a server that honoured neither credential.
Bugzilla's REST API answers `rest/user` with JSON, so the requirement rejects nothing
legitimate. `Response::check_bugzilla_200_error`
(`src/client/response.rs`) exists because "some servers (e.g. IBM LTC Bugzilla) include
error fields alongside valid data" in a 200; a status check alone would read two identical
200-error responses on those two legs as agreement. This probe's test is deliberately
broader than that helper's — any top-level `error` that is neither `false` nor `null` fails
the leg, where the helper additionally requires the absence of real data. The two answer
different questions: the helper must not discard a result the user asked for, while this
probe only decides whether to trust a leg, and the safe answer to an ambiguous leg is no.

The **anonymous leg is deliberately not held to that test**, and this is the one place the
two rules would otherwise collide. An anonymous caller being refused what a credentialed one
receives is the most auth-shaped observation the probe can make, and Bugzilla delivers that
refusal as a status *and* an error body together — issue #713's own table records
`rest/group?names=admin` answering `401` with `code 410` to an anonymous caller. Applying
the credentialed test to the anonymous leg would therefore discard the refusal twice over —
once for the status, once for the body — and void the confirming branch on every
`requirelogin` deployment, a configuration the enterprise forks this fallback exists for are
more likely to run, not less. So the anonymous leg is accepted with whatever body it
carries. The positive stays pinned by the query-parameter leg regardless: a header the
server ignored would have been refused exactly as the anonymous request was, so its body
would equal the anonymous body and the probe would stop at the second leg.

**The anonymous observation must repeat.** That last argument compares two sequential
requests and quietly assumes both observed the same server state. They need not. Reaching
the confirming branch means the anonymous leg differed from the header leg, and that
difference is evidence about auth only if it is stable: a rate limiter or WAF answering the
second request of a burst differently — a one-off `401`, a `200` HTML interstitial, a `200`
Bugzilla error — produces the same inequality. On a server that ignores the header *and*
does not discriminate at this endpoint, the query-parameter leg then matches the header leg,
so one anomaly would confirm header auth: the defect this record exists to close, reached by
a longer route. So on the path about to return `true`, the anonymous request is re-issued
once and must return the same status class and the same body. One extra request, only on the
confirming path. The check is deliberately not conditioned on the anonymous leg having been
a refusal — gating it that way would cover only the anomalies that happen to arrive as
`401`/`403` and leave the `200`-shaped ones confirming.

**2. The comparison is over the parsed body, not over status-plus-bytes.** This is measured,
not stylistic. Against the project's own `bz50` image (Bugzilla 5.0.6), three identical
authenticated `GET /rest/user?names=<email>` requests returned **three different byte
sequences and one identical value** — Perl hash ordering is randomised per response, so JSON
object keys come back in a different order each time. A byte comparison would therefore have
found the header and query-parameter legs unequal on essentially every request, returned
`false` unconditionally, and silently shipped the "remove the fallback" alternative this
record rejects while claiming not to. Each body is parsed with
`serde_json::from_str::<serde_json::Value>` and compared structurally; a body that does not
parse as JSON is compared as raw text, and a parsed body never equals an unparsed one.
`serde_json`'s own 128-level recursion limit bounds the parse of a server-controlled body.
The status is checked, per property 1, but is deliberately **not** part of the compared
value: including it would make each status guard unfalsifiable — a leg the guard rejects
would compare unequal anyway, for the same reason the guard rejected it — and a design whose
guards cannot be tested is how this defect got here.

**3. Discriminating power is measured, never assumed.** The probe asserts nothing about
which fields the server returns, which endpoints require a login, or which Bugzilla version
it is talking to. It observes whether *this* server distinguishes an authenticated caller
from an anonymous one at *this* endpoint, and declines to conclude anything when it does
not. The defect being fixed is exactly an assumption of that kind baked into a probe.

The endpoint is `rest/user?names={login}`, with `{login}` the configured email the
`valid_login` probe already required; the `?names=` query form is chosen so `reqwest`
encodes an email rather than bzr interpolating one into a path. Issue #713 measured the
**path** form, `rest/user/<login>`, on stock Bugzilla 5.2. The query form was measured
separately, against the project's `bz50` image (Bugzilla 5.0.6), because Bugzilla's
`User.get` applies different restrictions to `names`, `ids` and `match` and the two forms
are not equivalent by construction. `GET /rest/user?names=admin@test.bzr` in the three auth
modes:

| Request | Status | Body |
|---|---|---|
| no credentials | 200 | `{"users":[{"name":…,"real_name":…,"id":1}]}` |
| `X-BUGZILLA-API-KEY` | 200 | byte-identical to the anonymous response |
| `Bugzilla_api_key` | 200 | full record adding `groups`, `email`, `can_login`, `saved_searches`, `saved_reports` |

So the shipped form discriminates, and the header column reproduces the defect this record
fixes on a container the project already runs.

## Consequences

- On stock Bugzilla the anonymous and header responses agree, the probe stops after two
  requests, and bzr keeps query-parameter auth. REST reads run authenticated, and the
  fields the acting user is entitled to come back. This is the defect closed.
- On a server that genuinely honours the header while `valid_login` denies it, the header
  response matches the query-parameter response and differs from the anonymous one, and bzr
  still prefers header auth — including when the anonymous leg is refused outright.
- **Every failure resolves toward the method `valid_login` proved.** The probe can only
  ever upgrade to header auth on positive evidence; it can never downgrade a working
  configuration to a broken one. False negatives are therefore the accepted direction of
  error: a server whose `rest/user` record is identical for anonymous and authenticated
  callers, and any inconclusive leg, both keep query-parameter auth. That works, but puts
  the API key in URLs — redacted from bzr's own logs by `safe_url()`, still visible to the
  server's access log, exactly as the existing `whoami` and `valid_login` query-parameter
  probes already are.
- **Detection costs one to three extra round trips**, only in the `valid_login` +
  query-parameter branch, and only when auth is not already cached: two requests in the
  common not-confirmed case, four when header auth is confirmed, because the anonymous
  observation is re-issued before confirming. The old probe cost one.
- **A decline is announced, not silent.** The probe declining leaves the API key in request
  URLs, where the server's access log sees it, and ADR-recorded false negatives make that
  the outcome for a whole class of server. Each terminal decline logs at `info` naming that
  consequence, matching the level the confirming path already used, so `-v` shows an
  operator why detection chose what it chose. Per-leg diagnostics stay at `debug`.
- **Probe transport errors are redacted, through one seam.** `reqwest::Error`'s `Display`
  appends ` for url (<url>)`, and the query-parameter legs carry the API key in that query
  string, so formatting such an error verbatim writes the key to stderr on any timeout,
  reset, or DNS failure. `redacted_probe_error` composes two seams: `safe_url` reduces the
  attached URL to origin and path, keeping the message diagnosable, and the result then goes
  through `bugzilla_auth::redact_api_key` — the same marker- and thread-local-based
  redaction that already guards the user-facing `BzrError::Http` display — because
  `safe_url` rewrites only the exact string reqwest attached and a source chain can render a
  differently-encoded copy.

  **The enumeration boundary is the set of paths a credential can reach, not the set of
  files being edited.** That distinction is the durable lesson of this change and it was
  learned the hard way: the first enumeration drew the line around `src/client/auth/`,
  stated that boundary honestly, and still missed a site — because `detect_version_and_mode`
  runs on the line immediately *after* detection returns, carrying the method detection just
  chose. An honestly-stated wrong boundary is still a wrong boundary.

  Under the corrected boundary, every place a `reqwest::Error` from a credential-bearing
  request can reach output: `log_probe_send_error` and `network_error_outcome` in
  `auth/mod.rs`; the send and body-read arms of `read_probe_leg`, and the `valid_login`
  body-read arm, in `auth/valid_login.rs`; the `whoami` body-read arm in `auth/whoami.rs`;
  and the send-failure arm of `detect_version_and_mode` in `client/version.rs`. All seven
  route through the seam. Four were live leaks; the two body-read arms attach no URL in
  reqwest 0.12.28 and are routed anyway, since that is a property of a dependency version
  rather than of this code. Two further paths were checked and need nothing: `tls_hint`
  appends a constant and never renders the error, and `BzrError::Http`'s display already
  redacts via `format_http_error`.

  The `version.rs` site was the worst of the four and the one this change itself created:
  it logs at `warn`, the **default** filter level, so it needed no verbosity flag — and
  correcting the detection is what moved the key there, by selecting query-parameter auth
  for the whole stock 5.0/5.2 population where the old probe had wrongly selected header
  auth. Both it and `auth/whoami.rs` were outside this change's originally declared surface
  and were taken deliberately, so that no caller sits outside the shared seam.

- **Probe response bodies are redacted before tracing.** `trace_body_preview` bounds a
  probe body for `-vvv` logging and now routes it through `redact_api_key`. bzr never writes
  the key into a body, but the query-parameter probes put it in the request URL and an error
  page from a proxy or CGI::Carp typically echoes the request URI back — and both probes
  trace the body *before* checking the status, so error pages are exactly what this reaches.
- **This does not claim the whole binary is leak-free.** The enumeration above covers the
  paths a credential reaches *from auth detection*; nothing here audits the rest of the
  client. What it does claim is that no site this change makes reachable renders a
  credential-bearing `reqwest::Error` outside the seam.
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
- **R3 — the confirming branch — has no functional-tier test.** It is covered at the unit
  tier by a synthetic `wiremock` server, which proves the comparison logic. No container the
  project runs is a server that denies the header via `valid_login` and honours it elsewhere:
  the functional tier runs upstream Bugzilla (`bz50`, `bz52`, and `bz53` built from
  `bugzilla/bugzilla` `master`), so it exercises only the negative branch. Against a real
  server of that class, the confirming branch is reasoned, not measured.
- **What the functional tier does establish** is that the probe ran and reached its decision
  on real responses: on `bz50`/`bz52` it asserts, from the debug log, that the probe ended by
  finding the header response equal to the anonymous one. That keeps the endpoint
  measurement above from silently rotting — a future image whose `rest/user` stops
  discriminating, or a probe that dies at leg 1, both change that line — rather than
  asserting only that the outcome happened to be `query_param`, which four different code
  paths could produce.
- **Only `bz50` and `bz52` exercise this code at all.** The `bz53` image serves
  `rest/whoami`, so `detect_whoami_auth` resolves the method and `detect_auth_method`
  returns before the `valid_login` fallback is reached. The functional assertions are
  version-gated to `bz50`/`bz52` and skipped on `bz53`, rather than asserting a value on a
  version where nothing under test runs.
- **Per-request content stability holds on the images measured, and is assumed elsewhere.**
  Three identical authenticated `rest/user` requests against Bugzilla 5.0.6 returned the
  same value every time, including a stable `groups` array order. On a deployment whose
  `User.get` does vary per request — Bugzilla's own `last_seen_date` on authenticated
  access, or an activity field a customised installation adds — the header and
  query-parameter legs are unequal and header auth is never confirmed there. That is a
  permanent false negative pinned to exactly the customised class R3 exists for, and it
  resolves the safe way. Detecting it would mean a fourth request and a stability check,
  which costs more than the preference it protects.
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
- **Drop one of the three legs.** judgment: neither pair is sufficient. Without the
  query-parameter leg, "differs from anonymous" proves the header changed something, not
  that it authenticated — a leg that failed for an unrelated reason differs too. Without the
  anonymous leg, a server whose `rest/user` record does not discriminate at all returns
  equal header and query-parameter responses for a reason unrelated to auth, and the probe
  confirms on no evidence — the defect being fixed. The anonymous leg makes
  non-discrimination observable and the query-parameter leg pins the positive to what
  authenticated access actually produces.
- **Treat any non-2xx leg as inconclusive, uniformly.** judgment: simpler to state, and it
  was the first rule this record carried, but it discards a `401`/`403` on the anonymous
  leg — the one non-2xx that *is* an auth observation — and with it the confirming branch on
  every `requirelogin` server. Uniformity bought nothing here except a rule that reads
  tidily.
- **Apply the credentialed-leg test (2xx and no error body) to the anonymous leg too.**
  verified: issue #713's measured table records an anonymous `rest/group?names=admin`
  answering `401` with `code 410` — Bugzilla delivers a refusal as a status *and* an error
  body together. judgment: this is the same mistake as the bullet above, wearing the body
  half instead of the status half. It would discard the anonymous refusal twice over and
  reintroduce exactly the `requirelogin` false negative the previous bullet was rejected for
  causing. The credentialed legs need the stricter test because their job is to stand in for
  the authenticated response; the anonymous leg's job is only to be different.
- **Key on `Set-Cookie: Bugzilla_login_request_cookie`.** verified: measured by the #714
  worker across Bugzilla 5.0.6, 5.2 and 5.3 — the header was present on exactly the
  anonymously-answered replies and absent on exactly the honoured ones, with no exceptions.
  It is a genuine discriminator and would cost one request instead of two to four.
  judgment: rejected, and the reason is the same one that makes this record necessary. A
  header-presence check depends on an undocumented detail of Bugzilla's login machinery,
  where the differential depends only on credentialed and anonymous responses differing —
  which is the property actually under test. Keying on an implementation detail no contract
  pins is the *class* of mistake the `rest/bug` any-2xx probe was: an assumption about how
  this server behaves, held by a probe that cannot check it. That objection is sharpest
  exactly where the confirming branch matters, since the fallback exists for forks, and a
  fork is where cookie behaviour is most likely to diverge from the three upstream images
  the measurement covers. The cost of the differential is two extra requests on a cached,
  once-per-server path; that is a low price for not depending on a detail bzr cannot verify.
  records `rest/group?names=admin` answering 401 (code 410) anonymously and to the header,
  and 200 to query-parameter auth, on stock Bugzilla 5.2. judgment: it needs a group name
  bzr does not have, and whether a given endpoint requires a login is a per-deployment
  configuration fact. Choosing an endpoint on the belief that it always 401s anonymously is
  the same baked-in assumption the current probe is made of, pointed the other way.
- **Do nothing.** verified: the probe's positive is unconditional on stock Bugzilla, where
  no API-key-header code path exists at all. judgment: the failure mode is a correct answer
  being discarded in favour of a wrong one, with the damage appearing as absent fields in a
  200 response. That is worse than having no fallback.
