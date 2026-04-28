# TLS Security Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix two security vulnerabilities in the TLS TOFU implementation: session-only trust sending credentials over a fully unverified connection, and rotation re-probe TOCTOU enabling cert substitution.

**Architecture:** (1) Session-only trust ("y") pins the probed cert fingerprint in-memory instead of using `insecure: true`. (2) Rotation flow extracts the new cert's fingerprint from the verifier's error message instead of re-probing. (3) Issuer comparison uses raw DER bytes instead of parsed strings to prevent spoofing.

**Tech Stack:** Existing Rust + rustls stack, no new dependencies.

---

### Task 1: Session-only trust should pin the probed cert, not use insecure mode

**Files:**
- Modify: `src/commands/shared.rs`

The "y" (session-only) path currently sets `insecure: true`, which disables all cert verification and sends credentials over a fully unverified connection. It should instead pin the fingerprint captured during the probe — identical to the "always" path except without persisting to config.

- [ ] **Step 1: Fix the session-only TlsConfig**

In `src/commands/shared.rs`, find the `handle_tofu` function. Replace the `Some(false)` match arm (around line 97):

```rust
        Some(false) => {
            // "y" — trust for this session only (insecure mode)
            TlsConfig {
                insecure: true,
                server_name: Some(server_name.to_string()),
                ..Default::default()
            }
        }
```

with:

```rust
        Some(false) => {
            // "y" — trust this specific cert for this session only (no config change)
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer: Some(issuer),
                server_name: Some(server_name.to_string()),
                ..Default::default()
            }
        }
```

This uses the exact same pinned verification as "always" but skips the `config.save()`. The fingerprint was already captured by the probe, so this is a one-line semantic change.

Note: The `fingerprint` and `issuer` variables are moved into the "always" arm (line 86-87) via `.clone()`. Verify that the `Some(false)` arm can still use them — if "always" is matched first and consumes them, the "y" arm will fail to compile. Since both arms are in a `match`, only one executes, so the ownership is fine. But the "always" arm clones `fingerprint` for `srv.tls_pin_sha256 = Some(fingerprint.clone())` at line 86 and then moves `fingerprint` into `TlsConfig` at line 91. Check if `fingerprint` is still available for the `Some(false)` arm — it should be since the match arms are exclusive.

- [ ] **Step 2: Run tests**

Run: `cargo test --lib commands::shared::tests -- --quiet`

Expected: all pass. No test directly exercises the TOFU prompt (interactive), but compilation and existing tests should still pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/commands/shared.rs
git commit -m "security: session-only TOFU trust pins probed cert instead of insecure mode

The 'y' (session-only) TOFU response previously set insecure: true,
which disabled ALL certificate verification and sent credentials over
an unverified connection. Now pins the specific cert fingerprint
captured during the probe, matching the 'always' path minus the
config persistence."
```

---

### Task 2: Eliminate rotation re-probe TOCTOU by extracting cert from error

**Files:**
- Modify: `src/tls/verifier.rs` (include new fingerprint in PIN_MISMATCH error)
- Modify: `src/commands/shared.rs` (parse fingerprint from error, skip re-probe)

The rotation flow currently re-probes the server with verification disabled after a PIN_MISMATCH error. Between the failed handshake and re-probe, a MITM could swap certs. The fix: extract the new cert's fingerprint and issuer from the PIN_MISMATCH error message (which already contains the fingerprint) instead of making a second unverified connection.

- [ ] **Step 1: Add issuer to PIN_MISMATCH error message**

In `src/tls/verifier.rs`, in `PinnedCertVerifier::verify_server_cert`, the PIN_MISMATCH error (around line 88-92) currently includes the expected and actual fingerprints. Add the actual issuer DN so the caller can display it without re-probing:

Replace:
```rust
        let actual_fp = compute_fingerprint(end_entity.as_ref());
        Err(TlsError::General(format!(
            "PIN_MISMATCH for {}: expected {}, got {}",
            self.server_name, self.pin_str, actual_fp
        )))
```

with:
```rust
        let actual_fp = compute_fingerprint(end_entity.as_ref());
        let actual_issuer = extract_issuer_dn(end_entity.as_ref());
        Err(TlsError::General(format!(
            "PIN_MISMATCH for {}: expected {}, got {}, issuer {}",
            self.server_name, self.pin_str, actual_fp, actual_issuer
        )))
```

- [ ] **Step 2: Add a parser for the PIN_MISMATCH error message**

In `src/commands/shared.rs`, add a function to extract the new fingerprint and issuer from the error chain:

```rust
/// Extract the new fingerprint and issuer from a PIN_MISMATCH error.
///
/// The error message format is:
/// `PIN_MISMATCH for <server>: expected <old>, got <new>, issuer <issuer>`
fn parse_pin_mismatch_details(err: &BzrError) -> Option<(String, String)> {
    let chain = match err {
        BzrError::Http(e) => crate::error::format_error_chain(e),
        _ => return None,
    };
    let pin_mismatch_pos = chain.find("PIN_MISMATCH")?;
    let rest = &chain[pin_mismatch_pos..];
    let got_pos = rest.find(", got ")?;
    let after_got = &rest[got_pos + ", got ".len()..];
    let issuer_pos = after_got.find(", issuer ")?;
    let new_fp = after_got[..issuer_pos].to_string();
    let new_issuer = after_got[issuer_pos + ", issuer ".len()..].to_string();
    Some((new_fp, new_issuer))
}
```

- [ ] **Step 3: Update handle_pin_rotation to use parsed details**

In `src/commands/shared.rs`, modify `handle_pin_rotation` to accept the pre-parsed fingerprint and issuer instead of re-probing:

Change the signature from:
```rust
async fn handle_pin_rotation(
    server_name: &str,
    url: &str,
    api_key: &str,
    email: Option<&str>,
    api_override: Option<ApiMode>,
    old_pin: &str,
    config: &mut Config,
) -> Result<BugzillaClient> {
```

to:
```rust
async fn handle_pin_rotation(
    server_name: &str,
    url: &str,
    api_key: &str,
    email: Option<&str>,
    api_override: Option<ApiMode>,
    old_pin: &str,
    new_fingerprint: &str,
    new_issuer: &str,
    config: &mut Config,
) -> Result<BugzillaClient> {
```

Remove the `probe_server_cert` call at the top:
```rust
    // DELETE these lines:
    // let (new_fingerprint, issuer) = crate::tls::tofu::probe_server_cert(url).await?;
```

Update the rest of the function to use the passed-in `new_fingerprint` and `new_issuer` parameters (owned `String` → `&str` references — adjust `.clone()` calls to `.to_string()`).

- [ ] **Step 4: Update the call site in detect_with_tofu_fallback**

In `detect_with_tofu_fallback`, update the `is_pin_mismatch` branch to parse the details from the error and pass them:

Replace:
```rust
        Err(ref e) if is_pin_mismatch(e) => {
            let old_pin = tls_config.pin_sha256.as_deref().unwrap_or("<unknown>");
            let client = handle_pin_rotation(
                server_name,
                url,
                api_key,
                email,
                api_override,
                old_pin,
                config,
            )
            .await?;
            Ok(DetectOrClient::Client(client))
        }
```

with:
```rust
        Err(ref e) if is_pin_mismatch(e) => {
            let old_pin = tls_config.pin_sha256.as_deref().unwrap_or("<unknown>");
            let (new_fp, new_issuer) = parse_pin_mismatch_details(e)
                .unwrap_or_else(|| ("<unknown>".to_string(), "<unknown>".to_string()));
            let client = handle_pin_rotation(
                server_name,
                url,
                api_key,
                email,
                api_override,
                old_pin,
                &new_fp,
                &new_issuer,
                config,
            )
            .await?;
            Ok(DetectOrClient::Client(client))
        }
```

- [ ] **Step 5: Write test for parse_pin_mismatch_details**

Add a unit test:

```rust
#[test]
fn parse_pin_mismatch_extracts_details() {
    // Simulate the error chain format that reqwest wraps
    let inner_msg = "PIN_MISMATCH for test: expected sha256//old==, got sha256//new==, issuer CN=Test CA";
    let result = parse_pin_mismatch_details_from_str(inner_msg);
    assert_eq!(result, Some(("sha256//new==".to_string(), "CN=Test CA".to_string())));
}
```

Note: Since `parse_pin_mismatch_details` takes `&BzrError`, you may need a helper that operates on `&str` for testability, or construct a mock `BzrError::Http` with a fake reqwest error. The simplest approach is to extract the string parsing into a separate `fn parse_pin_mismatch_from_chain(chain: &str) -> Option<(String, String)>` and test that.

- [ ] **Step 6: Run tests**

Run: `cargo test --lib -- --quiet`

Expected: all pass.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -- -D warnings`

- [ ] **Step 8: Commit**

```bash
git add src/tls/verifier.rs src/commands/shared.rs
git commit -m "security: eliminate rotation re-probe TOCTOU by parsing cert from error

The rotation flow previously re-probed the server with verification
disabled after a PIN_MISMATCH, creating a TOCTOU window where a MITM
could substitute a different cert. Now extracts the new fingerprint
and issuer directly from the PIN_MISMATCH error message, which was
captured during the original (verified) TLS handshake."
```

---

### Task 3: Use raw DER bytes for issuer comparison

**Files:**
- Modify: `src/tls/verifier.rs` (store and compare raw issuer DER)
- Modify: `src/config.rs` (add `tls_pin_issuer_der` field)
- Modify: `src/tls/mod.rs` (add field to `TlsConfig`)
- Modify: `src/commands/shared.rs` (pass issuer DER through)
- Modify: `src/tls/tofu.rs` (capture raw issuer DER during probe)

The issuer DN comparison currently uses string comparison of parsed output from a hand-rolled DER parser. An attacker can craft a cert whose issuer DN parses to the same string. Fix: compare the raw DER bytes of the issuer field.

- [ ] **Step 1: Add issuer DER extraction function**

In `src/tls/verifier.rs`, add a function that extracts the raw issuer field bytes from a DER-encoded certificate (without parsing them into a string):

```rust
/// Extract the raw DER bytes of the issuer field from a certificate.
/// Returns `None` if parsing fails.
pub(crate) fn extract_issuer_der(cert_der: &[u8]) -> Option<Vec<u8>> {
    // Walk to the issuer field (same path as extract_issuer_dn)
    let (_, content) = parse_der_sequence(cert_der)?;
    let (_, tbs) = parse_der_sequence(content)?;
    let mut pos = tbs;

    // Skip optional version [0] EXPLICIT
    if pos.first()? & 0xe0 == 0xa0 {
        let (rest, _) = parse_der_element(pos)?;
        pos = rest;
    }
    // Skip serialNumber
    let (rest, _) = parse_der_element(pos)?;
    pos = rest;
    // Skip signature
    let (rest, _) = parse_der_element(pos)?;
    pos = rest;
    // The issuer field is next — capture its raw bytes
    let (_, issuer_content) = parse_der_element(pos)?;
    // Return the full TLV (tag + length + value) of the issuer
    let issuer_len = pos.len() - issuer_content.len();
    // Hmm, we need the raw bytes including tag and length.
    // Actually parse_der_element returns (rest_after, content).
    // The raw TLV is pos[..pos.len() - rest.len()]
    // Let me re-derive: rest starts after the element, so
    // element bytes = &pos[..pos.len() - rest_after.len()]
    // But we already consumed rest. Let me restructure.
    None // placeholder — see implementation note
}
```

Actually, the existing DER parser helper `parse_der_element` returns `(rest, content)`. The raw TLV bytes of the issuer are `&pos[..pos.len() - rest.len()]` where `rest` is the first element of the tuple. This requires a small refactor of the extraction logic.

A simpler approach: compute a SHA-256 hash of the raw issuer bytes and store that as `tls_pin_issuer_hash`. This avoids storing raw bytes in TOML and gives a fixed-size comparison.

- [ ] **Step 2: Add tls_pin_issuer_der to config**

In `src/config.rs`, add a new field to `ServerConfig`:

```rust
    /// Base64-encoded raw DER bytes of the pinned certificate's issuer field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer_der: Option<String>,
```

Add the field to `TlsConfig` in `src/tls/mod.rs`:

```rust
    pub pin_issuer_der: Option<String>,
```

Update `ServerConfig::tls_config()` to include the new field.

- [ ] **Step 3: Store issuer DER during pinning**

Update `probe_server_cert` in `src/tls/tofu.rs` to also return the base64-encoded raw issuer DER bytes (call `extract_issuer_der` on the captured cert).

Update all pinning sites (`handle_tofu`, `handle_pin_rotation`, `set_server --tls-pin-now`) to store `tls_pin_issuer_der`.

- [ ] **Step 4: Compare raw DER in the verifier**

In `PinnedCertVerifier`, store the expected issuer DER bytes. On pin mismatch, extract the actual issuer DER and compare bytes. Only fall through to the parsed-string comparison as a fallback for pins created before this change (where `tls_pin_issuer_der` is `None`).

- [ ] **Step 5: Update all ServerConfig test literals**

Add `tls_pin_issuer_der: None` to all test `ServerConfig` literals.

- [ ] **Step 6: Write tests**

Add a test that two certs with different issuers produce different raw DER bytes, and that two certs with the same issuer produce identical raw DER bytes.

- [ ] **Step 7: Run full test suite and clippy**

```bash
cargo test -- --quiet
cargo clippy -- -D warnings
```

- [ ] **Step 8: Commit**

```bash
git add src/tls/verifier.rs src/config.rs src/tls/mod.rs \
       src/commands/shared.rs src/tls/tofu.rs
git commit -m "security: compare raw issuer DER bytes to prevent spoofing

The issuer change detection previously compared parsed string output
from a hand-rolled DER parser, which an attacker could spoof. Now
stores and compares base64-encoded raw DER bytes of the issuer field,
making spoofing require byte-identical issuer DER."
```
