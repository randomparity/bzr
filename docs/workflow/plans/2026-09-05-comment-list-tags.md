# Implement comment-list tag output

Expose the tags already returned by Bugzilla's comment endpoint through the existing
comment-list domain, projection, output, and schema paths. Keep one REST/XML-RPC request per
list operation; normalize omitted tags to an empty array and preserve server order.

Tech stack: Rust 2021, serde/serde_json, the existing XML-RPC protocol adapter, clap, JSON
Schema draft 2020-12, Bash functional tests, and real Bugzilla containers.

Expected implementation size: 110–170 changed lines (M) — derived from three small runtime
edits, focused fixtures/assertions, one schema property, documentation, and mechanical schema
version pin updates.

## Global constraints

- Branch: `feat/comment-list-tags-700`; `BASE_BRANCH=main`; issue #700; scope token
  `q700-3d353b4e`.
- Host architecture is arm64 with BSD userland. Declared targets are x86_64, aarch64,
  powerpc64le, and s390x; the host architecture is included.
- `Comment.tags` is `Vec<String>` with `#[serde(default)]`; full JSON/NDJSON always emits
  `tags`, including `[]`.
- REST uses the shared serde type. XML-RPC uses the existing
  `get_str_array(&BTreeMap<String, Value>, &str) -> Vec<String>` helper and retains its
  lenient optional-array behavior.
- Table output prints `  Tags: <values joined by ", ">` only when tags are non-empty.
- Preserve server tag order and spelling; do not sort, normalize, or deduplicate.
- The additive payload change advances `SCHEMA_VERSION` exactly from `3.0.0` to `3.0.1`
  under ADR 0007, including all live pinned consumers. Historical ADR/spec text is not
  rewritten.
- Add no dependency, endpoint, auth behavior, configuration, or per-comment request.
- Rust test modules remain sibling `*_tests.rs` files. User output goes through `Writers` and
  writer helpers, never `println!`/`eprintln!`.
- Iteration commands: `make test-one T=<name-substring>`. Pre-commit guardrails:
  `make lint` and `make test`. Before PR: `make functional-test-all`.
- ADR index coupling: not coupled to an individually hard-gated check; this solo run already
  added the ADR 0048 row.

## File map

- Modify `src/types/comment.rs` and `src/types/comment_tests.rs`: domain field, projection
  allowlist, REST-shaped deserialization/serialization contracts.
- Modify `src/xmlrpc/resources/comment.rs` and `comment_tests.rs`: XML-RPC tag mapping and
  focused response coverage.
- Modify `src/output/resources/comment.rs` and `comment_tests.rs`: table tag metadata and JSON
  writer coverage.
- Modify `src/commands/schema_tests.rs` and `schemas/comment.json`: closed-schema conformance.
- Modify `src/cli/comment.rs`, `docs/bzr-cli.md`, and
  `tests/functional/phases/15-comments.sh`: executable help, reference, and live round trip.
- Modify `src/output/mod.rs`, `src/output/resources/bug_tests.rs`, `README.md`, live
  `docs/bzr-cli.md` version examples, `content/skills/bzr-reference/reference/{commands.md,
  json-recipes.md}`, `content/skills/bzr-dependency-analysis/scripts/collect.py`, its current
  tests/fixtures, and functional version assertions: schema version `3.0.1` lockstep.

## Task 1: Carry tags through the comment domain and transports

**Interfaces**

- Consumes `Comment` in `src/types/comment.rs` and
  `get_str_array(&BTreeMap<String, Value>, &str) -> Vec<String>` from
  `src/xmlrpc/resources/mappers.rs`.
- Produces `pub tags: Vec<String>` and adds the literal `"tags"` to `COMMENT_FIELDS`.
- Later tasks rely on `Comment.tags` always serializing and omitted input defaulting to `[]`.

**Verification**

- Mode: focused-test — REST-shaped deserialization and serialization. Add
  `comment_deserializes_tags` and extend `comment_deserializes_minimal`; before implementation
  `make test-one T=comment_deserializes_tags` must fail to compile because `Comment` has no
  `tags`, then pass after the field is added.
- Mode: focused-test — XML-RPC mapping. Add tags to
  `xmlrpc_get_comments_since_parses_full_response` and add
  `value_to_comment_without_tags_defaults_empty`; before mapping,
  `make test-one T=xmlrpc_get_comments_since_parses_full_response` must fail its tags
  assertion, then pass.
- Mode: focused-test — projection key registration. Existing
  `comment_fields_matches_serialized_keys` must fail after the field is added but before
  `COMMENT_FIELDS` changes; `make test-one T=comment_fields_matches_serialized_keys` passes
  after registration.

**Steps**

1. In `src/types/comment_tests.rs`, assert a minimal comment has `comment.tags.is_empty()` and
   serializes `"tags": []`; add a present-array case that preserves
   `vec!["needs-info", "follow-up"]`. Add `tags: vec![]` to all direct `Comment` fixtures in
   `src/types/comment_tests.rs`, `src/output/resources/comment_tests.rs`, and
   `src/commands/bug/history_tests.rs`. Run the named type test and retain the expected red
   compile failure.
2. In `src/types/comment.rs`, add:

   ```rust
   #[serde(default)]
   pub tags: Vec<String>,
   ```

   Add `"tags"` to `COMMENT_FIELDS`. Run the two named type/projection commands; expect pass.
3. In `src/xmlrpc/resources/comment_tests.rs`, add a `tags` XML-RPC member containing
   `needs-info` and `follow-up` to the full-response fixture and assert exact order. Add the
   absent-field test. Run the full-response test and retain the expected red assertion.
4. Import `get_str_array` in `src/xmlrpc/resources/comment.rs` and initialize
   `tags: get_str_array(m, "tags")` in `value_to_comment`. Run
   `make test-one T=xmlrpc_get_comments_since`; expect all matching tests pass.
5. Run `make test-one T=comment_`; expect every matching comment unit test passes. Commit as
   `feat(comment): carry tags through comment responses`.

## Task 2: Publish and present the additive contract

**Interfaces**

- Consumes `Comment.tags` and the existing `write_formatted_projected` serialization path.
- Produces the table line `  Tags: {c.tags.join(", ")}`, required schema property `tags`,
  and schema version `3.0.1` in every live version consumer.
- Task 3 relies on the writer, projection allowlist, help text, and versioned JSON envelope.

**Verification**

- Mode: focused-test — table/JSON presentation. Add
  `write_comments_table_renders_tags`, assert untagged output omits `Tags:`, and extend the
  JSON writer test; `make test-one T=write_comments_table_renders_tags` is red before the
  writer edit and green after it.
- Mode: focused-test — closed JSON schema. Add tags to the maximal comment sample and schema
  property; `make test-one T=comment_conforms` is red after only the sample changes and green
  after `schemas/comment.json` changes.
- Mode: focused-test — additive version pin. Rename the exact adjacency version test to
  `write_bug_adjacency_json_uses_schema_version_3_0_1`, change its expected literal first,
  and run `make test-one T=write_bug_adjacency_json_uses_schema_version_3_0_1`; it is red
  before `SCHEMA_VERSION` changes and green after all Rust pins change.

**Steps**

1. Add the writer tests described above. In `src/output/resources/comment.rs`, before the
   existing blank line, add:

   ```rust
   if !c.tags.is_empty() {
       let _ = writeln!(out, "  {} {}", "Tags:".bold(), c.tags.join(", "));
   }
   ```

   Run the named writer test; expect pass.
2. Add `"tags": ["needs-info"]` to the comment conformance sample and run its test; retain
   the expected schema-drift failure. Add this property to `schemas/comment.json` and append
   `"tags"` to its required list:

   ```json
   "tags": {
     "type": "array",
     "items": { "type": "string" }
   }
   ```

   Run `make test-one T=comment_conforms`; expect pass.
3. Restore `tags` in the printed-field sentence in `src/cli/comment.rs`. In the comment-list
   section of `docs/bzr-cli.md`, state that table output includes tags and add a
   `--fields id,tags` JSON example. Do not change tag mutation/search semantics.
4. Change `src/output/mod.rs::SCHEMA_VERSION` and the current exact `3.0.0` pins enumerated in
   the file map to `3.0.1`; rename version-specific test identifiers. Use one focused `rg`
   check to ensure no live (non-historical) `3.0.0` pin remains. Run the named Rust version
   test and `BZR_BIN="$PWD/target/debug/bzr" sh agent-skills/tests/run.sh`; expect pass.
5. Run `make test-one T=comment_`, then `make lint` and `make test`; expect green. Commit as
   `feat(comment): show tags in list output` so the compiled artifact change enters release
   notes.

## Task 3: Prove the live round trip on supported Bugzilla versions

**Interfaces**

- Consumes the existing `COMMENT_ID` produced by `comment-add-first`, `run_bzr`, and JSON
  assertion helpers in `tests/functional/phases/15-comments.sh`.
- Produces functional cases for help truth, full JSON tags, `--fields tags`, and an empty tag
  after removal.
- No later implementation task depends on this task; it is the end-to-end proof.

**Verification**

- Mode: focused-test — live tag round trip and projection. Extend phase 15 so the added tag is
  read before removal. A pre-implementation `make functional-test` must fail the new JSON tag
  assertion; after Tasks 1–2 it passes on the default image.
- Mode: focused-test — supported-version behavior. `make functional-test-all` must report
  bz50, bz52, and bz53 passed.

**Steps**

1. Update `comment-list-help-matches-output` to require `tags`. After `comment-tag-add`, run
   `comment list "$BUG1"`, select the entry with `COMMENT_ID`, and assert its `.tags` equals
   `["important"]`. Run again with `--fields tags` and assert the projected object has exactly
   the `tags` key and the same value. After removal, assert the same comment emits `tags: []`.
2. Verify the new functional assertion bites: temporarily change its expected tag to a value
   the fixture never writes, run `make functional-test`, and retain the phase-15 failure.
   Restore the assertion and rerun; expect every default-version case green.
3. Run `make functional-test-all`; expect the multi-version summary to report `bz50: PASSED`,
   `bz52: PASSED`, and `bz53: PASSED` with exit 0.
4. Re-run `make lint` and `make test`; expect green. Review `git diff main...HEAD` for scope,
   naming, and generated-version drift. Commit any functional-only change as
   `test(comment): prove tag round trip on Bugzilla`.

## Resume facts

- Current phase after this plan: design review, then oathbind scope audit, then forge.
- Routed review depth: iterating.
- Design review prior rounds: `0/0`.
- Open findings: none before review.
- Review deferrals: none before review.
- Guardrails: `make lint`, `make test`, `make functional-test-all`; focused
  `make test-one T=<name-substring>`.
