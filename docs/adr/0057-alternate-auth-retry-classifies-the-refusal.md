# 0057 — The alternate-auth retry classifies the refusal, not the status

- Status: Accepted
- Date: 2026-09-05
- Issue: #715
- Related: [0015](0015-server-errors-are-never-masked.md)

## Context

`BugzillaClient::send_raw` retries a 401 with the other auth method (header ↔
query parameter), because some deployments accept only one of them. It then asks
`alternate_auth_failed(status)` whether the retry proved authentication failed,
and that predicate reads `status == 401 || status == 403` — the status and
nothing else. When it says yes, the retried response is dropped whole and the
original 401 is returned.

Bugzilla makes that predicate wrong. `Bugzilla/WebService/Constants.pm`
(`REST_STATUS_CODE_MAP`, lines 270–284 on `release-5.2-stable`, 281–295 on
`master`) maps **fifteen** distinct WebService error codes onto HTTP 401:

    102 106 109 110 113 115 120 300 301 302 303 304 410 504 505

Six of them mean authentication did not succeed. The rest mean the caller
authenticated and was then refused. So the status cannot separate "you are not
logged in" from "you are logged in and may not do this", and the retry's
verdict is decided by the one signal that carries no such information.

Issue #715 measured the consequence. Restricting a bug to a group the product
does not allow is refused with code **120**, "you are not allowed to restrict
bugs to this group in the '<product>' product". `bzr bug update <id>
--groups-add <group>` reported `api_code 410`, **"You must log in"** — the stale
body of the first attempt, on a request the server had authenticated.

ADR 0015 settles the same principle — a server error is surfaced whenever it is
the only thing the server told us — but its Decision items bind
`has_data_fields` and the 100500 search fallback in
`src/client/resources/bug.rs`. Neither reaches `src/client/transport.rs`, and
neither answers the question this path actually raises: when the original
response and the retry disagree, which one does the user see?

## Decision

**The 401 alternate-auth retry is judged by what the server said, not by the
status it said it at. A retried 401 or 403 keeps the original response only when
its body proves that authentication itself failed.**

Concretely:

1. **Bugzilla's own taxonomy defines "authentication failed".** An error code in
   `300..=399`, or exactly `410`. That is not a list bzr invented: `Constants.pm`
   heads the block with `# Authentication errors are usually 300-400.` and marks
   the outlier with `# Except, historically, AUTH_NODATA, which is 410.` Keying
   on the band rather than an enumerated set means a deployment or extension that
   adds an authentication code inside it is classified correctly without a bzr
   release. Every other code Bugzilla maps to 401 is a refusal of an
   authenticated request.

2. **A retried 401/403 whose body carries a non-authentication Bugzilla error
   code replaces the original.** The user is told code 120 and its message, not
   code 410 and "You must log in". This is ADR 0015's principle applied to the
   transport: the retry is the attempt that reached the server's real answer,
   and discarding it substitutes a false answer for a true one.

3. **A retried 401/403 that carries no Bugzilla error envelope keeps the
   original.** A bare 401 from a fronting proxy, an HTML challenge page, an
   unreadable body — none of them distinguish anything, and the pre-existing
   behaviour is the safe reading when the server offered no signal. The same
   applies when the envelope carries no `code`.

4. **The reported error keeps its existing variant and exit code.** A relayed
   refusal is `BzrError::Api { code, message }`, exactly as
   `check_response_status` would have built it from the same status and body, so
   the exit code, the `error.type` and the structured-error keys are unchanged.
   Only `api_code` and `message` change, and they change from wrong to right.
   Introducing a distinct variant for policy refusals would move an exit code
   for no diagnostic gain: `Api` already carries the server's own code, which is
   the thing that distinguishes them.

5. **The classification is confined to the retry.** The first attempt's body is
   not inspected, and the retry still runs on every 401.

## Consequences

- A caller who is refused on policy grounds now sees the server's code and
  message. That is the point, and it is the only user-visible change: same
  variant, same exit code, same structured-error shape.
- Establishing (2) consumes the retried body, so a refusal cannot travel back to
  `check_response_status` as a response. It travels as the `BzrError` that
  function would have produced, from a helper both paths now share
  (`error_from_status_body`). One construction site, so the two cannot drift.
- The relayed body reaches the user through the same redaction the ordinary
  error path uses. Nothing new is logged: the debug line that prints a redacted
  body preview moves into the shared helper and is emitted once per error.
- **bzr becomes an existence oracle to the extent the server already is**, on
  the same terms ADR 0015 accepted deliberately. A relayed 120 tells the caller
  the group exists and the product does not allow it; that disclosure is
  Bugzilla's to make, and it already made it.
- The inverse defect is now reachable in principle: a deployment that refuses an
  unauthenticated request with a code outside the authentication band would have
  its refusal relayed instead of the original 401. Both bodies would then be the
  server's own words about the same request, so the failure mode is a less
  precise true answer rather than a false one.
- **A stock Bugzilla cannot reproduce the masking.** It needs the first attempt
  to fail authentication while the retry succeeds, which is the server-side
  condition #713 describes and not something a container is configured into.
  The functional tier therefore pins the *contract* — a policy refusal reports
  the server's own error and never a login failure — while wiremock tests drive
  the divergent-body path directly. This is the coverage split ADR 0015 already
  took for #504, for the same reason.

## Considered & rejected

- **Leave the predicate on status and fix the message instead.** verified: the
  original 401's body is the header attempt's, and in the measured case it reads
  `{"error":true,"code":410,"message":"You must log in..."}` (issue #715, against
  Bugzilla 5.2+ at `bzr 0.8.3-dev`, commit `63abb94e`). There is no better
  message to be had from it; the specific one is in the response that gets
  discarded.
- **Enumerate the nine non-authentication codes Bugzilla maps to 401 and treat
  those as refusals.** verified: `curl` of
  `Bugzilla/WebService/Constants.pm` at both `release-5.2-stable` and `master`
  shows the two maps agree exactly, but `REST_STATUS_CODE_MAP` is extensible at
  runtime — `Bugzilla::Hook::process('webservice_status_code_map', ...)` — so an
  extension can add a code the enumeration would misclassify as an
  authentication failure. The authentication band is the closed half of the
  question and the one Bugzilla documents.
- **Inspect the *original* 401's body and skip the retry when it already proves
  authentication succeeded.** judgment: a request saved on a path that only runs
  when something is already wrong, at the cost of consuming the original body
  too. It also does not remove the need for this decision — when the first
  attempt genuinely fails to log in and the retry is then refused on policy, the
  answer is still only in the retried body.
- **Report a policy refusal as a new `BzrError` variant with its own exit
  code.** judgment: a user-visible exit-code change to distinguish something
  `api_code` already distinguishes, on an error the CLI has always reported as
  `Api`.
- **Re-implement Bugzilla's `WS_ERROR_CODE` table in bzr.** judgment: ADR 0015
  settled that bzr does not re-implement Bugzilla's disclosure policy, and a
  vendored copy of a table the server extends at runtime is a maintenance
  liability with no reader.
- **Do nothing.** verified: the measured report is `api_code 410`, "You must log
  in", for a request Bugzilla authenticated (issue #715). The message names the
  wrong cause, and #713 makes the fallback run on every authenticated write on
  affected deployments, so all fifteen 401-mapped codes are masked identically.
