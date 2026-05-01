# Mutation Testing Baseline

This file tracks mutation-testing coverage per source file. It is the ratchet
target for the multi-wave rollout — every PR should leave the per-file score
≥ the value listed here. New sweeps update the numbers; never drop them
silently.

- **Tool:** `cargo-mutants` (currently `27.0.0`)
- **Config:** [`.cargo/mutants.toml`](../../.cargo/mutants.toml)
- **Baseline date:** 2026-05-01
- **Total mutants generated:** 1030 across 75 source files

## How to read this file

For each file we record one of:

- `swept` — the file has been worked through; counts are real outcomes
  (`caught` / `missed` / `unviable` / `timeout`)
- `pending` — only the inventory count is known; no run yet

A "score" is `caught / (caught + missed)`. Unviable mutants (won't compile
because they require a `Default` impl etc.) and timeouts are reported but do
not count against the score. Files with a target lower than 95 % carry an
explicit justification in their row.

## Workflow for a single file

```bash
# 1. Produce the report. --jobs caps how many cargo build+test pipelines run
#    in parallel; cargo-mutants warns above 8. Use a lower value if other work
#    is competing for the host. The Makefile defaults to 4 and can be
#    overridden via the MUTANTS_JOBS env var.
cargo mutants --file <path> --jobs 4

# 2. Inspect outcomes
cat mutants.out/missed.txt
cat mutants.out/unviable.txt
cat mutants.out/timeout.txt

# 3. For each entry in missed.txt:
#    a) Add a unit test that fails when the mutation is applied, OR
#    b) If the mutation is semantically equivalent, add `// mutants::skip`
#       on the offending line with a comment explaining why, OR
#    c) If a class of mutants is genuinely out of scope (e.g. log strings,
#       Display impls), broaden `exclude_re` in `.cargo/mutants.toml`
#
# 4. Re-run until missed.txt is acceptable. Update this file with the new
#    score and bump the "last swept" date.
```

To replay a single missed mutant locally:

```bash
cargo mutants --file <path> --in-place --no-shuffle --line-col <line>:<col>
```

## Sweep order

Modules are tackled in this order (highest leverage first). See the rollout
plan for rationale.

1. **Wave 1** — pure parsers: `url_parser.rs`, `commands/flags.rs`,
   `xmlrpc/parsing.rs`, `client/version.rs`
2. **Wave 2** — boundaries: `error.rs`, `config.rs`, `http.rs`,
   `tls/verifier.rs`, `tls/tofu.rs`
3. **Wave 3** — `client/auth/**`, `client/{bug,attachment,comment,user,group,
   product,component,classification,field,server}.rs`, `client/mod.rs`
4. **Wave 4** — `commands/**`
5. **Wave 5** — `output/**` (adopt `insta` snapshot tests before sweeping)
6. **Wave 6** — `cli/**`, `types/**`, `main.rs`, `lib.rs`

## Per-file inventory

Largest-first. `–` means the wave hasn't started yet.

| File | Mutants | Status | Caught | Missed | Unviable | Timeout | Score | Last swept |
|------|--------:|--------|-------:|-------:|---------:|--------:|------:|-----------|
| src/tls/verifier.rs                 | 102 | swept   | 96 | 0 |  6 | 0 | 100 % | 2026-05-01 |
| src/xmlrpc/parsing.rs               |  81 | swept   | 57 | 0 | 11 | 0 | 100 % | 2026-05-01 |
| src/types/bug.rs                    |  77 | pending | – | – | – | – | – | – |
| src/commands/shared.rs              |  49 | pending | – | – | – | – | – | – |
| src/config.rs                       |  47 | swept   | 33 | 0 |  5 | 0 | 100 % | 2026-05-01 |
| src/xmlrpc/client.rs                |  42 | pending | – | – | – | – | – | – |
| src/client/mod.rs                   |  41 | swept   | 30 | 0 | 10 | 0 | 100 % | 2026-05-01 |
| src/client/bug.rs                   |  38 | swept   | 28 | 0 | 10 | 0 | 100 % | 2026-05-01 |
| src/tls/tofu.rs                     |  36 | swept   | 28 | 0 |  4 | 0 | 100 % | 2026-05-01 |
| src/output/formatting.rs            |  34 | pending | – | – | – | – | – | – |
| src/xmlrpc/mod.rs                   |  28 | pending | – | – | – | – | – | – |
| src/url_parser.rs                   |  25 | swept   | 23 | 0 | 2 | 0 | 100 % | 2026-05-01 |
| src/error.rs                        |  19 | swept   | 17 | 0 |  1 | 0 | 100 % | 2026-05-01 |
| src/commands/query.rs               |  19 | pending | – | – | – | – | – | – |
| src/commands/config.rs              |  18 | pending | – | – | – | – | – | – |
| src/commands/flags.rs               |  17 | swept   | 10 | 0 |  7 | 0 | 100 % | 2026-05-01 |
| src/client/version.rs               |  17 | swept   | 13 | 0 |  4 | 0 | 100 % | 2026-05-01 |
| src/types/common.rs                 |  16 | pending | – | – | – | – | – | – |
| src/main.rs                         |  16 | pending | – | – | – | – | – | – |
| src/commands/attachment.rs          |  16 | pending | – | – | – | – | – | – |
| src/output/query.rs                 |  13 | pending | – | – | – | – | – | – |
| src/commands/bug/list.rs            |  13 | pending | – | – | – | – | – | – |
| src/client/group.rs                 |  13 | swept   | 12 | 0 |  1 | 0 | 100 % | 2026-05-01 |
| src/commands/comment.rs             |  12 | pending | – | – | – | – | – | – |
| src/client/attachment.rs            |  12 | swept   | 10 | 0 |  2 | 0 | 100 % | 2026-05-01 |
| src/http.rs                         |  11 | swept   | 10 | 0 |  2 | 0 | 100 % | 2026-05-01 |
| src/commands/bug/my.rs              |  11 | pending | – | – | – | – | – | – |
| src/client/auth/valid_login.rs      |  11 | swept   |  8 | 0 |  3 | 0 | 100 % | 2026-05-01 |
| src/output/result_types.rs          |  10 | pending | – | – | – | – | – | – |
| src/commands/bug/update.rs          |  10 | pending | – | – | – | – | – | – |
| src/commands/bug/search.rs          |  10 | pending | – | – | – | – | – | – |
| src/client/comment.rs               |  10 | swept   |  9 | 0 |  1 | 0 | 100 % | 2026-05-01 |
| src/output/config.rs                |   9 | pending | – | – | – | – | – | – |
| src/commands/template.rs            |   9 | pending | – | – | – | – | – | – |
| src/client/user.rs                  |   9 | swept   |  6 | 0 |  3 | 0 | 100 % | 2026-05-01 |
| src/xmlrpc/call.rs                  |   8 | pending | – | – | – | – | – | – |
| src/client/auth/mod.rs              |   8 | swept   |  6 | 0 |  1 | 0 | 100 % | 2026-05-01 |
| src/output/template.rs              |   7 | pending | – | – | – | – | – | – |
| src/output/product.rs               |   7 | pending | – | – | – | – | – | – |
| src/credentials/keyring.rs          |   7 | pending | – | – | – | – | – | – |
| src/client/field.rs                 |   7 | swept   |  4 | 0 |  3 | 0 | 100 % | 2026-05-01 |
| src/output/bug.rs                   |   6 | pending | – | – | – | – | – | – |
| src/client/product.rs               |   6 | swept   |  4 | 0 |  2 | 0 | 100 % | 2026-05-01 |
| src/client/auth/whoami.rs           |   6 | swept   |  4 | 0 |  2 | 0 | 100 % | 2026-05-01 |
| src/types/attachment.rs             |   5 | pending | – | – | – | – | – | – |
| src/tls/mod.rs                      |   5 | pending | – | – | – | – | – | – |
| src/output/user.rs                  |   5 | pending | – | – | – | – | – | – |
| src/tls/fingerprint.rs              |   4 | pending | – | – | – | – | – | – |
| src/credentials/keyring_stub.rs     |   4 | pending | – | – | – | – | – | – |
| src/commands/user.rs                |   4 | pending | – | – | – | – | – | – |
| src/types/product.rs                |   3 | pending | – | – | – | – | – | – |
| src/commands/field.rs               |   3 | pending | – | – | – | – | – | – |
| src/commands/bug/clone.rs           |   3 | pending | – | – | – | – | – | – |
| src/client/server.rs                |   3 | swept   |  2 | 0 |  1 | 0 | 100 % | 2026-05-01 |
| src/client/component.rs             |   3 | swept   |  2 | 0 |  1 | 0 | 100 % | 2026-05-01 |
| src/xmlrpc/fault.rs                 |   2 | pending | – | – | – | – | – | – |
| src/output/server.rs                |   2 | pending | – | – | – | – | – | – |
| src/output/group.rs                 |   2 | pending | – | – | – | – | – | – |
| src/output/field.rs                 |   2 | pending | – | – | – | – | – | – |
| src/output/classification.rs        |   2 | pending | – | – | – | – | – | – |
| src/types/user.rs                   |   1 | pending | – | – | – | – | – | – |
| src/output/comment.rs               |   1 | pending | – | – | – | – | – | – |
| src/output/attachment.rs            |   1 | pending | – | – | – | – | – | – |
| src/lib.rs                          |   1 | pending | – | – | – | – | – | – |
| src/commands/whoami.rs              |   1 | pending | – | – | – | – | – | – |
| src/commands/server.rs              |   1 | pending | – | – | – | – | – | – |
| src/commands/product.rs             |   1 | pending | – | – | – | – | – | – |
| src/commands/group.rs               |   1 | pending | – | – | – | – | – | – |
| src/commands/component.rs           |   1 | pending | – | – | – | – | – | – |
| src/commands/classification.rs      |   1 | pending | – | – | – | – | – | – |
| src/commands/bug/view.rs            |   1 | pending | – | – | – | – | – | – |
| src/commands/bug/mod.rs             |   1 | pending | – | – | – | – | – | – |
| src/commands/bug/history.rs         |   1 | pending | – | – | – | – | – | – |
| src/commands/bug/create.rs          |   1 | pending | – | – | – | – | – | – |
| src/client/classification.rs        |   1 | swept   |  1 | 0 |  0 | 0 | 100 % | 2026-05-01 |

## Sub-tree totals (informational)

| Sub-tree | Mutants | Notes |
|---------|--------:|-------|
| `src/commands/`  | 204 | Includes the `bug/*` split; many small files |
| `src/client/`    | 185 | API surface — wiremock-driven tests already exist |
| `src/xmlrpc/`    | 161 | Pure parsing — high-leverage |
| `src/tls/`       | 147 | Custom verifier; bugs here are silent |
| `src/types/`     | 102 | Mostly serde plumbing |
| `src/output/`    | 101 | Expect snapshot adoption before sweeping |
| `src/config.rs`  |  47 | Single file |
| `src/url_parser.rs` | 25 | Swept (Wave 1) |
| `src/error.rs`   |  19 | Single file |
| `src/main.rs`    |  16 | Mostly clap glue |
| `src/http.rs`    |  11 | Boundary code |
| `src/credentials/` | 11 | Two files |
| `src/lib.rs`     |   1 | Dispatch |

## Skip-list audit

Per-file `mutants::skip` attributes are tracked via `make audit-mutant-skips`.

Config-level exclusions in `.cargo/mutants.toml` (review these whenever the
related code changes — they may have rotted):

| Pattern | Files affected | Reason |
|---------|---------------|--------|
| `impl .* Display for` / `impl .* Debug for` | all | formatting; covered by snapshot/output tests when needed |
| `skip_to_end` | `src/xmlrpc/parsing.rs` | defensive depth-tracking; equivalent in normal XML-RPC flow (depth never exceeds 1) |
| `is_value_end -> bool with true` | `src/xmlrpc/parsing.rs` | empty `<value></value>` end arm — equivalent under permissive parser |
| `replace == with != in is_value_end` | `src/xmlrpc/parsing.rs` | same equivalence as above |
| `match guard is_value_end(...) with true` | `src/xmlrpc/parsing.rs` | guard call site of the equivalent helper |
| `is_tls_cert_error -> bool with false` | `src/http.rs` | `reqwest::Error` has no public constructor; cannot construct one with both `is_connect()` true and a TLS-keyword chain in unit tests |
| `read_interactive_line` / `confirm_pin` / `prompt_tofu` / `prompt_rotation` returning Ok(None)/Ok(false) | `src/tls/tofu.rs` | Each detects non-terminal stdin and short-circuits to None/false; cargo test stdin is never a terminal, so the mutation matches the test-mode path |
| `warn_security` / `warn_if_path_permissions_too_open` / `Config::warn_on_insecure_permissions` no-ops, plus the bitwise-mode-mask check | `src/config.rs` | Stderr-only side effects; the project has no `capture_stderr` helper, so these can't be observed in unit tests |
| `write_private_file` / `set_private_file_permissions -> Ok(())` | `src/config.rs` | The non-Unix `write_private_file` is dead code on the Linux test platform (`#[cfg(not(unix))]`); the Unix `set_private_file_permissions` is redundant since `write_private_file` already creates with `OpenOptions::mode(0o600)` |
| `delete ! in detect_auth_method` | `src/client/auth/mod.rs` | Tracing-only side effect for non-HTTPS URLs |
| `matches!(e, BzrError::Auth(_))` (Hybrid create_user) | `src/client/user.rs` | Defensive arm; `BzrError::Auth` is only constructed by detect_auth_method, so post_json_id never returns it |
| `CodeVisitor>::expecting -> std::fmt::Result` | `src/client/mod.rs` | The visitor's "expected" message is only surfaced on serde failure, which `check_response_status` swallows in favor of `BzrError::HttpStatus` |
