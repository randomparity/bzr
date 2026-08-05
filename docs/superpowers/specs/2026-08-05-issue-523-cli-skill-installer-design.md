# Issue 523: CLI Skill Installer Design

## Scope authority

- **Interaction:** interactive
- **Scope identity:** https://github.com/randomparity/bzr/issues/523; token
  `9A8D4DF9-AE06-43D3-8F6F-79DCEDEB841F`
- **Outcome:** add `bzr skills install` so bundled bzr skills can be installed for
  supported agents at an explicitly selected global or project location.
- **Provenance:** issue #523; the user's 2026-08-05 choice of explicit scope;
  repository instructions; accepted ADR 0013 for the standalone installer.
- **Exclusions:** uninstall, update, list, project-root discovery, new agent layouts,
  and changes to ADR 0013's remote-fetch trust policy.
- **Permitted surface:** CLI and dispatch, focused installer/build modules,
  `content/skills/`, directly affected standalone compatibility assets, unit and
  functional tests, CLI documentation, and changelog.
- **Ambiguities:** none.

## Requirements

1. `bzr skills install --agent <AGENT>` installs every bundled bzr skill for
   `standard`, `bob`, `codex`, `claude`, or `all`.
2. Exactly one scope is required: `--global` or `--project <PATH>`. `--project .`
   means the current directory. The flags conflict at parse time.
3. If scope is absent at an interactive terminal, the command reports both choices,
   their resolved destination patterns, and copyable examples. It does not guess a
   project. If scope is absent non-interactively, it fails without reading stdin and
   gives concise examples.
4. Global destinations preserve the current mappings:
   `standard`/`bob`/`codex` use `~/.agents/skills`; `claude` uses
   `~/.claude/skills`; `all` uses both. Project scope applies the same relative
   mappings beneath the supplied project path.
5. Installation is offline and uses the payload compiled into the running binary.
   Re-running updates command-owned or standalone-installer-owned skill directories.
6. The command refuses symlinked destination components and same-named foreign
   folders without modifying them. There is no force option in this issue.
7. Each skill is fully staged and checked for `SKILL.md` before its existing owned
   directory is moved aside. A failed replacement restores that directory. A failure
   after another skill was installed reports the partial outcome; valid installed
   skills are not rolled back.
8. User-facing output goes through `Writers`. Table output names each installed skill
   and destination; JSON-family output returns one result containing scope,
   destinations, and installed skill names.
9. The reusable source of truth is `content/skills/`. Agent-specific trees and the
   compiled binary consume it; they do not define a second skill or command source.

## Architecture

[ADR 0018](../../adr/0018-embed-canonical-skills-in-binary.md) supersedes ADR 0013,
selects compile-time embedding without a new dependency, and carries forward the
standalone fetcher's transport/trust policy. `build.rs` walks `content/skills/`, validates the
tree, sorts relative paths, and writes an `embedded_skills.rs` manifest to `OUT_DIR`.
The generated manifest is private build output and contains byte inclusions, not copied
payload bytes.

`src/cli/skills.rs` owns the clap types: an `install` action, `AgentTarget` value enum,
and mutually exclusive scope arguments. `src/commands/skills.rs` orchestrates scope
resolution and output. Focused leaf modules under `src/skills/` own the embedded
manifest and filesystem installation so command parsing, presentation, and mutation
logic remain independently testable.

The standalone installers use `content/skills/` in current checkouts and archives.
For an explicitly pinned historical ref whose archive predates that path, remote
extraction accepts `agent-skills/skills/` as a read-only compatibility layout. Current
trees never define or synchronize a second payload copy.

Both installation paths recognize the same ownership sentinel and intentionally use
last-writer-wins replacement. The binary is the release-matched path; the standalone
installer remains the no-binary or explicitly ref-selected path and follows `main` by
default. This issue adds no update or version-arbitration mechanism.

The installer expands an agent target into one or two destination roots. It validates
all destinations and existing skill folders before acquiring destination locks. For
each target it creates a same-filesystem staging directory, writes only generated
relative paths with `create_new`, adds the existing `.bzr-skill-managed` sentinel,
and verifies `SKILL.md`. Replacement uses `target -> aside`, `stage -> target`, then
removes `aside`; failure of the second rename restores `aside`. Cleanup guards remove
only staging/aside/lock paths created by this process.

## Error behavior

- Missing scope is an input-validation error (exit 7), with expanded guidance only
  when stdin and stderr are terminals.
- A missing home directory, nonexistent/non-directory project path, symlinked path
  component, foreign folder, active lock, malformed embedded manifest, or filesystem
  failure is actionable and names the operation and safe target path.
- The implementation does not follow a symlink to inspect its sentinel.
- Preflight detects deterministic conflicts across every selected destination before
  writes begin. Unpredictable I/O failures can still yield a partial install; the
  error names completed skills and the failed destination.
- Clap owns invalid agent values and the `--global`/`--project` conflict.

## Threat model

### Boundary inventory

- **Added:** the local operator controls `--project <PATH>` and can point it at any
  accessible directory. The command resolves that path and creates agent directories
  beneath it.
- **Added:** repository-authored skill paths cross from compile-time content into
  runtime filesystem paths.
- **Widened:** existing `.bzr-skill-managed` directories may be replaced by either the
  standalone installer or the new command.
- **Existing:** the global home path comes from the platform directory provider.

### Actors and trust

The local operator and the checked-in source/build are trusted. Another local process,
a malicious repository checkout, or a CI workspace can race or pre-place symlinks and
foreign directories under a selected destination. Skill Markdown is treated as data by
the installer; the consuming agent may later interpret it, so only the payload compiled
from the trusted source tree is installed.

### Controls

- The build rejects absolute paths, parent traversal, symlinks, non-files, empty
  payloads, and skill directories without `SKILL.md`; runtime joins only those
  generated relative paths.
- Project roots must already exist as directories and are canonicalized. Every
  destination component below the trusted root is inspected with
  `symlink_metadata`; absent components are created one at a time and rechecked.
- Existing skill targets are inspected without following links and are replaceable
  only when a regular directory contains the ownership sentinel.
- An atomic lock-directory acquisition serializes cooperating installers per
  destination. An unexplained/stale lock fails closed with recovery guidance.
- Staging uses unpredictable process-local names plus `create_dir`, and file creation
  uses `create_new`. Replacement stays on the destination filesystem and restores the
  previous owned directory if activation fails.
- Errors display filesystem paths but never file contents, environment secrets, or
  unrelated directory entries.

### Out of scope

- Defending against a privileged process or the same user changing path components
  after validation is not possible with portable stable `std::fs` path APIs; the
  lock coordinates supported installers, not hostile processes.
- The command does not validate the semantic safety of repository-authored Markdown;
  source review and build provenance own that trust.
- ADR 0013 governs the standalone remote installer's TLS/GitHub trust and remains
  unchanged.

## Testing and proof

- Parser tests cover every agent value, both scopes, conflicts, and omitted scope.
- Build/manifest tests prove nested reference files are embedded, every skill has
  `SKILL.md`, paths are normalized, and `cargo package --list` includes canonical
  content.
- Filesystem unit tests use temporary fixtures for global/project mappings,
  idempotent replacement, foreign folders, target and ancestor symlinks, locks,
  nested files, staged failure cleanup, and missing/invalid project paths.
- Tests verify error paths leave foreign/original content untouched and that a
  replacement failure restores the prior owned directory.
- A functional phase runs the actual binary with an isolated project directory and
  home override, proves both layouts and nested files, exercises `all`, and proves
  non-interactive missing-scope refusal without contacting Bugzilla.
- Run `cargo fmt --check`, `cargo clippy --all-targets --features test-helpers --
  -D warnings`, `make check-test-layout`, `make check-no-spawn`, `cargo test
  --features test-helpers`, `make skills-test`, and `make functional-test-all`.

## Documentation

Update `docs/bzr-cli.md`, the root README agent-skills section,
`agent-skills/README.md`, and `CHANGELOG.md` in the implementation commit. Examples
always spell the scope. Documentation keeps the standalone installer for environments
where `bzr` is not installed and leads with the binary command when it is available.
