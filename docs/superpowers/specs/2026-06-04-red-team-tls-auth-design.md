# Red-Team Engagement — Surface 1: TLS / Auth

**Date:** 2026-06-04
**Branch:** `security/red-team-tls-auth`
**Status:** approved design (revised after hostile `/challenge` review — see Method)

## Goal

Act as an adversary against the TLS and authentication surface of `bzr`.
Enumerate the invariants the trust model relies on, write property-based and
adversarial tests that try to break each one, fix every reproduced break via
TDD (failing-then-passing test), and ship one labeled remediation PR per fixed
defect with the threat model documented.

This is the first of three surfaces. Concurrency and supply-chain follow as
separate engagements after a checkpoint.

## Threat model

- **Adversary:** an active network MITM, or a malicious/compromised Bugzilla
  server, that controls (a) the TLS certificate chain presented to `bzr` and
  (b) the HTTP / XML-RPC response bodies.
- **Trust boundary:** every byte from the presented certificate and the
  response body inward is untrusted. The on-disk TOFU pin store and the config
  file are the trusted state the adversary wants to corrupt, bypass, or
  misrepresent to the user.
- **Assets under attack:**
  1. The pin / issuer trust decision (accept vs reject a certificate), including
     the confinement of the accept-any first-contact path so it never validates
     real API traffic.
  2. The integrity of the **persisted pin**: the fingerprint/issuer that
     `pin_failure::classify` recovers from a `PIN_MISMATCH`/`ISSUER_CHANGED`
     error is written to the config and becomes the verifier for the *next*
     connection (`src/commands/shared.rs:185-220`). The asset is that persisted
     value, not merely the text shown in the prompt — corrupting it would pin a
     wrong certificate going forward.
  3. Stored credentials (API key) and the confinement of which host they are
     transmitted to — including across HTTP redirects followed by the real API
     client.
  4. Process liveness — no panic or unbounded work when parsing crafted
     certificate bytes.

## Invariant catalog

The tests assert these. Each is written failing-first. Priority reflects the
hostile `/challenge` review and the empirical pre-checks recorded in Method:
the *real* sinks (INV-5, INV-2) lead; the defensively-coded paths (INV-0, INV-1,
INV-3, INV-4) land as regression/fuzz guards.

- **INV-0 — trust-establishment confinement.** The accept-any `CertCapture`
  verifier (which returns `Ok(ServerCertVerified)` for any certificate) is
  reachable *only* from `probe_server_cert`, never from the client that carries
  API traffic; and a default `TlsConfig` (no `insecure`/`ca_cert_path`/
  `pin_sha256`) yields full system-root CA validation. `CertCapture` is private
  to `tofu.rs` and `probe_server_cert` builds its own redirect-disabled client.
  Anchors: `src/tls/tofu.rs:14-60,69-107`, `src/tls/mod.rs:48-66`,
  `src/commands/shared.rs:35-50,127-169`. *Expected: holds — lands as a
  regression guard.*
- **INV-1 — accept-decision branches.** Drop the original proptest framing (it
  passes vacuously: random bytes never collide with the 32-byte pin preimage,
  `src/tls/verifier.rs:111-113`). Replace with targeted unit tests on
  `verify_server_cert`: a matching pin returns `Ok`; a one-bit-flipped pin
  returns `Err`; `check_issuer_change` runs *only* after a hash mismatch
  (short-circuit ordering — the matching-pin branch returns before it). Anchor:
  `src/tls/verifier.rs:100-123`. *Expected: holds — regression guard.*
- **INV-2 — pin-rotation fidelity & persisted-pin integrity.** Re-pointed from
  "prompt text spoofing" to the real sink: the `expected`/`actual`/`issuer`
  values `pin_failure::classify` recovers become the **persisted pin** and the
  next connection's verifier (`src/commands/shared.rs:185-220`). Assert the
  persisted pin always equals the genuine `compute_fingerprint` of the presented
  cert and cannot be corrupted by attacker-controlled issuer text. *Pre-check
  refuted exploitability (see Method) — the test lands as a fidelity/regression
  guard, not a reproduced break.* Anchors: `src/tls/verifier.rs:117-122`,
  `src/tls/pin_failure.rs:40-50`.
- **INV-3 — parser totality (guard).** Overstated as a prime suspect in the
  original draft: the DER walkers already use checked arithmetic and bounded
  length parsing (`src/tls/verifier.rs:304-319`). Keep a `proptest` totality
  property plus a `cargo-fuzz` target as cheap regression guards. Anchors:
  `src/tls/verifier.rs:216-399`. *Expected: holds — fuzz/property guard.*
- **INV-4 — issuer-change soundness.** Unchanged. When an issuer DER is pinned,
  a differing issuer is never silently accepted; a parser failure on the leaf
  degrades to *reject* (never *accept*). Anchor: `src/tls/verifier.rs:81-115`.
- **INV-5 — credential confinement across redirects.** **Primary suspect —
  pre-check confirmed a real leak (see Method).** The real API client
  (`build_tls_client`) follows redirects per reqwest defaults and attaches the
  API key as the custom header `X-BUGZILLA-API-KEY`. reqwest strips only
  known-sensitive headers (`Authorization`, `Cookie`, …) on a cross-host
  redirect, not custom ones, so the credential is forwarded to a redirect-target
  host. Assert the API key (both `Header` and `QueryParam` auth methods) is never
  sent to a host other than the configured one, including across HTTP redirects.
  Anchors: `src/tls/mod.rs:69-77`, `src/http.rs:24-59`,
  `src/client/mod.rs:277-286`.

## Method

- **Empirical pre-checks (run 2026-06-04, recorded here, throwaway tests
  reverted).** Two crate-internal throwaway tests established ground truth
  before committing the workflow:
  - *INV-2 probe.* Crafted a `PIN_MISMATCH` error chain whose attacker-controlled
    issuer field contained the classifier's own `, got ` and `, issuer `
    delimiters, then ran `pin_failure::classify_chain`. Result:
    `expected_corrupted=false`, `actual_corrupted=false` — the injected
    delimiters landed entirely inside `new_issuer`; `expected`/`actual` kept
    their genuine values. The parser anchors on the earlier (genuine) delimiters
    first, so the attacker cannot move them. **Refutes exploitability → INV-2
    demoted to a fidelity guard.**
  - *INV-5 probe.* With `wiremock`, host A (`127.0.0.1`) returned `301` to host B
    (`localhost`, a genuinely different `host_str`). A client built by
    `build_tls_client` sent the request with both `X-BUGZILLA-API-KEY` and
    `Authorization` set. Result on host B: `X-BUGZILLA-API-KEY = Some("…")`
    (**leaked**) while `Authorization = None` (reqwest stripped the
    known-sensitive header on the cross-host hop, but not the custom one). The
    query-param variant did **not** leak — the `301` `Location` URL replaces the
    query string, so `Bugzilla_api_key` was absent on host B. **Confirms a real
    defect, specific to header auth → INV-5 elevated to primary suspect.**
- **Targeted unit tests** for INV-0, INV-1, INV-4 — assert confinement,
  accept/reject branches with short-circuit ordering, and reject-bias on parser
  failure.
- **Property test** (`proptest`) + **fuzz target** for INV-3 — generate arbitrary
  and structured DER and assert totality.
- **Adversarial / regression tests** for INV-2 and INV-5 — the carried versions
  of the two pre-checks above, hardened into permanent guards.
- Every test is written failing-first against current code. A test that
  reproduces a real break drives a TDD fix (red → green); the fix diff carries
  that test. Per the pre-checks, only INV-5 is expected to reproduce a break.

## Orchestration

A multi-agent workflow (user opted in). A `pipeline` per invariant, led in
priority order so the real sinks are worked first:

**INV-5 → INV-2 → INV-0 → INV-1 → INV-4 → INV-3 (fuzz guard).**

1. **Finder** agent — writes the adversarial / property / regression test, runs
   it against current code, reports whether it reproduced a break. Runs in a git
   worktree so parallel test-writing does not collide.
2. **Adversarial verifier** agent — independently confirms each reported break
   is a real defect and not a test artifact, defaulting to skeptical.
3. **Synthesis** — the main session collects confirmed breaks.

Confirmed breaks are fixed via TDD locally, one labeled PR per fixed defect; the
user confirms before each push.

## Open implementation decisions (surface during the fix)

- **INV-5 fix shape** — decide with the user during TDD once the failing test is
  in hand. Candidates:
  (a) attach a reqwest `redirect::Policy` that strips auth / aborts on host
      change;
  (b) disallow off-host redirects on the API client entirely (mirror the probe's
      `Policy::none()`, or a host-allowlist policy);
  (c) bind auth attachment to the configured host at send time.
  Recommended default: (b) — the simplest fail-closed behavior, and consistent
  with the probe client already using `Policy::none()`.

## Delivery

- New tests land as permanent regression guards even where the invariant holds.
- One branch + PR per *fixed* defect, labeled `security` and `red-team`, with
  the threat model in the body. Each PR is confirmed with the user before push.
- A checkpoint with the user before moving on to the concurrency surface.

## Out of scope (this engagement)

- Concurrency surface (OnceLock cert capture, keyring init races, parallel
  fetches) — separate engagement.
- Supply-chain surface (`deny.toml`, lockfile, CI provenance, XML-RPC parsing
  as malicious input) — separate engagement.
- Cryptographic review of rustls / ring themselves — out of scope; we trust the
  vetted crypto primitives and attack only `bzr`'s use of them.
