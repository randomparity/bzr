# Resource comparison phases implementation plan

Goal: Compare comments, attachments, users, groups, products, and components between `bzr` and
python-bugzilla against every supported live Bugzilla container, and publish stable parity evidence.

Architecture: Generalize the existing fixed sidecar adapter, add one shared shell layer for
resource-comparison mechanics, and keep fixtures/projections in four ordered resource-family
phases. Live operations compare canonical persisted state and observed transport. The one
Red Hat-only checkpoint uses explicit no-network evidence. Gap ownership fails closed.

Tech stack: Bash, Python 3, python-bugzilla 3.3.0, jq, Docker/Podman functional containers.

Expected implementation size: 2200–3400 changed lines (L) — summed from 250–450 adapter lines,
150–250 shared-helper lines, 900–1400 phase lines, 800–1200 focused-fixture lines, and
100–100 runner/report lines. The upper line count is large test data flow, while the change
remains one test-only comparison-harness slice with no compiled-product contract.

## Global Constraints

- python-bugzilla stays pinned at 3.3.0 in the existing sidecar image.
- The supported live matrix remains bz50, bz52, and bz53.
- Host architecture is arm64; declared release targets are x86_64 Linux, aarch64 Linux,
  powerpc64le Linux, s390x Linux, aarch64 macOS, x86_64 Windows, and aarch64 Windows. This test-only
  shell/Python change does not infer one set from the other.
- Comparison IDs use `compare/<phase>/<slug>` and remain unique across both functional trees.
- Every successful network operation records an observed `REST` or `XMLRPC` boundary. Requested
  transport alone is not evidence.
- A capability gap is eligible for `expect_gap` only after the python-bugzilla side is validated
  and the bzr side reaches a recognized semantic or exact parser mismatch. Infrastructure,
  malformed evidence, auth, and connection failures remain failures.
- API keys live only in private request files or process environment, never command arguments,
  output, diagnostics, or retained fixture data.

BASE_BRANCH: `main`

Guardrails: `bash tests/functional/pybz/container-tests.sh`; `make lint`; `make test`;
`make functional-compare-all`; `make functional-test-all`. Observed cost is unknown before this
branch's first run. The focused fixture needs Bash, Python, and container tooling used by its image
arm; live suites need Docker or Podman and may take at least ten minutes per version. CI hard-gates
format, clippy, unit/integration tests, shell checks, and functional-test ID checks; live functional
containers run on the scheduled workflow rather than the PR gate. The ADR index is not coupled by
an individually hard-gated check.

## Task 1: Generalize and harden the fixed sidecar adapter

### Interfaces

- Modifies and renames `tests/functional/compare/bug-lifecycle.py` to
  `tests/functional/compare/python-bugzilla-adapter.py`.
- Modifies `tests/functional/lib.sh`: `run_pybz_adapter OP INPUT OUTPUT` invokes fixed command
  `python /work/compare/python-bugzilla-adapter.py OP INPUT OUTPUT`.
- Modifies `tests/functional/run-compare.sh` to stage the renamed adapter privately.
- Modifies `tests/functional/pybz/container-tests.sh` for the renamed fixed command, new operations,
  transport selection, request validation, file confinement, and safe failures.
- Provides fixed operations for comment add/list; attachment upload/list/get/download/flag update;
  user create/get/search/update permissions; group get/list; product catalogue get; component add;
  and the local component-update shape proof.

### Verification

- Renamed fixed command and retained lifecycle behavior — Mode: focused-test. Update the existing
  wrapper/adapter fixture, first observe failure because the new staged path is absent, then run
  `bash tests/functional/pybz/container-tests.sh` and expect exit 0 with all legacy operations.
- New operation dispatch and canonical JSON serialization — Mode: focused-test. Add fake pinned
  backend cases for every new operation, first observe unsupported-operation failures, then expect
  the focused fixture to exit 0 with exact request shapes and `{transport,result}` objects. The
  component-update proof alone requires a null transport and an exact recorded request.
- Transport and file-boundary controls — Mode: focused-test. Add REST/XMLRPC/invalid transport,
  outside-path, symlink, public-mode attachment, unknown-key, and upstream-exception cases; first
  observe at least one unsafe input is accepted or unsupported, then expect the focused fixture to
  exit 0 only when all are rejected without secret/upstream text.

### Steps

1. Update focused fixtures to require the generalized filename, legacy operation compatibility,
   all new fixed operation shapes, explicit transport selection, and attachment-file controls.
2. Run `bash tests/functional/pybz/container-tests.sh`; expect non-zero because the generalized
   adapter path and operations do not yet exist.
3. Rename the adapter, change its usage label, add closed transport parsing, preserve fixed backend
   rules for legacy operations, and add the new small handlers with strict request-key validation.
4. Require attachment input to resolve beneath `/work/compare`, be a regular non-symlink file, and
   have mode 0600; preserve no-follow mode-0600 output and safe exception handling.
5. Update the fixed wrapper and runner staging path with no compatibility alias.
6. Run the focused fixture; expect exit 0.
7. Commit with `test(functional): generalize python-bugzilla adapter`.

## Task 2: Add shared resource comparison mechanics and comment coverage

### Interfaces

- Extends `tests/functional/lib.sh` with fixed adapter request/capture helpers, observed transport
  validation, positive-ID extraction, canonical JSON equality, and gap eligibility
  reset/allow/apply functions. A separate `compare/*.sh` helper is forbidden because every shell
  file there is a comparison phase under the functional-ID guardrail. Resource initialization
  creates a temporary named bzr server with query-parameter API-key auth to match
  python-bugzilla's REST behavior; both XML-RPC clients authenticate in the request body.
- Creates `tests/functional/compare/02-comments.sh` with stable `public-comments`,
  `private-comments-rest`, and `private-comments-xmlrpc` IDs.
- Modifies `tests/functional/run-compare.sh` to initialize shared resource state and add
  `02-comments` to the ordered phase list.
- Modifies `tests/functional/pybz/container-tests.sh` with deterministic shell fixtures and fault
  controls for the helper and comment phase.

### Verification

- Shared capture and transport behavior — Mode: focused-test. Add fake bzr/sidecar commands that
  emit REST, XMLRPC, missing, and mixed debug events; first observe the resource helper/phase is
  absent, then expect the fixture to accept exactly one observed class and reject the rest.
- Comment persisted-state and private visibility — Mode: focused-test. Simulate paired comments
  with matching query-parameter auth for REST and matching request-body auth for XML-RPC. First
  observe missing IDs, then expect all three IDs to pass with the controlled records present
  directly. Remove the private record,
  flip `is_private`, omit bzr's matching auth configuration, or falsify a transport; each
  controlled run must exit non-zero and name the ID. A failed named-server setup must abort before
  any resource test.

### Steps

1. Add focused helper/comment fixtures, including missing/mixed transport and private-record faults;
   run them and observe the expected missing-phase failure.
2. Implement the shared private JSON request files, adapter invocation, bzr capture, observed
   transport, canonical comparison, fail-closed gap state, and temporary named bzr server using
   query-parameter auth to match python-bugzilla.
3. Implement paired public comments and private REST/XMLRPC arms with unique bugs and positive
   controls; compare normalized text/privacy and exact read transports. Validate bzr writes on
   their actual REST path and fail if either read filters the controlled private record.
4. Add `02-comments` to the runner and run the focused fixture; expect exit 0.
5. Enable each fault control, observe non-zero with the stable ID, remove the fault, and rerun
   green.
6. Run `make check-functional-test-ids`; expect exit 0 with all three exact IDs unique.
7. Run `make functional-compare`; expect the comment IDs to pass on the default live version.
8. Commit with `test(functional): compare comments across clients`.

## Task 3: Add attachment parity and #674 baselines

### Interfaces

- Creates `tests/functional/compare/03-attachments.sh` with exact IDs
  `upload-metadata-comment`, `download-content`, `attachment-flags`,
  `private-attachments-rest`, `private-attachments-xmlrpc`, `multi-bug-upload`, and
  `ignore-obsolete`; only the final two map to #674.
- Extends `tests/functional/lib.sh` with an idempotent
  `seed_comparison_attachment_flag_type` prerequisite and modifies
  `tests/functional/run-compare.sh` to call it before `03-attachments`. The function writes a
  private temporary SQL file containing fixed `flagtypes`/`flaginclusions` rows for
  `bzr_compare_attachment_review`,
  executes it through existing `run_bugzilla_sql_file PATH`, verifies exactly one matching type and
  one unrestricted inclusion (`product_id` and `component_id` both null), and removes the file on
  every return.
- Modifies `tests/functional/pybz/container-tests.sh` for attachment fixtures, digest comparison,
  private transport arms, and stale-gap controls.
- Extends `tests/functional/compare/python-bugzilla-adapter.py` with a bounded
  `attachment_cli_download_bug` operation that invokes the pinned CLI's `_do_get_attach` obsolete
  filter while replacing its output opener with a private basename-only exchange-directory writer.
  Focused controls cover path-bearing names, symlink/non-directory destinations, a same-name
  collision sentinel, and private directory/file modes.
- Consumes Task 1's other attachment operations and Task 2's shared comparison helpers.

### Verification

- Upload, metadata, comments, download bytes, flags, and private visibility — Mode: focused-test.
  Add deterministic attachment responses and file bytes, first observe missing IDs, then expect
  the fixture to pass all five parity IDs. Perturb summary, comment, digest, flag, privacy, and
  transport one at a time; each must produce non-zero for its stable ID.
- Exact #674 ownership and stale-gap behavior — Mode: focused-test. Require both gap IDs to map
  only to 674 after validated python-bugzilla results, assert the rendered owner per ID, and make a
  substituted owner turn the fixture red; first observe missing markers, then expect GAP.
  Multi-bug upload runs
  `bzr attachment upload <BUG1> <BUG2> <FILE>` and accepts only exit 2 plus the exact controlled
  `unexpected argument '<FILE>' found` line and upload usage. Obsolete filtering runs
  `bzr attachment download --bug <BUG> --ignore-obsolete --out-dir <DIR>` and accepts only exit 2
  plus `unexpected argument '--ignore-obsolete' found` and download usage. An unrelated exit-2
  diagnostic, any other non-zero status, or invalid python evidence remains FAIL. Make either bzr
  probe semantically pass and require the stale marker to fail the fixture.
- Flag prerequisite isolation — Mode: focused-test and live-functional. Run the attachment phase
  fixture without ordinary functional phases, first observe flag update cannot succeed, then
  require the seed helper to create and verify the fixed attachment flag on bz50, bz52, and bz53.
  Restricted-only inclusion, seed, or readback failure must remain FAIL and must not become a #674
  gap.

### Steps

1. Add focused attachment phase fixtures and the complete ID-to-owner assertions; run and observe
   non-zero because the phase is absent.
2. Implement `seed_comparison_attachment_flag_type` with one fixed idempotent flag type and
   unrestricted inclusion, call it before the attachment phase, and fail before comparison when
   execution or exact readback fails. Do not rely on `02-server-auth.sh` state.
3. Create two private exchange files with identical controlled bytes and mode 0600. Implement
   paired uploads, metadata/comment normalization, single and bulk content reads, SHA-256 digest
   comparison, flag update/readback, and private REST/XMLRPC list/get checks.
4. Implement multi-bug upload and obsolete-filter probes with the exact commands and exit-2
   diagnostics in the verification inventory. Permit #674 gap conversion only after the python
   operation and evidence are valid. Add a controlled unrelated exit-2 diagnostic and prove it
   remains FAIL.
5. Add `03-attachments` to the runner; run focused fixtures and expect exit 0 with five passes and
   two #674 gaps.
6. Prove each mismatch, seed failure, and stale-gap control turns the fixture red; restore green.
7. Run `make check-functional-test-ids`; expect exit 0 with all seven exact IDs unique.
8. Run `make functional-compare`; expect attachment parity passes and #674 gaps on the default live
   version.
9. Commit with `test(functional): compare attachments across clients`.

## Task 4: Add user, group, product, and component coverage

### Interfaces

- Creates `tests/functional/compare/04-users-groups.sh` with exact IDs
  `user-create-get-search`, `group-get-and-list`, and `membership-add-remove`.
- Creates `tests/functional/compare/05-products-components.sh` with exact IDs
  `product-catalogues`, `component-create`, and `component-update-redhat`; only the final ID maps
  to #675.
- Modifies `tests/functional/run-compare.sh` to add both phases.
- Modifies `tests/functional/pybz/container-tests.sh` for all new phase fixtures, negative
  membership proof, catalogue controls, component persisted state, and #675 stale-gap behavior.
- Consumes Task 1 adapter operations and Task 2 shared mechanics.
- Extends the runner cleanup contract: shared resource helpers record each added `(user, group)`
  membership; explicit successful removal clears it, and EXIT cleanup removes remaining entries.

### Verification

- User/group persisted outcomes — Mode: focused-test. First observe missing IDs, then run the
  controlled phase fixture and expect three passes. Create paired accounts without the optional
  display name, require empty real-name readback, and use a run-unique comparison group so default
  effective memberships cannot satisfy the proof. Omit an exact search result, retain membership
  after removal, or substitute a non-member; each control must fail its stable ID.
- Partial-failure cleanup — Mode: focused-test. Interrupt the user/group phase after membership
  add, require EXIT cleanup to attempt that exact removal, and require a cleanup failure to change
  an otherwise-successful run to non-zero.
- Product/component persisted outcomes — Mode: focused-test. First observe missing IDs, then
  expect catalogue and component-create passes. Exercise python-bugzilla's `addcomponent` over
  XML-RPC with the stock endpoint's accepted `name` field, then read both components through bzr.
  Remove a catalogue type/positive control or alter a component field; each must fail its stable
  ID.
- Exact #675 boundary — Mode: focused-test. Inside the ordinary pinned sidecar, construct
  `Bugzilla` without a URL, install an in-process recorder implementing only
  `component_update`, call public `editcomponent` with
  `{product:P, component:C, initialowner:A, description:D, is_active:false}`, and require exactly
  `{names:[{product:P, component:C}], updates:{default_assignee:A, description:D,
  is_active:false}}`. Require `{transport: null, result}` and fail if the operation attempts network
  or server access. Then run `bzr component update` and accept only exit 2 with literal
  `error: unrecognized subcommand 'update'` and `Usage: bzr component [OPTIONS] <COMMAND>` lines;
  first observe the ID absent, then expect GAP #675. A perturbed converted field, unrelated exit-2
  diagnostic, or passing bzr substitute must remain FAIL or make the stale marker fail.

### Steps

1. Add focused fixtures for all six IDs, negative membership, catalogue differentiation, persisted
   component fields, exact #675 ownership, and stale-gap behavior; run and observe missing phases.
2. Implement unique paired-user creation, exact get/search normalization, run-unique group reads, and
   add/prove/remove/prove-absent membership flow. Register each successful add before its next
   assertion, clear it only after verified removal, and drain remaining registrations from the
   runner's EXIT cleanup without masking the original exit status.
3. Implement the three product catalogue comparisons with positive controls, paired unique product
   and component creation, and canonical component readback.
4. Implement #675's no-network pinned-library update-shape proof plus exact bzr parser rejection
   without invoking the extension on stock live servers.
5. Add both phases to the runner; run focused fixtures and expect five passes plus one #675 gap.
6. Run `make check-functional-test-ids`; expect exit 0 with all six exact IDs unique.
7. Run `make functional-compare`; expect all new default-version results green.
8. Commit with `test(functional): compare admin resources across clients`.

## Task 5: Publish parity evidence and verify the branch

### Interfaces

- Modifies `docs/dev/python-bugzilla-parity.md` with one row for every stable ID from Tasks 2–4.
- Modifies `tests/functional/pybz/container-tests.sh` to assert exact row presence, parity status,
  and #674/#675 ownership.
- Provides durable evidence consumed by #674, #675, and #683.

### Verification

- Parity-report coverage — Mode: focused-test. Add assertions that every new stable ID occurs
  once, parity rows say `parity`, both attachment gap rows name only #674, and the component row
  names only #675; first observe missing rows, then expect the focused fixture to exit 0.

### Steps

1. Add focused parity-row assertions and observe non-zero while the report lacks the rows.
2. Add the exact parity and expected-gap rows with their stable IDs; rerun the fixture green.
3. Run `make lint`; expect exit 0.
4. Run `make test`; expect exit 0.
5. Run `make functional-compare-all`; expect bz50, bz52, and bz53 green.
6. Run `make functional-test-all`; expect bz50, bz52, and bz53 green.
7. Commit with `docs(parity): record resource comparison evidence`.
