# 0005 — `server capabilities` reports the anonymously-derivable surface

- Status: Accepted
- Date: 2026-06-26
- Issue: #457

## Context

Issue #457 adds `bzr server capabilities`, a structured dump of what a Bugzilla
instance supports, so an agent can self-configure instead of probing by
trial-and-error. The issue sketches a JSON shape that mixes fields with very
different availability:

- Reliably served anonymously by a stock Bugzilla 5.x: `version`, status
  transitions, custom-field definitions (all via `/rest/version` and
  `/rest/field/bug`).
- Not served anonymously: `max_attachment_size` lives in `/rest/parameters`,
  which is permission-gated; there is no global flag-type REST endpoint at all
  (flag types are per-product).
- Ambiguous: the sketch's `auth_modes: ["api_key","env","keyring"]` are bzr's
  *local* credential-storage kinds (`CredentialSourceKind`), which describe client
  config, not the server — yet criterion 2 requires the command to work under
  `--server-url` with no saved config, where no local credential exists.

The command is a published `--json` contract with a checked-in schema, so the
field semantics are a committed interface, not an implementation detail.

## Decision

`server capabilities` reports **what is derivable from a stock Bugzilla server
served anonymously**, and marks anything else explicitly absent rather than
omitting it or erroring.

1. **`auth_modes` describes the server, not the client.** It lists the auth
   mechanisms the *server* accepts. Bugzilla ≥ 5.0 REST accepts API-key auth
   (header or query param, both modeled by `AuthMethod`), reported as the single
   capability `["api_key"]`; a pre-5.0 XML-RPC-only server reports `[]`. The value
   is independent of local credential configuration, so it is identical with or
   without a saved key.

2. **Unavailable fields are published as nullable, not dropped.**
   `max_attachment_size` is reported **in bytes** (Bugzilla's `maxattachmentsize`
   is kilobytes; bzr normalizes by `* 1024` so the unit is unambiguous). It is
   fetched best-effort from `/rest/parameters`, **only when a credential is
   present** — that parameter is absent from the anonymous whitelist, so a
   credentialless probe would always waste a round-trip and still yield `null`.
   `flag_types` is `null` in this version because no anonymous server-wide data
   path exists. Both keys stay in the schema (`["...","null"]`) so the contract is
   forward-compatible; `null` means **undetermined**, not "absent/no limit".

3. **Derived fields are derived, not probed, and labelled as such.** `api_modes`
   and the `supports_*` booleans are computed from the already-detected
   `ApiMode`/version, adding no requests. The `supports_*` booleans are
   **transport-capability** signals — "the server's REST surface exposes this
   endpoint" — not "this feature is configured/populated". Consequently
   `supports_flag_requests: true` (the flag-update endpoint exists) coexists with
   `flag_types: null` (bzr has not determined the types); an agent reads the pair
   as "flag requests are accepted; discover types via product detail", not as a
   contradiction. `status_transitions` and `custom_fields` reuse the existing
   `/rest/field/bug` data path.

4. **Two failure classes.** `version` is required (its failure fails the command —
   nothing is knowable). `status_transitions` and `custom_fields` surface their
   errors (a server that cannot answer `/rest/field/bug` is not stock; the agent
   should see that). Only `max_attachment_size` swallows errors into `null`. A
   `NotFound` status field degrades to an empty transition list, since "no
   transitions" is a representable state.

## Consequences

- The command satisfies criterion 2 (works under `--server-url`, no key) honestly:
  every always-populated field is anonymous-derivable, and the rest are visibly
  `null`.
- `flag_types` ships as a permanent `null` until a follow-up adds a per-product
  path. The key's presence is the only forward-compatibility cost; agents that
  need flag types still must fall back to product detail today.
- "Degrade gracefully" is scoped to *availability*, not *correctness*: only the
  optional field nulls on error. This avoids returning a misleadingly-empty
  document for a genuinely broken server.
- `auth_modes` is a coarse, version-derived signal (`["api_key"]` / `[]`) rather
  than a probe of every accepted mechanism (Basic, login token, etc.), which bzr
  cannot determine anonymously without attempting auth.

## Considered & rejected

- **`auth_modes` = configured `CredentialSourceKind` (inline/env/keyring).**
  Rejected: describes the client, not the server, and is empty in the
  `--server-url` flow the issue requires to work — the opposite of a server
  capability.
- **Drop `max_attachment_size` and `flag_types` from the v1 schema.** Rejected:
  evolving a published schema by *adding required-shape keys later* is a breaking
  change for validators; publishing them nullable now keeps the contract stable.
- **Make the whole document depend on auth state (credentialed flag-type and
  parameter enrichment).** Rejected for v1: the always-populated fields (`version`,
  `api_modes`, `auth_modes`, `status_transitions`, `custom_fields`, `supports_*`)
  must be identical with or without a key so the anonymous acceptance test is
  meaningful. The single deliberate exception is `max_attachment_size`: it is
  *only* fetched when a credential is present (the parameter is not in the anonymous
  whitelist, so an unauthenticated fetch is pure waste) and is `null` otherwise.
  This keeps the credentialless document fully determined while letting an
  authenticated caller learn one extra fact. Broader authenticated enrichment
  (per-product flag types) is left to a follow-up.
- **Swallow all fetch errors into nulls for maximum robustness.** Rejected: hides
  real breakage (e.g. `/rest/field/bug` failing) behind an empty-looking but
  valid document, which is worse for an agent than a surfaced error.
