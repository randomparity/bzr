# Goal

Remove expired bug platform compatibility aliases at schema major 3.0.0.

## Architecture

Canonical `platform` stays at the bug domain/CLI boundary. Compatibility removal is
implemented at the existing serialization, serde, field-token, and clap seams; templates
retain their separate persisted model. Schemas and consumers are updated in lockstep.

## Tech stack

Rust/serde/clap, JSON Schema, shell functional phases.

## Global Constraints

- Preserve template persisted-config `rep_platform` per ADR 0034.
- `SCHEMA_VERSION` follows ADR 0007 and becomes `3.0.0` for this breaking removal.
- User-facing changes require functional phase coverage and full functional verification.

Expected implementation size: 120–220 changed lines (L) — synchronized source, schema, documentation, fixture, and functional-consumer updates.

## Tasks

1. Remove compatibility seams in `src/output/mod.rs`, `src/types/bug.rs`,
   `src/types/bug/fields.rs`, `src/commands/bug/create_json.rs`,
   `src/cli/bug/create.rs`, and `src/cli/bug/clone.rs`; update sibling tests so canonical
   input/output succeeds and removed CLI/JSON/field spellings fail. Verify with
   `make test-one T=platform` and `make test-fast`.
2. Update `schemas/bug.json` and `schemas/bug-create-input.json` for 3.0.0 removal and
   update schema/version assertions. Verify schema drift tests with `make test-one T=schema`.
3. Replace deprecated bug spellings in `docs/bzr-cli.md`, embedded skill references,
   README examples, fixtures, and functional phases; add phase assertions for rejected
   old spellings and canonical success. Verify with `make lint`, `make test`, and
   `make functional-test-all`.

Rollback is a normal git revert; template configuration is not migrated.
