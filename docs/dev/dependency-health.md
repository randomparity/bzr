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

## Password Prompt Dependency

`rpassword 7.5.2` remains in use because `bzr config set-keyring` needs
cross-platform no-echo terminal input. It currently pulls `rtoolbox 0.0.3`,
which contributes a `windows-sys 0.52.0` path in
`cargo tree --duplicates --target all --edges normal`; no lighter replacement
was found that preserves behavior and reduces the target-all duplicate graph.

Current target-all graph evidence:

```text
rpassword v7.5.2
└── bzr v0.4.0-dev

rtoolbox v0.0.3
└── rpassword v7.5.2
    └── bzr v0.4.0-dev

windows-sys v0.52.0
├── ring v0.17.14
└── rtoolbox v0.0.3
    └── rpassword v7.5.2
        └── bzr v0.4.0-dev
```

## Platform Transitive Dependencies

Task 4 of the platform drift plan attempted conservative updates for broad platform
transitives on May 8, 2026, before the keyring direct-store refactor landed:

```console
$ cargo update -p windows-sys
error: There are multiple `windows-sys` packages in your project, and the specification
`windows-sys` is ambiguous.
Please re-run this command with one of the following specifications:
  windows-sys@0.52.0
  windows-sys@0.59.0
  windows-sys@0.61.2

$ cargo update -p windows-sys@0.52.0
$ cargo update -p windows-sys@0.59.0
$ cargo update -p windows-sys@0.61.2
Locking 0 packages to latest Rust 1.88 compatible versions

$ cargo update -p windows-targets
$ cargo update -p windows-targets@0.52.6
Locking 0 packages to latest Rust 1.88 compatible versions

$ cargo update -p security-framework
Locking 0 packages to latest Rust 1.88 compatible versions

$ cargo update -p core-foundation
error: There are multiple `core-foundation` packages in your project, and the
specification `core-foundation` is ambiguous.
Please re-run this command with one of the following specifications:
  core-foundation@0.9.4
  core-foundation@0.10.1

$ cargo update -p core-foundation@0.9.4
$ cargo update -p core-foundation@0.10.1
$ cargo update -p core-foundation-sys
Locking 0 packages to latest Rust 1.88 compatible versions
```

The generated lockfile diff did not reduce duplicate families. It only moved some
existing `windows-sys` dependency edges from newer already-present versions to older
already-present versions, so the lockfile change was discarded.

After wiring `keyring-core` to native stores directly, the duplicate graph is smaller
but still constrained by upstream dependency ranges:

```text
security-framework: unified at 3.7.0 via apple-native-keyring-store and
rustls-native-certs

windows-sys: 0.52.0 via ring/rtoolbox, 0.59.0 via dbus-secret-service, and
0.61.2 via clap/anstyle, colored, dirs-sys, tokio, tracing-subscriber, schannel,
socket2, rpassword, and windows-native-keyring-store

windows-targets: only 0.52.6 remains in the normal target-all duplicate graph
```
