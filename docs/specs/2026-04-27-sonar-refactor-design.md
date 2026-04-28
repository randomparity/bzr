# SonarCloud Refactor — Design

**Date:** 2026-04-27
**Branch:** `refactor/sonar-cleanup`
**Source dashboard:** https://sonarcloud.io/project/overview?id=randomparity_bzr

## Background

The project's SonarCloud quality gate is currently passing (status `OK`, all
ratings A, 0 bugs, 0 vulnerabilities). However, the dashboard surfaces
maintenance hotspots that warrant a focused cleanup pass:

- **2 open `rust:S3776` issues** — function-level cognitive complexity over
  the 15 limit:
  - `src/commands/bug.rs:117` (`handle_search`) — 16
  - `src/url_parser.rs:72` (`parse_bugzilla_url`) — 26
- **5.6% duplication density** — concentrated in `commands/bug.rs`,
  `client/group.rs`, `client/bug.rs`, `client/auth/mod.rs`.
- **File-level complexity hotspots** — `xmlrpc/mod.rs` (cog 64),
  `commands/bug.rs` (cog 54), `tls/verifier.rs` (cog 38).
- **Coverage gaps** — overall is 93.3%, but several files sit well below:
  `commands/shared.rs` 64.6%, `main.rs` 67.3%, `output/comment.rs` 71.6%,
  `output/attachment.rs` 73.2%, `lib.rs` 74.3%.

This refactor lands on a single branch as a stack of focused commits.

## Goals (success criteria)

Verifiable on the SonarCloud dashboard after merge:

| Metric | Current | Target |
|---|---|---|
| Open issues | 2 | **0** |
| Overall line coverage | 93.3% | **≥95%** |
| Per-file line coverage (currently <85%) | 64.6%–84.5% | **≥85%** for every file |
| Duplication density | 5.6% | **≤3%** (stretch) |
| `commands/bug.rs` per-file cog (currently 54) | one file at 54 | no single submodule >20 after split |
| `xmlrpc/mod.rs` per-file cog (currently 64) | 64 | no single submodule >25 after split |
| `tls/verifier.rs` per-file cog | 38 | ≤25 (in-place) |
| New-code gate (configured in SonarCloud UI) | default | coverage ≥85%, duplication ≤2% |

Hard requirements: 0 issues, ≥85% per-file coverage on previously-deficient
files, ≥95% overall. Duplication and file-cog targets are best-effort — if
hitting them requires bad abstractions, document the deviation and stop.

## Non-goals

- No behavioral changes — pure refactor; no new flags, commands, or APIs.
- No `tls/verifier.rs` module split (the file is fresh, well-reviewed code
  where complexity is intrinsic to TLS verification).
- No performance tuning unless a Sonar finding implicates it.
- No changes outside `src/`, `tests/`, and minimal `docs/` for the gate
  policy note. CLI reference (`docs/bzr-cli.md`) only updates if module
  renames affect public command paths (they do not).

## Sequencing strategy

Single branch, stacked commits. Each commit is independently buildable,
lints-clean, and tests-clean. After every commit: `cargo fmt`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`. No commit lands
red. `cargo llvm-cov --summary-only` runs on the coverage commits (#8–#12)
and at end-of-stack — not after every commit, since instrumented test runs
are slow.

Module-rename commits (#3, #4) do `git mv` first, edits second, to preserve
`git log --follow`.

| # | Commit | Notes |
|---|---|---|
| 1 | `refactor: extract handle_search helpers in commands/bug.rs` | Fixes the small S3776 (16→<15). Independent of #3. |
| 2 | `refactor: extract url_parser query-pair handlers` | Fixes the second S3776 (26→<15). Independent. |
| 3 | `refactor: split commands/bug.rs into submodules` | `commands/bug.rs` → `commands/bug/{mod,search,show,modify,history,my,attach,shared}.rs`. Eliminates 267 dup lines. After #1 so `handle_search` is already cleaner. |
| 4 | `refactor: split xmlrpc/mod.rs into call/fault/parsing` | `xmlrpc/mod.rs` → `xmlrpc/{mod,call,fault,parsing}.rs`. Independent of #1–3. |
| 5 | `refactor: dedupe client/group.rs and client/auth/mod.rs` | Hoist shared REST helpers and auth-probe response handling. |
| 6 | `refactor: dedupe client/bug.rs against new helpers` | Sequenced after #5 to reuse the helpers. |
| 7 | `refactor: simplify tls/verifier.rs in place` | Extract `verify_pin`, `verify_san`, `verify_issuer` helpers. No public API change. No file split. |
| 8 | `test: cover commands/shared.rs to ≥85%` | Worst gap: 181 uncovered lines. |
| 9 | `test: cover main.rs and lib.rs entrypoints to ≥85%` | Via integration tests against `Cli::parse_from + dispatch`. |
| 10 | `test: cover output/{comment,attachment,user,product,field,group} to ≥85%` | All output formatters under 85%. Single commit because they are structurally identical. |
| 11 | `test: cover tls/verifier.rs and tls/tofu.rs to ≥85%` | Easier after #7's helper extractions. |
| 12 | `chore: opportunistic coverage on touched files` | Final pass on files we touched in #1–11. |

After each affected commit, push to a draft PR and wait for the SonarCloud
check before stacking the next commit on top — Sonar's `cognitive_complexity`
scoring occasionally disagrees with `cargo clippy::cognitive_complexity`,
and we want to catch mismatches early.

If the branch reaches 15+ commits, split into a second PR.

## Technical approach

### Function-level S3776 fixes (commits #1, #2)

**`handle_search` in `commands/bug.rs:117`** — current cog 16. Complexity is
concentrated in the `if let Some(url_str) = from_url` branch, which does URL
parse → server resolution → save_info computation → params construction in
one block. Extract:

- `fn resolve_save_info(save_as, parsed_suggested_name, parsed_query) -> Result<Option<(String, SavedQuery)>>`
  — handles the `Some("")` / `Some(name)` / `None` triage.
- `fn build_params_from_url(parsed_query, limit, fields, exclude_fields) -> SearchParams`
  — handles the params + override application.

`handle_search` shrinks to a flat sequence: branch on `from_url`, call
helpers, do search, persist. Target cog ≤10.

**`parse_bugzilla_url` in `url_parser.rs:72`** — current cog 26. Complexity
is the long `for (key, value) in url.query_pairs()` loop with 6 conditional
branches per iteration. Extract a dispatch helper:

```rust
enum ParamKind<'a> {
    Ignored,
    KnownName,
    QueryBasedOn,
    Limit,
    Mapped(&'a FieldMapping),
    Credential,
    Raw,
}

fn classify_param(key: &str) -> ParamKind { … }
```

The `for` loop becomes a `match classify_param(key)` with one arm per kind,
each ≤3 lines. Target cog ≤10.

### Module splits (commits #3, #4)

**`commands/bug.rs` → `commands/bug/`**:

- `mod.rs` — public `execute(action, server, format, api)` dispatcher only
  (~80 lines)
- `search.rs` — `handle_search` + helpers
- `show.rs` — `handle_show`
- `modify.rs` — `handle_modify` (and `handle_set_*` variants if separable)
- `history.rs` — `handle_history`
- `my.rs` — `handle_my`
- `attach.rs` — `handle_attach`
- `shared.rs` — formerly-duplicated helpers (params construction, output
  triage, etc.)

The 267 duplicated lines mostly trace to repeated "load config → connect →
call client → format" boilerplate per handler. The
`commands::shared::connect_and_configure` call already exists; the dedup is
in the post-call output/save logic. After split, `commands/bug/` aggregate
cog target ≤30.

**`xmlrpc/mod.rs` → `xmlrpc/`**:

- `mod.rs` — re-exports + `XmlRpcRequest`/`XmlRpcResponse` types only
- `call.rs` — request building (param serialization)
- `fault.rs` — fault parsing (the `Fault` struct + decoding)
- `parsing.rs` — XML response parsing (`parse_value`, `parse_member`, etc.)
- `xmlrpc/client.rs` stays where it is

Aggregate cog target ≤30 in `mod.rs`.

### Duplication kills (commits #5, #6)

Inspect each duplicated block reported by Sonar and apply the right fix:

- **`client/group.rs` ↔ `client/bug.rs`** — likely shared REST shape
  (list/get/create/update). Hoist into `client/rest_helpers.rs` (a
  generic-over-T `get_resource`, `list_resources`, `create_resource` that
  takes the endpoint path and serde types).
- **`client/auth/mod.rs` ↔ `client/auth/{whoami,valid_login}.rs`** — shared
  probe response handling. Hoist common error-mapping into
  `auth/probe_common.rs`.
- **`commands/bug.rs` internal duplication** — handled by the split in #3.

Each helper introduced needs unit tests covering at minimum: success path,
404, malformed JSON, network error.

**Abstraction discipline:** only dedup blocks Sonar actually flags. If
extracting requires generics with >2 type parameters or non-trivial trait
bounds, leave the duplication. Bad abstractions are worse than the duplication
they replace.

### In-place complexity reduction in `tls/verifier.rs` (commit #7)

Three private helper extractions, no public API change:

- `verify_pin(&self, leaf: &CertificateDer) -> Result<(), Error>` — SHA-256
  SPKI computation + comparison.
- `verify_san(&self, leaf: &ParsedCert, server_name: &ServerName) -> Result<(), Error>`
  — SAN/CN extraction + match.
- `verify_issuer(&self, leaf: &ParsedCert, expected: &[u8]) -> Result<(), Error>`
  — issuer DER comparison.

`verify_server_cert` becomes a flat sequence of these three calls plus
expiry/chain checks. Target cog ≤25, file coverage ≥85%.

**Strict no-behavior-change discipline.** Extract helpers via
copy-then-replace-call-sites, not rewrite. All existing TLS tests must pass
without modification at every step. If any existing test needs adjusting,
that is a red flag — stop and reassess.

### Coverage commits (#8–#11)

For each target file, the test approach:

- **`commands/shared.rs` (64.6%, 181 uncov)** — the hot file.
  `connect_and_configure` does config-load + auth-resolution + client-build.
  Tests use `wiremock` for the server side and a temp `XDG_CONFIG_HOME` for
  config side. Cover: missing server, default-server fallback, auth-detection
  success/failure, TLS pin happy path, TLS pin mismatch.
- **`main.rs` (67.3%)** — most of `main` is `dispatch()` glue. The bulk of
  `main.rs` coverage comes from integration tests in `tests/integration.rs`
  driving `Cli::parse_from(...)` + `dispatch()`. Add cases for `--version`,
  `--help` (exit-0 paths), config errors (exit-non-zero), and `RUST_LOG`
  parsing.
- **`lib.rs` (74.3%)** — the `dispatch()` matcher. Cover each `Commands::*`
  arm via a smoke test that builds an `Action` and routes it.
- **`output/*.rs`** — uniform pattern: each formatter has
  `print_X(values, format)`. Use `test_helpers::capture_stdout` to assert
  output for human/JSON/CSV variants. Empty input, single item, multiple
  items, edge fields (Unicode, empty strings, null/None).
- **`tls/verifier.rs`, `tls/tofu.rs`** — after #7 introduces helpers,
  unit-test each helper directly with hand-crafted `CertificateDer`
  fixtures (matching the existing pattern from b7896df, f34e674).

**Test discipline (per CLAUDE.md):**

- Use `writeln!(io::stdout(), ...)` not `println!` in code under test —
  `test_helpers::capture_stdout` redirects fd 1.
- Test inputs/outputs through public APIs, not private helpers from
  #3a/3d unless the helper is genuinely a public-facing primitive.
- All API tests use `#[tokio::test]`.
- Test modules use `#[expect(clippy::unwrap_used)]` to allow `.unwrap()`.

**Keyring path note:** `commands/shared.rs` and `credentials/keyring.rs`
have OS-specific keyring code that is hard to test in CI. Skip the keyring
path under `cfg(test)`; it is not blocking the 85% target for those files.

### Per-commit verification

After each commit (cheap, run every time):

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

After coverage commits (#8–#12) and at end-of-stack (expensive, instrumented
build):

```bash
cargo llvm-cov --summary-only
```

The branch is **not** complete until `cargo llvm-cov` reports overall ≥95%
line coverage and per-file ≥85% on every previously-deficient file, and the
SonarCloud check on the draft PR shows 0 open issues.

### New-code gate configuration

After the PR merges:

1. In SonarCloud project settings, set the new-code gate to coverage ≥85%
   and duplication ≤2%.
2. Add a one-paragraph note to `docs/sonarcloud-gate.md` documenting the
   policy so it is visible in the repo even though enforcement lives in
   the SonarCloud UI.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Module split breaks `git log --follow` and obscures blame for security-sensitive code | `git mv` of unmodified content first, then a separate commit edits. Listed in commit message which paths are pure renames. |
| Dedup helpers introduce wrong abstractions (three similar call sites become a generic that fits two) | Only dedup blocks Sonar flags. If extracting requires generics with >2 type parameters or non-trivial trait bounds, leave it. The 5.6% target has slack — stop at 3% if the last bit gets ugly. |
| Coverage chasing produces tests that lock in implementation, not behavior | Test through public APIs (`execute`, `print_*`, `Cli::parse_from + dispatch`). Output formatter tests use `capture_stdout`, not internal mocks. |
| `commands/shared.rs` coverage push requires mocking system state and may slow/flake the suite | `tempfile::TempDir` + `XDG_CONFIG_HOME` override (existing pattern). `wiremock` for HTTP. Skip keyring under `cfg(test)`. |
| `tls/verifier.rs` refactor regresses security-critical code | Strict no-behavior-change: copy-then-replace, not rewrite. All existing TLS tests must pass unmodified. Adjusting an existing TLS test is a red flag — stop. |
| Sonar's `cognitive_complexity` scoring disagrees with `cargo clippy::cognitive_complexity` | Push to draft PR after each affected commit; wait for SonarCloud check before stacking. |
| Stacked-commit branch grows large; review fatigue | Cap at 12 listed commits. Split into a second PR if it reaches 15+. Each commit message names the Sonar metric it moves and by how much. |
| New-code gate is set in SonarCloud's web UI — not visible in code review | Document the policy in `docs/sonarcloud-gate.md`. |
| Duplication and file-cog targets unachievable without bad abstractions | Treat as best-effort. Hard requirements remain: 0 issues, ≥85% per-file coverage, ≥95% overall. |

## Out of scope (will not address in this branch)

- Behavioral changes, new flags, new commands.
- `tls/verifier.rs` module split.
- Performance tuning unless a specific Sonar finding implicates it.
- Files outside `src/` and `tests/` (other than the one-paragraph
  `docs/sonarcloud-gate.md` policy note).
- Keyring-path coverage (OS-specific, would need platform-specific test
  infrastructure).

## Definition of done

1. Branch `refactor/sonar-cleanup` merged to `main` via PR.
2. SonarCloud dashboard shows: 0 open issues, ≥95% overall coverage,
   ≥85% per-file coverage on every file currently below it.
3. `cargo llvm-cov --summary-only` agrees with the dashboard numbers.
4. New-code gate (≥85% cov, ≤2% dup) configured in SonarCloud UI.
5. `docs/sonarcloud-gate.md` exists and documents the gate policy.
