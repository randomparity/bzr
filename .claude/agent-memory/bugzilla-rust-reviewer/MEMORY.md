# bzr Codebase — Reviewer Memory

## Project layout (src/)
- `main.rs` — entry point, tokio runtime, dispatches to commands
- `cli.rs` — clap derive structs (Cli, Commands, *Action enums)
- `client.rs` — BugzillaClient: all HTTP calls, base64 attachment handling
- `config.rs` — Config (TOML at ~/.config/bzr/config.toml), ServerConfig, AuthMethod
- `auth.rs` — resolve_auth_method(), detect_auth_method() for header vs query-param
- `error.rs` — BzrError enum (thiserror), Result alias
- `output.rs` — print_* display functions, OutputFormat (Table/Json)
- `commands/{bug,comment,attachment,config_cmd,product,field,user,group}.rs`

## Key conventions
- Auth: `X-BUGZILLA-API-KEY` header OR `Bugzilla_api_key` query param (server-detected, cached)
- BugzillaClient::new() signature: `(base_url, api_key, auth_method: AuthMethod)`
- All command handlers delegate to `super::shared::build_client(server)` (extracted helper as of group cmd)
- Cargo.toml HAS `[lints.clippy]` with pedantic + panic-prevention lints
- HTTP timeouts ARE set: CONNECT_TIMEOUT=10s, REQUEST_TIMEOUT=30s in client.rs
- `tracing` + `tracing-subscriber` (env-filter feature) are dependencies
- User-facing status uses `tracing::info!`/`warn!` in auth.rs; fatal errors still use `eprintln!` in main.rs

## Recurring issues to watch
- `println!` in output.rs is suppressed via `#[expect(clippy::print_stdout, clippy::expect_used)]`
  on the `mod output;` declaration in main.rs — module-level expect annotation.
- Command handler boilerplate was extracted to `commands/shared.rs::build_client()` — use it
- `serde_json::to_string_pretty(...).expect("serializable to JSON")` is the established pattern
  in output.rs JSON branches — technically infallible, module-level expect suppress applies.
- QueryParam auth sends API key in URL (logged by servers, proxies, shells) — by design
- 48 tests as of feat/api-gap-implementation (10 auth, 30 client, 8 shared::parse_flags)
- New admin methods (create/update product/component/user/group) have NO unit tests — gap to track
- `upload_attachment` and `upload_attachment_with_flags` are near-duplicate; could be unified
- `component` command execute() drops the OutputFormat param — inconsistent with other commands
- Flag parsing (parse_flags in shared.rs) does not support status "X" (clear/delete flag in Bugzilla)
- `search_comment_tags` interpolates user query into URL path segment — no encoding
- auth.rs has its own private `WhoAmIResponse { id: u64 }` struct; client.rs has the public
  `WhoamiResponse` with more fields. Both serve different purposes; not worth consolidating.

## Endpoints implemented (as of feat/api-gap-implementation)
- GET  /rest/bug                     (search; include_fields/exclude_fields/creator/priority/severity/alias/id[])
- GET  /rest/bug/{id}                (get single; id is &str to support aliases)
- POST /rest/bug                     (create)
- PUT  /rest/bug/{id}                (update; flags supported via Vec<FlagUpdate>)
- GET  /rest/bug/{id}/history        (bug history; new_since param)
- GET  /rest/bug/{id}/comment        (new_since param)
- POST /rest/bug/{id}/comment
- PUT  /rest/bug/comment/{id}/tags   (add/remove comment tags; returns Vec<String>)
- GET  /rest/bug/comment/tags/{q}    (search comment tags by prefix — NOTE: user input goes into URL path)
- GET  /rest/bug/{id}/attachment
- GET  /rest/bug/attachment/{id}
- POST /rest/bug/{id}/attachment     (upload, with optional flags)
- PUT  /rest/bug/attachment/{id}     (update attachment metadata and flags)
- GET  /rest/whoami                  (both auth detection and `bzr whoami` command)
- GET  /rest/version                 (bzr server info)
- GET  /rest/extensions              (bzr server info)
- GET  /rest/valid_login             (auth detection fallback, Bugzilla 5.0)
- GET  /rest/product_accessible|selectable|enterable
- GET  /rest/product?ids=...         (chunked at 50)
- GET  /rest/product?names=...
- POST /rest/product                 (create)
- PUT  /rest/product/{name_or_id}    (update)
- GET  /rest/classification/{name}
- POST /rest/component               (REST availability varies by Bugzilla version)
- PUT  /rest/component/{id}
- GET  /rest/field/bug/{field_name}
- GET  /rest/user?match=...
- GET  /rest/user?group=...          (group member listing — param is `group` singular)
- POST /rest/user                    (create; body uses "email" key, which is correct for Bugzilla REST)
- PUT  /rest/user/{login_or_id}      (update; group membership via groups.add/groups.remove)
- GET  /rest/group?names=&membership=1
- POST /rest/group
- PUT  /rest/group/{name_or_id}

## Bugzilla API notes
- `/rest/valid_login` returns `{"result": 1}` (integer) NOT `{"result": true}` (boolean)
  in Bugzilla 5.0. JSON::RPC serializes Perl booleans as integers. Code comparing to
  `serde_json::Value::Bool(true)` will always fail on real Bugzilla 5.0 servers.
- `/rest/whoami` returns `{"id": N, "name": "...", "real_name": "..."}` on success
- ServerConfig now has optional `email` field (for valid_login fallback); stored in config.toml
- `/rest/bug/{id}/history` returns `{"bugs": [{"id": N, "alias": [], "history": [...]}]}`
  HistoryEntry has: who (string), when (ISO 8601 string), changes (array of FieldChange)
  FieldChange has: field_name, removed, added (all default ""), attachment_id (optional u64)
- GET /rest/user with `groups=` filter: Bugzilla docs say the param is `group` (singular),
  not `groups` (plural). The PUT body for group membership uses `{"groups": {"add": [...]}}`
  which IS correct (plural). The query param for listing needs verification.
- `groups` field on BugzillaUser is NOT returned by default; requires include_fields or
  Bugzilla admin permission. The current `get_group_members` requests include_fields correctly.
