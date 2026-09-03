# Bug lifecycle comparison design

## Scope and authority

Issue #667 extends the comparison harness from ADR 0044 to the bug lifecycle. The change is test
infrastructure only. It covers create/new, query, modify/update, info/view, and history against the
same live Bugzilla instance. It also establishes one fail-closed comparison baseline for each
confirmed python-bugzilla-only capability owned by #670, #671, #672, #679, and #680. Those probes
exercise python-bugzilla live, demonstrate bzr's current gap, and attach the exact owning issue with
`expect_gap`; they do not implement or claim the dependent capabilities or the final report work in
#683.

No new architecture decision is required. ADR 0044 already fixes the python-bugzilla sidecar,
semantic comparison, transport recording, and `expect_gap` contracts. This design instantiates
those decisions for one resource.

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

Any command failure is an ordinary comparison failure. An `expect_gap` marker is permitted only
after python-bugzilla has completed the named operation against the live server, the observable
result has been validated, and the bzr attempt has failed or produced a different persisted result.
Each marker is bound to the issue that owns that exact capability:

- `saved-search`: run the administrator's `My Bugs` server-side saved search, validate a JSON
  result set, then probe `bug search --saved-search "My Bugs"`; owner #670.
- `arbitrary-fields`: create and update a bug through generic field maps, validate both persisted
  values, then probe equivalent repeatable `--field` create and update invocations; owner #671.
- `update-options`: post an update with `minor_update` and a tagged comment, validate the comment
  tag, then probe `bug update --minor-update --comment-tag`; owner #672.
- `query-match-types`: query a controlled whiteboard value with an exact match-type modifier,
  recover only the controlled bug, then probe `bug list --status-whiteboard-type equals`; owner
  #679.
- `bug-tags`: add a personal bug tag, retrieve the controlled bug through the tag filter, then
  probe `bug tag` followed by `bug list --tag`; owner #680.

The bzr probe is part of the semantic test, not merely a help-text or parser check. A dependent
implementation that makes its command exit successfully but stores or returns the wrong state still
remains a gap. Conversely, `expect_gap` converts a fully passing comparison into a failure, so a
landed dependent capability makes its stale marker fail closed until that issue flips the row and
removes the marker.

## Components and interfaces

`tests/functional/compare/01-bug-lifecycle.sh` owns the five parity IDs and five gap-baseline IDs,
client invocation order, capture files, normalization, semantic comparisons, exact gap ownership,
and result reporting. It invokes bzr with explicit REST selection and invokes the helper inside the
existing sidecar through `run_pybz_adapter`.

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

`docs/dev/python-bugzilla-parity.md` gains five parity rows for the common lifecycle and five
expected-gap rows for the dependent capabilities. Every row cites one stable
`compare/01-bug-lifecycle/<slug>` test ID. Gap rows name exactly one owning issue and cannot report
parity while their test still calls `expect_gap`.

## Transport evidence

Each test writes a transport evidence record in the exchange directory. The bzr record is `REST`,
matching the explicit `--api rest` invocation. The python-bugzilla helper reports the concrete
backend class selected after connecting; the phase rejects an empty or unknown value. Each parity
row's test therefore proves both the semantic result and the transports used to obtain it.

## Failure handling and cleanup

The phase stops an ordinary parity test at the first failed client operation, malformed JSON
result, missing ID, or semantic mismatch and reports the captured command output through the
existing harness. A gap test first validates its live python-bugzilla result, then reports the bzr
failure or mismatch and immediately applies its one exact `expect_gap` marker. Unique bug summaries
and run-specific values make repeated runs independent. The comparison runner's existing EXIT trap
removes the sidecar and exchange directory, including API-key-bearing input files.

## Threat model

The existing ADR 0044 boundaries remain. This phase additionally passes the repository's fixed
disposable-test API key into the sidecar and parses JSON produced by both clients. Only the local
operator or CI job can invoke the harness; the disposable Bugzilla controls responses.

Controls are: pass only operation names and private exchange-file paths as quoted argv, set umask
077 before writing request files, place JSON and credentials only under the runner's private
exchange directory, never evaluate response text, constrain helper operations to a fixed dispatch
table, validate IDs as positive integers before reuse, and normalize with fixed field selectors.
The staged helper is the only repository-derived file mounted into the sidecar. Error output names
the input path, never its API key content. Production credentials, the rest of the repository, and
the container runtime socket remain outside the sidecar.

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
  result to prove each stale `expect_gap` becomes a test failure.
- `make lint` validates semantic IDs, Bash syntax, ShellCheck, formatting, and Rust lints; it does
  not require host Python.
- `make test` proves the Rust suite remains green.
- `make functional-compare-all` proves the common lifecycle and all five live expected gaps against
  bz50, bz52, and bz53.
- `make functional-test-all` proves the established real-container suite remains green.
