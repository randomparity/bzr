# Python-bugzilla comparison harness design

## Scope

Issue #666 adds a functional comparison suite that runs bzr and python-bugzilla 3.3.0 against the
same existing Bugzilla containers. It changes test, developer, and CI infrastructure only; it does
not change compiled bzr behavior or claim parity beyond the shipped product-list smoke test.

The design follows [ADR 0044](../../adr/0044-python-bugzilla-comparison-sidecar.md). It preserves
ADR 0029's `<phase>/<slug>` semantic test IDs and ADR 0030's checkout-scoped Bugzilla lifecycle.

## Architecture

`run-compare.sh` owns comparison-run setup, sidecar lifecycle, result files, the ordered comparison
phase list, and the final summary. It sources the existing `lib.sh`; additions there provide the
comparison result counter, expected-gap transition, python-bugzilla command capture, and sidecar
helpers. The first phase runs both clients, normalizes their product names, and compares the sorted
sets.

The sidecar image is built from Python's slim image and pins `python-bugzilla==3.3.0`. The runtime
joins the already-running Bugzilla container's network namespace. A checkout/version-derived name
prevents sibling worktrees from colliding, while a named home volume preserves python-bugzilla cache
state. The comparison runner bind-mounts its mode-private config directory at `/work` for captured
output and generated configuration.

## Contracts

### Runner contract

- `tests/functional/run-compare.sh` resolves the runtime and active Bugzilla container with the
  existing container helpers and fails with an actionable message when either is unavailable.
- It creates one private `FUNC_CONFIG_DIR`, starts one sidecar, sources each declared
  `tests/functional/compare/*.sh` phase with `CURRENT_TEST_GROUP` set, and removes the sidecar and
  temporary files through an EXIT trap.
- The initial phase list contains `00-products`; future files must be explicitly added to the list.
- The runner exits non-zero when ordinary failures or stale expected gaps exist. Expected gaps alone
  are green.
- The summary prints separate PASS, FAIL, SKIP, and EXPECTED GAP counts. `GITHUB_STEP_SUMMARY`, when
  set, receives a one-line table containing the version and all four counts.

### Sidecar and command contract

- `tests/functional/pybz/Containerfile` installs exactly python-bugzilla 3.3.0 without a shell
  heredoc and leaves a literal long-lived process as its default command.
- Sidecar names and cache volumes are derived from `bugzilla_checkout_id` and `BZ_VERSION`; user
  input is never evaluated as shell source.
- `run_pybz <args...>` executes `bugzilla` in the running sidecar and records `PYBZ_STDOUT`,
  `PYBZ_STDERR`, and `PYBZ_EXIT`. It always returns zero so phase assertions decide the result.
- Sidecar startup builds the pinned image, removes a stopped same-name sidecar if present, and
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
phase loop points at the supplied directory basename, so the same guard covers `run-tests.sh` /
`phases` and `run-compare.sh` / `compare`. Fixture tests prove defaults, alternate paths, missing
paths, mismatched source paths, malformed IDs, and duplicate IDs.

### Make and CI contract

- `make functional-compare` starts the selected Bugzilla version, builds the release bzr binary,
  and runs `run-compare.sh`.
- `make functional-compare-all` performs the comparison serially for bz50, bz52, and bz53.
- `make lint` validates both phase trees and shell-checks/parses all new Bash files.
- The daily/manual functional workflow adds a comparison job that builds bzr once, runs all three
  versions, and always removes Bugzilla and sidecar containers. Pull-request CI is not expanded.

## Smoke comparison

`compare/00-products.sh` invokes `bzr --json product list` and
`bugzilla --bugzilla http://127.0.0.1 info --products`. Each output is reduced to a sorted unique
list of non-empty product names. The test passes only when both commands succeed and the normalized
lists are byte-identical. This proves the sidecar, shared server, command capture, normalization,
and result reporting on a real Bugzilla instance.

## Parity report

`docs/dev/python-bugzilla-parity.md` identifies python-bugzilla 3.3.0 and contains the columns
Capability, bzr equivalent, Status, and Evidence test ID. The first row records product listing as
covered by `00-products/list-products`; future entries can use covered, gap, or not compared.

## Error handling and cleanup

Container build/start failures stop before phases. Test command failures remain captured and are
reported through the phase result. Cleanup does not mask the runner's result. The all-version runner
continues across versions, reports each result, and exits non-zero if any version failed.

## Threat model

### Boundaries and actors

- The local operator or CI job controls environment overrides and invokes the container runtime.
- The real test Bugzilla controls HTTP responses consumed by both CLIs.
- The python-bugzilla image build consumes a pinned package from PyPI and the pinned Python base
  image selected in the Containerfile.
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
- The dependency is version-pinned and the repository's scheduled workflow is the first supported
  execution environment. This change does not widen workflow token permissions.

### Out of scope

The harness does not secure the disposable local Bugzilla service against other processes on the
host, verify PyPI package signatures, or provide tenant isolation. Those risks belong to the
existing functional-test/container and dependency supply-chain environments; this change exposes no
new public service and uses no production credentials.

## Verification

- Guard fixtures prove both default and parameterized semantic-ID validation.
- `make lint` proves formatting, shell syntax/static analysis, and both test-ID trees.
- `make test` proves the Rust suite remains green.
- `make functional-compare-all` proves the smoke comparison on bz50, bz52, and bz53.
- `make functional-test-all` proves the existing real-container suite remains green.
