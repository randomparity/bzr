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
`has_data_fields`, which governs the HTTP-200 error path in
`src/client/response.rs`, and the 100500 search fallback in
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
   authenticated request. bzr reads Bugzilla's "300-400" as the half-open band
   `300..=399`: `400` is Bugzilla's own `STATUS_BAD_REQUEST` constant and no
   `WS_ERROR_CODE` entry uses it as an error code. An extension that chose
   exactly `400` for an authentication error would be relayed as a refusal;
   widening the band to include it would conflate an error code with a status
   code for no gain.

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

4. **The reported error is built from the retried body by the same
   construction the original path uses.** A relayed refusal is
   `BzrError::Api { code, message }`, exactly as `check_response_status` would
   have built it from the same status and body. Introducing a distinct variant
   for policy refusals would move an exit code for no diagnostic gain: `Api`
   already carries the server's own code, which is the thing that distinguishes
   them.

   **Where the original 401 was itself a Bugzilla error envelope**, only
   `api_code` and `message` change, and they change from wrong to right; the
   variant, `error.type`, the structured-error key set, and the exit code are
   all unchanged. **Where it was not** — a bare 401, an HTML challenge page from
   a fronting proxy — the old path produced `BzrError::HttpStatus` and the new
   one produces `BzrError::Api`, which is a different variant with a different
   exit code. Both transitions are user-visible; see Consequences.

   `api_code` is not display text, so this is not a cosmetic change. bzr reads
   it as control flow: `BzrError::is_permissive_bug_view_error`
   (`src/error.rs:247`) treats codes 100, 101 and 102 as per-resource faults
   that `bug view --permissive` (`src/commands/bug/view.rs:199`) and
   `comment list --permissive` (`src/commands/comment/list.rs:61`) skip rather
   than abort on. Code 102 is one of the fifteen Bugzilla maps to 401, so
   relaying it is the one place this decision moves a process exit code. That is
   accepted; see Consequences.

5. **The classification is confined to the retry.** The first attempt's body is
   not inspected, and the retry still runs on every 401.

## Consequences

- A caller who is refused on policy grounds now sees the server's code and
  message. Where the original 401 was itself a Bugzilla error envelope — the
  ordinary case, and the one the issue reported — that is the whole of the
  change: same variant, same `error.type`, same structured-error key set, same
  exit code.
- **Two reachable user-visible transitions, both accepted deliberately.** They
  are listed here because a record that names one and denies the other is worse
  than one that names neither.
- **Transition (a): a bare 401 becomes an API error.** When the first attempt's
  401 carries no Bugzilla error envelope and the retry's does, the old path
  reported `BzrError::HttpStatus { status: 401 }` — exit 5, `error.type`
  `"http"`, structured key `status` — because the original's unparseable body
  was all it had. The new path reports `BzrError::Api { code, message }` — exit
  4, `error.type` `"api"`, structured key `api_code`. A `--json` consumer keying
  on `error.type` or on the presence of `api_code`, or a script keying on exit
  5, sees the difference. The retried envelope is the server's actual answer and
  the bare 401 was not, so this is the same principle the rest of this decision
  rests on; it is pinned by a wiremock test.
- **Transition (b): a `--permissive` batch can exit 0 where it exited 4.**
  `bzr bug view A,B --permissive` (and `comment list --permissive`) suppress a
  per-resource fault — codes 100, 101, 102 — and abort on anything else. On a
  deployment where the first auth method draws a 401 (the #713 shape), a
  restricted bug previously produced the header attempt's `Api { code: 410 }`,
  which is not suppressible, so the whole batch aborted with exit 4. It now
  produces the retry's `Api { code: 102 }`, which is suppressible, so the bug is
  listed as failed and the command exits 0. That is the behaviour
  `--permissive` was built for — 102 is the true answer, and the flag exists to
  skip inaccessible bugs — and the old exit 4 was a consequence of the masking,
  not a contract. It is pinned by a wiremock test so it is a contract rather
  than an accident, and it is what the release note has to say.
- Establishing (2) consumes the retried body, so a refusal cannot travel back to
  `check_response_status` as a response. It travels as the `BzrError` that
  function would have produced, from a helper both paths now share
  (`error_from_status_body`). One construction site, so the two cannot drift.
- The retried body, previously discarded unread, now reaches the debug log as a
  redacted preview and the user as an error message. The *number* of preview
  lines per error is unchanged — the existing line moves into the shared helper
  — but the *content* is new, and it is server-controlled. Both destinations are
  already governed: the log line applies
  `crate::bugzilla_auth::redact_api_key` over a `BODY_PREVIEW_MAX_BYTES` prefix,
  and `BzrError`'s own `Display` applies the same redaction to `Api.message` and
  `HttpStatus.body` (`src/error.rs:16`, `:37`). No new control is needed; what is
  new is that these controls now carry a body they previously never saw.
- `ErrorResponse` — the deserialization struct the ADR-0015-governed HTTP-200
  error path also reads — is not changed. `bugzilla_error_code` reuses it and
  treats its existing "no `code`" sentinel as "no signal", so the 200-error path
  keeps its behaviour without a second struct or a shared type change.
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
- **Relay only the retried body's `message` and keep the original's `code`.**
  This is the one variant under which Decision item 4's exit code would be
  literally unchanged, since `api_code` is what `is_permissive_bug_view_error`
  reads. judgment: it reports a code and a message that describe different
  refusals — `code 410` beside "you are not allowed to restrict bugs to this
  group" — and `api_code` is the field a machine consumer keys on, so the
  variant that keeps the exit code stable is the one that lies to the consumer
  most likely to act on it.
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
