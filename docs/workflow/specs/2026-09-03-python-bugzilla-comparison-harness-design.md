# Python-bugzilla comparison harness design

## Scope

Issue #666 adds a functional comparison suite that runs bzr and python-bugzilla 3.3.0 against the
same existing Bugzilla containers. It changes test, developer, and CI infrastructure only; it does
not change compiled bzr behavior or claim parity beyond the shipped product-list smoke test.

The design follows [ADR 0044](../../adr/0044-python-bugzilla-comparison-sidecar.md). It extends ADR
0029's semantic test IDs with a comparison-tree prefix and preserves ADR 0030's checkout-scoped
Bugzilla lifecycle.

## Architecture

`run-compare.sh` owns comparison-run setup, sidecar lifecycle, result files, the ordered comparison
phase list, and the final summary. It sources the existing `lib.sh`; additions there provide the
comparison result counter, expected-gap transition, python-bugzilla command capture, and sidecar
helpers. The first phase runs both clients, normalizes their product names, and compares the sorted
sets.

The sidecar image selects the versioned `python:3.14.7-slim-bookworm` tag and fixes the top-level
package at `python-bugzilla==3.3.0`. The runtime joins the already-running Bugzilla container's
network namespace. Checkout-derived image identity and checkout/version-derived container names
prevent sibling worktrees from racing or colliding, while a named home volume preserves
python-bugzilla cache state. The comparison runner bind-mounts its mode-private config directory at
`/work` with a private SELinux relabel (`:Z`) for captured output and generated configuration.

## Contracts

### Runner contract

- `tests/functional/run-compare.sh` resolves the runtime and active Bugzilla container with the
  existing container helpers and fails with an actionable message when either is unavailable.
- It creates one private `FUNC_CONFIG_DIR`, starts one sidecar, sources each declared
  `tests/functional/compare/*.sh` phase with `CURRENT_TEST_GROUP` set, and removes the sidecar and
  temporary files through an EXIT trap.
- The initial phase list contains `00-products`; future files must be explicitly added to the list.
- It sets `TEST_ID_PREFIX=compare`, producing IDs in the exact
  `compare/<phase>/<slug>` namespace while the ordinary runner's empty prefix preserves its
  existing `<phase>/<slug>` IDs.
- The runner exits non-zero when ordinary failures or stale expected gaps exist. Expected gaps alone
  are green.
- The summary prints separate PASS, FAIL, SKIP, and EXPECTED GAP counts. `GITHUB_STEP_SUMMARY`, when
  set, receives a one-line table containing the version and all four counts.

### Sidecar and command contract

- `tests/functional/pybz/Containerfile` uses `python:3.14.7-slim-bookworm`, installs exactly
  python-bugzilla 3.3.0 without a shell heredoc, and leaves a literal long-lived process as its
  default command.
- Sidecar names and cache volumes are derived from `bugzilla_checkout_id` and `BZ_VERSION`; user
  input is never evaluated as shell source.
- The built image tag includes `bugzilla_checkout_id`, preventing another worktree's concurrent
  build from changing the image used at sidecar creation.
- The `/work` bind mount uses `:Z`, and the container fixture proves container-to-host write-through
  so enforcing SELinux hosts do not silently lose the exchange path.
- `run_pybz <args...>` executes `bugzilla` in the running sidecar and records
  `BZR_STDOUT`, `BZR_STDOUT_RAW`, `BZR_STDERR`, and `BZR_EXIT`. It always returns zero so existing
  phase assertions decide the result unchanged. A phase copies the first client's capture to a
  file under `FUNC_CONFIG_DIR` before invoking the second.
- Sidecar startup builds the configured image, removes a stopped same-name sidecar if present, and
  refuses to replace a running same-name sidecar.

### Result contract

- `expect_gap <issue>` accepts exactly one decimal GitHub issue number and applies to the current
  comparison result.
- A failed comparison followed by `expect_gap` moves that result from FAIL to EXPECTED GAP and emits
  `GAP (#<issue>)`.
- A passed comparison followed by `expect_gap` moves it from PASS to FAIL and explains that issue
  #N appears resolved. This makes closed gaps visible instead of silently permanent.
- Calling it without a current pass/fail result, with a malformed issue, or twice for one test is a
  harness error and returns non-zero.

### Static guard contract

`tools/check-functional-test-ids.sh` accepts optional `runner` and `phase directory` paths after the
repository root. Defaults preserve the existing suite. It validates that the runner's canonical
phase loop points at the supplied directory basename and derives an empty namespace for `phases`
or that basename (`compare`) for another tree. Runtime `test_begin` composes the matching prefix.
Fixture tests prove defaults, alternate paths, missing paths, mismatched source paths, exact
qualified output, malformed IDs, duplicate IDs, and cross-tree distinction.

### Make and CI contract

- `make functional-compare` starts the selected Bugzilla version, builds the release bzr binary,
  and runs `run-compare.sh`.
- `make functional-compare-all` performs the comparison serially for bz50, bz52, and bz53.
- `make lint` validates both phase trees and shell-checks/parses all new Bash files.
- The daily/manual functional workflow adds a comparison job that builds bzr once, runs all three
  versions, and always removes Bugzilla and sidecar containers. Pull-request CI is not expanded.

## Smoke comparison

`compare/00-products.sh` invokes `bzr --json --server-url "$BZ_URL" product list` from the fresh,
empty config and
`bugzilla --bugzilla http://127.0.0.1 info --products`. Each output is reduced to a sorted unique
list of non-empty product names. The test passes only when both commands succeed and the normalized
lists are byte-identical. This proves the sidecar, shared server, command capture, normalization,
and result reporting on a real Bugzilla instance.

## Parity report

`docs/dev/python-bugzilla-parity.md` identifies python-bugzilla 3.3.0 and contains the columns
Capability, bzr equivalent, Status, and Evidence test ID. The first row records product listing as
`parity` only after `compare/00-products/list-products` passes. Later report entries and their
terminal classification remain owned by the follow-on comparison and report issues.

## Error handling and cleanup

Container build/start failures stop before phases. Test command failures remain captured and are
reported through the phase result. Cleanup does not mask the runner's result. The all-version runner
continues across versions, reports each result, and exits non-zero if any version failed.

## Threat model

### Boundaries and actors

- The local operator or CI job controls environment overrides and invokes the container runtime.
- The real test Bugzilla controls HTTP responses consumed by both CLIs.
- The python-bugzilla image build consumes a version-fixed top-level package from PyPI and a
  versioned Python base tag. Their artifacts, base-image digest, and transitive dependency closure
  remain mutable upstream.
- GitHub Actions controls `GITHUB_STEP_SUMMARY`; the runner appends only fixed labels and numeric
  counters.

### Controls

- Runtime/container names are passed as quoted argv and must match the existing derived-name path;
  no value is evaluated.
- The sidecar receives only the functional runner's fresh temporary directory, never the repository
  root, host home, credentials, or container socket.
- Product responses are treated as data and normalized with line-oriented filters; they never form
  commands or paths.
- Cleanup targets only names computed for this checkout and version. It does not enumerate or prune
  unrelated containers or volumes.
- The top-level dependency version is fixed and the repository's scheduled workflow is the first
  supported execution environment. This change does not widen workflow token permissions.

### Out of scope

The harness does not secure the disposable local Bugzilla service against other processes on the
host, pin the base-image digest or complete Python dependency closure, verify PyPI package
signatures, or provide tenant isolation. A compromised upstream could execute inside the disposable
sidecar and falsify comparison evidence, but receives no repository mount, runtime socket,
production credentials, or writable workflow token. This change exposes no new public service.

## Verification

- Guard fixtures prove both default and parameterized semantic-ID validation.
- Result fixtures drive the summary path and prove an expected-gap-only result exits zero, all four
  terminal/GitHub counters are present, and a stale gap makes the aggregate exit non-zero.
- `make lint` proves formatting, shell syntax/static analysis, and both test-ID trees.
- `make test` proves the Rust suite remains green.
- `make functional-compare-all` proves the smoke comparison on bz50, bz52, and bz53 in Ubuntu CI
  and on local hosts providing Bash, Make, jq, rg, and Docker/Podman.
- `make functional-test-all` proves the existing real-container suite remains green.
