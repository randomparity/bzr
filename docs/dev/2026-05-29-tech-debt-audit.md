# Technical-Debt Audit — bzr

_Date: 2026-05-29 · Branch: `refactor/tech-debt-hunt` · Scope: `src/`, `tests/`, doc-comments, `docs/`_

Read-only audit. 10 finders (6 module clusters + 4 global lenses) → adversarial per-finding
verification against source → synthesis. **44 raw findings → 27 confirmed, 16 rejected** as
false-positive/intentional. One verify agent failed to return a verdict, so one additional
candidate was dropped unverified (coverage gap; re-run to recover it).

## (A) Summary

The debt is overwhelmingly low-severity and concentrated in two themes: **code duplication**
and **magic-number / structural drift**. There is exactly one High-severity item.

Highest-impact themes (finding IDs reference the table below):

1. **CLI/docs contract drift (the only High finding).** `query save --search` is documented
   (`docs/bzr-cli.md:1274`) as mutually exclusive with the structured filter flags, but the
   `search` arg carries no `conflicts_with_all` constraint, so both can be supplied and both
   are sent (**TD-001**). It pairs directly with the *opposite* problem on the same command:
   `from_url` hardcodes a 16-entry `conflicts_with_all` list that must be hand-maintained
   (**TD-017**). The same screen has both a missing constraint and a brittle over-specified one.

2. **Input-validation gaps on date/limit fields.** Validation is applied per-field rather than
   uniformly: `--deadline` bypasses `parse_optional_date` while sibling `--created-since`
   correctly exits 7 (**TD-005**); URL `limit=` silently swallows `abc`, overflow, and `limit=0`
   (**TD-025**); and the century leap-year branches are untested (**TD-024**).

3. **Duplication clusters.** Near-identical patterns repeat: the `SearchParams`/`SavedQuery`
   filter-field accessor match (**TD-004**), the `fields`/`exclude_fields` arms across four
   `BugAction` variants (**TD-008**), `read_description_file`/`read_comment_file` (**TD-009**),
   `ColumnSpec` construction across the bug handlers (**TD-010**), per-resource "empty-check then
   write table" boilerplate (**TD-012**), and the `"expected struct response"` XML-RPC string in
   5 sites (**TD-013**).

4. **Magic numbers / no centralized constants.** Body-truncation widths 2048/512 (**TD-018**),
   the search default limit 50 (**TD-021**), display truncation widths 72/60/60 (**TD-022**), and
   a hardcoded `"─".repeat(60)` divider that ignores the existing `write_divider` helper
   (**TD-011**) all lack named constants and risk silent divergence.

5. **Lingering compat shims & silent error swallowing.** A pre-`pin_issuer_der` string-comparison
   fallback persists against the project's "no migration paths" rule (**TD-015**); `valid_login.rs`
   drops the TLS-specific diagnostics that `whoami.rs` provides (**TD-002**); `check_response_status`
   swallows body-read errors (**TD-019**); and non-atomic bug clone can leave a created bug
   unreported on comment failure (**TD-006**).

16 candidate findings were rejected during verification and are not included.

## (B) Findings

| ID | Category | Location | Description | Severity | Confidence | Suggested remediation | Effort |
|----|----------|----------|-------------|----------|------------|-----------------------|--------|
| TD-001 | 6-test-doc | `docs/bzr-cli.md:1274`, `src/cli/query.rs` (Save `search` arg) | `query save --search` lacks `conflicts_with_all`; docs claim mutual exclusivity with `--product`/`--component`/etc. Both can be supplied and both are sent at runtime, contradicting docs. | High | High | Add `conflicts_with_all = [...]` to the `search` arg, or remove the docs claim and document OR semantics. Add a clap negative test asserting `ArgumentConflict`. | M |
| TD-002 | 5-structural | `src/client/auth/valid_login.rs:86-91` | `probe_valid_login()` logs `req.send()` failures generically and skips the TLS cert detection (`is_tls_cert_error` + `tls_hint`) that `whoami.rs:49-56` performs, so users get no TLS guidance during auth probing. | Medium | High | Apply the same `is_tls_cert_error` + `tls_hint` logic from `whoami.rs:49-56` to the `valid_login.rs` network-error path. | S |
| TD-003 | 5-structural | `src/commands/bug/update.rs:99` | `build_update_params` is 97 lines, destructuring `BugAction::Update` into `UpdateBugParams` with 20+ field mappings; simple but hard to maintain as the variant changes. | Medium | Medium | Split into a destructuring step plus separate params construction, use a builder, or document the field-mapping strategy. | M |
| TD-004 | 3-duplication | `src/types/bug.rs:237-247`, `src/types/bug.rs:790-800` | `SearchParams` and `SavedQuery` each implement an identical ~15-line `get_filter_field` match over multi-value filters, differing only in `assigned_to` vs `assignee`. | Medium | High | Extract the shared match into a helper method or macro. | M |
| TD-005 | 6-test-doc | `src/commands/bug/update.rs:108,146` | `--deadline` is passed through without `parse_optional_date`, so `--deadline garbage` ships to the server whereas `--created-since garbage` correctly exits 7. | Medium | High | Apply `validation_parsing::parse_optional_date` to `deadline` before building `UpdateBugParams`; add a test asserting `InputValidation` exit 7. | S |
| TD-006 | 6-test-doc | `src/commands/bug/clone.rs:87-101` | Non-atomic clone: bug creation succeeds (line 87) but if the "Cloned from bug #N" comment POST fails, the created bug ID is never printed; user may re-clone and duplicate. | Medium | High | Capture the created bug ID; on comment failure print it with a warning. Add an integration test mocking a comment POST 500 after create succeeds, asserting the ID is printed. | M |
| TD-007 | 6-test-doc | `src/error_tests.rs:1-258` | No unit test for `BatchPartialFailure` exit code / `error_type` mapping, unlike all sibling variants (defined `src/error.rs:46`, handled at lines 180 and 200). | Medium | High | Add a test asserting `exit_code() == 11` and `error_type() == "batch_partial_failure"` for `BatchPartialFailure`. | S |
| TD-008 | 3-duplication | `src/cli/bug.rs:59-102, 187-210, 246-287, 447-485` (fields at 480-485) | `BugAction::List`, `View`, `Search`, and `My` each redefine identical `fields`/`exclude_fields` with near-identical doc comments. | Low | High | Use shared clap derive doc/reuse helpers or a custom derive helper to DRY the repeated field definitions. | M |
| TD-009 | 3-duplication | `src/commands/bug/create.rs:11`, `src/commands/bug/update.rs:43` | `read_description_file` and `read_comment_file` both wrap `fs::read_to_string` with identical error handling, differing only in message text. | Low | High | Extract a generic `read_file_with_context(path, context_name)` into `shared.rs`. | S |
| TD-010 | 3-duplication | `src/commands/bug/view.rs:84,104`, `src/commands/bug/search.rs:75`, `src/commands/bug/my.rs:27`, `src/commands/bug/list.rs:73` | Identical `ColumnSpec { include: fields.as_deref(), exclude: exclude_fields.as_deref() }` construction repeated across query handlers with no variation. | Low | High | Extract a `make_column_spec(fields, exclude_fields)` helper or macro. | S |
| TD-011 | 3-duplication | `src/output/resources/comment.rs:31` | Hardcoded `"─".repeat(60)` reimplements `write_divider` (`src/output/formatting.rs:69`), which is already exported and used in `bug.rs`. | Low | High | Import `write_divider` from `crate::output::formatting` and call it. | S |
| TD-012 | 3-duplication | `src/output/resources/user.rs:65-73`, `src/output/resources/product.rs:60-78`, `src/output/resources/group.rs` | `write_users`, `write_products`, and other resource writers repeat the same empty-check then map-to-rows then `writeln!` table boilerplate. | Low | High | Extract the empty-check-and-table-write pattern into a generic helper in `formatting.rs`. | S |
| TD-013 | 3-duplication | `src/xmlrpc/client.rs:174,236,284,347,441` | The error string `"expected struct response"` is duplicated across 5 sites (`get_group`, `extract_id`, `extract_bugs`, `lookup_bug_entry`, `extract_attachment_by_id`). | Low | High | Extract into a `const` or helper. | S |
| TD-014 | 4-deprecated | `Cargo.toml:47` | `toml = "0.8"` is two majors behind (0.9, 1.0, 1.1 shipped); 0.8 no longer receives fixes. `src/config.rs` is the sole consumer, so the upgrade surface is small. | Low | High | Bump to `toml = "1"` and run config round-trip tests; the serde API is stable across 0.8→1.x. | S |
| TD-015 | 4-deprecated | `src/tls/verifier.rs:109-119` | `check_issuer_change` keeps a string-comparison fallback (`pin_issuer`) for pins written before `pin_issuer_der` existed; all current write paths populate `pin_issuer_der`. A lingering compat shim against the "no migration paths" philosophy. | Low | High | Decide whether legacy on-disk pins must keep working; if not, drop the `pin_issuer` field/branch and rely on `pin_issuer_der`. | M |
| TD-016 | 4-deprecated | `Cargo.toml` (`dirs = "6"`) | `dirs` is on the current major but the dirs/directories family has a history of intermittent maintenance. Informational, not currently deprecated. | Low | High | No action now; monitor RUSTSEC/cargo-deny advisories at the next dependency audit. | S |
| TD-017 | 5-structural | `src/cli/query.rs:54` | `from_url` in `QueryAction::Save` hardcodes a 16-item `conflicts_with_all` string-literal list. Currently complete, but adding a filter field requires manual sync and can drift. | Low | High | Compute the conflict list dynamically from the variant's filter fields, or centralize via derive helpers. | M |
| TD-018 | 5-structural | `src/client/mod.rs:378,386,391` | Magic numbers 2048 (trace truncation) and 512 (debug/error) are hardcoded in `parse_body_to_value()`, inconsistent with `BODY_PREVIEW_MAX_BYTES = 512` defined at line 555. | Low | High | Define `BODY_TRACE_MAX_BYTES = 2048` alongside `BODY_PREVIEW_MAX_BYTES` and use both. | S |
| TD-019 | 5-structural | `src/client/mod.rs:526` | `check_response_status()` swallows body-read errors: `response.text().await.unwrap_or_else(|e| { warn!(...); String::new() })`, losing the actual HTTP error message on failure. | Low | High | Include the body-read error in the subsequent `ErrorResponse` check (line 535) or propagate it. | S |
| TD-020 | 5-structural | `src/commands/attachment.rs:153-157` | `guess_content_type` allocates a lowercase `String` per call via `map(str::to_lowercase)` before comparing against constant alias lists; `eq_ignore_ascii_case` is already used elsewhere (`src/url_parser.rs:41`). | Low | High | Use `eq_ignore_ascii_case` to avoid the per-call allocation. | S |
| TD-021 | 5-structural | `src/commands/bug/search.rs:43,104` | Magic number 50 (default search limit) appears twice as a literal in `build_params_from_url` and the quicksearch branch; documented in comments but no named constant. | Low | High | Define `const DEFAULT_SEARCH_LIMIT: u32 = 50` and reference it in both sites. | S |
| TD-022 | 5-structural | `src/output/resources/bug.rs:72`, `src/output/resources/product.rs:69`, `src/output/resources/classification.rs:24` | Three truncation widths (72, 60, 60) hardcoded in `truncate()` calls with no explanation of why bug summaries use 72 vs 60 for descriptions. | Low | High | Define named constants (e.g. `SUMMARY_TRUNCATE_WIDTH`, `DESCRIPTION_TRUNCATE_WIDTH`) in `formatting.rs` and use them consistently. | S |
| TD-023 | 6-test-doc | `src/commands/config.rs:410-425` | The unset-keyring path calls `save_config_without_validation`, skipping the `0o600`/`0o700` hardening `Config::save()` applies; a recreated config file could be world-readable. | Low | High | Call `Config::save()` or apply equivalent `0o600` hardening; add a Unix-only test forcing recreation and asserting `mode & 0o077 == 0`. | M |
| TD-024 | 6-test-doc | `src/validation/datetime.rs` | Date validation does not test century leap-year boundaries; `1900-02-29` (century non-leap) and `0000-01-01` are untested, leaving the century-rule branches mutation-undetected. | Low | High | Add tests asserting `1900-02-29` is `Err`, `2000-02-29` is `Ok`, and decide/test behavior for year 0000. | S |
| TD-025 | 6-test-doc | `src/url_parser.rs:162-166` | `limit=` silently accepts invalid/overflow values (`abc`, `99999999999`) by dropping them, and accepts `limit=0` verbatim even though Bugzilla treats 0 as "no limit"; behavior unvalidated and undocumented. | Low | High | Reject invalid limits with `InputValidation`; document `limit=0`. Add tests for `limit=abc`, `limit=0`, and overflow. | S |
| TD-026 | 6-test-doc | `src/main.rs:75`, `src/error.rs:102` | Stale comment states exit codes are `1..=12`, but `EXIT_CODE_TLS = 13` exists for `PinMismatch`/`IssuerChanged`. The comment contradicts reality. | Low | High | Update the comment to `1..=13`; add a test asserting exit code 13 for `PinMismatch` and `IssuerChanged`. | S |
| TD-027 | 6-test-doc | `docs/bzr-cli.md` | Minor doc/behavior drift: field-negation (`--whiteboard '!value'`) exact semantics and `--exclude-fields` with `--json` returning `{}` lack explicit examples. | Low | High | Spot-check 3-5 documented examples against code; add a regression test for the field-negation example. | M |

All six categories produced at least one confirmed finding; none were clean.

## Quick wins

Low-effort (S), low-risk items safe to clean up immediately:

- **TD-026** — Correct the stale `1..=12` exit-code comment (`src/main.rs:75`) to `1..=13`; pure comment fix matching existing `EXIT_CODE_TLS = 13`.
- **TD-011** — Replace the hardcoded `"─".repeat(60)` in `comment.rs:31` with the already-exported `write_divider`.
- **TD-013** — Hoist the duplicated `"expected struct response"` XML-RPC string into a `const`.
- **TD-018 / TD-021 / TD-022** — Introduce named constants for the body-truncation (2048/512), default search limit (50), and display truncation widths (72/60/60), replacing inline literals.
- **TD-020** — Swap `to_lowercase` for `eq_ignore_ascii_case` in `guess_content_type` to drop the per-call allocation.
- **TD-009** — Extract the shared `read_file_with_context` helper for the two near-identical file readers.
- **TD-010** — Extract a `make_column_spec` helper for the repeated `ColumnSpec` construction.
- **TD-007** — Add the missing `BatchPartialFailure` exit-code/`error_type` unit test.
- **TD-024** — Add the century leap-year date-validation tests (`1900-02-29`, `2000-02-29`).
- **TD-005** — Apply `parse_optional_date` to `--deadline` and add the exit-7 test (small, self-contained validation parity fix).
