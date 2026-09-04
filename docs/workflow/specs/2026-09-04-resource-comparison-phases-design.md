# Resource comparison phases design

## Scope and authority

Issue #668 extends the python-bugzilla comparison harness to comments, attachments, users, groups,
products, and components. It covers epic #665 requirements R9 and R10, records observed transport,
adds expected-gap baselines owned by #674 and #675, and publishes parity-report evidence. It does
not implement those gaps, auth/config/TLS comparisons owned by #669, bug-lifecycle comparisons
owned by #667, or final catalogue consolidation owned by #683.

[ADR 0044](../../adr/0044-python-bugzilla-comparison-sidecar.md) governs sidecar isolation,
semantic comparison, and `expect_gap` behavior.
[ADR 0045](../../adr/0045-observe-comparison-transport-from-debug-events.md) governs observed
transport evidence. [ADR 0046](../../adr/0046-share-python-bugzilla-comparison-adapter.md)
generalizes the fixed library adapter used by the phases in this change.

## Global constraints

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

## Architecture

The runner stages one general python-bugzilla adapter in its private exchange directory before
starting the sidecar. It already sources `tests/functional/lib.sh`; after the sidecar starts, it
initializes that library's shared resource-comparison state before running four ordered phases:
comments, attachments, users/groups, and products/components. Resource families that share state
remain together: group tests mutate a user membership, and component tests create inside a product.

Before the attachment phase, the runner idempotently seeds one comparison-owned attachment flag
type and its unrestricted inclusion row through the existing `run_bugzilla_sql_file PATH` helper.
The fixed name `bzr_compare_attachment_review` prevents repeated single-version runs from growing
flag metadata. A failed seed aborts comparison as infrastructure failure before any flag parity
claim; the comparison never depends on the ordinary functional phase having run first.

Each comparison follows the same data flow:

1. create run-unique fixtures through one client;
2. perform the equivalent operation through the other client on a distinct fixture when mutation
   would otherwise collide;
3. read both results back from the live server;
4. reduce generated IDs, timestamps, client-specific aliases, and ordering to a canonical JSON
   projection;
5. compare the canonical persisted facts and exact transport records.

The existing bug-lifecycle phase continues unchanged except for the adapter's internal filename.

## Adapter and shell interfaces

`tests/functional/compare/python-bugzilla-adapter.py` accepts exactly:

```text
python-bugzilla-adapter.py OP INPUT OUTPUT
```

`INPUT` and `OUTPUT` are absolute paths under `/work/compare`; inputs are private JSON objects and
outputs are private JSON containing `{transport, result}`. The fixed operation table retains all
lifecycle operations and adds narrow handlers for:

- comment creation/readback through `build_update`/`update_bugs` and `get_comments`;
- attachment upload, list/get, content download, and flag update through `attachfile`,
  `get_attachments`, `openattachment`, and `updateattachmentflags`;
- user create/get/search and group membership updates through `createuser`, `getuser`,
  `searchusers`, and `updateperms`;
- group reads through `getgroup` and `getgroups`;
- product catalogue reads through `product_get` for `accessible`, `enterable`, and `selectable`;
- component creation through `addcomponent`;
- a local component-update shape proof used only to establish #675's client-surface gap without
  claiming a stock server implements the Red Hat extension.

Transport selection is a closed adapter field: absent means normal python-bugzilla probing;
`REST` sets `force_rest`; `XMLRPC` sets `force_xmlrpc`; any other value fails before connecting.
Operations that historically require a fixed backend retain and validate that backend.

The existing `tests/functional/lib.sh` provides the shared phase mechanics: private request-file
creation, the fixed adapter invocation, capture preservation, bzr debug-event transport
observation, positive-ID validation, canonical JSON comparison, and fail-closed gap eligibility.
It does not own resource fixtures or projections. Keeping these functions in the established
library also preserves the invariant that every `tests/functional/compare/*.sh` file is a phase.

## Comparison coverage

### Comments

Create paired bugs with the same normalized description. Add the same public comment through each
client, read both through `get_comments`/`bzr comment list`, and compare normalized text and
privacy.
Then add private comments and read each fixture through forced REST and forced XML-RPC for both
clients. On Bugzilla 5.0, API-key REST reads filter private comments for both clients; that arm
must prove each controlled comment exists with `is_private=true` through an independent XML-RPC
read before comparing the two empty REST projections. The XML-RPC arm must contain the controlled
private text directly. A missing persistence proof, unexpected REST exposure, or unequal result
fails rather than silently comparing two empty results. Because bzr comment creation is REST-only,
its write is validated as REST while the case's selected transport governs the read.

Stable IDs:

- `compare/02-comments/public-comments`
- `compare/02-comments/private-comments-rest`
- `compare/02-comments/private-comments-xmlrpc`

### Attachments

Create paired bugs and private exchange files with identical bytes. Upload through both clients
with a controlled filename, summary, `text/plain` content type, comment, and privacy value. Read
metadata and bytes back through both clients, normalize attachment fields and linked comment text,
and compare content by a deterministic SHA-256 digest. Set the same attachment flag through
`updateattachmentflags` and `bzr attachment update --flag`, then compare normalized flag records.
The flag is the runner-seeded `bzr_compare_attachment_review` fixture, not a Bugzilla image default
or a flag provisioned by the separate ordinary functional runner.

Private attachment list/get operations run through forced REST and XML-RPC for both clients and
must return the controlled private attachment and exact digest. The download comparison covers
python-bugzilla `--get`/library single attachment and `--getall`/library per-bug retrieval against
`bzr attachment download <id>` and `--bug <id>`.

Two #674 gap baselines remain separate so either capability can close independently:

- multi-bug upload: python-bugzilla attaches one file to two controlled bug IDs; bzr must accept
  equivalent multiple targets and create one attachment per bug;
- obsolete filtering: after marking one attachment obsolete, python-bugzilla's
  `--getall --ignore-obsolete` result excludes it; bzr bulk download must do the same.

Stable IDs:

- `compare/03-attachments/upload-metadata-comment`
- `compare/03-attachments/download-content`
- `compare/03-attachments/attachment-flags`
- `compare/03-attachments/private-attachments-rest`
- `compare/03-attachments/private-attachments-xmlrpc`
- `compare/03-attachments/multi-bug-upload`
- `compare/03-attachments/ignore-obsolete`

### Users and groups

Create two run-unique users with equivalent email/name/password data, one per client. Read each by
exact login and by a discriminating search term; normalize email, real name, enabled state, and
sorted group names. Read a known fixture group through both `getgroup` and `getgroups`, requiring
the same ID/name/description/active state.

Add each paired user to the known group through `updateperms` and `bzr group add-user`, prove exact
membership through both client read paths, then remove them and prove absence. The negative check
is mandatory because reusable containers can retain membership from earlier runs. The shared
resource layer records every membership it adds and the runner's existing EXIT cleanup removes any
still-recorded membership after a partial phase failure; successful explicit removal clears the
record. Cleanup failure changes an otherwise-successful run to failure.

Stable IDs:

- `compare/04-users-groups/user-create-get-search`
- `compare/04-users-groups/group-get-and-list`
- `compare/04-users-groups/membership-add-remove`

### Products and components

For each of `accessible`, `enterable`, and `selectable`, retrieve the live catalogue through
`product_get` and `bzr product list --type`, then compare sorted product names while requiring the
known functional product as a positive control where that catalogue promises it. Create paired
run-unique products and add one equivalent component to each through `addcomponent` and
`bzr component create`; fresh product/component reads compare name, description, active state,
and default assignee.

The #675 baseline proves only the absent component-update client surface. Inside the ordinary
sidecar, the adapter constructs the pinned python-bugzilla `Bugzilla` object without a URL and
replaces its backend with an in-process recorder that implements only `component_update`. Calling
the library's public `editcomponent` method must dispatch the exact normalized `names` and
`updates` request to that recorder. The operation writes `{transport: null, result}` and rejects
any attempted network or server access; the null transport is an explicit local-proof contract,
not missing evidence. The bzr arm then returns the exact unrecognized-subcommand parser failure.
This does not call the unsupported extension on the stock live containers and does not decide
whether #675 later uses a proxy or records a non-goal.

Stable IDs:

- `compare/05-products-components/product-catalogues`
- `compare/05-products-components/component-create`
- `compare/05-products-components/component-update-redhat`

## Failure handling

Every test stops its comparison arm on command failure, malformed JSON, unknown transport, missing
positive control, non-positive generated ID, digest mismatch, or normalization failure. Such faults
remain ordinary failures. Gap conversion is permitted only for the two exact #674 capability
probes and the exact #675 controlled parser probe after the pinned library's local request-shape
evidence succeeds.
Run-unique names prevent reused containers from turning creates into accidental idempotent passes.

Created bugs, comments, attachments, users, products, and components follow the established
functional-suite lifecycle: the all-version runner removes every version container in its EXIT
trap, while the single-version developer target keeps its checkout/version-scoped container for
fast follow-up. Stock Bugzilla does not offer a uniform delete path for those resources, so the
single-version path uses run-unique bounded names and exact selectors instead of pretending to
roll back server records. Membership is the exception because it can contaminate later assertions;
it is always removed through the EXIT-safe cleanup above. Running `setup-bugzilla.sh reset` remains
the explicit way to discard a retained local container and all of its fixtures.

## Threat model

### Boundaries and actors

- Added boundary: resource phases supply JSON and attachment paths to the staged adapter. The only
  actor is the repository-owned comparison runner in a developer or CI job.
- Widened boundary: the adapter reads a file containing test attachment bytes from the bind-mounted
  exchange directory and sends them to the disposable Bugzilla container.
- Existing boundary: the adapter sends the API key to the local disposable server and emits safe
  result JSON to the bind mount.

No anonymous network or tenant-controlled input reaches the adapter. Trust is placed in the checked
out repository, the local container runtime, and the existing functional-test administrator secret.

### Controls

- Input/output/attachment paths resolve beneath `/work/compare`; attachment input is a regular,
  non-symlink file with no group/other permission bits.
- Operation names and transport values use closed dispatch tables; request keys are operation-
  specific and unexpected fields fail before network access.
- The API key remains in a mode-0600 JSON input and is never included in adapter results, argv, or
  error text. Upstream exception messages remain suppressed.
- Output uses no-follow creation and mode 0600. Phases retain only normalized non-secret evidence.
- Fixture names and content are bounded constants plus short run tokens; no arbitrary host path or
  URL is accepted from a comparison phase.
- The flag seed is a fixed repository-owned SQL statement sent through `run_bugzilla_sql_file`; it
  interpolates no phase input and verifies the fixed type/inclusion rows before comparison.

### Out of scope

Compromise of the checked-out repository, container runtime, pinned upstream package supply chain,
or disposable Bugzilla image is outside this test change. ADR 0044 already records the package
artifact/digest residual. The adapter is not installed with `bzr` and is not a supported user entry
point.

## Verification

Focused container fixtures must fail before the generalized adapter, shared resource helpers,
stable phase IDs, exact gap mappings, and parity rows exist; then pass after implementation. Fault
controls perturb one canonical field, remove one private result, report the wrong transport, expose
a non-private attachment path, make flag seeding fail, interrupt membership removal, and make each
stale gap pass; every control must turn the suite red. The cleanup control must prove the EXIT path
attempts the recorded removal and converts cleanup failure into a failed run.

Repository proof is:

- `bash tests/functional/pybz/container-tests.sh`
- `make lint`
- `make test`
- `make functional-compare-all`
- `make functional-test-all`

The live commands must pass for bz50, bz52, and bz53 before delivery.
