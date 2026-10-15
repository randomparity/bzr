# ADR 0083: Sequoia floor for macOS release binaries and bottles

## Status

Accepted by the maintainer's explicit option-2 decision for issue #884,
2026-10-15.

## Context

GitHub retires its macOS 14 hosted labels on November 2, 2026. The existing
Sonoma bottle policy used the oldest arm64 runner so Homebrew could pour the
same bottle on newer systems. Homebrew's host-derived bottle tag does not
establish backward runtime compatibility on older operating systems.

## Decision

Use explicit native arm64 `macos-15` for the four affected CI/release jobs.
The prebuilt Apple Silicon binary and Homebrew bottle support macOS 15
Sequoia or newer. Set the release binary deployment target to 15.0 and let
Homebrew generate `arm64_sequoia` naturally. Validate the actual candidate's
Mach-O floor/dependencies and clean bottle installs on macOS 15 and 26.

## Consequences

Sonoma is no longer in the supported prebuilt/bottled-install contract.
No recurring Sonoma host or bottle retagging is required. The release flow,
Keychain coverage and Linux/Windows coverage remain intact. Record actual
artifact evidence in the migration PR before merge.

## Considered & rejected

- Preserve Sonoma with a recurring builder — judgment: the maintainer selected
  the higher floor after being offered this infrastructure/maintenance option.
- Select `macos-latest` — judgment: a moving runner can silently raise the
  package floor again; explicit `macos-15` makes the support choice reviewable.
- Relabel a newer bottle as Sonoma — verified: `brew bottle --help` on
  Homebrew 7.0.9-64-gbe45bb1 has no production-tag override; inspected
  `extend/os/mac/utils/bottles.rb` derives the tag from the host and resolves
  older compatible tags. Neither metadata rewriting nor a deployment flag
  proves older-runtime behavior, and the maintainer did not authorize it.
