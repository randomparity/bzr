# Password-token transport implementation plan

Goal: implement persisted Bugzilla login-token resolution and REST request
transport for #676. The credential boundary will carry an explicit kind so
API-key detection/XML-RPC behavior remains unchanged while tokens are REST-only.

Tech stack: Rust 1.89.0, serde TOML configuration, reqwest, wiremock, shell
functional comparison tests.

## Global Constraints

- Rust toolchain: 1.89.0.
- No dependency is added.
- Preserve the current-thread Tokio runtime and sibling `*_tests.rs` layout.
- Do not add token lifecycle commands, bugzillarc import, Bearer transport,
  client certificates, or comparison-harness rework.
- API keys remain compatible; token secrets never appear in diagnostics.

Expected implementation size: 220–250 changed lines (M) — derived from the
credential/config boundary, client transport selection, focused tests, and one
existing comparison phase/parity row.

## File map

- `src/config/model.rs`, `src/config/mod_tests.rs`: persisted token source and
  exclusivity contract.
- `src/credentials/mod.rs`, `src/credentials/mod_tests.rs`: typed resolution.
- `src/bugzilla_auth.rs`, `src/bugzilla_auth_tests.rs`: token marker and
  credential-neutral redaction.
- `src/client/{mod,transport}.rs`, `src/client/transport_tests.rs`: prepared
  REST token auth, no API-key retry, and XML-RPC/hybrid rejection.
- `src/commands/runtime/shared/connection/{target,mod,detect}.rs` and sibling
  tests: token-aware connection construction and detection-cache behavior.
- `src/output/resources/config.rs`, `src/commands/config/{keyring,migrate}.rs`
  and sibling tests: token-safe display and existing API-key keyring behavior.
- `tests/functional/compare/06-auth-config-tls.sh`,
  `docs/dev/python-bugzilla-parity.md`, `docs/bzr-cli.md`: parity proof and
  supported configuration documentation.

## Task 1 — Model credential kind and redaction

**Interfaces:** `ServerConfig::credential_source()` returns one typed source;
the resolver returns a credential value plus its API-key/token kind; auth
application accepts a prepared typed credential.

**Verification:**

- Mode: focused-test. Contract: a token is valid alone and conflicts with all
  API-key sources. Red: add the model test before implementation; green:
  `make test-one T=credential_source`.
- Mode: focused-test. Contract: `Bugzilla_token` and a bare active token are
  redacted and preview boundaries do not split them. Red: add tests first;
  green: `make test-one T=redact`.

1. Add `token` and a distinct token credential variant in `src/config/model.rs`.
2. Update `src/credentials/mod.rs` to resolve and register either credential
   kind without exposing its value in errors.
3. Generalize `src/bugzilla_auth.rs` marker/bare-secret redaction and add the
   `Bugzilla_token` constant.
4. Update config display to mask/label tokens, reject token migration to the
   API-key keyring, and ensure API-key keyring replacement clears a token.
5. Commit the focused model, resolver, redaction, display, and keyring contract.

## Task 2 — Select REST token transport at the connection boundary

**Interfaces:** `ConnectContext` carries a typed resolved credential;
`BugzillaClientConfig` builds either detected API-key auth or fixed token query
auth; token configurations force default `ApiMode::Rest`, reject explicit
XML-RPC/hybrid overrides, and never take API-key alternate auth.

**Verification:**

- Mode: focused-test. Contract: a token produces only
  `Bugzilla_token=<value>` on REST requests and no API-key header/parameter.
  Red: add a transport test first; green: `make test-one T=token`.
- Mode: focused-test. Contract: token connections neither probe nor persist
  API-key auth-method state, force REST despite a cached/detected Hybrid mode,
  and reject explicit XML-RPC/hybrid selection before wire dispatch. Red: add
  connection tests first; green:
  `make test-one T=token`.
- Mode: focused-test. Contract: a token 401 produces no alternate request with
  API-key header or `Bugzilla_api_key`. Red: add the transport test first;
  green: `make test-one T=token`.

1. Thread the typed credential through target resolution and client
   construction.
2. Preserve API-key detection and XML-RPC construction unchanged for API keys.
3. Add REST-default/explicit-override validation and suppress alternate
   API-key auth for tokens.
4. Commit the connection and transport contract.

## Task 3 — Prove and document parity

**Interfaces:** the existing auth comparison phase writes a private token-only
config from a private `/rest/login` fixture response, then verifies a REST
identity request without new CLI flags; the parity row changes from expected
gap to parity.

**Verification:**

- Mode: functional-test. Contract: a token from config authenticates a real
  REST request and the comparison phase no longer reports #676 as an expected
  gap. Command: `make functional-compare`; expected: all comparison phases pass.
- Mode: functional-test. Contract: ordinary real-container behavior remains
  intact. Command: `make functional-test`; expected: all phases pass.
- Mode: guardrail. Command: `make lint && make test`; expected: exit 0.

1. Replace only #676's controlled-gap assertion with a configured-token REST
   assertion in the existing auth comparison phase. Obtain its disposable token
   using existing fixture credentials into a mode-0600 file; never print the
   token or retain it in comparison artifacts.
2. Update the parity row and CLI reference with the persisted token behavior
   and REST-only limitation; do not document unimplemented login commands.
3. Run focused tests, full lint/test, and the required functional test.
4. Commit the proof and documentation.

## Rollback

A revert removes the additive optional configuration field and returns the
client to API-key-only behavior. No migration or external state is created.
