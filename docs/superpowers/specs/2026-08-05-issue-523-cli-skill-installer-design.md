# Issue 523: CLI Skill Installer Design

## Scope authority

- **Interaction:** interactive
- **Scope identity:** https://github.com/randomparity/bzr/issues/523; token
  `9A8D4DF9-AE06-43D3-8F6F-79DCEDEB841F`
- **Outcome:** add `bzr skills install` so bundled bzr skills can be installed for
  supported agents at an explicitly selected global or project location.
- **Provenance:** issue #523; the user's 2026-08-05 choice of explicit scope;
  repository instructions; accepted ADR 0018, which supersedes ADR 0013 and carries
  forward the standalone fetch policy.
- **Exclusions:** uninstall, update, list, project-root discovery, new agent layouts,
  standalone no-binary installer behavior beyond directly required canonical-source
  path compatibility, and changes to ADR 0018's carried-forward remote-fetch trust
  policy.
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
   directory is moved aside. A failed replacement attempts to restore that directory.
   A failure after another skill was installed reports the partial outcome; valid
   installed skills are not rolled back.
8. User-facing output goes through `Writers`. Table output names each installed skill
   and destination. JSON-family success output has the exact shape defined below.
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

The installer expands an agent target into one or two destination roots. It performs a
read-only preliminary validation, acquires every destination lock in sorted path order,
then repeats the authoritative symlink, ownership-sentinel, and conflict checks while
holding all locks. It uses the existing POSIX installer's `.bzr-skill.lock` convention,
but guarantees serialization only among binary invocations. The standalone POSIX
cleanup can remove a lock it did not acquire, and changing that behavior is excluded
from this issue. Each target is checked once more immediately before its first rename.
All standalone versions are therefore uncoordinated writers; concurrent use with them
is unsupported and receives the same best-effort checks as any other same-user race.

For each target the command creates a same-filesystem staging directory, writes only
generated relative paths with `create_new`, adds the existing
`.bzr-skill-managed` sentinel, and verifies `SKILL.md`. Replacement uses
`target -> aside`, `stage -> target`, then removes `aside`. Cleanup code removes only
staging, aside, and lock paths created and still owned by this process.

### Replacement failure states

| Failure | Authoritative content | Residual and behavior | Report |
|---|---|---|---|
| staging/write/validation | existing target, if any | remove stage; do not rename target; stop | failed, plus stage path if cleanup fails |
| `target -> aside` | existing target | remove stage; stop | failed, original untouched |
| `stage -> target` with no prior target | no installed target | remove stage; if cleanup fails retain it only as a residual | nonzero, empty stdout, failed path plus residual stage path when present |
| `stage -> target`, restore succeeds | restored existing target | remove stage; stop | failed, original restored |
| `stage -> target`, restore fails | aside holds previous content; target absent | retain aside and stage; stop immediately | failed with both recovery paths; never call it restored |
| aside removal after activation | new target | retain aside; continue because activation succeeded | exit 0; normal success output plus warning naming residual aside |
| stage cleanup after a failure | state from the preceding row | retain stage; stop | primary failure plus residual stage path |
| lock release | all completed target states remain authoritative | retain lock; no more writes | exit 0; normal success output plus warning naming lock and safe recovery check |

A test-only filesystem-operation seam injects rename, restore, removal, and lock-release
failures. It exists only to prove these user-visible states and is not a reusable runtime
abstraction.

## Output contract

On success, JSON emits the normal `schema_version` envelope and NDJSON emits its bare
`data` object as one compact line. The data object is:

```json
{
  "action": "install",
  "agent": "all",
  "scope": "project",
  "project": "/canonical/project/path",
  "destinations": [
    {
      "layout": "agents",
      "path": "/canonical/project/path/.agents/skills",
      "installed": ["bzr-bulk-triage", "bzr-file-bug"]
    },
    {
      "layout": "claude",
      "path": "/canonical/project/path/.claude/skills",
      "installed": ["bzr-bulk-triage", "bzr-file-bug"]
    }
  ]
}
```

`agent` is the requested clap value. `scope` is `global` or `project`; `project` is
`null` for global installs and otherwise the canonical absolute root. Destinations are
deduplicated and ordered `agents`, then `claude`; skill names are complete and sorted
lexicographically. Each destination path is canonical-root-based and absolute. The
example abbreviates the installed array; real success contains every embedded skill.

On a failure that prevents activation or leaves authoritative content unresolved,
stdout is empty: the command does not emit a success object for a partial install. The
standard stderr error contains the failed operation/path and a deterministically ordered
list of already installed skill/destination pairs. JSON-family error output remains the
repository's existing structured error envelope; partial detail is in `message`, and
this issue adds no error-schema keys. Once activation succeeds, aside-removal and final
lock-release cleanup failures are warnings: the command exits 0, emits the complete
normal success result, and writes deterministic recovery guidance to stderr.

## Error behavior

- Missing scope is an input-validation error (exit 7), with expanded guidance only
  when stdin and stderr are terminals.
- A missing home directory, nonexistent/non-directory project path, symlinked
  destination component below the selected root, foreign folder, active lock,
  malformed embedded manifest, or filesystem failure is actionable and names the
  operation and safe target path.
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
- **Existing:** the global home path comes from the platform directory provider, which
  may consult process environment such as Unix `HOME`.

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
- Project roots must already exist as directories and are canonicalized. An explicitly
  supplied symlink alias for the root is accepted and output uses the canonical absolute
  path. Every component created or traversed below that trusted root is inspected with
  `symlink_metadata`; `.agents`, `.claude`, `skills`, and skill targets may not be
  symlinks. The provider-supplied global home must be absolute and resolve to an existing
  directory, then is canonicalized once before destinations are derived. A home symlink
  alias is accepted, and installation plus output both use that same canonical path;
  relative, missing, and non-directory homes fail before filesystem mutation.
- Existing skill targets are inspected without following links. A target is owned only
  when it is a regular directory containing a regular, non-symlink
  `.bzr-skill-managed` file that parses exactly one nonempty `managed-by`,
  `installed-skill`, `source-version`, and `source-commit` field. `managed-by` must be
  `bzr-skill`, and `installed-skill` must equal the target directory name. Unknown extra
  fields are tolerated for forward compatibility; duplicate fields are malformed.
  Missing, unreadable, malformed, empty, mismatched, directory, or symlink sentinels
  make the target foreign and untouchable. This is the binary's read rule; the existing
  standalone installers continue writing the compatible four-field sentinel without
  changing their ownership-read behavior.
- Sentinel bytes must be UTF-8 with an optional leading UTF-8 BOM. LF and CRLF line
  endings are accepted, as is one trailing newline. Each nonempty line is exactly
  `key: value`; required keys and values are ASCII, values are not trimmed beyond the
  format's single separator space, and malformed lines make the sentinel foreign.
  This accepts the exact current POSIX and Windows PowerShell output while keeping
  duplicate/missing/value checks strict. A sentinel is at most 16 KiB: the current four
  fields use well under 1 KiB, so this leaves room for many forward-compatible fields
  without allowing an ownership claim to drive an unbounded allocation. Metadata over
  the limit is rejected before opening; otherwise the parser reads at most 16 KiB plus
  one byte and rejects that extra byte so file growth between inspection and reading
  cannot bypass the bound.
- Atomic lock-directory acquisition in deterministic path order serializes binary
  invocations per destination.
  Authoritative checks run only after all locks are held and immediately before rename.
  A recoverable stale lock has exactly one entry: a regular, non-symlink `pid` file
  containing a numeric PID. Any additional file, directory, or symlink makes the lock
  unexplained, so the command fails closed with recovery guidance and leaves the lock
  unchanged. Recovery rechecks this exact shape before detaching the stale lock and
  removes the PID and now-empty directory individually rather than recursively deleting
  the detached path.
- Staging uses unpredictable process-local names plus `create_dir`, and file creation
  uses `create_new`. Replacement stays on the destination filesystem and restores the
  previous owned directory if activation fails.
- Errors display filesystem paths but never file contents, environment secrets, or
  unrelated directory entries.

### Out of scope

- Defending against a privileged process or the same user changing path components
  after validation is not possible with portable stable `std::fs` path APIs; the
  lock coordinates binary invocations, not standalone or hostile processes.
- Concurrent installation by any standalone installer is unsupported. The current POSIX
  cleanup can remove a lock it failed to acquire, while PowerShell does not acquire this
  lock. The binary rechecks immediately before rename but cannot promise race-free
  replacement against either uncoordinated same-user writer.
- The command does not validate the semantic safety of repository-authored Markdown;
  source review and build provenance own that trust.
- ADR 0018 governs and carries forward ADR 0013's standalone remote-fetch TLS/GitHub
  trust policy unchanged.

## Testing and proof

- Parser tests cover every agent value, both scopes, conflicts, and omitted scope.
- A focused command/output test injects interactive stdin/stderr terminal status and
  invokes omitted scope. It asserts exit 7, both resolved destination patterns,
  copyable `--global` and `--project .` examples, and no filesystem writes. Scope
  guidance receives no stdin reader, proving the command presents choices without
  reading a selection. The complementary non-interactive test asserts concise examples
  and no interactive destination expansion.
- Build/manifest tests prove nested reference files are embedded, every skill has
  `SKILL.md`, paths are normalized, and `cargo package --list` includes canonical
  content.
- Filesystem unit tests use temporary fixtures for global/project mappings,
  idempotent replacement, foreign folders, target and ancestor symlinks, locks,
  nested files, a project-root symlink alias, state changes before lock acquisition,
  staged failure cleanup, and missing/invalid project paths.
- A filesystem-seam race test changes an owned or absent target into a byte-distinct
  foreign directory after locks and the ordinary authoritative pass but immediately
  before the first rename. The final recheck must refuse and preserve those bytes.
- Tests verify error paths leave foreign/original content untouched and that a
  replacement failure restores the prior owned directory. Fault injection covers
  activation failure with and without a prior target, failed restore, post-activation
  aside cleanup, stage cleanup, and lock release, asserting the state table above.
- Ownership tests cover missing/wrong/duplicate `managed-by`, empty and malformed
  sentinels, duplicate/missing fields, wrong-skill names, unreadable sentinels where the
  platform permits, and sentinel directories and symlinks. Each case leaves the
  original tree byte-for-byte intact.
- Compatibility tests create sentinels through the real POSIX installer and, when
  PowerShell is available, the real PowerShell installer, then prove the binary accepts
  and replaces them. Checked byte fixtures cover BOM+CRLF PowerShell output even on
  hosts without PowerShell.
- A functional phase runs the actual binary with an isolated project directory and
  home override, proves both layouts and nested files, exercises `all`, and proves
  non-interactive missing-scope refusal without contacting Bugzilla.
- Unit and functional assertions pin the full JSON and NDJSON success shapes,
  deterministic ordering, canonical project path, global `project: null`, `all`
  destination deduplication, stdout-empty partial failure, and stderr recovery detail.
  Aside-removal and lock-release warning cases explicitly assert exit 0, the normal
  success object, and the warning; activation/restore failures assert nonzero and empty
  stdout.
- Cross-process tests pin binary-to-binary `.bzr-skill.lock` serialization:
  `bzr_lock_helper` acquires the binary's real guard while a real binary invocation must
  refuse it without deleting the live lock. Protocol fixtures cover missing, transient
  empty, live, and dead PID files. An `all` contention test proves a later lock failure
  releases every earlier lock.
- Run `cargo fmt --check`, `cargo clippy --all-targets --features test-helpers --
  -D warnings`, `make check-test-layout`, `make check-no-spawn`, `cargo test
  --features test-helpers`, `make skills-test`, and `make functional-test-all`.

## Documentation

Update `docs/bzr-cli.md`, the root README agent-skills section,
`agent-skills/README.md`, and `CHANGELOG.md` in the implementation commit. Examples
always spell the scope. Documentation keeps the standalone installer for environments
where `bzr` is not installed and leads with the binary command when it is available.
