# bzr Codebase — Reviewer Memory

## Project layout (src/)
- `main.rs` — entry point, tokio runtime, dispatches to commands
- `cli.rs` — clap derive structs (Cli, Commands, *Action enums)
- `client.rs` — BugzillaClient: all HTTP calls, base64 attachment handling
- `config.rs` — Config (TOML at ~/.config/bzr/config.toml), ServerConfig, AuthMethod
- `auth.rs` — resolve_auth_method(), detect_auth_method() for header vs query-param
- `error.rs` — BzrError enum (thiserror), Result alias; now has exit_code() and error_type()
- `output.rs` — print_* display functions, OutputFormat (Table/Json), print_result()
- `commands/{bug,comment,attachment,config,product,field,user,group,component,...}.rs`

## Key conventions
- Auth: `X-BUGZILLA-API-KEY` header OR `Bugzilla_api_key` query param (server-detected, cached)
- BugzillaClient::new() signature: `(base_url, api_key, auth_method: AuthMethod)`
- All command handlers delegate to `super::shared::build_client(server)` (extracted helper)
- Cargo.toml HAS `[lints.clippy]` with pedantic + panic-prevention lints
- HTTP timeouts ARE set: CONNECT_TIMEOUT=10s, REQUEST_TIMEOUT=30s in client.rs
- `tracing` + `tracing-subscriber` (env-filter feature) are dependencies

## Output / format system (as of feat/config-path-on-mutation)
- `OutputFormat` derives `PartialEq + Eq` (needed for format == Json check in main.rs)
- `resolve_format()` precedence: `--json` > `--output` > `BZR_OUTPUT` env > TTY detect
- TTY auto-detect: non-TTY stdout → JSON; TTY → Table
- `--no-color` + non-TTY both call `colored::control::set_override(false)`
- `colored` v3 handles `NO_COLOR` and `CLICOLOR` env vars natively (no extra code needed)
- `--quiet` calls `suppress_stdout()` (dup2 to /dev/null) inside `run()`, AFTER resolve_format
- Mutation commands use `output::print_result(json_value, human_msg, format)`
- Error output: JSON errors go to stderr as `{"error":{"type":..,"message":..,"exit_code":..}}`
- Exit codes: 0=ok, 1=other, 2=clap (reserved), 3=config/toml, 4=api, 5=http, 6=io

## suppress_stdout() implementation
- Uses `libc::dup2(devnull_fd, STDOUT_FILENO)` — Unix-only, no Windows/cfg guard
- Return value of `dup2` is not checked (non-critical: devnull open failure is handled)
- `std::os::unix::io::AsRawFd` import is local to the function

## Known gaps / issues to track
- `suppress_stdout()` is Unix-only with no `#[cfg(unix)]` guard — won't compile on Windows
- `print_comment_tags()` (SearchTags command) ignores `format` — JSON output broken for search-tags
- `let _ = write!(human, ...)` in commands/config.rs is unconventional; `human.push_str` preferred
- JSON error fallback string in main.rs uses `format!("{e}")` interpolation — invalid JSON if e contains `"`
- `--json` and `--output` conflict silently (`--json` wins); no user-visible error
- No tests for `resolve_format()`, `print_result()`, `exit_code()`, `error_type()`
- New admin methods (create/update product/component/user/group) have NO unit tests
- upload_attachment and upload_attachment_with_flags are near-duplicate
- Flag parsing (parse_flags in shared.rs) does not support status "X" (clear/delete flag)
- `search_comment_tags` interpolates user query into URL path segment — no encoding

## Endpoints implemented (as of feat/config-path-on-mutation)
- GET  /rest/bug, /rest/bug/{id}, POST /rest/bug, PUT /rest/bug/{id}
- GET  /rest/bug/{id}/history, /rest/bug/{id}/comment, POST /rest/bug/{id}/comment
- PUT  /rest/bug/comment/{id}/tags, GET /rest/bug/comment/tags/{q} (unencoded path)
- GET/POST/PUT /rest/bug/{id}/attachment, /rest/bug/attachment/{id}
- GET  /rest/whoami, /rest/version, /rest/extensions, /rest/valid_login
- GET/POST/PUT /rest/product, /rest/classification/{name}
- POST/PUT /rest/component, GET /rest/field/bug/{field_name}
- GET/POST/PUT /rest/user, /rest/group

## Bugzilla API notes
- `/rest/valid_login` returns `{"result": 1}` (integer) in Bugzilla 5.0 (not boolean)
- `/rest/whoami` returns `{"id": N, "name": "...", "real_name": "..."}`
- Bug history: `{"bugs": [{"id": N, "alias": [], "history": [...]}]}`
- `groups` field on BugzillaUser requires include_fields or admin permission

## Recurring issues to watch
- `println!` in output.rs suppressed via `#[expect(clippy::print_stdout, clippy::expect_used)]`
  on `mod output;` in main.rs — module-level expect annotation
- `serde_json::to_string_pretty(...).expect("serializable to JSON")` pattern in output.rs
- QueryParam auth sends API key in URL (by design)
- 67 tests as of feat/config-path-on-mutation
