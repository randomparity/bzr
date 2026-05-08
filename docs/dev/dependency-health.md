# Dependency Health Notes

## Reqwest TLS Stack

`reqwest` remains pinned to `0.12` for now. The `0.13.3` TLS feature set
does not currently offer a safe lighter replacement for this crate's explicit
TLS behavior.

Evidence from the `dep-health-reqwest` evaluation:

- `reqwest = { version = "0.13", default-features = false, features = [
  "json", "query", "rustls" ] }` compiles and passes the client, HTTP, and TLS
  tests, including custom `use_preconfigured_tls` paths. It also adds
  `rustls-platform-verifier 0.7.0` and `aws-lc-rs 1.16.3`, which expands the
  TLS dependency surface instead of reducing dependency-health drift.
- `reqwest = { version = "0.13", default-features = false, features = [
  "json", "query", "rustls-no-provider" ] }` removes `aws-lc-rs`, but
  `cargo tree --invert rustls-platform-verifier@0.7.0 --target all --edges
  normal` still shows `rustls-platform-verifier 0.7.0 -> reqwest 0.13.3 ->
  bzr`. The existing default `reqwest::Client::new()` and builder paths also
  panic with `No provider set` unless a process-wide rustls provider is
  installed.
- `reqwest = { version = "0.13", default-features = false, features = [
  "json", "query", "__rustls" ] }` removes `rustls-platform-verifier` from the
  lockfile, but `cargo check` fails inside `reqwest 0.13.3` because its rustls
  backend still references the unlinked `rustls_platform_verifier` crate.

The current `0.12` stack keeps the existing explicit rustls behavior without
adding `aws-lc-rs` or `rustls-platform-verifier`. Revisit this when reqwest
publishes a public rustls feature that supports explicit provider selection
without pulling the platform verifier dependency.
