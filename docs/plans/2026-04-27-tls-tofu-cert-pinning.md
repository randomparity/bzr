# TLS Trust-On-First-Use and Certificate Pinning

**Date:** 2026-04-27
**Status:** Draft

## Problem

The only option for connecting to Bugzilla servers with untrusted TLS
certificates (internal CAs, self-signed certs) is `--tls-insecure`, which
disables all certificate verification. This leaves connections vulnerable
to MITM attacks — any attacker on the network can intercept the API key.

Enterprise users (e.g. IBM internal Bugzilla) need a way to trust their
server's certificate without disabling verification entirely.

## Solution

Implement two complementary trust mechanisms:

1. **Custom CA certificate** (`--tls-ca-cert`) — user provides their
   internal CA's PEM file, which is added to the trust store for that
   server.
2. **Certificate pinning with TOFU** — pin the SHA-256 fingerprint of a
   server's leaf certificate. Can be set explicitly or learned
   interactively on first connection (SSH `known_hosts` model).

Both mechanisms share a custom `rustls` `ServerCertVerifier` that
replaces the current `build_http_client` function.

## Data Model

### New `ServerConfig` fields

```toml
[servers.ibm-ltc]
url = "https://bugzilla.linux.ibm.com"
api_key = "..."

# Option A: trust a custom CA
tls_ca_cert = "/path/to/ibm-ca.pem"

# Option B: pin a specific certificate fingerprint
tls_pin_sha256 = "sha256//a1b2c3d4e5f6..."
tls_pin_issuer = "CN=IBM Internal CA, O=IBM"
```

**`tls_ca_cert`** (`Option<PathBuf>`): Path to a PEM-encoded CA
certificate file. When set, this CA is added to the trust store for this
server only.

**`tls_pin_sha256`** (`Option<String>`): SHA-256 fingerprint of the
server's leaf certificate. Format: `sha256//<base64-hash>` (matches
curl's `--pinnedpubkey` convention). Set manually via
`--tls-pin-sha256`, via `--tls-pin-now`, or automatically via TOFU.

**`tls_pin_issuer`** (`Option<String>`): Distinguished Name of the
certificate issuer, stored automatically alongside the pin. Used for
display and rotation detection (same-CA vs. different-CA). Not used for
cryptographic verification.

### Mutual exclusivity

`tls_insecure` is the "I don't care" escape hatch. When set,
`tls_ca_cert` and `tls_pin_sha256` are ignored.

`tls_ca_cert` and `tls_pin_sha256` conflict with each other — pick one
trust model per server.

Validated at config load time with a clear error if conflicting fields
are set.

## TLS Verifier Architecture

### Module structure

```
src/tls/
  mod.rs              TlsConfig struct, build_tls_client() function
  verifier.rs         Custom ServerCertVerifier implementation
  fingerprint.rs      SHA-256 compute/parse utilities
  tofu.rs             Probe connection, interactive prompt, config update
```

### `TlsConfig`

```rust
pub struct TlsConfig {
    pub insecure: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub pin_sha256: Option<String>,
}
```

Constructed from `ServerConfig` fields in `commands/shared.rs`. Passed to
`build_tls_client()` which returns a configured `reqwest::Client`.

### Verification logic (in order)

1. **`insecure = true`** — accept everything. Current behavior via
   `danger_accept_invalid_certs(true)`.
2. **`ca_cert_path` is set** — build a `WebPkiServerVerifier` with the
   custom CA added to the root store. Verify against it.
3. **`pin_sha256` is set** — compute SHA-256 of the presented leaf cert,
   compare against pin. Match = accept. Mismatch = reject with
   fingerprint diff (see Rotation Handling).
4. **None of the above** — delegate to the default system roots verifier.
   Current behavior for unconfigured servers.

The custom `ServerCertVerifier` is only constructed when `ca_cert_path`
or `pin_sha256` is set. The default and insecure cases use reqwest's
built-in configuration with no custom verifier overhead.

## TOFU Flow

When a connection fails with a certificate error and no trust mechanism
is configured (`tls_insecure`, `tls_ca_cert`, `tls_pin_sha256` are all
unset/false), intercept the error and offer to pin.

### Steps

1. Command runs, TLS handshake fails with `UnknownIssuer` or similar.
2. Make a probe connection with verification disabled to retrieve the
   server's certificate. **No credentials are sent on the probe** — the
   API key is never transmitted over an unverified connection.
3. Compute the fingerprint and display to the user:

```
warning: server certificate is not trusted
  server:      ibm-ltc (bugzilla.linux.ibm.com)
  fingerprint: sha256//a1b2c3d4e5f6...
  issuer:      CN=IBM Internal CA, O=IBM

Trust this certificate? [y/N/always]
  y     = trust for this session only
  N     = abort (default)
  always = pin fingerprint to config and trust permanently
```

4. **`y`** — retry the original request with verification disabled for
   this session only. No config change.
5. **`always`** — write `tls_pin_sha256` and `tls_pin_issuer` to config,
   retry with pin-based verification.
6. **`N`** or non-interactive — surface the original error with
   actionable hints.

### Non-interactive detection

Check `stdin.is_terminal()`. If not a TTY (scripts, CI, piped input),
skip the prompt and fail with an error suggesting:

```
error: HTTP request failed: ... invalid peer certificate: UnknownIssuer
  hint: to trust this server's certificate, re-run the command interactively,
    or pre-pin with:  bzr config set-server <NAME> --tls-pin-now
    or provide a CA:  bzr config set-server <NAME> --tls-ca-cert <PATH>
    or skip verification: bzr config set-server <NAME> --tls-insecure
```

## Certificate Rotation Handling

When a pinned server presents a certificate with a different fingerprint:

### Same CA issuer — prompt to update

The issuer DN of the new cert matches `tls_pin_issuer`. This is likely a
routine certificate rotation.

```
warning: server certificate has changed (likely rotation)
  server:      ibm-ltc (bugzilla.linux.ibm.com)
  old pin:     sha256//a1b2c3d4...
  new pin:     sha256//e7f8g9h0...
  issuer:      CN=IBM Internal CA, O=IBM  (unchanged)

Accept new certificate? [y/N]
```

**`y`** — update `tls_pin_sha256` in config, proceed.
**`N`** or non-interactive — abort with actionable message.

### Different CA issuer — hard fail

The issuer changed. This could indicate a MITM attack.

```
error: server certificate issuer has changed — possible MITM attack
  server:      ibm-ltc (bugzilla.linux.ibm.com)
  expected:    CN=IBM Internal CA, O=IBM
  got:         CN=Evil Corp, O=Attacker

Connection refused. If this change is expected, remove the pin:
    bzr config set-server ibm-ltc --tls-pin-clear
```

No prompt. The user must explicitly clear the pin and re-pin.

## CLI Changes

### `bzr config set-server` — new flags

| Flag | Description |
|------|-------------|
| `--tls-ca-cert <PATH>` | Path to PEM CA certificate file |
| `--tls-pin-sha256 <HASH>` | Pin a certificate fingerprint manually |
| `--tls-pin-now` | Connect to server and pin its current certificate |
| `--tls-pin-clear` | Remove stored pin and `tls_pin_issuer` |

### Mutual exclusivity (clap)

- `--tls-insecure` conflicts with `--tls-ca-cert`, `--tls-pin-sha256`,
  `--tls-pin-now`
- `--tls-ca-cert` conflicts with `--tls-pin-sha256`, `--tls-pin-now`
- `--tls-pin-clear` conflicts with `--tls-pin-sha256`, `--tls-pin-now`

### `bzr config show` — updated output

```
[ibm-ltc]
  URL           https://bugzilla.linux.ibm.com
  Email         drc@ibm.com
  API Key       yOVzU8pz...
  Auth          header
  TLS CA Cert   /path/to/ibm-ca.pem
  TLS Pin       sha256//a1b2c3d4... (CN=IBM Internal CA, O=IBM)
```

### Updated TLS hint

When a cert error occurs and TOFU is declined or non-interactive, the
hint now covers all options:

```
error: HTTP request failed: ... invalid peer certificate: UnknownIssuer
  hint: to trust this server's certificate, re-run the command interactively,
    or pre-pin with:  bzr config set-server <NAME> --tls-pin-now
    or provide a CA:  bzr config set-server <NAME> --tls-ca-cert <PATH>
    or skip verification: bzr config set-server <NAME> --tls-insecure
```

## Changes to Existing Modules

| File | Change |
|------|--------|
| `src/config.rs` | Add `tls_ca_cert`, `tls_pin_sha256`, `tls_pin_issuer` to `ServerConfig`. Add validation for mutual exclusivity. |
| `src/cli/config.rs` | Add `--tls-ca-cert`, `--tls-pin-sha256`, `--tls-pin-now`, `--tls-pin-clear` flags. |
| `src/commands/config.rs` | Handle new flags in `set-server`. Implement `--tls-pin-now` (probe + confirm + store). |
| `src/commands/shared.rs` | Build `TlsConfig` from `ServerConfig`, pass to `build_tls_client`. Intercept cert errors for TOFU flow. |
| `src/http.rs` | Remove `build_http_client` (replaced by `tls::build_tls_client`). Keep `looks_like_tls_error`. Update `tls_hint` message text. |
| `src/output/config.rs` | Display `TLS CA Cert` and `TLS Pin` fields. |
| `src/error.rs` | Add `PinMismatch` and `IssuerChanged` error variants. |

## Dependencies

**Production:** None new. `rustls` is already pulled in via reqwest's
`rustls-tls-native-roots` feature.

**Dev-only:** `rcgen` for generating test CA and leaf certificates.

## Testing Strategy

### Unit tests (`src/tls/`)

- **`fingerprint.rs`** — compute fingerprint from known DER cert, verify
  `sha256//` format, round-trip through `parse_pin`.
- **`verifier.rs`** — test each verification path with synthetic certs
  (generated via `rcgen` at test time):
  - Default verifier rejects self-signed certs.
  - Custom CA verifier accepts certs signed by that CA, rejects others.
  - Pin verifier accepts matching fingerprint, rejects mismatch.
  - Pin verifier with same issuer on mismatch returns rotation error.
  - Pin verifier with different issuer on mismatch returns MITM error.

### Integration tests (`tests/`)

- Mock servers with self-signed certs via `wiremock` + `rustls`.
- TOFU flow with simulated TTY input — verify pin is written to config
  after `always`.
- `--tls-pin-now` — verify it connects, extracts fingerprint, stores in
  config.
- `--tls-ca-cert` — verify custom CA is loaded and connection succeeds.
- Mutual exclusivity — verify clap rejects conflicting flag combinations.

### What we don't test

Real network TLS handshakes against external servers. All TLS tests use
locally-generated certs and local mock servers.
