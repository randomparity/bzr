# Mutation Testing Baseline

This file tracks mutation-testing coverage per source file. It is the ratchet
target — every PR should leave a file's score ≥ the value listed here. New
sweeps update the numbers; never drop them silently.

- **Tool:** `cargo-mutants` (currently `27.0.0`)
- **Config:** [`.cargo/mutants.toml`](../../.cargo/mutants.toml)
- **Baseline date:** 2026-06-24
- **Source files mutated:** 166
- **Full-run totals (per build target):** 3431 mutants — 2751 caught, 248
  missed, 424 unviable, 8 timeout. Wall clock ≈ 18h at `--jobs 8` on an
  18-core host.
- **Unique totals (deduplicated across targets):** 1796 viable — 1655 caught,
  141 missed, 239 unviable, 5 timeout. As-measured kill rate **92.1 %**.

### Why two sets of totals

`bzr`'s library sources are compiled into the `bzr` library **and** the `bzr` /
`bzr_lock_helper` binary targets, so `cargo-mutants` generates and tests most
mutants once per target. The full-run totals are the tool's raw per-target
counts; the per-file table and the disposition below report **unique** mutants
(deduplicated across targets), since killing a defect kills every copy.

## What this refresh did (#417)

The prior baseline (2026-05-01, 1030 mutants over 75 files) predated a large
restructure (~950 commits): `config.rs` → `config/{model,store}.rs`,
`client/mod.rs` → `client/{transport,response,request}.rs`,
`commands/shared.rs` → `commands/runtime/shared/connection/**`, `xmlrpc/*` →
`xmlrpc/{protocol,resources}/**`, plus new modules (`validation/**`,
`batch.rs`, `field_aliases.rs`, `commands/completion.rs`, `commands/schema.rs`).
The whole-crate sweep was re-run and every one of the 141 unique missed mutants
was dispositioned (see [Disposition](#disposition-of-missed-mutants)):

- **95 killed** by new behavior tests,
- **40 excluded** as equivalent or unreachable (`.cargo/mutants.toml`),
- **6 documented residual gaps** (transport-error retry — see below).

Effective post-refresh kill rate of testable mutants: **≈ 99.7 %**.

## How to read this file

For each file the table records the unique mutant outcome counts
(`caught` / `missed` / `unviable` / `timeout`). A "score" is
`caught / (caught + missed)`. Unviable mutants (won't compile — e.g. they
require a `Default` impl) and timeouts are reported but do not count against
the score. The `missed` column is the **as-measured** value; every missed
mutant is resolved in the [Disposition](#disposition-of-missed-mutants)
section (killed by a test added in this PR, or excluded in `mutants.toml`, or
listed as a documented gap). A future full sweep will re-measure missed → 0
for the fixed files.

## Workflow for a single file

```bash
# 1. Produce the report (--jobs caps parallel build+test pipelines).
cargo mutants --file <path> --jobs 8

# 2. Inspect outcomes
cat mutants.out/missed.txt
cat mutants.out/unviable.txt

# 3. For each entry in missed.txt:
#    a) Add a unit test that fails when the mutation is applied, OR
#    b) If the mutation is semantically equivalent / unreachable, add a narrow
#       pattern to `exclude_re` in .cargo/mutants.toml with a comment, OR a
#       `#[cfg_attr(test, mutants::skip)]` on the item, explaining why.
#
# 4. Re-run until missed.txt is acceptable; update this file.
```

To replay specific missed mutants (cargo-mutants 27 filters by description
regex, not line:col):

```bash
cargo mutants --file <path> --re '<mutant description regex>'
```

A fast way to verify a fix without re-sweeping the file: hand-apply the
mutation to the source, run the focused test, confirm it FAILS, then revert.

## Sweep order

Modules are tackled highest-leverage first:

1. **Parsers / pure logic** — `validation/datetime.rs`, `url_parser.rs`,
   `commands/runtime/flags.rs`, `xmlrpc/protocol/parsing.rs`,
   `client/version.rs`, `http.rs`
2. **Boundaries** — `error.rs`, `config/{model,store}.rs`,
   `tls/{verifier,tofu,pin_failure}.rs`
3. **Client** — `client/{transport,response,request}.rs`,
   `client/auth/**`, `client/{bug,attachment,comment,user,group,product,
   component,classification,field,server}.rs`
4. **Commands** — `commands/**` (incl. `commands/runtime/**`)
5. **Output** — `output/**` and `output/resources/**`
6. **CLI / types** — `cli/**`, `types/**`, `main.rs`, `lib.rs`

## Disposition of missed mutants

### Excluded as equivalent or unreachable

These mutants cannot be killed by any feasible unit test, or are
observationally equivalent to the original. Each has a justification comment in
[`.cargo/mutants.toml`](../../.cargo/mutants.toml):

| Category | Examples | Why unkillable |
|----------|----------|----------------|
| `#[cfg]`-gated dead code | `fuzz::*` (`cfg(fuzzing)`), non-Unix `open_lock_file`/`create_new_private`, non-macOS `has_display` | Never compiled on the platform the baseline runs on; `cargo-mutants` ignores `#[cfg]`. |
| `reqwest::Error` classifiers | `should_retry_transport`, `is_transient`, `pin_failure::classify` | `reqwest::Error` has no public constructor; wiremock returns HTTP responses, not transport errors. |
| Tracing / stderr-only side effects | `log_probe_send_error`, `detect_auth_method` non-HTTPS warn, the `warn_*` family, `fetch_all_pages` paginate cap | Emit only `tracing`/stderr with no return-value or `Writers` impact. |
| Interactive TTY / stdin readers | `read_comment_body`, `read_stdin_to_string`, TOFU/PIN prompts | Require a pty; in tests stdin is never a terminal. |
| OS keyring setup | `ensure_default_store`, `native_store` | Talk to the platform secret service, absent in CI. |
| Observationally equivalent | `safe_basename` guard (already guaranteed by `Path::file_name()`), `<impl Serialize for Bug>` `+`/`-` (a serde capacity hint), redundant `bug update` pre-validation, `confirm_batch` (no-TTY), non-deterministic temp-collision retry guards | Same observable result under the mutation. |

### Documented residual gaps

`src/client/transport.rs` retains **6** unique missed mutants in the
transport-error retry branch (`&&`/`<`/`+=` inside `BugzillaClient::send` at
the `is_transient` arm). Killing them needs a real `reqwest` connect/timeout
error, which has no public constructor, plus real backoff timing; the
status-retry branch (429/5xx) is fully covered by wiremock tests. These are
left visible rather than excluded so the gap stays honest; close them if a
transport-error test seam is added.

### Killed by new tests (this PR)

New behavior tests were added next to each source file (sibling `*_tests.rs`).
Highlights: the `validation/datetime.rs` parser boundary cases; `bug clone`
field-wiring and `close`/`reopen`/`dup` comment bodies; `lib.rs` dispatch
credential/dry-run match arms; `config` temp-reaper prefix+suffix guard;
`attachment` `update_has_changes`/partial-failure warning; `bug update`
draft-merge helpers and stdin-source rejection; CLI `query overrides` and
`bug my`/`search` paging fields; `client` version/response/user-fallback
paths; `output/resources` record formatters; and the connection-context
accessors.

## Per-file inventory

Largest-first (unique mutant counts from the 2026-06-24 full run). See
[Disposition](#disposition-of-missed-mutants) for how each `Missed` entry is
resolved.

| File | Mutants | Caught | Missed | Unviable | Timeout | Score |
|------|--------:|-------:|-------:|---------:|--------:|------:|
| src/validation/datetime.rs                     |   99 |  88 |  11 |   0 |  0 |  89% |
| src/tls/verifier.rs                            |   99 |  93 |   0 |   6 |  0 | 100% |
| src/xmlrpc/protocol/parsing.rs                 |   68 |  57 |   0 |  11 |  0 | 100% |
| src/client/bug.rs                              |   54 |  43 |   0 |  11 |  0 | 100% |
| src/types/bug/search.rs                        |   43 |  43 |   0 |   0 |  0 | 100% |
| src/types/bug_fields.rs                        |   43 |  40 |   0 |   3 |  0 | 100% |
| src/client/response.rs                         |   40 |  35 |   2 |   3 |  0 |  95% |
| src/commands/bug/create.rs                     |   38 |  36 |   0 |   2 |  0 | 100% |
| src/xmlrpc/resources/mappers.rs                |   38 |  35 |   0 |   3 |  0 | 100% |
| src/commands/bug/update_json.rs                |   37 |  29 |   6 |   2 |  0 |  83% |
| src/config/model.rs                            |   36 |  32 |   1 |   3 |  0 |  97% |
| src/output/formatting.rs                       |   34 |  34 |   0 |   0 |  0 | 100% |
| src/config/store.rs                            |   34 |  26 |   6 |   2 |  0 |  81% |
| src/client/transport.rs                        |   33 |  14 |  11 |   5 |  3 |  56% |
| src/tls/tofu.rs                                |   32 |  28 |   0 |   4 |  0 | 100% |
| src/commands/bug/clone.rs                      |   32 |  23 |   9 |   0 |  0 |  72% |
| src/output/resources/bug.rs                    |   32 |  31 |   0 |   1 |  0 | 100% |
| src/commands/attachment/mod.rs                 |   30 |  23 |   7 |   0 |  0 |  77% |
| src/types/query.rs                             |   30 |  29 |   0 |   1 |  0 | 100% |
| src/commands/runtime/context.rs                |   28 |  12 |   5 |  11 |  0 |  71% |
| src/lib.rs                                     |   27 |  13 |  12 |   2 |  0 |  52% |
| src/http.rs                                    |   26 |  20 |   5 |   0 |  1 |  80% |
| src/types/common.rs                            |   26 |  22 |   0 |   4 |  0 | 100% |
| src/commands/bug/view.rs                       |   25 |  19 |   3 |   3 |  0 |  86% |
| src/commands/runtime/url_parser.rs             |   25 |  23 |   0 |   2 |  0 | 100% |
| src/tls/pin_failure.rs                         |   24 |  21 |   1 |   2 |  0 |  95% |
| src/xmlrpc/protocol/value.rs                   |   24 |  15 |   0 |   9 |  0 | 100% |
| src/client/version.rs                          |   24 |  12 |   2 |  10 |  0 |  86% |
| src/commands/attachment/download.rs            |   24 |  18 |   3 |   3 |  0 |  86% |
| src/cli/bug/mod.rs                             |   23 |  22 |   1 |   0 |  0 |  96% |
| src/client/attachment.rs                       |   22 |  13 |   0 |   9 |  0 | 100% |
| src/commands/runtime/from_json.rs              |   22 |  12 |   0 |  10 |  0 | 100% |
| src/commands/runtime/paging.rs                 |   22 |  18 |   0 |   4 |  0 | 100% |
| src/commands/template/mod.rs                   |   21 |  21 |   0 |   0 |  0 | 100% |
| src/commands/component/update.rs               |   20 |  16 |   0 |   4 |  0 | 100% |
| src/commands/runtime/shared/body_source.rs     |   20 |  17 |   1 |   2 |  0 |  94% |
| src/commands/bug/verbs.rs                      |   20 |  17 |   3 |   0 |  0 |  85% |
| src/commands/bug/my.rs                         |   18 |  15 |   3 |   0 |  0 |  83% |
| src/commands/runtime/flags.rs                  |   18 |  10 |   0 |   7 |  1 | 100% |
| src/output/resources/attachment.rs             |   17 |  16 |   0 |   1 |  0 | 100% |
| src/error.rs                                   |   17 |  15 |   0 |   2 |  0 | 100% |
| src/client/comment.rs                          |   16 |  12 |   0 |   4 |  0 | 100% |
| src/commands/bug/search.rs                     |   16 |  13 |   2 |   1 |  0 |  87% |
| src/commands/bug/update/execute.rs             |   16 |  15 |   1 |   0 |  0 |  94% |
| src/xmlrpc/resources/bug.rs                    |   15 |  11 |   0 |   4 |  0 | 100% |
| src/output/result_types.rs                     |   14 |   3 |   0 |  11 |  0 | 100% |
| src/output/resources/query.rs                  |   13 |  13 |   0 |   0 |  0 | 100% |
| src/commands/bug/mod.rs                        |   13 |  10 |   3 |   0 |  0 |  77% |
| src/cli/query.rs                               |   13 |   7 |   6 |   0 |  0 |  54% |
| src/commands/runtime/shared/connection/target.rs |   13 |   4 |   7 |   2 |  0 |  36% |
| src/commands/query/update.rs                   |   13 |  13 |   0 |   0 |  0 | 100% |
| src/commands/bug/update/payload.rs             |   13 |  13 |   0 |   0 |  0 | 100% |
| src/client/group.rs                            |   13 |  10 |   0 |   3 |  0 | 100% |
| src/xmlrpc/resources/attachment.rs             |   11 |   3 |   3 |   5 |  0 |  50% |
| src/main.rs                                    |   11 |  11 |   0 |   0 |  0 | 100% |
| src/commands/bug/create_json.rs                |   11 |  11 |   0 |   0 |  0 | 100% |
| src/commands/runtime/shared/connection/tls_trust.rs |   11 |   9 |   0 |   2 |  0 | 100% |
| src/output/resources/user.rs                   |   11 |   9 |   0 |   2 |  0 | 100% |
| src/commands/runtime/confirm.rs                |   11 |  11 |   0 |   0 |  0 | 100% |
| src/credentials/keyring.rs                     |   11 |   4 |   4 |   3 |  0 |  50% |
| src/commands/bug/list.rs                       |   11 |  11 |   0 |   0 |  0 | 100% |
| src/client/auth/valid_login.rs                 |   11 |   9 |   0 |   2 |  0 | 100% |
| src/output/resources/product.rs                |   11 |   7 |   3 |   1 |  0 |  70% |
| src/commands/attachment/upload.rs              |   11 |   9 |   1 |   1 |  0 |  90% |
| src/tls/error.rs                               |   10 |  10 |   0 |   0 |  0 | 100% |
| src/client/mod.rs                              |   10 |   8 |   0 |   2 |  0 | 100% |
| src/commands/schema.rs                         |   10 |  10 |   0 |   0 |  0 | 100% |
| src/client/auth/mod.rs                         |   10 |   4 |   1 |   5 |  0 |  80% |
| src/validation/mod.rs                          |    9 |   9 |   0 |   0 |  0 | 100% |
| src/output/resources/config.rs                 |    9 |   5 |   2 |   2 |  0 |  71% |
| src/commands/query/save.rs                     |    9 |   9 |   0 |   0 |  0 | 100% |
| src/client/user.rs                             |    9 |   5 |   1 |   3 |  0 |  83% |
| src/commands/user/update.rs                    |    9 |   9 |   0 |   0 |  0 | 100% |
| src/commands/runtime/editor.rs                 |    9 |   6 |   1 |   2 |  0 |  86% |
| src/types/bug/payload.rs                       |    8 |   8 |   0 |   0 |  0 | 100% |
| src/commands/bug/update/validate.rs            |    8 |   7 |   1 |   0 |  0 |  88% |
| src/client/auth/whoami.rs                      |    8 |   6 |   0 |   2 |  0 | 100% |
| src/xmlrpc/protocol/call.rs                    |    8 |   8 |   0 |   0 |  0 | 100% |
| src/tls/mod.rs                                 |    8 |   6 |   0 |   2 |  0 | 100% |
| src/bugzilla_auth.rs                           |    8 |   6 |   0 |   2 |  0 | 100% |
| src/commands/comment/mod.rs                    |    7 |   7 |   0 |   0 |  0 | 100% |
| src/output/resources/template.rs               |    7 |   7 |   0 |   0 |  0 | 100% |
| src/types/bug.rs                               |    7 |   3 |   1 |   3 |  0 |  75% |
| src/client/request.rs                          |    7 |   4 |   0 |   3 |  0 | 100% |
| src/commands/config/remove.rs                  |    7 |   7 |   0 |   0 |  0 | 100% |
| src/client/product.rs                          |    6 |   4 |   0 |   2 |  0 | 100% |
| src/commands/product/update.rs                 |    6 |   6 |   0 |   0 |  0 | 100% |
| src/commands/component/mod.rs                  |    6 |   6 |   0 |   0 |  0 | 100% |
| src/output/resources/component.rs              |    6 |   6 |   0 |   0 |  0 | 100% |
| src/commands/runtime/shared/connection/mod.rs  |    6 |   4 |   1 |   1 |  0 |  80% |
| src/commands/group/mod.rs                      |    6 |   6 |   0 |   0 |  0 | 100% |
| src/commands/bug/update/draft.rs               |    6 |   3 |   3 |   0 |  0 |  50% |
| src/output/resources/classification.rs         |    6 |   3 |   3 |   0 |  0 |  50% |
| src/commands/config/keyring.rs                 |    6 |   6 |   0 |   0 |  0 | 100% |
| src/types/attachment.rs                        |    6 |   4 |   0 |   2 |  0 | 100% |
| src/commands/product/mod.rs                    |    6 |   6 |   0 |   0 |  0 | 100% |
| src/commands/user/mod.rs                       |    6 |   6 |   0 |   0 |  0 | 100% |
| src/commands/config/rename.rs                  |    5 |   4 |   1 |   0 |  0 |  80% |
| src/commands/bug/update/output.rs              |    5 |   5 |   0 |   0 |  0 | 100% |
| src/xmlrpc/resources/comment.rs                |    5 |   2 |   0 |   3 |  0 | 100% |
| src/commands/group/update.rs                   |    5 |   5 |   0 |   0 |  0 | 100% |
| src/types/field.rs                             |    5 |   5 |   0 |   0 |  0 | 100% |
| src/output/resources/field.rs                  |    5 |   5 |   0 |   0 |  0 | 100% |
| src/commands/query/mod.rs                      |    5 |   5 |   0 |   0 |  0 | 100% |
| src/commands/bug/search_support/policy.rs      |    5 |   5 |   0 |   0 |  0 | 100% |
| src/commands/bug/search_support/fields.rs      |    5 |   5 |   0 |   0 |  0 | 100% |
| src/commands/runtime/shared/connection/detect.rs |    4 |   1 |   0 |   3 |  0 | 100% |
| src/commands/runtime/shared/merge.rs           |    4 |   4 |   0 |   0 |  0 | 100% |
| src/xmlrpc/resources/user.rs                   |    4 |   4 |   0 |   0 |  0 | 100% |
| src/commands/query/run.rs                      |    4 |   3 |   1 |   0 |  0 |  75% |
| src/client/classification.rs                   |    4 |   2 |   0 |   2 |  0 | 100% |
| src/tls/fingerprint.rs                         |    4 |   4 |   0 |   0 |  0 | 100% |
| src/commands/config/migrate.rs                 |    4 |   4 |   0 |   0 |  0 | 100% |
| src/commands/config/set_server.rs              |    3 |   3 |   0 |   0 |  0 | 100% |
| src/client/component.rs                        |    3 |   3 |   0 |   0 |  0 | 100% |
| src/commands/bug/update/mod.rs                 |    3 |   1 |   2 |   0 |  0 |  33% |
| src/client/server.rs                           |    3 |   0 |   0 |   3 |  0 |  n/a |
| src/types/product.rs                           |    3 |   3 |   0 |   0 |  0 | 100% |
| src/commands/attachment/update.rs              |    3 |   3 |   0 |   0 |  0 | 100% |
| src/commands/field.rs                          |    3 |   3 |   0 |   0 |  0 | 100% |
| src/commands/template/update.rs                |    2 |   2 |   0 |   0 |  0 | 100% |
| src/commands/config/set_default.rs             |    2 |   2 |   0 |   0 |  0 | 100% |
| src/commands/component/view.rs                 |    2 |   2 |   0 |   0 |  0 | 100% |
| src/client/field.rs                            |    2 |   1 |   0 |   1 |  0 | 100% |
| src/commands/user/create.rs                    |    2 |   1 |   0 |   1 |  0 | 100% |
| src/commands/user/search.rs                    |    2 |   1 |   0 |   1 |  0 | 100% |
| src/xmlrpc/protocol/client.rs                  |    2 |   1 |   0 |   1 |  0 | 100% |
| src/xmlrpc/resources/group.rs                  |    2 |   0 |   0 |   2 |  0 |  n/a |
| src/output/resources/server.rs                 |    2 |   1 |   0 |   1 |  0 | 100% |
| src/commands/comment/tag.rs                    |    2 |   2 |   0 |   0 |  0 | 100% |
| src/bin/bzr_lock_helper.rs                     |    2 |   2 |   0 |   0 |  0 | 100% |
| src/commands/component/create.rs               |    2 |   1 |   0 |   1 |  0 | 100% |
| src/commands/group/create.rs                   |    2 |   1 |   0 |   1 |  0 | 100% |
| src/xmlrpc/protocol/fault.rs                   |    2 |   1 |   0 |   1 |  0 | 100% |
| src/commands/group/list_users.rs               |    2 |   1 |   0 |   1 |  0 | 100% |
| src/output/resources/group.rs                  |    2 |   2 |   0 |   0 |  0 | 100% |
| src/commands/product/create.rs                 |    2 |   1 |   0 |   1 |  0 | 100% |
| src/commands/query/delete.rs                   |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/comment/search_tags.rs            |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/component/list.rs                 |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/config/mod.rs                     |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/comment/add.rs                    |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/product/list.rs                   |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/query/show.rs                     |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/bug/history.rs                    |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/template/save.rs                  |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/completion.rs                     |    1 |   1 |   0 |   0 |  0 | 100% |
| src/output/resources/comment.rs                |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/runtime/mutation.rs               |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/template/delete.rs                |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/attachment/list.rs                |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/query/list.rs                     |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/product/view.rs                   |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/group/add_user.rs                 |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/whoami.rs                         |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/attachment/view.rs                |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/group/view.rs                     |    1 |   1 |   0 |   0 |  0 | 100% |
| src/cli/template.rs                            |    1 |   0 |   0 |   1 |  0 |  n/a |
| src/commands/comment/list.rs                   |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/template/list.rs                  |    1 |   0 |   1 |   0 |  0 |   0% |
| src/commands/classification.rs                 |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/template/show.rs                  |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/server.rs                         |    1 |   1 |   0 |   0 |  0 | 100% |
| src/types/user.rs                              |    1 |   0 |   0 |   1 |  0 |  n/a |
| src/commands/config/show.rs                    |    1 |   1 |   0 |   0 |  0 | 100% |
| src/commands/group/remove_user.rs              |    1 |   1 |   0 |   0 |  0 | 100% |
