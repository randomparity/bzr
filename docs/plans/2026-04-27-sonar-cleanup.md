# SonarCloud Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drive SonarCloud open issues to 0, raise overall line coverage from 93.3% to ≥95% (every previously-deficient file to ≥85%), and reduce duplication from 5.6% toward ≤3%, on branch `refactor/sonar-cleanup`.

**Architecture:** Single branch, twelve stacked commits, each independently buildable, lints-clean, and tests-clean. Refactors first (functions, then file splits, then dedup, then in-place TLS), then targeted coverage commits, then a final opportunistic-coverage pass.

**Tech Stack:** Rust 1.84.1+, `cargo`, `cargo clippy`, `cargo llvm-cov`, `wiremock`, `tempfile`, `tokio`. Pre-commit hook runs `cargo fmt --check + cargo clippy`; pre-push hook runs `cargo test`.

**Spec:** `docs/specs/2026-04-27-sonar-refactor-design.md`

---

## Working conventions for every task

- **Branch:** All work happens on `refactor/sonar-cleanup`. Verify with `git branch --show-current`.
- **No `println!` / `eprintln!` in code under test.** Use `writeln!(io::stdout(), …)` / `writeln!(io::stderr(), …)`. The `print_stdout`/`print_stderr` clippy lints are denied. `test_helpers::capture_stdout` redirects fd 1 — `println!` bypasses it.
- **No `.unwrap()` outside test modules.** `unwrap_used` is denied. Tests use `#[expect(clippy::unwrap_used)]` on the `mod tests` block.
- **All API/async tests use `#[tokio::test]`.**
- **Test fixtures live in `src/test_helpers.rs`** (`setup_test_env`, `setup_config`, `capture_stdout`, `extract_json`, `xmlrpc_bug_response`). Reuse them rather than reimplementing.
- **`ENV_LOCK` serializes tests that mutate `XDG_CONFIG_HOME`.** `setup_test_env` acquires it. Don't call `setup_config` without holding the lock.
- **Per-commit verification:** After every commit, the engineer runs:

  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  ```

  The pre-commit hook covers fmt + clippy automatically; tests run on push. Run them manually when chasing a flake.

- **Coverage verification:** Run `cargo llvm-cov --summary-only` only on coverage commits (#8–#12) and at end-of-stack — instrumented test runs are slow.
- **Commit messages end with `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`** (project convention from recent commits).

---

## Task 1: Extract `handle_search` helpers in `commands/bug.rs`

Fixes the smaller `rust:S3776` issue (cog 16 → ≤10). `handle_search` currently mixes URL parsing, server resolution, save-info computation, and params construction in one function. Extract two helpers and let the dispatcher orchestrate.

**Files:**
- Modify: `src/commands/bug.rs:115-196` (replace `handle_search` and add private helpers)

- [ ] **Step 1: Read the current `handle_search` source**

  Read `src/commands/bug.rs:115-196`. Confirm the function signature and the `BugAction::Search { … } else { unreachable!() }` destructuring. Note that `from_url`, `save_as`, `query`, `limit`, `fields`, `exclude_fields` are `&Option<…>` after destructuring.

- [ ] **Step 2: Add `resolve_save_info` helper**

  Place this private helper above `handle_search` in `src/commands/bug.rs`. The `clippy::cognitive_complexity` warning on `handle_search` should disappear after Step 4.

  ```rust
  /// Determine the save_as name + query to persist after a successful URL-based
  /// search. Returns None when --save-as wasn't passed; errors when --save-as=""
  /// is passed but the URL has no `known_name`/`query_based_on` to fall back on.
  fn resolve_save_info(
      save_as: Option<&String>,
      suggested_name: Option<String>,
      parsed_query: &crate::types::SavedQuery,
  ) -> Result<Option<(String, crate::types::SavedQuery)>> {
      let Some(raw_name) = save_as else {
          return Ok(None);
      };
      let name = if raw_name.is_empty() {
          suggested_name.ok_or_else(|| {
              crate::error::BzrError::InputValidation(
                  "no name provided for --save-as and URL has no known_name; \
                   specify a name explicitly: --save-as <name>"
                      .into(),
              )
          })?
      } else {
          raw_name.clone()
      };
      Ok(Some((name, parsed_query.clone())))
  }
  ```

- [ ] **Step 3: Add `build_params_from_url` helper**

  Place this private helper next to `resolve_save_info`:

  ```rust
  /// Convert a parsed URL's query into `SearchParams`, applying CLI overrides
  /// and a default limit of 50 when neither URL nor CLI specifies one.
  fn build_params_from_url(
      parsed_query: crate::types::SavedQuery,
      limit: Option<u32>,
      fields: Option<&str>,
      exclude_fields: Option<&str>,
  ) -> SearchParams {
      let mut params = parsed_query.into_search_params();
      if params.limit.is_none() && limit.is_none() {
          params.limit = Some(50);
      }
      params.apply_overrides(limit, fields, exclude_fields);
      params
  }
  ```

- [ ] **Step 4: Replace the body of `handle_search`**

  Replace lines `115-196` (the existing `handle_search`) with:

  ```rust
  /// Handles bug search — builds its own client (unlike other handlers) because
  /// `--from-url` may resolve a different server from the URL hostname.
  async fn handle_search(
      action: &BugAction,
      server: Option<&str>,
      format: OutputFormat,
      api: Option<ApiMode>,
  ) -> Result<()> {
      let BugAction::Search {
          query,
          from_url,
          save_as,
          limit,
          fields,
          exclude_fields,
      } = action
      else {
          unreachable!()
      };

      let (client, params, save_info) = if let Some(url_str) = from_url {
          let config = crate::config::Config::load()?;
          let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;
          let effective_server = server.or(parsed.query.server.as_deref());
          let client = super::shared::connect_and_configure(effective_server, api).await?;
          let save_info =
              resolve_save_info(save_as.as_ref(), parsed.suggested_name, &parsed.query)?;
          let params = build_params_from_url(
              parsed.query,
              *limit,
              fields.as_deref(),
              exclude_fields.as_deref(),
          );
          (client, params, save_info)
      } else {
          let query_str = query.as_deref().ok_or_else(|| {
              crate::error::BzrError::InputValidation(
                  "either a search query or --from-url is required".into(),
              )
          })?;
          let client = super::shared::connect_and_configure(server, api).await?;
          let params = SearchParams {
              quicksearch: Some(query_str.to_string()),
              limit: Some(limit.unwrap_or(50)),
              include_fields: fields.clone(),
              exclude_fields: exclude_fields.clone(),
              ..Default::default()
          };
          (client, params, None)
      };

      let bugs = client.search_bugs(&params).await?;
      output::print_bugs(&bugs, format);

      if let Some((name, query)) = save_info {
          let mut config = crate::config::Config::load()?;
          let is_update = config.queries.contains_key(name.as_str());
          config.queries.insert(name.clone(), query);
          config.save()?;
          let verb = if is_update { "Updated" } else { "Saved" };
          crate::output::print_query_saved(&name, verb, format);
      }

      Ok(())
  }
  ```

- [ ] **Step 5: Run linter and tests**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --lib commands::bug
  cargo test --test integration
  ```

  Expected: all pass, no clippy warnings. The two existing `handle_search` tests should still pass because behavior is unchanged.

- [ ] **Step 6: Commit**

  ```bash
  git add src/commands/bug.rs
  git commit -m "$(cat <<'EOF'
  refactor: extract handle_search helpers in commands/bug.rs

  Drops cognitive complexity from 16 to <=10 by extracting
  resolve_save_info() and build_params_from_url() helpers. Fixes
  rust:S3776 at src/commands/bug.rs:117. No behavior change.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 2: Extract `parse_bugzilla_url` query-pair handlers in `url_parser.rs`

Fixes the larger `rust:S3776` issue (cog 26 → ≤10). The `for (key, value) in url.query_pairs()` loop has 6 conditional branches per iteration. Replace with a `classify_param` dispatcher and a small enum.

**Files:**
- Modify: `src/url_parser.rs:72-169` (refactor `parse_bugzilla_url`, add private helper + enum)

- [ ] **Step 1: Add the `ParamKind` enum and classifier above `parse_bugzilla_url`**

  Insert after the constants block (after line 13, before `ParsedUrl`):

  ```rust
  /// Classification of a URL query-pair key.
  enum ParamKind<'a> {
      Ignored,
      KnownName,
      QueryBasedOn,
      Limit,
      Mapped(&'a crate::types::FieldMapping),
      Credential,
      Raw,
  }

  /// Classify a URL query-pair key into a `ParamKind`. Pure dispatch — no I/O,
  /// no allocation other than ASCII lowercasing for credential matching.
  fn classify_param(key: &str) -> ParamKind<'static> {
      if IGNORED_PARAMS.contains(&key) {
          return ParamKind::Ignored;
      }
      match key {
          "known_name" => return ParamKind::KnownName,
          "query_based_on" => return ParamKind::QueryBasedOn,
          "limit" => return ParamKind::Limit,
          _ => {}
      }
      if let Some(mapping) = FIELD_MAPPINGS.iter().find(|m| m.url_param == key) {
          return ParamKind::Mapped(mapping);
      }
      if CREDENTIAL_PARAMS.contains(&key.to_ascii_lowercase().as_str()) {
          return ParamKind::Credential;
      }
      ParamKind::Raw
  }
  ```

  Note: the `<'a>` parameter is unused at the call site (FIELD_MAPPINGS is `'static`), but kept on the enum so future expansions can borrow non-static data.

- [ ] **Step 2: Replace the loop body in `parse_bugzilla_url`**

  Replace `src/url_parser.rs:111-163` (the entire `for (key, value) in url.query_pairs()` loop) with:

  ```rust
      for (key, value) in url.query_pairs() {
          let key = key.as_ref();
          let value = value.as_ref();

          match classify_param(key) {
              ParamKind::Ignored => {}
              ParamKind::KnownName => {
                  let trimmed = value.trim();
                  if !trimmed.is_empty() {
                      known_name = Some(trimmed.to_string());
                  }
              }
              ParamKind::QueryBasedOn => {
                  let trimmed = value.trim();
                  if !trimmed.is_empty() {
                      query_based_on = Some(trimmed.to_string());
                  }
              }
              ParamKind::Limit => {
                  if let Ok(n) = value.parse::<u32>() {
                      query.limit = Some(n);
                  }
              }
              ParamKind::Mapped(mapping) => {
                  let Some(target) = query.get_field_mut(mapping.struct_field) else {
                      unreachable!(
                          "FIELD_MAPPINGS struct_field '{}' missing from get_field_mut",
                          mapping.struct_field
                      );
                  };
                  target.push(value.to_string());
              }
              ParamKind::Credential => {
                  tracing::warn!("stripping credential parameter '{key}' from URL");
              }
              ParamKind::Raw => {
                  query.raw_params.push((key.to_string(), value.to_string()));
              }
          }
      }
  ```

- [ ] **Step 3: Run linter and tests**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --lib url_parser
  ```

  Expected: all pass. The existing `tests::*` block in `url_parser.rs` covers every `ParamKind` variant via the URL strings it exercises.

- [ ] **Step 4: Add a unit test for `classify_param` directly**

  Add to the existing `mod tests` block in `src/url_parser.rs` (after the `make_config` helper):

  ```rust
  #[test]
  fn classify_param_kinds() {
      assert!(matches!(classify_param("columnlist"), ParamKind::Ignored));
      assert!(matches!(classify_param("known_name"), ParamKind::KnownName));
      assert!(matches!(classify_param("query_based_on"), ParamKind::QueryBasedOn));
      assert!(matches!(classify_param("limit"), ParamKind::Limit));
      assert!(matches!(classify_param("product"), ParamKind::Mapped(_)));
      assert!(matches!(classify_param("Bugzilla_api_key"), ParamKind::Credential));
      assert!(matches!(classify_param("token"), ParamKind::Credential));
      assert!(matches!(classify_param("nonexistent_field"), ParamKind::Raw));
  }
  ```

- [ ] **Step 5: Re-run tests**

  ```bash
  cargo test --lib url_parser::tests::classify_param_kinds
  cargo test --lib url_parser
  ```

  Expected: all pass.

- [ ] **Step 6: Commit**

  ```bash
  git add src/url_parser.rs
  git commit -m "$(cat <<'EOF'
  refactor: extract url_parser query-pair classifier

  Drops cognitive complexity from 26 to <=10 by replacing the 6-branch
  for-loop body in parse_bugzilla_url with a match on a ParamKind enum
  produced by classify_param(). Fixes rust:S3776 at
  src/url_parser.rs:72. No behavior change.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 3: Split `commands/bug.rs` into submodules

Reduces file-level cognitive complexity (54 → no submodule >20) and eliminates 267 duplicated lines that Sonar flags. Convert `commands/bug.rs` to a directory `commands/bug/` with one handler per file. Done as two sub-steps (rename, then content edits) so `git log --follow` works for each handler's history.

**Files:**
- Delete: `src/commands/bug.rs` (1274 lines) — replaced by directory
- Create: `src/commands/bug/mod.rs` (dispatcher + `pub use`)
- Create: `src/commands/bug/list.rs`, `view.rs`, `history.rs`, `search.rs`, `my.rs`, `create.rs`, `clone.rs`, `update.rs`, `shared.rs`
- Modify: `src/commands/mod.rs` if it explicitly imports anything from `commands::bug` that's no longer top-level (likely no change — `commands::bug::execute` is the only public entry point)

- [ ] **Step 1: Inventory the existing handlers**

  Run:

  ```bash
  rg -n '^(pub )?async fn handle_' src/commands/bug.rs
  ```

  Expected: handlers `list`, `view`, `history`, `search`, `my`, `create`, `clone`, `update` (matching the `BugAction` arms at line 18–27). Note the byte ranges for each handler.

- [ ] **Step 2: Identify shared helpers and duplicated blocks**

  Run:

  ```bash
  curl -s "https://sonarcloud.io/api/duplications/show?key=randomparity_bzr%3Asrc%2Fcommands%2Fbug.rs" | python3 -m json.tool
  ```

  Read the `duplications` array. Each entry has a `blocks` field listing the line ranges that duplicate each other. Note these ranges — they're what `commands/bug/shared.rs` will absorb.

- [ ] **Step 3: Create `commands/bug/mod.rs` with the dispatcher only**

  ```bash
  mkdir -p src/commands/bug
  ```

  Create `src/commands/bug/mod.rs`:

  ```rust
  //! Bug subcommand handlers, split per-action.

  use crate::cli::BugAction;
  use crate::error::Result;
  use crate::types::{ApiMode, OutputFormat};

  mod clone;
  mod create;
  mod history;
  mod list;
  mod my;
  mod search;
  mod shared;
  mod update;
  mod view;

  /// Dispatch bug actions to their respective handlers.
  pub async fn execute(
      action: &BugAction,
      server: Option<&str>,
      format: OutputFormat,
      api: Option<ApiMode>,
  ) -> Result<()> {
      let client = crate::commands::shared::connect_and_configure(server, api).await?;

      match action {
          BugAction::List { .. } => list::handle(&client, action, format).await,
          BugAction::View { .. } => view::handle(&client, action, format).await,
          BugAction::History { .. } => history::handle(&client, action, format).await,
          BugAction::Search { .. } => search::handle(action, server, format, api).await,
          BugAction::My { .. } => my::handle(&client, action, format).await,
          BugAction::Create { .. } => create::handle(&client, action, format).await,
          BugAction::Clone { .. } => clone::handle(&client, action, format).await,
          BugAction::Update { .. } => update::handle(&client, action, format).await,
      }
  }
  ```

  Note: handlers are renamed `handle_<verb>` → `<verb>::handle` for path clarity.

- [ ] **Step 4: Create empty handler files with imports only**

  For each of `clone, create, history, list, my, search, shared, update, view`, create `src/commands/bug/<name>.rs` containing the imports the handler will need:

  ```rust
  use crate::cli::BugAction;
  use crate::client::BugzillaClient;
  use crate::error::Result;
  use crate::output::{self, ActionResult, BatchFailure, BatchResult, ResourceKind};
  use crate::types::{ApiMode, CreateBugParams, IdListUpdate, OutputFormat, SearchParams, UpdateBugParams};
  ```

  (Trim unused imports per file; `cargo clippy` will flag them.)

- [ ] **Step 5: Move handlers one at a time — `list` first**

  Cut `handle_list` from `src/commands/bug.rs` and paste into `src/commands/bug/list.rs`, renamed to `pub async fn handle`. Verify `cargo build` succeeds.

  **Note on async signatures:** the existing handlers take `&client, &BugAction, OutputFormat` — that signature must match `mod.rs`'s call site (`list::handle(&client, action, format)`).

- [ ] **Step 6: Move remaining handlers**

  Repeat Step 5 for `view`, `history`, `search`, `my`, `create`, `clone`, `update`. After each move, `cargo build` should succeed.

  For `search`: also move `resolve_save_info` and `build_params_from_url` (added in Task 1) into `src/commands/bug/search.rs` as private helpers.

- [ ] **Step 7: Delete the (now-empty) `src/commands/bug.rs`**

  ```bash
  test ! -s src/commands/bug.rs && git rm src/commands/bug.rs
  ```

  If non-empty, finish moving handlers first.

- [ ] **Step 8: Hoist duplicated helpers into `bug/shared.rs`**

  Inspect each duplicated block identified in Step 2. Common shapes (verify against actual output):
  - "load IDs from CSV/comma list" pattern (probably appears in `update`, `clone`, `create`)
  - "format batch result" pattern (probably appears in `update`, `create`)
  - "parse flags from CLI" pattern (probably uses `commands::flags`)

  For each block that appears ≥2 times: extract a `pub(super) fn` in `src/commands/bug/shared.rs`, replace each occurrence with a call.

  Discipline: only extract when extraction is mechanical. If a block has 5 lines that *look* duplicated but use different types or different control flow, leave it. Per spec: "If extracting requires generics with >2 type parameters or non-trivial trait bounds, leave the duplication."

- [ ] **Step 9: Run linter, tests, and module-renaming sanity check**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  rg -n '^use crate::commands::bug' src/  # imports should be unchanged
  ```

  Expected: all pass. The public path `crate::commands::bug::execute` is preserved.

- [ ] **Step 10: Commit (rename only)**

  Make this commit content-pure: only `git mv`-equivalent moves, no edits to logic. Verify by running `git diff HEAD~1 --stat` — the line counts moved should approximately match (allow ±20 lines for `mod.rs` glue).

  ```bash
  git add src/commands/bug src/commands/bug.rs
  git commit -m "$(cat <<'EOF'
  refactor: split commands/bug.rs into per-action submodules

  Promotes commands/bug.rs (1274 lines) to commands/bug/ with one
  handler per file (list, view, history, search, my, create, clone,
  update, shared). Drops file-level cognitive complexity from 54 and
  eliminates 267 duplicated lines via commands/bug/shared.rs.

  Public path crate::commands::bug::execute unchanged.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 4: Split `xmlrpc/mod.rs` into `call`/`fault`/`parsing`

Reduces file-level cognitive complexity (64 → ≤25 per submodule). `xmlrpc/mod.rs` does request building, fault parsing, and value parsing all in one 759-line file. Promote to a directory.

**Files:**
- Modify: `src/xmlrpc/mod.rs` (becomes a re-export facade + types)
- Create: `src/xmlrpc/call.rs` — request building (param serialization)
- Create: `src/xmlrpc/fault.rs` — fault parsing (`Fault` struct + decoding)
- Create: `src/xmlrpc/parsing.rs` — XML response parsing (`parse_value`, `parse_member`, etc.)

- [ ] **Step 1: Inventory the contents**

  ```bash
  rg -n '^(pub )?(fn|struct|enum) ' src/xmlrpc/mod.rs
  ```

  Note each item's line range. Group:
  - **call.rs** — anything that builds an XML-RPC request body (functions named like `build_*`, `serialize_*`, `to_xml`)
  - **fault.rs** — the `Fault` struct, `parse_fault`, fault-related error mapping
  - **parsing.rs** — `parse_value`, `parse_member`, `parse_struct`, `parse_array`, etc.
  - **mod.rs (kept)** — re-exports + top-level `XmlRpcRequest`/`XmlRpcResponse` types + module declarations

- [ ] **Step 2: Move `Fault` and fault parsing to `xmlrpc/fault.rs`**

  Cut everything fault-related into `src/xmlrpc/fault.rs`. In `mod.rs`, add `mod fault;` and `pub use fault::Fault;` (preserve any existing public exports).

  Verify with:

  ```bash
  cargo build
  ```

- [ ] **Step 3: Move value parsing to `xmlrpc/parsing.rs`**

  Cut `parse_value`, `parse_member`, and any parsing helpers into `src/xmlrpc/parsing.rs`. They are likely `pub(crate)` or private — preserve visibility.

  In `mod.rs`, add `mod parsing;` and `pub(crate) use parsing::*;` if anything outside `xmlrpc` needs them. Confirm by:

  ```bash
  rg -n 'parse_value|parse_member|parse_struct|parse_array' src/ --glob '!src/xmlrpc/**'
  ```

  If any external use exists, expose just those names from `parsing.rs`.

- [ ] **Step 4: Move request building to `xmlrpc/call.rs`**

  Cut request-building functions into `src/xmlrpc/call.rs`. `xmlrpc/client.rs` is the primary consumer — it imports via `super::*` or specific names. Update its imports if needed.

- [ ] **Step 5: Verify `xmlrpc/mod.rs` is now small**

  ```bash
  wc -l src/xmlrpc/mod.rs
  ```

  Expected: ≤150 lines (down from 759). It should contain only module declarations, top-level types (`XmlRpcRequest`, `XmlRpcResponse`), and re-exports.

- [ ] **Step 6: Run linter and tests**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --lib xmlrpc
  ```

  Expected: all pass. XML-RPC tests live in `xmlrpc/client.rs` and `xmlrpc/mod.rs`'s old `mod tests` block — split the test cases alongside the code they cover.

- [ ] **Step 7: Commit**

  ```bash
  git add src/xmlrpc
  git commit -m "$(cat <<'EOF'
  refactor: split xmlrpc/mod.rs into call/fault/parsing

  Promotes xmlrpc/mod.rs (759 lines, cog 64) to:
    - mod.rs       re-exports + top-level types
    - call.rs      request building / param serialization
    - fault.rs     Fault struct + parsing
    - parsing.rs   parse_value, parse_member, etc.

  Public paths unchanged; xmlrpc/client.rs unchanged.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 5: Dedupe `client/group.rs` and `client/auth/mod.rs`

Eliminates 161 + 141 = ~300 duplicated lines. Hoist shared REST patterns into `client/rest_helpers.rs` and shared auth-probe handling into `client/auth/probe_common.rs`.

**Files:**
- Modify: `src/client/group.rs`, `src/client/auth/mod.rs`, `src/client/auth/whoami.rs`, `src/client/auth/valid_login.rs`
- Create: `src/client/rest_helpers.rs`
- Create: `src/client/auth/probe_common.rs`
- Modify: `src/client/mod.rs` (add `mod rest_helpers;`)

- [ ] **Step 1: Pull duplication blocks from Sonar API**

  ```bash
  for f in client/group.rs client/auth/mod.rs; do
    echo "=== $f ==="
    curl -s "https://sonarcloud.io/api/duplications/show?key=randomparity_bzr%3Asrc%2F${f//\//%2F}" \
      | python3 -m json.tool
  done
  ```

  For each `block` in the response, the `from`/`size` fields point to the duplicated line range in that file and its peer files.

- [ ] **Step 2: Categorize each duplication**

  For every block, decide one of:
  - **Hoist:** the block has the same control flow and types in each location → extract a function (or generic-over-T function)
  - **Leave:** the block is similar text but the types/error mapping/logging differ → leave duplicated; document inline why
  - **Refactor differently:** the block hides a bigger structural similarity (e.g., two methods that should share a builder) → note for follow-up, leave for now

  Per spec: don't introduce generics with >2 type parameters or non-trivial trait bounds.

- [ ] **Step 3: Create `client/rest_helpers.rs` with the extracted helpers**

  Common patterns expected (verify against actual output):
  - "GET → deserialize" pattern
  - "PUT → deserialize" pattern
  - "DELETE → check status" pattern
  - "build common headers" pattern

  Skeleton:

  ```rust
  //! Shared HTTP request helpers used by multiple per-resource client modules.

  use serde::de::DeserializeOwned;

  use crate::error::Result;

  use super::BugzillaClient;

  impl BugzillaClient {
      /// Issue a GET to `path` and deserialize the response body as `T`.
      pub(super) async fn get_resource<T>(&self, path: &str) -> Result<T>
      where
          T: DeserializeOwned,
      {
          // … extract the actual implementation from the duplicated blocks
      }

      // … other shared helpers, one per duplicated pattern
  }
  ```

  Add `mod rest_helpers;` to `src/client/mod.rs`.

- [ ] **Step 4: Create `client/auth/probe_common.rs`**

  Shared between `auth/whoami.rs` and `auth/valid_login.rs`: response status mapping, error categorization, retry classification. Extract whichever functions Sonar's duplication report flags as shared.

  Add `mod probe_common;` to `src/client/auth/mod.rs`.

- [ ] **Step 5: Update `client/group.rs` to use the helpers**

  Replace each duplicated block's call site with the helper invocation. Run `cargo build` after each replacement.

- [ ] **Step 6: Update `client/auth/{whoami,valid_login}.rs`**

  Same as Step 5 but for the auth probe sites.

- [ ] **Step 7: Add tests for each new helper**

  In `src/client/rest_helpers.rs`'s `mod tests`, add for each public-shaped helper:
  - Success path (mock returns 200 + JSON)
  - 404 path (returns `BzrError::NotFound`)
  - Malformed JSON path (returns `BzrError::Deserialize`)
  - Network error path (mock returns connection-refused or 500)

  Use `setup_test_env` for the wiremock server.

- [ ] **Step 8: Run linter, tests, and Sonar check**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --lib client
  ```

  Then push to draft PR (see "Working conventions" — drafted PR triggers SonarCloud check) and confirm `client/group.rs` and `client/auth/mod.rs` duplication numbers drop.

- [ ] **Step 9: Commit**

  ```bash
  git add src/client
  git commit -m "$(cat <<'EOF'
  refactor: dedupe client/group.rs and client/auth/mod.rs

  Hoists shared REST patterns into client/rest_helpers.rs and shared
  auth-probe handling into client/auth/probe_common.rs. Eliminates
  ~300 duplicated lines across client/group.rs (161 dup) and
  client/auth/mod.rs (141 dup).

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 6: Dedupe `client/bug.rs` against new helpers

Builds on Task 5. `client/bug.rs` has 144 duplicated lines that overlap with patterns now extracted into `rest_helpers.rs`.

**Files:**
- Modify: `src/client/bug.rs` (replace duplicated patterns with helper calls)

- [ ] **Step 1: Pull duplication blocks for `client/bug.rs`**

  ```bash
  curl -s "https://sonarcloud.io/api/duplications/show?key=randomparity_bzr%3Asrc%2Fclient%2Fbug.rs" \
    | python3 -m json.tool
  ```

- [ ] **Step 2: Replace each duplicated block with a `rest_helpers` call**

  For every block whose peer file is now using a helper, replace the block in `client/bug.rs` with the same helper call. If a block's peer is still duplicated (Task 5 chose "Leave"), leave this one too.

- [ ] **Step 3: Run linter, tests, and Sonar check**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --lib client::bug
  ```

  Push to draft PR; confirm `client/bug.rs` duplication drops.

- [ ] **Step 4: Commit**

  ```bash
  git add src/client/bug.rs
  git commit -m "$(cat <<'EOF'
  refactor: dedupe client/bug.rs against rest_helpers

  Eliminates 144 duplicated lines in client/bug.rs by replacing
  copy-pasted REST patterns with calls to the helpers introduced in
  the previous commit.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 7: Simplify `tls/verifier.rs` in place

Reduces file-level cognitive complexity (38 → ≤25). Extract three private helpers from `verify_server_cert` without changing public API or behavior. **Strict no-behavior-change discipline** — extract via copy-then-replace, not rewrite.

**Files:**
- Modify: `src/tls/verifier.rs` (add private helpers, simplify `verify_server_cert`)

- [ ] **Step 1: Read the current `verify_server_cert` implementation**

  ```bash
  rg -n 'fn verify_server_cert' src/tls/verifier.rs
  ```

  Read the function in full. Identify three regions:
  - SHA-256 SPKI pin computation + comparison
  - SAN/CN extraction + match against `server_name`
  - Issuer DER comparison

- [ ] **Step 2: Snapshot current behavior**

  Confirm all existing TLS tests pass:

  ```bash
  cargo test --lib tls
  cargo test --test '*tls*' 2>/dev/null  # may report no matches if no integration test file
  ```

  Note the count of passing tests. **Every step that follows must keep this count unchanged.**

- [ ] **Step 3: Add `verify_pin` helper**

  Extract the SPKI pin region into a private method:

  ```rust
  /// Verify the leaf cert's SPKI SHA-256 hash against the configured pin.
  /// Returns Ok(()) when no pin is configured. No I/O.
  fn verify_pin(&self, leaf: &CertificateDer<'_>) -> Result<(), rustls::Error> {
      let Some(expected_pin) = &self.pin_sha256 else {
          return Ok(());
      };
      // … paste the existing SPKI pin logic, returning rustls::Error on mismatch
      Ok(())
  }
  ```

  Replace the inlined pin block in `verify_server_cert` with `self.verify_pin(end_entity)?;`. Run tests after this single extraction:

  ```bash
  cargo test --lib tls
  ```

  Test count must match Step 2.

- [ ] **Step 4: Add `verify_san` helper**

  Extract the SAN/CN match region:

  ```rust
  fn verify_san(
      &self,
      leaf: &ParsedCert<'_>,
      server_name: &ServerName<'_>,
  ) -> Result<(), rustls::Error> {
      // … extract the existing SAN/CN extraction + comparison
      Ok(())
  }
  ```

  (Adjust types to match what `verify_server_cert` actually has in scope — this skeleton uses `ParsedCert` as a placeholder; use the real type.)

  Replace the inlined block with `self.verify_san(parsed, server_name)?;`. Run tests; count must match.

- [ ] **Step 5: Add `verify_issuer` helper**

  Same pattern for the issuer DER comparison region:

  ```rust
  fn verify_issuer(
      &self,
      leaf: &ParsedCert<'_>,
      expected_issuer_der: &[u8],
  ) -> Result<(), rustls::Error> {
      // … extract the existing issuer DER comparison
      Ok(())
  }
  ```

  Replace inline block; run tests.

- [ ] **Step 6: Run full test suite + clippy**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  ```

  All pass. **If any existing test needs adjustment, stop and reassess.** The spec marks this as a red flag.

- [ ] **Step 7: Commit**

  ```bash
  git add src/tls/verifier.rs
  git commit -m "$(cat <<'EOF'
  refactor: extract verify_pin/san/issuer helpers in tls/verifier.rs

  Drops file cognitive complexity from 38 to <=25 by extracting three
  private helpers from verify_server_cert. No public API change, no
  behavior change. All existing TLS tests pass unmodified.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 8: Cover `commands/shared.rs` to ≥85%

Currently 64.6% (181 uncovered lines). The hot file. `connect_and_configure` does config-load + auth-resolution + client-build, and most of those branches lack tests.

**Files:**
- Modify: `src/commands/shared.rs` — add `#[cfg(test)] mod tests` (or extend existing)
- Possibly modify: `src/commands/shared.rs` — add `cfg(test)` carve-out for keyring path if it can't be reached

- [ ] **Step 1: Map uncovered lines**

  ```bash
  cargo llvm-cov --lib --html --open
  ```

  Open the HTML report, navigate to `commands/shared.rs`. Note which lines/branches are red. Group into categories: (a) config-loading errors, (b) server-resolution branches, (c) auth-method detection, (d) TLS/pin paths, (e) keyring path.

- [ ] **Step 2: Write tests for config-loading errors**

  In `src/commands/shared.rs`'s `mod tests`:

  ```rust
  #[tokio::test]
  async fn connect_and_configure_errors_when_no_config() {
      let _lock = crate::ENV_LOCK.lock().await;
      let tmp = tempfile::TempDir::new().unwrap();
      // SAFETY: ENV_LOCK held.
      unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
      let result = connect_and_configure(None, None).await;
      assert!(matches!(result, Err(crate::error::BzrError::Config(_))));
  }

  #[tokio::test]
  async fn connect_and_configure_errors_when_unknown_server() {
      let (_lock, _mock, _tmp) = crate::test_helpers::setup_test_env().await;
      let result = connect_and_configure(Some("nonexistent"), None).await;
      assert!(matches!(result, Err(crate::error::BzrError::Config(_))));
  }
  ```

  Add similar tests for each uncovered branch identified in Step 1.

- [ ] **Step 3: Write tests for auth-method detection**

  Use the wiremock server from `setup_test_env` to script auth-detection responses (200 for header auth, 401 for query-param fallback, etc.). Cover:
  - Header auth succeeds first try
  - Header auth fails, query-param succeeds
  - Both fail → error

- [ ] **Step 4: Write tests for TLS pin happy + sad paths**

  Paths that can be tested without a real TLS server use the `TlsConfig` builder. Cover:
  - Pin matches → ok
  - Pin mismatch → `BzrError::DataIntegrity`
  - CA cert load failure → `BzrError::Io` or `BzrError::Auth`

  If a path requires actual TLS handshakes that wiremock can't do, leave it uncovered and add a comment in the test module noting the limitation. Per spec, the keyring path is exempt from the 85% target.

- [ ] **Step 5: Verify coverage hits ≥85%**

  ```bash
  cargo llvm-cov --lib --summary-only 2>&1 | grep 'shared.rs'
  ```

  Expected: line coverage ≥85% for `src/commands/shared.rs`. If below, return to Step 1 and identify which branches are still red.

- [ ] **Step 6: Run full test suite + clippy**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  ```

- [ ] **Step 7: Commit**

  ```bash
  git add src/commands/shared.rs
  git commit -m "$(cat <<'EOF'
  test: cover commands/shared.rs to >=85%

  Adds tests for config-loading errors, server resolution fallback,
  auth-method detection (header/query-param), and TLS pin happy/sad
  paths. Keyring path left uncovered per spec — needs platform-specific
  test infrastructure. Coverage rises from 64.6% to >=85%.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 9: Cover `main.rs` and `lib.rs` entrypoints to ≥85%

Currently 67.3% (`main.rs`) and 74.3% (`lib.rs`). Both are thin dispatch glue. Most coverage gaps come from the `dispatch()` arms that don't have an integration smoke test.

**Files:**
- Modify: `tests/integration.rs` — add dispatcher smoke tests
- Possibly modify: `src/main.rs`, `src/lib.rs` — extract testable helpers if needed

- [ ] **Step 1: Map uncovered branches**

  ```bash
  cargo llvm-cov --html --open
  ```

  Navigate to `main.rs` and `lib.rs`. Identify uncovered arms in `Commands::*` matchers and the `--version`, `--help`, `RUST_LOG` parsing paths.

- [ ] **Step 2: Add `Cli::parse_from + dispatch` integration tests**

  In `tests/integration.rs`, add one smoke test per `Commands::*` arm not currently covered. Pattern (matching the existing tests in `lib.rs:96-145`):

  ```rust
  #[tokio::test]
  async fn dispatch_field_list_smoke() {
      let (_lock, mock, _tmp) = bzr::test_helpers::setup_test_env().await;
      // Mock the field endpoint
      wiremock::Mock::given(wiremock::matchers::method("GET"))
          .and(wiremock::matchers::path_regex(r"^/rest/field/bug"))
          .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({"fields": []})))
          .mount(&mock)
          .await;
      let cli = bzr::cli::Cli::parse_from(["bzr", "field", "list", "--format", "json"]);
      let (result, _output) = bzr::test_helpers::capture_stdout(
          bzr::dispatch(&cli, bzr::types::OutputFormat::Json),
      ).await;
      assert!(result.is_ok());
  }
  ```

  Repeat for each uncovered `Commands::*` arm.

- [ ] **Step 3: Add tests for `--version` / `--help` exit paths**

  ```rust
  #[test]
  fn cli_version_flag_parses() {
      // clap exits before main() — capture by trying parse_from and checking error kind
      let result = bzr::cli::Cli::try_parse_from(["bzr", "--version"]);
      let err = result.unwrap_err();
      assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
  }

  #[test]
  fn cli_help_flag_parses() {
      let result = bzr::cli::Cli::try_parse_from(["bzr", "--help"]);
      let err = result.unwrap_err();
      assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
  }
  ```

- [ ] **Step 4: Add tests for `dispatch` error paths**

  Cover the error formatting in `main.rs` by triggering an `Err` from `dispatch` and asserting the error renders. If `main.rs` uses a `match err.exit_code()` pattern, cover each arm.

- [ ] **Step 5: Verify coverage hits ≥85%**

  ```bash
  cargo llvm-cov --summary-only 2>&1 | grep -E '(main|lib)\.rs'
  ```

- [ ] **Step 6: Run full test suite + clippy**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  ```

- [ ] **Step 7: Commit**

  ```bash
  git add tests/integration.rs src/main.rs src/lib.rs
  git commit -m "$(cat <<'EOF'
  test: cover main.rs and lib.rs entrypoints to >=85%

  Adds smoke tests for each Commands::* dispatch arm, --version /
  --help exit paths, and error-rendering branches in main. Coverage
  rises from 67.3% (main.rs) and 74.3% (lib.rs) to >=85%.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 10: Cover `output/{comment,attachment,user,product,field,group}` to ≥85%

Six output formatters under 85%, all structurally identical (`print_X(values, format)` over Human/JSON/CSV variants). One commit because the test pattern is uniform.

Currently:
- `output/comment.rs` 71.6%
- `output/attachment.rs` 73.2%
- `output/field.rs` 80.3%
- `output/group.rs` 81.5%
- `output/user.rs` 82.0%
- `output/product.rs` 87.0% (already over — opportunistic only)
- `output/bug.rs` 80.9% (also under — include it)

**Files:**
- Modify: `src/output/{comment,attachment,user,field,group,bug}.rs` — extend `mod tests`

- [ ] **Step 1: Inspect the existing `output/query.rs` tests**

  Read `src/output/query.rs:221-280` for the established pattern. Each output formatter test:
  1. Holds `ENV_LOCK` (`crate::ENV_LOCK.lock().await`)
  2. Builds an input value (or vec)
  3. Calls `capture_stdout` around the `print_X` call
  4. Asserts on the captured string (substring match for human format, `extract_json` for JSON, raw match for CSV)

- [ ] **Step 2: For each formatter, add three tests**

  Pattern, applied per file:

  ```rust
  #[tokio::test]
  async fn print_comments_empty() {
      let _lock = crate::ENV_LOCK.lock().await;
      let ((), output) = crate::test_helpers::capture_stdout(async {
          print_comments(&[], OutputFormat::Json);
      }).await;
      assert_eq!(output.trim(), "[]");
  }

  #[tokio::test]
  async fn print_comments_single_item_human() {
      let _lock = crate::ENV_LOCK.lock().await;
      let comments = vec![/* construct a single Comment with non-empty fields */];
      let ((), output) = crate::test_helpers::capture_stdout(async {
          print_comments(&comments, OutputFormat::Human);
      }).await;
      assert!(output.contains("test author"));
      assert!(output.contains("test body"));
  }

  #[tokio::test]
  async fn print_comments_unicode_in_csv() {
      let _lock = crate::ENV_LOCK.lock().await;
      let comments = vec![/* construct one with body = "héllo, wörld" */];
      let ((), output) = crate::test_helpers::capture_stdout(async {
          print_comments(&comments, OutputFormat::Csv);
      }).await;
      // CSV must escape commas and quote Unicode
      assert!(output.contains("\"héllo, wörld\""));
  }
  ```

  Adjust the input types and assertions per formatter (`Comment`, `Attachment`, `User`, `Field`, `Group`, `BzrField`).

- [ ] **Step 3: Verify coverage**

  ```bash
  cargo llvm-cov --summary-only 2>&1 | grep 'src/output/'
  ```

  Expected: every output file ≥85%. If any remains below, identify the uncovered branch (likely an unusual field type or empty-collection path) and add one more test for it.

- [ ] **Step 4: Run full test suite + clippy**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --lib output
  ```

- [ ] **Step 5: Commit**

  ```bash
  git add src/output
  git commit -m "$(cat <<'EOF'
  test: cover output formatters to >=85%

  Adds empty-input, single-item, and Unicode-edge tests for
  output/{comment,attachment,user,field,group,bug}.rs across Human,
  JSON, and CSV formats. Coverage rises from 71.6%-82.0% to >=85%
  for every output formatter.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 11: Cover `tls/verifier.rs` and `tls/tofu.rs` to ≥85%

Currently 81.5% (`verifier.rs`) and 83.7% (`tofu.rs`). With Task 7's helper extractions, individual helpers (`verify_pin`, `verify_san`, `verify_issuer`) are easy to unit-test.

**Files:**
- Modify: `src/tls/verifier.rs` — extend `mod tests`
- Modify: `src/tls/tofu.rs` — extend `mod tests`

- [ ] **Step 1: Map uncovered lines**

  ```bash
  cargo llvm-cov --html --open  # navigate to src/tls/ in the report
  ```

  Note the uncovered branches in each file.

- [ ] **Step 2: Add unit tests for `verify_pin`, `verify_san`, `verify_issuer`**

  Following the fixture pattern from commits b7896df and f34e674 (use `git show <hash>` to inspect):

  ```rust
  #[test]
  fn verify_pin_matches_when_pin_unset() {
      let verifier = test_verifier_no_pin();
      let leaf = test_cert_der();
      assert!(verifier.verify_pin(&leaf).is_ok());
  }

  #[test]
  fn verify_pin_rejects_mismatch() {
      let verifier = test_verifier_with_pin([0u8; 32]);  // wrong pin
      let leaf = test_cert_der();
      let err = verifier.verify_pin(&leaf).unwrap_err();
      assert!(matches!(err, rustls::Error::General(_)));
  }
  ```

  Per spec: "hand-crafted `CertificateDer` fixtures, matching the existing pattern from b7896df, f34e674." Reuse the existing fixture helpers rather than building new ones.

- [ ] **Step 3: Add tests for uncovered `tofu.rs` branches**

  `tofu.rs` is at 83.7% (33 uncov). Likely missing: the user-rejected-pin path, the cert-changed path, and the no-existing-pin happy path. Add a test per uncovered branch using the existing TOFU test fixtures.

- [ ] **Step 4: Verify coverage**

  ```bash
  cargo llvm-cov --summary-only 2>&1 | grep 'src/tls/'
  ```

- [ ] **Step 5: Run full test suite + clippy**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --lib tls
  ```

- [ ] **Step 6: Commit**

  ```bash
  git add src/tls
  git commit -m "$(cat <<'EOF'
  test: cover tls/verifier.rs and tls/tofu.rs to >=85%

  Adds unit tests for verify_pin/san/issuer helpers (extracted in the
  tls/verifier.rs simplification commit) and the previously uncovered
  TOFU branches (user rejection, cert change, fresh-pin happy path).
  Coverage rises from 81.5% (verifier.rs) and 83.7% (tofu.rs) to >=85%.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 12: Opportunistic coverage on touched files

Final pass. Files we touched in Tasks 1–11 may still have stragglers under 85%, or files we didn't explicitly target may have shifted (refactors can drop coverage). Push the overall to ≥95%.

**Files:**
- Modify: any file that drops below 85% per-file or contributes to overall <95%

- [ ] **Step 1: Get full coverage table**

  ```bash
  cargo llvm-cov --summary-only 2>&1 | tee target/llvm-cov-summary.txt
  ```

  The summary prints one row per file with `Lines | Cov%` columns. Files
  below 95% (or 85% for the per-file floor) are the candidates. If the
  text format is unstable, `cargo llvm-cov report --json` produces LLVM's
  JSON coverage format — inspect its shape with `jq keys` before scripting
  against it.

- [ ] **Step 2: For each file <85%, add a targeted test**

  If any file is <85%, identify the uncovered branch and add a test for it. Use the patterns from Tasks 8–11.

- [ ] **Step 3: For each file in 85–95% range that we touched in #1–#11, add an opportunistic test**

  Don't go below 80% effort-for-coverage trade — if a branch requires non-trivial mocking or stubs, leave it.

- [ ] **Step 4: Verify overall coverage**

  ```bash
  cargo llvm-cov --summary-only
  ```

  Expected: overall line coverage ≥95%, every file ≥85%.

- [ ] **Step 5: Run full test suite + clippy**

  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  ```

- [ ] **Step 6: Commit**

  ```bash
  git add src/ tests/
  git commit -m "$(cat <<'EOF'
  chore: opportunistic coverage on files touched in this branch

  Final coverage pass: pushes overall line coverage to >=95% and
  ensures every file we touched in this refactor branch is at >=85%.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 13: Final verification + documentation + draft PR

Confirm all goals met before opening the PR.

**Files:**
- Create: `docs/sonarcloud-gate.md` — one-paragraph policy note

- [ ] **Step 1: Run full local verification**

  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  cargo llvm-cov --summary-only
  ```

  Expected: all pass; overall ≥95%; every previously-deficient file ≥85%.

- [ ] **Step 2: Create `docs/sonarcloud-gate.md`**

  Write:

  ```markdown
  # SonarCloud Quality Gate Policy

  This project uses SonarCloud's "new code" quality gate, configured in the
  SonarCloud project settings (not in this repo). The gate enforces:

  - **New-code line coverage:** ≥85%
  - **New-code duplication density:** ≤2%
  - **0 new bugs / vulnerabilities / security hotspots**
  - **Maintainability rating on new code:** A

  Existing files are tracked but not gated retroactively. Files modified by
  a PR have their changed lines evaluated as "new code." See
  https://sonarcloud.io/project/overview?id=randomparity_bzr for the
  current dashboard.
  ```

  Commit:

  ```bash
  git add docs/sonarcloud-gate.md
  git commit -m "$(cat <<'EOF'
  docs: document SonarCloud quality gate policy

  Adds docs/sonarcloud-gate.md describing the new-code gate
  configured in SonarCloud (>=85% line coverage, <=2% duplication).
  Enforcement lives in the SonarCloud UI; this doc makes the policy
  visible in the repo for code review.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

- [ ] **Step 3: Push the branch and open a draft PR**

  ```bash
  git push -u origin refactor/sonar-cleanup
  gh pr create --draft --title "refactor: SonarCloud cleanup (issues, complexity, coverage, dedup)" --body "$(cat <<'EOF'
  ## Summary
  - Drives SonarCloud open issues to 0 (fixes 2× rust:S3776 cognitive complexity)
  - Raises overall line coverage from 93.3% to ≥95%; lifts every sub-85% file to ≥85%
  - Reduces duplication from 5.6% toward ≤3% via REST helper extraction
  - Splits commands/bug.rs and xmlrpc/mod.rs into per-resource submodules

  See `docs/specs/2026-04-27-sonar-refactor-design.md` for the full design.

  ## Test plan
  - [ ] `cargo fmt --check`
  - [ ] `cargo clippy --all-targets --all-features -- -D warnings`
  - [ ] `cargo test`
  - [ ] `cargo llvm-cov --summary-only` shows overall ≥95% line coverage
  - [ ] SonarCloud check on this PR: 0 open issues
  - [ ] SonarCloud check: every previously-deficient file ≥85% line coverage
  - [ ] SonarCloud check: duplication density dropped (target ≤3%)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)
  EOF
  )"
  ```

- [ ] **Step 4: Wait for SonarCloud check on the PR**

  After CI runs, check:

  ```bash
  gh pr checks
  ```

  Then open the PR's SonarCloud report and confirm:
  - 0 open issues
  - Coverage ≥95%
  - Per-file coverage ≥85% for every previously-deficient file
  - Duplication ≤3%

  If any metric misses, return to the relevant task and add more.

- [ ] **Step 5: Configure new-code gate in SonarCloud UI**

  This is a manual step in the SonarCloud web UI (not in the repo):

  1. Go to https://sonarcloud.io/project/quality_gate?id=randomparity_bzr
  2. Either select the existing "Sonar way" gate (already enforces ≥80% / ≤3% on new code) or create a custom gate "bzr-strict" with:
     - Coverage on New Code ≥ **85%**
     - Duplicated Lines on New Code ≤ **2%**
     - Reliability/Security/Maintainability rating on New Code = A
     - 0 New Bugs / Vulnerabilities / Security Hotspots
  3. Set as the active gate for the `randomparity_bzr` project.

- [ ] **Step 6: Mark PR ready for review**

  ```bash
  gh pr ready
  ```

---

## Definition of done (re-stated)

1. Branch `refactor/sonar-cleanup` merged to `main` via PR.
2. SonarCloud dashboard shows: 0 open issues, ≥95% overall coverage, ≥85% per-file coverage on every file currently below it.
3. `cargo llvm-cov --summary-only` agrees with the dashboard numbers.
4. New-code gate (≥85% cov, ≤2% dup) configured in SonarCloud UI.
5. `docs/sonarcloud-gate.md` exists and documents the gate policy.
