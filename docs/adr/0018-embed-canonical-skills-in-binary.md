# ADR 0018: Embed canonical skills in the bzr binary

## Status

Accepted

## Context

Issue #523 adds `bzr skills install` while the existing shell and PowerShell
installers must continue to work before `bzr` is installed. The command needs an
offline, version-matched payload, and repository policy permits reusable workflow
sources only under `content/skills/`. Viable designs differ in payload ownership and
how a compiled binary discovers a changing directory tree.

## Decision

Keep one canonical payload under `content/skills/`. Extend `build.rs` to traverse
directories, enumerate regular payload files deterministically, and generate a Rust
manifest in `OUT_DIR`; the command compiles that manifest with each file embedded as
bytes. Empty directories are ignored. The build rejects every other entry type,
invalid relative paths, a missing `SKILL.md`, or an empty payload. Cargo rebuild
directives cover the canonical tree.

The standalone installers continue to provide the no-binary bootstrap path, but
read and remotely extract `content/skills/` rather than owning a second skill tree.
When a user explicitly pins a pre-migration branch, tag, or commit, remote extraction
falls back to that archive's historical `agent-skills/skills/` path. The fallback is
read-only compatibility for immutable old layouts, not a second current source. The
installers retain ADR 0013's transport and verification policy. The binary never
fetches skills at runtime and adds no embedding dependency.

## Consequences

- The installed payload always matches the running binary and works offline.
- A skill addition is automatically embedded without maintaining a second file
  manifest, while deterministic sorting keeps builds reproducible.
- The installed binary grows by approximately the raw Markdown payload plus manifest
  overhead; compression reduces the effect on release-archive download size only.
- `build.rs` becomes responsible for validating and enumerating non-Rust content.
- Release archives, crates.io packages, and local builds use the same checked-in
  canonical source; packaging tests must prove that source is present.
- Standalone installer path fixtures change, but their public behavior and trust
  model do not.

## Considered & rejected

- **Add an embedding crate.** It reduces build-script code but adds supply-chain
  surface for directory walking and byte inclusion that the standard library can
  express directly.
- **Download the payload when the command runs.** This makes installation depend on
  network availability and recreates ADR 0013's weaker remote trust boundary even
  though the executable can carry its matching payload.
- **Hard-code one `include_bytes!` entry per file.** This avoids generated code but
  creates a manifest that silently drifts whenever reference files are added.
- **Keep `agent-skills/skills/` as a second source tree.** Two editable copies can
  diverge and conflicts with the repository's canonical-source rule.
- **Do nothing and retain only the standalone installers.** Users would still need
  to discover repository-hosted installation instructions, leaving issue #523
  unresolved.
