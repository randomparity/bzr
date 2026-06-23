# TLS Functional Coverage for Ad-hoc `--server-url` Trust Flags — Design

**Date:** 2026-06-23
**Status:** Proposed
**Issue:** #406 (follows #381, parent #379)
**Scope:** `tests/functional/` — adds an HTTPS fixture and end-to-end coverage for
the stateless `--server-tls-*` trust controls. No production (`src/`) changes.

## Context

#381 added ad-hoc TLS trust controls to stateless `--server-url` invocations:
`--server-tls-insecure`, `--server-tls-ca-cert <PATH>`, `--server-tls-pin-sha256
<PIN>`, and `--server-tls-pin-now`. They carry unit/integration coverage
(`src/cli/mod_tests.rs`, `tests/integration.rs`) and clap-level argument-validation
coverage in the functional suite (`tests/functional/phases/17b-arg-validation.sh`
tests 125b/125c: require `--server-url`, mutual exclusion).

What is missing is end-to-end exercise against a **real HTTPS endpoint**. The
Bugzilla functional containers expose HTTP only (`setup-bugzilla.sh` publishes
container port 80), so there is no path today that drives these flags through a
genuine TLS handshake to a real Bugzilla server. The parent functional-coverage
effort (`docs/superpowers/specs/2026-06-20-functional-test-coverage-expansion-design.md`)
explicitly deferred this to #406 to avoid coupling TLS-fixture complexity to that work.

## Goal

Exercise each ad-hoc TLS trust mode through a real TLS handshake to a real Bugzilla
server, asserting the observable CLI outcome (exit code + output), while:

- not requiring host TLS tooling that may be unavailable (clean skip path), and
- not destabilizing `run-all-versions.sh` (the scheduled `functional-tests.yml` run).

## Empirically-established behavior

All exit codes below were verified locally by running the release `bzr` binary
through a TLS-terminating reverse proxy in front of a stub HTTP backend serving
`/rest/version` and `/rest/extensions` (the two endpoints `server info` calls):

| Mode | Invocation | Observed exit | Outcome |
|---|---|---|---|
| insecure | `--server-tls-insecure` | 0 | self-signed cert accepted |
| custom CA | `--server-tls-ca-cert ca.pem` | 0 | leaf verified against provided CA + IP SAN |
| pin (correct) | `--server-tls-pin-sha256 <good>` | 0 | presented cert matches pin |
| pin (wrong) | `--server-tls-pin-sha256 <wrong>` | **5** | connection rejected (`type:"http"`) |
| pin-now | `--server-tls-pin-now` | 0 | TOFU-pins presented cert for the process |

### Wrong-pin exit code (deviation from the issue text)

Issue #406 states a wrong pin should be "rejected with exit 13." The current
implementation rejects it with **exit 5**: a pin mismatch surfaces as a transport
error, version detection then falls back to XML-RPC over the same (still
wrong-pinned) connection, that handshake fails too, and the final error is
`BzrError::Http` → exit 5. The `PinMismatch` variant (exit 13) is only ever produced
for `ISSUER_CHANGED`, never for a plain wrong pin
(`src/commands/runtime/shared/connection.rs::classify_and_handle_tls_failure`,
non-interactive `handle_pin_rotation` returns a `Config` error; the version-detection
fallback masks it earlier still). The issue was AI-generated during triage and its
"exit 13" line is an unverified assumption.

**Decision (confirmed with maintainer):** the functional test asserts the *actual*
contract — a wrong pin is **rejected** (non-zero exit, no successful `server info`
payload) — without hard-coding `13`. The exit-5 / masked-reason behavior is recorded
here and flagged in the PR as a candidate follow-up (improve pin-mismatch
diagnostics / exit code), out of scope for this test-only issue.

## Fixture design

A small **TLS-terminating reverse proxy** runs on the host in front of the already
running HTTP Bugzilla container:

```
bzr  --(HTTPS, https://127.0.0.1:$TLS_PORT)-->  tls-proxy  --(HTTP, 127.0.0.1:$BZ_PORT)-->  Bugzilla container
```

- **Proxy:** `python3` (stdlib `ssl` + `http.server` + `http.client`), built on
  `ThreadingHTTPServer` so kept-alive / pooled / concurrent connections from reqwest
  cannot deadlock a single-threaded handler. Forwards method, path+query, headers
  (minus hop-by-hop), and body verbatim to the HTTP backend and streams the response
  back with a fixed `Content-Length`. ~40 lines, no third-party packages.
- **Certs:** generated with `openssl` — a self-signed CA and a leaf signed by it
  carrying `subjectAltName = IP:127.0.0.1, DNS:localhost`,
  `extendedKeyUsage = serverAuth`, `basicConstraints = CA:FALSE`. The IP SAN lets
  `--server-tls-ca-cert` pass full hostname verification when connecting to
  `https://127.0.0.1:$TLS_PORT`.
- **Pin:** computed as `sha256//` + base64(SHA-256 of the **leaf cert DER**), matching
  `src/tls/fingerprint.rs::compute_fingerprint`. The wrong pin is a syntactically
  valid but non-matching 32-byte value.

### Why python3, not socat / a proxy container

| Option | Rejected because |
|---|---|
| `socat OPENSSL-LISTEN` | Not installable/verifiable in the dev environment; adds a system dependency to CI and the prereq list; one extra moving part with TLS-version/cipher edge cases I cannot test before the nightly run. |
| Reverse-proxy container (nginx/caddy on a shared network) | Container-to-host-published-port networking (`host.containers.internal` / `--network host`) differs across podman/docker and Linux/macOS; adds an image build or pull; more fragile than a host-side terminator. |
| Modify Bugzilla images to serve HTTPS | The parent spec and #406 both reject changing the Bugzilla images; couples TLS to the container build. |
| **python3 TLS reverse proxy (chosen)** | `python3` is already a de-facto suite dependency (`setup-bugzilla.sh` uses `python3 -m json.tool`); `openssl` is ubiquitous; the full proxy→backend→`bzr` path is verifiable locally without containers; runtime-agnostic. |

## Capability detection & skip behavior

The phase probes for `python3` and `openssl` at start. If either is missing, every
TLS case is `test_skip`-ped with a clear reason and the phase returns cleanly — so
`run-all-versions.sh` stays predictable and never hard-fails for missing host TLS
tooling. `functional-tests.yml` already has `python3` and `openssl` on
`ubuntu-latest`, so the scheduled run exercises the cases rather than skipping them
(no CI dependency to add).

The fixture is **default-safe**: it runs automatically when the tooling is present
and skips cleanly otherwise. No opt-in flag is required.

## Test matrix (one phase, `02c-tls-inline.sh`)

Runs against the single already-running container for the active `BZR_BZ_VERSION`
(satisfies "at least one supported Bugzilla container version"). All cases use the
public, read-only `server info`, so no credentials are needed.

1. **insecure** — `--server-tls-insecure server info` → exit 0, `.version` present.
2. **custom CA** — `--server-tls-ca-cert <ca.pem> server info` → exit 0, `.version`.
3. **pin correct** — `--server-tls-pin-sha256 <good> server info` → exit 0, `.version`.
4. **pin wrong** — `--server-tls-pin-sha256 <wrong> server info` → rejected:
   non-zero exit, no `.version` payload. (Documents actual exit 5; assertion does
   not hard-code the number.)
5. **pin-now** — `--server-tls-pin-now server info` → exit 0, `.version`.
6. **no config write** — snapshot the isolated `config.toml` (or its absence) before
   the TLS cases and assert byte-identical afterward, proving none of the ad-hoc TLS
   options persist state.

Mutual-exclusion and require-`--server-url` validation already live in
`17b-arg-validation.sh` (tests 125b/125c) and are **not** duplicated here; the phase
references them in a comment.

### Determinism notes

- **Readiness, not a fixed sleep.** After backgrounding the proxy, the phase polls
  `curl -sk https://127.0.0.1:$TLS_PORT/rest/version` in a bounded retry loop
  (mirroring `setup-bugzilla.sh::wait_for_ready`, ~30 × 1s). Only once the proxy
  completes a TLS handshake and returns do the cases run. If it never comes up, the
  phase fails fast with a diagnostic (proxy stderr tail) rather than hanging or
  flaking on connection-refused — this is what keeps the scheduled run predictable.
- All `bzr` invocations redirect stdin from `/dev/null` so the wrong-pin /
  TOFU-rotation prompts never block on a TTY when the suite is run interactively.
- The proxy is started before the cases and torn down at end of phase. Cleanup is
  also registered on the runner's `EXIT` trap, composed as `trap 'cleanup;
  _tls_cleanup' EXIT`. `_tls_cleanup` is **idempotent and `set -u`-safe**: it guards
  every variable with `${VAR:-}`, tolerates an absent/already-dead proxy PID and an
  already-removed temp dir, so it is harmless whether the phase skipped before
  assigning those vars, ran to completion (where it also tears down inline), or
  exited early — and whether it fires once or twice.
- The TLS port defaults to `$BZ_PORT + 1000` (overridable via `BZR_FUNC_TLS_PORT`)
  to avoid colliding with the published Bugzilla port.

## Acceptance criteria mapping

| #406 criterion | Satisfied by |
|---|---|
| Opt-in or default-safe HTTPS fixture path | Default-safe python3+openssl proxy with capability skip |
| Runs against ≥1 supported Bugzilla version | Phase runs against the active container |
| Fixture documented in `tests/functional/README.md` | README section: mechanism, env vars, skip path |
| `run-all-versions.sh` stays predictable, no unavailable host TLS tooling without a skip/setup path | Capability probe → clean `test_skip`; python3/openssl only |
| insecure accepts invalid cert | Case 1 |
| custom CA trusts provided CA | Case 2 |
| pin accepts correct, rejects wrong | Cases 3 & 4 (wrong → rejected; exit-code deviation documented) |
| pin-now pins for the process without persisting | Cases 5 & 6 |
| TLS options mutually exclusive, require `--server-url` | Existing 125b/125c (referenced) |
| No ad-hoc TLS option writes config | Case 6 |

## Out of scope

- Changing the wrong-pin exit code / diagnostics in `src/` (candidate follow-up).
- HTTPS coverage for authenticated/write commands (public `server info` suffices).
- Persisted named-server TLS flows (covered elsewhere; this is the ad-hoc path).
