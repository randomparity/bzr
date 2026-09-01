# Platform naming implementation plan

Goal: make `platform` the canonical Bugzilla hardware field while carrying one release of
published and CLI compatibility. The existing layered CLI remains intact: CLI input builds
payload types, clients transport canonical names, domain types normalize responses, and
writers publish the versioned envelope.

Tech stack: Rust, clap, serde, JSON Schema, Bash functional harness.

## Global Constraints

- Host arm64 macOS; targets x86_64/aarch64 Linux, ppc64le, s390x, aarch64 macOS,
  and x86_64/aarch64 Windows; relationship different.
- Canonical name is `platform` on REST/XML-RPC reads and create/update writes.
- Schema version is 2.1.0 with `rep_platform` as a one-release published alias.
- CLI `--rep-platform` is deprecated for the same one-release transition and then removed.
- Do not edit `docs/adr/README.md`; campaign ownership leaves its row pending.
- Guardrails: focused `make test-one T=<substring>`; `make test-fast`; `make lint`;
  `make test`; mandatory `make functional-test-all`.

Expected implementation size: 180–340 changed lines (L) — derived from the domain,
payload/CLI, schema/documentation, fixture, and functional-test file map.

## Task 1: Prove and correct read/serialization behavior

Files: `src/client/resources/bug.rs`, `src/types/bug.rs`,
`src/types/bug/fields.rs`, `src/output/resources/bug.rs`,
`src/xmlrpc/resources/bug.rs`, and their sibling tests.

Interfaces: `Bug.platform: Option<String>` is consumed by clone/output; `BugField::Platform`
maps selection tokens to REST `platform`; `Serialize for Bug` emits canonical plus legacy
transition keys.

1. Change the compromised clone fixture response key to `platform` and run
   `make test-one T=clone_`; expect failure showing the source platform is absent.
2. Rename the domain/wire/adapter field and default request name; update field selection
   aliases and output access.
3. Add serialization/deserialization assertions for canonical and transition alias keys.
4. Run `make test-one T=clone_` and the relevant bug type/output tests; expect success.

## Task 2: Canonicalize create, update, clone, and search writes

Files: `src/types/bug/payload.rs`, `src/cli/bug/create.rs`,
`src/cli/bug/clone.rs`, `src/cli/bug/update.rs`, `src/commands/bug/create*.rs`,
`src/commands/bug/clone.rs`, `src/commands/bug/update/*.rs`,
`src/types/bug/search.rs`, and sibling tests.

Interfaces: `CreateBugParams.platform` and `UpdateBugParams.platform` serialize as the
Bugzilla external name; CLI structs expose `platform` with hidden `rep-platform` alias;
create JSON accepts legacy `rep_platform` for the transition.

1. Add failing payload/CLI tests that expect `platform`, update support, and legacy alias.
2. Implement the minimal field renames and update wiring.
3. Correct search mapping to `platform` throughout.
4. Run focused create/update/clone/search tests; expect success.

## Task 3: Cascade the published contract and functional proof

Files: `src/output/mod.rs`, `schemas/bug.json`, `schemas/bug-create-input.json`,
`schemas/bug-update-input.json`, parser/schema drift tests, `docs/bzr-cli.md`, `README.md`,
the embedded bzr-reference, dependency-analysis, and release-readiness skill consumers and
fixtures, every active runtime/test/functional `2.0.0` version pin found by bounded search,
and `tests/functional/phases/08-bugs.sh`, `10-bug-clone.sh`, `18a-json-envelope.sh`, and
`99-sequences.sh`. Historical ADR/spec examples remain unchanged.

Interfaces: all versioned envelopes report `2.1.0`; functional helpers read `.platform`
from actual command output on each matrix server.

1. Update schemas and contract examples to canonical `platform`, retaining only the
   explicitly deprecated output/input alias documentation. Encode at most one of the two
   create-input names for object and array-item forms, and test canonical-only, alias-only,
   and conflicting inputs.
2. Add functional create/view/update/clone readback assertions, force REST/Hybrid/XML-RPC
   platform comparison in the existing sequence, and update every active schema-version pin.
3. Run `make test-fast`, `make lint`, `make test`, and `make functional-test-all`; all
   commands must exit zero, with all three Bugzilla arms passing.
4. Review the diff for accidental template-persistence migration and leave it unchanged.
