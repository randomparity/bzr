# Red-Team Engagement — Surface 1: TLS / Auth

**Date:** 2026-06-04
**Branch:** `security/red-team-tls-auth`
**Status:** approved design

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
  1. The pin / issuer trust decision (accept vs reject a certificate).
  2. The fidelity of the fingerprint / issuer text shown in the
     trust-on-first-use "certificate changed — re-trust?" prompt, which drives
     a human security decision.
  3. Stored credentials (API key) and the confinement of which host they are
     transmitted to.
  4. Process liveness — no panic or unbounded work when parsing crafted
     certificate bytes.

## Invariant catalog

The tests assert these. Each is written failing-first.

- **INV-1 — pin integrity.** A leaf certificate whose SHA-256 differs from the
  pinned value is *always* rejected by `PinnedCertVerifier::verify_server_cert`,
  for any certificate bytes. No crafted issuer, DN, or DER length encoding
  causes an `Ok(ServerCertVerified)` return.
- **INV-2 — prompt fidelity.** The `expected` / `got` / `issuer` values surfaced
  by `tls::pin_failure::classify` round-trip faithfully from what the verifier
  observed. An attacker-controlled issuer DN cannot alter the displayed expected
  pin, swap the displayed fingerprints, or mask that a mismatch occurred.
  *(Primary suspected defect: the verifier formats these fields into a single
  error string using `, got ` / `, issuer ` delimiters, and `pin_failure.rs`
  recovers them by naive substring search. The issuer DN is attacker-controlled
  via `extract_issuer_dn`, so it can contain those delimiters.)*
- **INV-3 — parser totality.** `extract_issuer_der`, `extract_issuer_dn`, and
  the DER walkers (`parse_der_sequence`, `skip_der_element`, `parse_der_length`,
  `extract_rdns`, `parse_attribute_type_and_value`) never panic and always
  terminate on arbitrary byte input.
- **INV-4 — issuer-change soundness.** When an issuer DER is pinned, a differing
  issuer is never silently accepted. A parser failure on the leaf degrades to
  *reject* (never *accept*).
- **INV-5 — auth confinement.** Credentials / auth headers are attached only to
  the configured host. The cert-detection probe path sends no auth header and
  does not follow redirects. (Largely already coded; tests assert it stays
  true and that the real client's redirect-following cannot leak the API key
  off-host.)

## Method

- **Property tests** (`proptest`) for INV-1, INV-3, INV-4 — generate arbitrary
  and structured DER and assert totality plus reject-bias.
- **Adversarial unit tests** for INV-2 and INV-5 — hand-craft a certificate /
  issuer DN containing the classifier's own delimiters and assert the prompt
  cannot be spoofed; assert auth headers stay host-confined.
- **Fuzz target** for INV-3 added under the existing `fuzz.yml` harness.
- Every test is written failing-first against current code. A test that
  reproduces a real break drives a TDD fix (red → green); the fix diff carries
  that test.

## Orchestration

A multi-agent workflow (user opted in). A `pipeline` per invariant:

1. **Finder** agent — writes the adversarial / property test, runs it against
   current code, reports whether it reproduced a break. Runs in a git worktree
   so parallel test-writing does not collide.
2. **Adversarial verifier** agent — independently confirms each reported break
   is a real defect and not a test artifact, defaulting to skeptical.
3. **Synthesis** — the main session collects confirmed breaks.

Confirmed breaks are fixed via TDD locally, one labeled PR per fixed defect.

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
