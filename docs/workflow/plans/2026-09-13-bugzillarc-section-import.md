# Section-only bugzillarc import — implementation plan

**Goal.** Safely import conventional explicit URL section-only and multi-server
bugzillarc files without broadening credential routing.

**Architecture.** `import_bugzillarc.rs` remains the single resolution owner:
its DEFAULT URL path retains exact-authority matching, while its section-only
path admits only parseable absolute HTTP(S) section names without inheriting
DEFAULT credentials. The config phase invokes
the compiled CLI against an isolated config. Stack: Rust 1.89.0, Bash, Docker
or podman for the live suite.

**Global Constraints.** Preserve API-key-only import; do not import usernames,
passwords, or certificates. Never route credentials by arbitrary substring.
Use sibling test files, Writers output, and the existing functional phase.

Expected implementation size: 70–130 changed lines (M) — resolver branching,
direct tests, one functional scenario, and concise CLI documentation.

**File map.** Modify `src/commands/config/import_bugzillarc.rs` (resolution),
`src/commands/config/import_bugzillarc_tests.rs` (contracts),
`tests/functional/phases/01-config.sh` (compiled binary proof), and
`docs/bzr-cli.md` (accepted configuration form).

## Task 1 — resolve explicit URL sections safely

**Interfaces.** `resolved_servers` consumes parsed sections and returns only
valid `ImportedServer` records. It keeps the DEFAULT-url route unchanged and
adds the no-DEFAULT explicit-URL-section route.

**Verification.** Contract: no-DEFAULT explicit URL sections resolve in
deterministic order; a host or substring section does not. Mode:
`focused-test`, `make test-one T=import_bugzillarc`. Write the failing
section-only test before implementation, then prove it passes.

**Steps.**

1. Add resolver tests for two explicit URL sections, DEFAULT inheritance, and
   non-URL/invalid section errors.
2. Implement the minimal URL-section resolver and retain the legacy branch.
3. Run focused tests and commit the implementation with its tests.

**Acceptance.** Each imported API key originates only from the explicit URL
section for that URL or bounded DEFAULT inheritance; no arbitrary section wins.

## Task 2 — prove and publish the migration behavior

**Interfaces.** The config functional phase consumes a temporary rc file and
isolated config; CLI docs describe exactly the supported forms.

**Verification.** Contract: the compiled binary imports two URL sections and
does not persist fixture password data. Mode: `focused-test` in phase 01,
followed by `make functional-test`. Documentation is verified by existing
`make lint` checks; no narrower executable doc consumer exists.

**Steps.**

1. Add one isolated section-only multi-server functional scenario.
2. Document DEFAULT-url and explicit URL-section forms plus unresolvable
   non-URL sections.
3. Run lint, full tests, and the live functional tier; commit the proof/docs.

**Acceptance.** The live phase reports two imports, contains both URL records,
and leaves no password fixture data in config.
