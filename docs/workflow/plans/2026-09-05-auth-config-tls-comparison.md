# Auth, configuration, and TLS comparison implementation plan

Goal: Add issue #669's R11 comparison evidence without changing production bzr behavior.

Architecture: bzr retains the existing host-side fixtures. The long-lived python-bugzilla sidecar
starts matching proxy processes on its shared Bugzilla loopback, stages inputs through `/work`, and
emits secret-free request-kind evidence. One new ordered phase owns the live comparisons and gap
markers.

Tech stack: Bash functional harness, Python 3 stdlib proxy fixtures, Docker/Podman, jq, openssl,
python-bugzilla 3.3.0, and Markdown parity documentation.

Expected implementation size: 850–1100 net changed lines (XL) — revised after Tasks 1–3 measured
712 net changed lines and the build-time checkpoint established that the live API-key comparison
requires one additional private-file network adapter operation. The remaining allowance covers
that operation, one multi-contract comparison phase, runner wiring, and parity rows.

## Global constraints

- Supported servers are Bugzilla `bz50`, `bz52`, and `bz53`; python-bugzilla is exactly 3.3.0.
- Host architecture is `arm64`; declared targets are x86_64/aarch64 Linux, powerpc64le Linux,
  s390x Linux, aarch64 macOS, and x86_64/aarch64 Windows; the host differs from the full target set.
- Preserve ADR 0044's one long-lived `--network container:<bugzilla-container>` sidecar and private
  `/work` mount. Do not add a host-network sidecar or production bzr behavior.
- Namespace proxies listen only on `127.0.0.1`, forward only to `127.0.0.1:80`, and record no
  credential values.
- Secret-bearing files are mode 0600 beneath the runner's mode-0700 `FUNC_CONFIG_DIR`.
- Shell is Bash with `set -euo pipefail`; quote argv, validate numeric ports/PIDs, and never eval.
- User-facing test identifiers are stable `compare/06-auth-config-tls/<slug>` values.
- Gap ownership is #676, #681, #682, #677, and #678; a gap follows a successful positive control.
- Guardrails are `make lint`, `make test`, `make functional-compare-all`, and
  `make functional-test-all`. Focused iteration uses the exact commands named below.

## File map

- Modify `tests/functional/redhat-shape-proxy.py`: observe credential kind and translate Bearer in
  explicit fixture mode.
- Modify `tests/functional/lib.sh`: stage/start/read/stop namespace proxies and install the alias.
- Modify `tests/functional/compare/python-bugzilla-adapter.py`: private-file token lifecycle and
  client-certificate surface observations.
- Modify `tests/functional/pybz/container-tests.sh`: executable fixtures for proxy helpers and phase.
- Create `tests/functional/compare/06-auth-config-tls.sh`: R11 live comparison and gaps.
- Modify `tests/functional/run-compare.sh`: stage scripts, register phase, and order cleanup.
- Modify `docs/dev/python-bugzilla-parity.md`: append evidence rows.

## Task 1: Make request credential evidence safe and testable

Interfaces:

- Consume existing `make_handler(backend_port)` and `_HOP_BY_HOP` in
  `tests/functional/redhat-shape-proxy.py`.
- Provide fixture mode `BZR_FUNC_REDHAT_MODE=bearer-auth`.
- Provide log records `auth-kind query count=1`, `auth-kind header count=1`, and
  `auth-kind bearer count=1`; each line describes one request. Later phase code requires one or
  more exact whole-line records, all of the expected kind and none of another kind.

Verification:

- Mode: focused-test — request credential classification and Bearer translation; extend
  `ShapeTests` with query, API-key-header, Bearer, empty-Bearer, and secret-redaction cases; the
  initial `python3 tests/functional/redhat-shape-proxy.py --self-test` must fail because the new
  cases or implementation are absent, and the green command is the same command ending `OK`.

Steps:

1. Add self-tests that run a loopback backend capturing request headers and assert: query keys stay
   in the URL; API-key headers forward unchanged; explicit Bearer mode removes Authorization and
   adds the API-key header; an empty Bearer value is rejected with 400; captured evidence contains
   only the exact kind/count line and not the sentinel credential.
2. Add a request classifier that parses the query with `urllib.parse`, checks headers case-
   insensitively, rejects multiple simultaneous credential kinds in bearer mode, and returns one
   enum-like string or `None` without retaining values.
3. In `_forward`, before opening the backend connection, apply translation only when
   `bearer-auth` is enabled. Reject empty or ambiguous credentials. Remove Authorization before
   adding `X-BUGZILLA-API-KEY`; emit the kind/count record only after the backend responds.
4. Run `python3 tests/functional/redhat-shape-proxy.py --self-test`; expect all tests and `OK`.
5. Commit as `test(functional): observe comparison auth transport`.

Acceptance: no credential value is logged or returned in an error, existing rewrite self-tests
remain green, and Bearer translation is impossible outside its explicit mode.

## Task 2: Run bounded proxies inside the existing sidecar

Interfaces:

- Add `pybz_stage_proxy <source> <destination-name>`; it accepts a repository-owned readable file
  and a basename destination, copies it mode 0600 below `COMPARE_EXCHANGE_DIR`, and returns its
  `/work/compare/<destination-name>` path.
- Add `pybz_proxy_start <tls|redhat> <listen-port> [cert-relative-dir]`; it validates the fixed kind
  and decimal port, rejects a live prior PID, removes stale PID state, atomically creates a new
  mode-0600 evidence log, launches the staged program with backend `127.0.0.1:80`, and proves
  readiness from the sidecar within 30 attempts. It returns the exact current log path.
- Add `pybz_proxy_stop <tls|redhat>`; it validates the PID record and is idempotent.
- Add `pybz_redhat_alias_install`; it idempotently maps `bugzilla.redhat.com` to `127.0.0.1` inside
  the running sidecar.
- Later phase code consumes `PYBZ_TLS_URL=https://127.0.0.1:<port>`,
  `PYBZ_REDHAT_URL=http://bugzilla.redhat.com:<port>`, and the returned per-start log paths.

Verification:

- Mode: focused-test — staging, argv, validation, readiness, alias, PID handling, and cleanup;
  extend `tests/functional/pybz/container-tests.sh` with a fake runtime recording each invocation.
  The red run is `bash tests/functional/pybz/container-tests.sh` failing on the first missing helper;
  the green command is the same script with every fixture ending successfully. Exact assertions
  require host `COMPARE_EXCHANGE_DIR/<name>` to map to container `/work/compare/<name>`.
- Mode: focused-test — TLS material visibility; add a fixture that creates a mode-0600 certificate
  tree beneath `FUNC_CONFIG_DIR`, maps it to `/work`, and proves the generated namespace command
  references only the mapped path. The same fixture command is red before implementation and green
  afterward.

Steps:

1. Write fake-runtime fixtures for valid TLS/Red Hat starts, malformed kind/port/PID rejection,
   readiness exhaustion, alias idempotency, stale-log replacement, live-PID refusal, and repeated
   stop. Assert no runtime call receives a secret sentinel.
2. Implement the four helpers beside existing sidecar lifecycle functions. Use fixed `sh -c`
   programs with data passed as positional parameters; validate before invocation. Never compose
   parameters into shell source.
3. Change TLS fixture directory creation to prefer a template beneath `FUNC_CONFIG_DIR` when it is
   set, retaining `/tmp` for callers without it. Keep certificate/key permissions private.
4. Stage `tls-proxy.py` and `redhat-shape-proxy.py` in `run-compare.sh` before sidecar start and add
   explicit namespace proxy cleanup ahead of `pybz_sidecar_stop`.
5. Run `bash tests/functional/pybz/container-tests.sh`; expect all fixture checks to pass.
6. Commit as `test(functional): run comparison proxies in sidecar`.

Acceptance: no new container is created, no proxy binds beyond namespace loopback, every wait is
bounded, and sidecar removal still cleans up after an early phase exit.

## Task 3: Add private-file python-bugzilla auth operations

Interfaces:

- Extend `python-bugzilla-adapter.py` with `login`, `cached_auth`, `logout`, and
  `client_certificate_surface` operations.
- Preserve ADR 0051's fixed local-proof registry: only `client_certificate_surface` is local in this
  task; login, cached auth, and logout must report real REST or XML-RPC transport.
- Each operation accepts one direct-child path beneath `/work/compare` to a mode-0600 JSON request,
  validates its exact allowed keys and value types, and returns a mode-0600 result containing only
  booleans and bounded non-secret identifiers.
- `login` accepts URL, username, password, and restrict-login; `cached_auth` accepts URL and username
  but no password; `logout` accepts URL and invalidates the cached token; the certificate operation
  accepts a dummy certificate path and reports only whether the constructed session retained it.
- Task 4 consumes these operations through existing `run_pybz_adapter`.

Verification:

- Mode: focused-test — extend adapter fixtures in `container-tests.sh` for restricted login request
  shape, cache reuse with the password file removed, logout invalidation, wrong-mode request files,
  path confinement, and secret-free failure output. The red command is
  `bash tests/functional/pybz/container-tests.sh` failing on the unknown `login` operation; the green
  command is the same script with all adapter cases passing.
- Mode: focused-test — client-certificate surface proof; inject the existing adapter fixture module,
  construct a client from a private dummy path, assert the session receives it, and assert neither
  the path nor contents appear in output. The same fixture command is red on the unknown operation
  and green after implementation.

Steps:

1. Add adapter fixture cases and controlled failures before changing the operation registry.
2. Reuse the existing `/work/compare` direct-child confinement, regular-file and mode-0600 checks,
   exact-key validation, and JSON result path contract; add the bounded auth-specific fields.
3. Implement token lifecycle operations through the pinned python-bugzilla library. Construct a new
   client per operation so `cached_auth` proves persisted cache behavior rather than object reuse.
4. Implement the local certificate operation without issuing a network request and return only
   `{\"configured\": true|false}`.
5. Run `bash tests/functional/pybz/container-tests.sh`; expect all adapter fixtures to pass and all
   secret-sentinel scans to remain empty.
6. Commit as `test(functional): adapt python-bugzilla auth lifecycle`.

Acceptance: passwords and tokens never enter process argv or emitted JSON, logout uses the library
surface absent from the CLI, and certificate evidence is explicitly configuration-only.

## Task 3a: Add the authorized API-key identity operation

Interfaces:

- Extend `python-bugzilla-adapter.py` with network operation `api_key_identity`.
- Accept exactly URL, API key, and username through the existing direct-child, mode-0600 JSON
  request-file boundary; accept none of those values through argv or stdin.
- Construct the pinned REST client at the supplied namespace-proxy URL, perform a real user lookup,
  and return only `authenticated` and `identity_matched` booleans with observed REST transport.
- Do not return URL, username, API key, user fields, or upstream exception text.

Verification:

- Mode: focused-test — extend adapter fixtures in `container-tests.sh` with a fake network backend
  that requires the API key, records the identity lookup, and returns a matching user. Assert exact
  request keys, non-null REST transport, true booleans, and absence of every secret sentinel from
  result and failure output. The red command is
  `bash tests/functional/pybz/container-tests.sh` failing on the unknown `api_key_identity`
  operation; the green command is the same script with all adapter cases passing.
- Mode: focused-test — wrap `jq` and the fake container runtime with argv recorders while assembling
  and invoking the identity request. Require the API-key, URL, and username sentinels to be absent
  from every recorded argument. The same fixture command is red until the request writer uses only
  fixed private source-file paths.

Steps:

1. Add the focused adapter fixture and controlled wrong-mode, extra-key, failed-auth, and
   mismatched-identity cases before changing the operation registry.
2. Add a request writer that creates three mode-0600 source files with the Bash `printf` builtin,
   invokes `jq --rawfile` with only fixed source paths in argv to assemble the mode-0600 request,
   and removes the source files immediately afterward. Do not adapt the existing `jq --arg`
   resource helper for this secret-bearing operation.
3. Reuse the adapter's existing request confinement, permission, exact-key, safe-error, transport,
   and result-file machinery.
4. Construct a new forced-REST client from the private URL and API key, call the pinned library's
   user lookup with the private username, and reduce the result to the two booleans.
5. Run `bash tests/functional/pybz/container-tests.sh`; expect all adapter fixtures, argv capture,
   and secret scans
   to pass.
6. Commit as `test(functional): observe python-bugzilla API-key identity`.

Acceptance: the operation proves a real proxy-observed authenticated lookup, null transport cannot
pass, and no credential, selector, URL, returned user data, or upstream exception is emitted.

## Task 4: Add the R11 comparison phase and parity rows

Interfaces:

- Create phase `06-auth-config-tls` and register it after `05-products-components`.
- Use existing `test_begin`, `test_pass`, `test_fail`, `expect_gap`, `run_bzr`, `run_pybz`, and
  `run_pybz_adapter`.
- Consume Task 1's exact evidence lines, Task 2's URLs/helpers, Task 3's lifecycle operations, and
  Task 3a's `api_key_identity` operation.
- Produce stable IDs for API-key placement, restricted login, cached token, logout, bugzillarc
  precedence/default/section, nosslverify, token transport gap, login-command gap, bugzillarc-import
  gap, client-certificate surface gap, and Bearer gap.

Verification:

- Mode: focused-test — every gap requires its python-bugzilla positive control and exact bzr
  absence; add fake-client phase fixtures in `container-tests.sh`. The red command is
  `bash tests/functional/pybz/container-tests.sh` failing because the new phase is absent; the green
  command is the same script proving positive-control failure stays FAIL and controlled absences
  become the expected issue numbers.
- Mode: focused-test — semantic phase registration and unique IDs; the red command is
  `make check-functional-test-ids` failing when the new file is unregistered, and the green command
  is the same command with no diagnostics.
- Mode: focused-test — parity rows correspond one-to-one with phase IDs; extend the existing parity
  fixture in `container-tests.sh` to require each exact row. The shared fixture command is red before
  the rows and green after them.

Steps:

1. Add phase fixtures for each live contract and five gap owners. Include stale-home and stale-log
   controls, secret-sentinel scans, unsupported-version handling, an omitted-`/etc` precedence
   fault, and a forced positive-control failure.
2. Implement `06-auth-config-tls.sh`: reset owned sidecar state; run API-key observations through
   the private-file `api_key_identity` operation, assembling its request only through Task 3a's
   source-file/`jq --rawfile` writer; prove
   login/restricted/cache/logout via private adapter request files; copy the staged system
   bugzillarc to the sidecar's `/etc`, write both home-path files, and prove their precedence; start
   the namespace TLS proxy and prove default rejection plus `--nosslverify` success; install the Red
   Hat alias, start the Bearer proxy, and prove translated authentication from the current log. For
   each observation, accept the constructor's mandatory authenticated version probe by requiring
   one-or-more homogeneous credential-kind records; reject missing or mixed records.
3. For missing bzr surfaces, use exact parser or transport observations before calling
   `expect_gap` for #676, #681, #682, #677, and #678. Never convert a failed python-bugzilla
   operation or proxy readiness failure into a gap.
4. Run the local client-certificate adapter proof and pair it with bzr's exact parser/config absence;
   label its parity row as a surface gap rather than live mutual-TLS evidence.
5. Register the phase in `run-compare.sh` and append the matching rows to
   `docs/dev/python-bugzilla-parity.md`.
6. Run `bash tests/functional/pybz/container-tests.sh`; expect all fixture checks to pass.
7. Run `make check-functional-test-ids`; expect exit 0 with both phase trees valid.
8. Commit as `test(functional): compare auth config and TLS`.

Acceptance: every sourced R11 situation has a stable live test, each known bzr absence points to its
own open issue, and no row or expected gap can pass without its positive control.

## Task 5: Verify the complete change

Interfaces:

- Consume all earlier tasks; provide no new code interface.

Verification:

- Mode: focused-test — complete comparison across supported servers; `make functional-compare-all`
  must exit 0 and report bz50, bz52, and bz53 with no ordinary failures.
- Mode: focused-test — existing real-container regression suite; `make functional-test-all` must
  exit 0 for all three versions.

Steps:

1. Run `make lint`; expect exit 0 with rustfmt, clippy, build-script, test-layout, functional-ID,
   no-spawn, release-note, ShellCheck, Bash syntax, and shfmt checks green.
2. Run `make test`; expect exit 0 and all Rust/integration suites green.
3. Run `make functional-compare-all`; expect all three versions green with only declared gaps.
4. Run `make functional-test-all`; expect all three versions green and all containers cleaned up.
5. Re-read `git diff origin/main...HEAD`, verify no production `src/` or unrelated surface changed,
   and commit any verification-only correction separately after its focused proof.

Acceptance: all four guardrails are green at HEAD and functional output contains no secret
sentinels.
