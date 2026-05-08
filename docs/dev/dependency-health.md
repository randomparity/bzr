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
transitives on May 8, 2026:

```console
$ cargo update -p windows-sys
error: There are multiple `windows-sys` packages in your project, and the specification
`windows-sys` is ambiguous.
Please re-run this command with one of the following specifications:
  windows-sys@0.52.0
  windows-sys@0.59.0
  windows-sys@0.60.2
  windows-sys@0.61.2

$ cargo update -p windows-sys@0.52.0
$ cargo update -p windows-sys@0.59.0
$ cargo update -p windows-sys@0.60.2
$ cargo update -p windows-sys@0.61.2
Locking 0 packages to latest Rust 1.88 compatible versions

$ cargo update -p windows-targets
error: There are multiple `windows-targets` packages in your project, and the
specification `windows-targets` is ambiguous.
Please re-run this command with one of the following specifications:
  windows-targets@0.52.6
  windows-targets@0.53.5

$ cargo update -p windows-targets@0.52.6
$ cargo update -p windows-targets@0.53.5
Locking 0 packages to latest Rust 1.88 compatible versions

$ cargo update -p security-framework
error: There are multiple `security-framework` packages in your project, and the
specification `security-framework` is ambiguous.
Please re-run this command with one of the following specifications:
  security-framework@2.11.1
  security-framework@3.5.1

$ cargo update -p security-framework@2.11.1
$ cargo update -p security-framework@3.5.1
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

The before and after duplicate graph remains constrained by upstream dependency ranges:

```text
core-foundation: 0.9.4 via security-framework 2.11.1, and 0.10.1 via
security-framework 3.5.1

security-framework: 2.11.1 via keyring 3.6.3, and 3.5.1 via keyring 3.6.3 plus
rustls-native-certs 0.8.3

windows-sys: 0.52.0 via ring/rtoolbox, 0.59.0 via dbus, 0.60.2 via keyring, and
0.61.2 via clap/anstyle, colored, dirs-sys, tokio, tracing-subscriber, schannel,
socket2, and rpassword

windows-targets: 0.52.6 via windows-sys 0.52.0 and 0.59.0, and 0.53.5 via
windows-sys 0.60.2
```
