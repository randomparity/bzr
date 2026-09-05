# Auth, configuration, and TLS comparison implementation plan

Goal: Add issue #669's R11 comparison evidence without changing production bzr behavior.

Architecture: bzr retains the existing host-side fixtures. The long-lived python-bugzilla sidecar
starts matching proxy processes on its shared Bugzilla loopback, stages inputs through `/work`, and
emits secret-free request-kind evidence. One new ordered phase owns the live comparisons and gap
markers.

Tech stack: Bash functional harness, Python 3 stdlib proxy fixtures, Docker/Podman, jq, openssl,
python-bugzilla 3.3.0, and Markdown parity documentation.

Expected implementation size: 320–520 changed lines (L) — derived from two proxy/helper fixture
surfaces, one multi-contract comparison phase, runner wiring, and parity rows.

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
  `auth-kind bearer count=1`; later phase code relies on exact whole lines.

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
  `/work/<destination-name>` path.
- Add `pybz_proxy_start <tls|redhat> <listen-port> [cert-relative-dir]`; it validates the fixed kind
  and decimal port, launches the staged program with backend `127.0.0.1:80`, writes a fixed PID/log
  under `/work`, and proves readiness from the sidecar within 30 attempts.
- Add `pybz_proxy_stop <tls|redhat>`; it validates the PID record and is idempotent.
- Add `pybz_redhat_alias_install`; it idempotently maps `bugzilla.redhat.com` to `127.0.0.1` inside
  the running sidecar.
- Later phase code consumes `PYBZ_TLS_URL=https://127.0.0.1:<port>`,
  `PYBZ_REDHAT_URL=http://bugzilla.redhat.com:<port>`, and fixed log paths.

Verification:

- Mode: focused-test — staging, argv, validation, readiness, alias, PID handling, and cleanup;
  extend `tests/functional/pybz/container-tests.sh` with a fake runtime recording each invocation.
  The red run is `bash tests/functional/pybz/container-tests.sh` failing on the first missing helper;
  the green command is the same script with every fixture ending successfully.
- Mode: focused-test — TLS material visibility; add a fixture that creates a mode-0600 certificate
  tree beneath `FUNC_CONFIG_DIR`, maps it to `/work`, and proves the generated namespace command
  references only the mapped path. The same fixture command is red before implementation and green
  afterward.

Steps:

1. Write fake-runtime fixtures for valid TLS/Red Hat starts, malformed kind/port/PID rejection,
   readiness exhaustion, alias idempotency, and repeated stop. Assert no runtime call receives a
   secret sentinel.
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

## Task 3: Add the R11 comparison phase and parity rows

Interfaces:

- Create phase `06-auth-config-tls` and register it after `05-products-components`.
- Use existing `test_begin`, `test_pass`, `test_fail`, `expect_gap`, `run_bzr`, and `run_pybz`.
- Consume Task 1's exact evidence lines and Task 2's URLs/helpers.
- Produce stable IDs for API-key placement, restricted login, cached token, logout, bugzillarc
  precedence/default/section, nosslverify, token transport gap, login-command gap, bugzillarc-import
  gap, client-certificate gap, and Bearer gap.

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

1. Add phase fixtures for each live contract and five gap owners. Include stale-home controls,
   secret-sentinel scans, unsupported-version handling, and a forced positive-control failure.
2. Implement `06-auth-config-tls.sh`: reset owned sidecar state; run API-key observations; prove
   login/restricted/cache/logout; write and select bugzillarc fixtures; start the namespace TLS
   proxy and prove default rejection plus `--nosslverify` success; install the Red Hat alias, start
   the Bearer proxy, and prove translated authentication.
3. For missing bzr surfaces, use exact parser or transport observations before calling
   `expect_gap` for #676, #681, #682, #677, and #678. Never convert a failed python-bugzilla
   operation or proxy readiness failure into a gap.
4. Register the phase in `run-compare.sh` and append the matching rows to
   `docs/dev/python-bugzilla-parity.md`.
5. Run `bash tests/functional/pybz/container-tests.sh`; expect all fixture checks to pass.
6. Run `make check-functional-test-ids`; expect exit 0 with both phase trees valid.
7. Commit as `test(functional): compare auth config and TLS`.

Acceptance: every sourced R11 situation has a stable live test, each known bzr absence points to its
own open issue, and no row or expected gap can pass without its positive control.

## Task 4: Verify the complete change

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
