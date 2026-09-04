# Bug lifecycle comparison design

## Scope and authority

Issue #667 extends the comparison harness from ADR 0044 to the bug lifecycle. The change is test
infrastructure only. It covers create/new, query, modify/update, info/view, and history against the
same live Bugzilla instance. It also establishes one fail-closed comparison baseline for each
confirmed python-bugzilla-only capability owned by #670, #671, #672, #679, and #680. Those probes
exercise python-bugzilla live, demonstrate bzr's current gap, and attach the exact owning issue with
`expect_gap`; they do not implement or claim the dependent capabilities or the final report work in
#683.

ADR 0044 fixes the python-bugzilla sidecar, semantic comparison, transport recording, and
`expect_gap` contracts. [ADR 0045](../../adr/0045-observe-comparison-transport-from-debug-events.md)
now fixes how the harness obtains that transport evidence: both clients normalize observations
from their exercised request boundaries instead of copying the transport requested by a wrapper.

## Comparison shape

One shell phase creates two bugs with the same canonical initial fields: bzr creates one through
its forced REST path, and a small python-bugzilla helper creates the other through the library's
auto-detected backend. Each summary is a shared run-specific stem plus one exact client suffix,
` [bzr]` or ` [pybz]`, so queries select one record without making generated IDs part of parity.

After each operation, the phase reads both bugs from the server and reduces them to the same
canonical JSON projection. It compares persisted fields, never CLI presentation. The bug-read
projection contains product, component, version, summary stem, operating system, platform,
severity, priority, status, resolution, URL, whiteboard, and sorted CC/keyword values when present.
Because Bugzilla stores the creation description as the first comment rather than a bug field, the
create test also reads both comment lists through the same forced-REST bzr observer and compares
their normalized first-comment bodies. Generated IDs, timestamps, reporter identity, and
backend-specific aliases are excluded.

The lifecycle runs in this order:

1. create both records and compare their canonical initial state;
2. query each record by its unique summary and compare the canonical result sets;
3. apply the same summary, URL, whiteboard, severity, and priority update to both records, then
   compare fresh server reads;
4. fetch each record individually and compare the canonical view;
5. fetch history and compare normalized field transitions for the fields changed in step 3. For
   the `summary` transition only, map an old value exactly equal to either controlled initial
   summary to the shared stem before comparison. Preserve every other field name, old value, new
   value, and transition order without normalization.

Any command failure is an ordinary comparison failure unless the gap probe positively recognizes
its exact unsupported CLI surface: exit 2 plus a clap diagnostic naming the option or subcommand
that probe exercised. An `expect_gap` marker is permitted only after python-bugzilla has completed
the named operation against the live server, the observable result has been validated, and the bzr
attempt has either produced that recognized parser rejection or a validated semantic mismatch.
Connection, timeout, TLS, authentication, server, malformed-output, and harness failures are never
gap-eligible.
Each marker is bound to the issue that owns that exact capability:

- `saved-search`: after the two controlled lifecycle bugs exist, seed a run-specific server-side
  saved search for the administrator whose canonical query selects exactly those two IDs. Run it
  through python-bugzilla, require the sorted result to equal those two IDs, then probe
  `bug search --saved-search <run-specific-name>` and require the same exact set; owner #670. The
  seed helper uses Bugzilla's application database handle and the `namedqueries(userid, name,
  query)` contract present in every supported bz50, bz52, and bz53 image; it parameterizes the
  administrator ID, name, and query and verifies the stored row before the probe.
- `arbitrary-fields`: create and update a bug through generic field maps, validate both persisted
  values, then probe equivalent repeatable `--field` create and update invocations; owner #671.
- `update-options`: keep two independently falsifiable evidence arms under owner #672. The live
  arm posts and reads back a tagged comment. The request-shape arm uses the pinned image's fake
  backend to require python-bugzilla to send `minor_update: true`; the live server must also accept
  that update. The equivalent bzr probe must both persist the comment tag and emit
  `minor_update: true` in the controlled request capture before the shared marker can flip. No mail
  delivery or notification infrastructure is introduced.
- `query-match-types`: seed one bug with an exact run-specific whiteboard value and a second decoy
  with that value plus a suffix. First prove the ordinary substring query returns both IDs, then
  run python-bugzilla with `status_whiteboard_type=equals` and require only the exact ID. Probe
  `bug list --status-whiteboard-type equals` against the same pair and require the same exact
  result; owner #679.
- `bug-tags`: add a personal bug tag, retrieve the controlled bug through the tag filter, then
  probe `bug tag` followed by `bug list --tag`; owner #680. This probe records transport per
  operation instead of inheriting the phase's REST default: python-bugzilla must report its
  XML-RPC backend, while both bzr operations use explicit XML-RPC selection and must record
  `XML-RPC`. A future implementation that still runs this XML-RPC-only mutation through forced
  REST cannot flip the marker.

The bzr probe is part of the semantic test, not merely a help-text or parser check. A dependent
implementation that makes its command exit successfully but stores or returns the wrong state still
remains a gap. Conversely, `expect_gap` converts a fully passing comparison into a failure, so a
landed dependent capability makes its stale marker fail closed until that issue flips the row and
removes the marker.

## Components and interfaces

`tests/functional/compare/01-bug-lifecycle.sh` owns the five parity IDs and five gap-baseline IDs,
client invocation order, capture files, normalization, semantic comparisons, exact gap ownership,
and result reporting. It invokes bzr with explicit REST selection by default, selects XML-RPC only
for #680's update and tag-filter operations, and invokes the helper inside the existing sidecar
through `run_pybz_adapter`.

`tests/functional/lib.sh` keeps one private `_run_pybz_command COMMAND [ARG ...]` boundary that
executes the named command in the current sidecar and preserves the existing `BZR_STDOUT`,
`BZR_STDOUT_RAW`, `BZR_STDERR`, and `BZR_EXIT` capture semantics. Thin `run_pybz` and
`run_pybz_adapter` wrappers select respectively the fixed `bugzilla` CLI and the fixed
`python /work/compare/bug-lifecycle.py` command. Comparison phases cannot supply the executable.

`tests/functional/compare/bug-lifecycle.py` is a narrow adapter around python-bugzilla 3.3.0. It
accepts one operation plus JSON input/output paths under `/work`, connects to
`http://127.0.0.1` with the test API key read from the private input JSON, and emits JSON containing
both the operation result and the detected backend name. Its fixed dispatch table supports create,
query, update, view, history, saved-search query, generic-field create/update, update options,
match-type query, and personal bug-tag update/query operations; malformed input or an unsupported
operation exits non-zero with no fallback.
Before sidecar startup, the runner copies the repository helper to
`$COMPARE_EXCHANGE_DIR/bug-lifecycle.py`; sidecar commands invoke only that `/work/compare` copy.
Missing or unreadable staging stops the runner before any lifecycle operation.

`tests/functional/run-compare.sh` supplies the existing functional administrator identity and API
key to the phase, exports the key only for the lifetime of the comparison process, stages the
helper in the private exchange directory, and adds the phase to its explicit ordered list. The
sidecar reads the key from each mode-private request file; it is absent from command argv, the
image, the home cache volume, and error messages.

For the saved-search fixture, the runner also exposes
`seed_server_saved_search LOGIN NAME BUG_ID BUG_ID`. It validates both IDs as positive decimal
integers and NAME as a non-empty value no longer than Bugzilla's 64-character column, constructs
the canonical `bug_id=<id>,<id>&bug_id_type=anyexact` query, and sends the repository-owned Perl
script to `perl -I. -` over standard input in the primary Bugzilla application container. LOGIN,
NAME, and the query are non-secret arguments; no credential is forwarded. The script loads
Bugzilla's configured database handle, resolves the disposable administrator, and inserts or
updates only the run-specific `namedqueries` row with bound parameters. It reads the row back and
fails before either client query if the name, owner, or query differs. The same path runs in bz50,
bz52, and bz53; there is no assumed built-in saved-search name or ambient server state, and no seed
script or query file remains in the application container.

`docs/dev/python-bugzilla-parity.md` gains five parity rows for the common lifecycle and five
expected-gap rows for the dependent capabilities. Every row cites one stable
`compare/01-bug-lifecycle/<slug>` test ID. Gap rows name exactly one owning issue and cannot report
parity while their test still calls `expect_gap`.

## Transport evidence

Each successful invocation expected to exercise a client operation writes a normalized `REST` or
`XMLRPC` transport record in the exchange directory. A shared harness helper runs bzr with
`bzr=debug` tracing and classifies the captured request-boundary events: `API response` is REST and
`XML-RPC call` is XML-RPC. Repeated
observations of one class, such as retries, retain that class; no recognized event or events from
both classes are ambiguous and fail closed. The lifecycle phase copies only this observed value.
It never receives an expected transport argument and never derives evidence from `--api`.

A bzr command rejected by argument parsing before client dispatch exercised no client operation
and writes no transport record. It establishes an expected capability gap only when exit 2 and
the captured clap diagnostic name the exact unsupported option or subcommand expected by that
probe. A dedicated `lifecycle_bzr_no_dispatch` wrapper owns only the successful #672 `--dry-run`
request-shape control; it asserts success and copies request output without writing transport
evidence, and fails if captured stderr contains either recognized request-boundary event. Every
invocation through the ordinary REST or XML-RPC wrappers is a claimed client
operation, so success without exactly one observed class sets a distinct evidence-failure state.
Neither that defect nor any unrecognized non-zero outcome can pass through `expect_gap`. A
successful operation whose observed transport is a valid but unexpected class remains a
transport-specific capability gap and may become GAP.

The python-bugzilla adapter maps exactly the pinned 3.3.0 backend classes `_BackendREST` and
`_BackendXMLRPC` to the same closed values. A missing backend, any other class, or an output outside
the two-value vocabulary fails before semantic evidence is accepted. The phase compares exact
values rather than substrings.

Common bzr operations are expected to observe `REST`. A gap probe that reaches its client boundary
must likewise record and validate the resulting class; the currently unsupported bzr probe syntax
may instead supply its probe-specific recognized parser rejection with no transport claim. The
#672 dry-run control remains a separate local request-shape assertion and cannot exempt any live
operation from transport classification. The #680 bug-tag mutation and query are
expected to observe `XMLRPC` once those commands exist. Expectations remain assertions, not
evidence sources. A controlled fixture makes the #680 operations succeed semantically while the
request-boundary log reports REST and requires the result to remain GAP. Separate controls prove
that successful client invocations with missing or ambiguous bzr events remain FAIL, while
connection-style no-event failures and server/command errors also remain FAIL. The no-dispatch
fixture proves the dedicated dry-run path omits transport evidence and fails if that path emits a
REST or XML-RPC boundary event. Unknown python-bugzilla backend classes and normalized values
outside the closed vocabulary are rejected.

## Failure handling and cleanup

The phase stops an ordinary parity test at the first failed client operation, malformed JSON
result, missing ID, or semantic mismatch and reports the captured command output through the
existing harness. A gap test first validates its live python-bugzilla result. A probe-specific
terminal classifier marks the bzr side gap-eligible only after one of three complete outcomes: a
recognized parser rejection; successful client operations with required transport observations
and structurally valid response evidence; or the dedicated successful no-dispatch dry-run with a
structurally valid request payload. A semantic, request-shape, or valid-transport mismatch within
those outcomes remains eligible, as does a complete match so the stale marker fails. Any other
command failure, malformed evidence, observation defect, or harness assertion failure leaves the
test ineligible. The phase applies its exact `expect_gap` marker only through a helper that checks
this state. Unique bug summaries and run-specific values make repeated runs independent. The
comparison runner's existing EXIT trap removes the sidecar and exchange directory, including
API-key-bearing input files.

## Threat model

The existing ADR 0044 boundaries remain. This phase additionally passes the repository's fixed
disposable-test API key into the sidecar and parses JSON produced by both clients. Only the local
operator or CI job can invoke the harness; the disposable Bugzilla controls responses.

Controls are: pass only operation names and private exchange-file paths as quoted sidecar argv, set
umask 077 before writing request files, place JSON and credentials only under the runner's private
exchange directory, never evaluate response text, constrain helper operations to a fixed dispatch
table, validate IDs as positive integers before reuse, and normalize with fixed field selectors.
The saved-search seed separately validates its bounded non-secret arguments, sends a fixed script
over standard input, uses database placeholders for every value, and verifies the one owned row;
neither query text nor server output is evaluated as shell input. The staged helper is the only
repository-derived file mounted into the sidecar. Error output names the input path, never its API
key content. Production credentials, the rest of the repository, and the container runtime socket
remain outside the sidecar.

Out of scope are protecting the disposable server from other host processes, changing the mutable
upstream image/dependency exposure accepted by ADR 0044, and implementing any capability owned by
#670, #671, #672, #679, or #680.

## Verification

- Controlled fixtures inside the pinned comparison image prove helper syntax and adapter
  dispatch/output. Harness fixtures also prove helper staging failure, capture-compatible thin
  wrappers, all ten stable test IDs, transport records, first-comment description comparison,
  exact initial-summary history normalization, strict preservation of other history values and
  ordering, and mismatch failure behavior before live execution. The fixture asserts the complete
  mapping from the five gap IDs to #670, #671, #672, #679, and #680, and injects a passing bzr
  result to prove each stale `expect_gap` becomes a test failure. The fixtures additionally prove
  the saved-search row selects exactly the two controlled IDs on bz50, bz52, and bz53; #679's
  substring control returns the near-match decoy while its exact-match query excludes it; #672's
  tagged-comment live result and `minor_update: true` request shape fail independently; #680 stays
  GAP when semantics succeed over an observed REST request; bzr transport classification rejects
  missing or mixed request events; connection-style no-event failures and server/command errors
  cannot become GAP; exit 2 with a non-matching clap diagnostic and exit 1 with the exact expected
  diagnostic remain FAIL; one or multiple same-class events are accepted; a malformed response
  after valid transport evidence and a downstream assertion failure after valid evidence remain
  FAIL; and the successful #672 dry-run produces no transport claim while either a boundary event
  or structurally invalid request evidence on that path remains FAIL. A sequential control proves
  eligibility from one probe cannot leak into the next. The adapter rejects an unknown
  python-bugzilla backend.
- `make lint` validates semantic IDs, Bash syntax, ShellCheck, formatting, and Rust lints; it does
  not require host Python.
- `make test` proves the Rust suite remains green.
- `make functional-compare-all` proves the common lifecycle and all five live expected gaps against
  bz50, bz52, and bz53.
- `make functional-test-all` proves the established real-container suite remains green.
