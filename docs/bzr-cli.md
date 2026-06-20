# bzr CLI Reference

Complete command reference for bzr, a CLI for Bugzilla REST API servers.
For installation and quick start, see [README.md](../README.md).

## Contents

- [Global Options](#global-options)
- [Environment Variables](#environment-variables)
- [Exit Codes](#exit-codes)
- [bug](#bzr-bug----bug-operations)
- [comment](#bzr-comment----comment-operations)
- [attachment](#bzr-attachment----attachment-operations)
- [product](#bzr-product----product-operations)
- [field](#bzr-field----field-value-lookup)
- [user](#bzr-user----user-operations)
- [group](#bzr-group----group-management)
- [whoami](#bzr-whoami)
- [server](#bzr-server----server-diagnostics)
- [classification](#bzr-classification----classification-operations)
- [component](#bzr-component----component-operations)
- [config](#bzr-config----configuration-management)
- [Credential storage](#credential-storage)
- [template](#bzr-template----bug-template-management)
- [query](#bzr-query----saved-query-management)
- [completion](#bzr-completion----shell-completion)
- [Flag Syntax](#flag-syntax)
- [JSON Output](#json-output)
- [Configuration File Format](#configuration-file-format)
- [Authentication](#authentication)
- [API Transport](#api-transport)

## Global Options

| Option | Description |
|--------|-------------|
| `--server <NAME>` | Use a specific server from config instead of the default |
| `--output <FORMAT>` | Output format: `table` or `json`. Defaults to table at a TTY; auto-selects json when stdout is not a TTY. |
| `--json` | Shorthand for `--output json` |
| `--config <PATH>` | Use an alternate `config.toml` for reads and writes. Takes precedence over `BZR_CONFIG`; both override the default config directory. |
| `--no-color` | Disable colored output. Color is also suppressed automatically when stdout is not a TTY. |
| `--quiet` | Suppress stdout and tracing logs (exit code confirms success) |
| `--api <MODE>` | Override API transport: `rest`, `xmlrpc`, or `hybrid`. Auto-detected from server version if not set. |
| `--timeout <SECS>` | Per-request timeout in seconds (default 30). Takes precedence over `BZR_TIMEOUT`. The 10s connect timeout is unaffected. |
| `--retry <N>` | Retry transient failures up to N times with exponential backoff honoring `Retry-After`. 429 and connect failures are retried for any operation; 5xx and read timeouts only for safe reads (GET/HEAD), never for writes (create, update, comment) where a replay could duplicate the effect. Default 0 (disabled); max 10. Exhausted retries exit 5. |
| `--dry-run` | Preview a bug mutation without writing. Resolves and validates the request, then prints the would-be payload and affected bug IDs as `{"resource":"bug","action":"dry-run","ids":[...],"changes":{...}}` instead of calling the write API. Exits 0 on a valid request. Only valid for `bug create`, `update`, `clone`, `resolve`, `close`, `reopen`, and `dup`; on any other command it exits 7. `clone` still reads the source bug to build the preview. |
| `-y, --yes` | Skip the confirmation prompt for a large batch mutation. A `bug update`/`resolve`/`close`/`reopen` targeting more than 10 bugs prompts for confirmation at an interactive terminal; `--yes` bypasses it. Non-interactive runs (piped stdin, agents) never prompt, so this is only needed in an interactive session. |
| `-v, --verbose` | Increase log verbosity (`-v`=info, `-vv`=debug, `-vvv`=trace; `RUST_LOG` overrides) |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

Agent note: at an interactive TTY, `bzr` defaults to table output. For agent workflows, prefer `--json` on read operations so downstream parsing is deterministic.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `BZR_OUTPUT` | Default output format (`table` or `json`). Overridden by `--output` or `--json`. |
| `BZR_CONFIG` | Full path to an alternate `config.toml`. Overrides the default config directory; overridden by `--config`. |
| `BZR_TIMEOUT` | Per-request timeout in seconds. Overridden by `--timeout`; invalid values are ignored with a warning. |
| `NO_COLOR` | Disable colored output (any value). Supported natively by the `colored` crate. |
| `CLICOLOR` | Set to `0` to disable colored output (standard convention respected by the `colored` crate). |
| `CLICOLOR_FORCE` | Set to `1` to force colored output even when stdout is not a TTY. |
| `RUST_LOG` | Override log verbosity (e.g. `bzr=debug`). |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General/unknown error |
| 2 | Resource not found (from bzr), or invalid CLI arguments (from clap, before bzr runs)* |
| 3 | Config or TOML parse error |
| 4 | Bugzilla API or XML-RPC error |
| 5 | HTTP/network error |
| 6 | IO error |
| 7 | Input validation error (e.g. invalid flag syntax, empty comment) |
| 8 | Response deserialization error |
| 9 | Authentication error |
| 10 | Data integrity error (e.g. missing attachment data) |
| 11 | Batch partial failure (some operations succeeded, some failed) |
| 12 | Keyring error (OS keychain access failed, e.g. locked keyring or missing daemon) |
| 13 | TLS error (certificate pin mismatch or issuer changed; use `--tls-pin-now` to re-pin or `--tls-pin-clear` to remove the pin) |

*Exit code 2 is produced by clap for argument errors before bzr's error handling runs, in addition to resource-not-found errors from bzr itself.

## Command Tree

```
bzr [--server <NAME>] [--output table|json] [--json] [--config <PATH>] [--no-color] [--quiet] [--api rest|xmlrpc|hybrid] [--timeout <SECS>] [--retry <N>] [--dry-run] [--yes] [-v...]
├── bug
│   ├── list [--product <P>...] [--component <C>...] [--status <S>...] [--assignee <A>...]
│   │        [--creator <C>...] [--priority <P>...] [--severity <S>...] [--id <ID>...]
│   │        [--alias <A>] [--summary <S>] [--resolution <R>...] [--version <V>...] [--op-sys <OS>...]
│   │        [--platform <P>...] [--whiteboard <W>] [--target-milestone <M>...] [--qa-contact <Q>...] [--url <U>]
│   │        [--limit <N>] [--count] [--fields <F>] [--exclude-fields <F>]
│   │        [--created-since <D>] [--changed-since <D>] [--sort <FIELD>] [--order asc|desc]
│   ├── view <ID> [--fields <F>] [--exclude-fields <F>] [--permissive] [--web]
│   ├── search [<QUERY>] [--from-url <URL>] [--save-as [NAME]] [--limit <N>] [--count] [--fields <F>] [--exclude-fields <F>]
│   │          [--sort <FIELD>] [--order asc|desc]
│   ├── history <ID> [--since <DATE>]
│   ├── my [--created] [--cc] [--all] [--status <S>...] [--limit <N>] [--count]
│   │       [--fields <F>] [--exclude-fields <F>] [--sort <FIELD>] [--order asc|desc]
│   ├── create [--template <T>] [--product <P>] [--component <C>] --summary <S>
│   │          [--version <V>] [--description <D>] [--description-file <PATH>] [--priority <P>] [--severity <S>]
│   │          [--assignee <A>] [--op-sys <OS>] [--rep-platform <PLAT>]
│   │          [--blocks <IDs>] [--depends-on <IDs>] [--alias <A>] [--url <U>]
│   │          [--whiteboard <W>] [--target-milestone <T>] [--deadline <DATE>]
│   │          [--cc <C>...] [--keywords <K>...] [--groups <G>...] [--flag <F>...]
│   ├── clone <ID> [--summary <S>] [--product <P>] [--component <C>] [--version <V>]
│   │              [--description <D>] [--priority <P>] [--severity <S>] [--assignee <A>]
│   │              [--op-sys <OS>] [--rep-platform <PLAT>]
│   │              [--no-comment] [--add-depends-on] [--add-blocks] [--no-cc] [--no-keywords]
│   ├── update <ID...> [--status <S>] [--resolution <R>] [--dupe-of <ID>] [--assignee <A>]
│   │                   [--priority <P>] [--severity <S>] [--summary <S>]
│   │                   [--alias <A>] [--deadline <DATE>] [--estimated-time <HOURS>]
│   │                   [--remaining-time <HOURS>] [--work-time <HOURS>]
│   │                   [--whiteboard <W>] [--reset-assigned-to] [--reset-qa-contact]
│   │                   [--flag <F>...] [--blocks-add <IDs>]
│   │                   [--blocks-remove <IDs>] [--depends-on-add <IDs>]
│   │                   [--depends-on-remove <IDs>] [--keywords-add <K>] [--keywords-remove <K>]
│   │                   [--cc-add <C>] [--cc-remove <C>] [--groups-add <G>] [--groups-remove <G>]
│   │                   [--see-also-add <URL>] [--see-also-remove <URL>]
│   │                   [--comment <BODY>] [--comment-file <PATH>] [--comment-private]
│   ├── resolve <ID...> [--as <RESOLUTION>] [--comment <BODY>] [--comment-file <PATH>] [--comment-private]
│   ├── close <ID...> [--as <RESOLUTION>] [--comment <BODY>] [--comment-file <PATH>] [--comment-private]
│   ├── reopen <ID...> [--comment <BODY>] [--comment-file <PATH>] [--comment-private]
│   └── dup <ID> <TARGET> [--comment <BODY>] [--comment-file <PATH>] [--comment-private]
├── comment
│   ├── list <BUG_ID> [--since <DATE>]
│   ├── add <BUG_ID> [--body <TEXT>] [--body-file <PATH>] [--private]
│   ├── tag <COMMENT_ID> [--add <TAG>...] [--remove <TAG>...]
│   └── search-tags <QUERY>
├── attachment
│   ├── list <BUG_ID>
│   ├── view <ATTACHMENT_ID>
│   ├── download <ATTACHMENT_ID> [--bug <ID>] [-o|--out <FILE>] [--out-dir <DIR>]
│   ├── upload <BUG_ID> <FILE> [--summary <S>] [--content-type <MIME>] [--comment <BODY>]
│   │                          [--comment-private] [--private|--no-private] [--patch|--no-patch] [--flag <F>...]
│   └── update <ATTACHMENT_ID> [--summary <S>] [--file-name <N>] [--content-type <MIME>]
│                               [--obsolete|--no-obsolete] [--patch|--no-patch]
│                               [--private|--no-private] [--flag <F>...]
├── product
│   ├── list [--type <TYPE>]
│   ├── view <NAME>
│   ├── create --name <N> --description <D> [--version <V>] [--is-open <BOOL>]
│   └── update <NAME> [--description <D>] [--default-milestone <M>] [--is-open <BOOL>]
├── field
│   ├── aliases
│   └── list <FIELD_NAME>
├── user
│   ├── search <QUERY> [--details]
│   ├── create --email <E> [--full-name <N>] [--password <P>] [--login <L>]
│   └── update <USER> [--real-name <N>] [--email <E>] [--disable-login <BOOL>]
│                      [--login-denied-text <T>]
├── group
│   ├── add-user --group <G> --user <U>
│   ├── remove-user --group <G> --user <U>
│   ├── list-users --group <G> [--details]
│   ├── view <GROUP>
│   ├── create --name <N> --description <D> [--is-active <BOOL>]
│   └── update <GROUP> [--description <D>] [--is-active <BOOL>]
├── whoami
├── server
│   └── info
├── classification
│   ├── list
│   └── view <NAME>
├── component
│   ├── list --product <P>
│   ├── view <PRODUCT> <COMPONENT>
│   ├── create --product <P> --name <N> --description <D> --default-assignee <E>
│   └── update <ID> [--name <N>] [--description <D>] [--default-assignee <E>]
├── config
│   ├── set-server <NAME> --url <URL> (--api-key <KEY> | --api-key-env <ENV_VAR>) [--email <EMAIL>] [--auth-method <METHOD>]
│   │                     [--tls-insecure] [--tls-ca-cert <PATH>] [--tls-pin-sha256 <HEX>] [--tls-pin-now] [--tls-pin-clear]
│   ├── set-keyring <NAME> [--service <S>] [--account <A>]
│   ├── unset-keyring <NAME>
│   ├── migrate-to-keyring <NAME> [--service <S>] [--account <A>] [--yes]
│   ├── set-default <NAME>
│   ├── remove-server <NAME>
│   ├── rename-server <OLD> <NEW>
│   └── show
├── template
│   ├── save <NAME> [--product <P>] [--component <C>] [--version <V>] [--priority <P>]
│   │               [--severity <S>] [--assignee <A>] [--op-sys <OS>] [--rep-platform <PLAT>]
│   │               [--description <D>]
│   ├── list
│   ├── show <NAME>
│   ├── update <NAME> [--product <P>] [--component <C>] [--version <V>] [--priority <P>]
│   │                 [--severity <S>] [--assignee <A>] [--op-sys <OS>] [--rep-platform <PLAT>]
│   │                 [--description <D>] [--clear <FIELD>]
│   └── delete <NAME>
├── query
│   ├── save <NAME> (--from-url <URL> | [--product <P>...] [--component <C>...] [--status <S>...]
│   │               [--assignee <A>...] [--creator <C>...] [--priority <P>...] [--severity <S>...]
│   │               [--resolution <R>...] [--version <V>...] [--op-sys <OS>...] [--platform <P>...]
│   │               [--whiteboard <W>] [--target-milestone <M>...] [--qa-contact <Q>...] [--url <U>]
│   │               [--search <Q>]) [--limit <N>] [--fields <F>] [--exclude-fields <F>]
│   │               [--created-since <D>] [--changed-since <D>] [--sort <FIELD>] [--order asc|desc]
│   ├── list
│   ├── show <NAME>
│   ├── update <NAME> [--search <Q>] [--product <P>...] [--component <C>...] [--status <S>...]
│   │                 [--assignee <A>...] [--creator <C>...] [--priority <P>...] [--severity <S>...]
│   │                 [--resolution <R>...] [--version <V>...] [--op-sys <OS>...] [--platform <P>...]
│   │                 [--whiteboard <W>...] [--target-milestone <M>...] [--qa-contact <Q>...] [--url <U>...]
│   │                 [--limit <N>] [--fields <F>] [--exclude-fields <F>] [--created-since <D>]
│   │                 [--changed-since <D>] [--clear <FIELD>] [--sort <FIELD>] [--order asc|desc]
│   ├── delete <NAME>
│   └── run <NAME> [--limit <N>] [--fields <F>] [--exclude-fields <F>] [--server <NAME>]
│                  [--resolution <R>...] [--version <V>...] [--op-sys <OS>...] [--platform <P>...]
│                  [--whiteboard <W>] [--target-milestone <M>...] [--qa-contact <Q>...] [--url <U>]
│                  [--created-since <D>] [--changed-since <D>] [--sort <FIELD>] [--order asc|desc]
└── completion <bash|zsh|fish|powershell>
```

---

## `bzr bug` -- Bug Operations

### `bzr bug list`

List bugs matching filter criteria.

```bash
bzr bug list --product Fedora --status ASSIGNED --limit 20
bzr bug list --assignee user@example.com
bzr bug list --product Fedora --fields id,summary,status
bzr bug list --product Fedora --fields id,summary,cf_release
# Select table columns
bzr bug list --product Fedora --fields id,priority,severity,status,summary
bzr bug list --id 100 --id 200 --id 300
bzr bug list --status NEW --status ASSIGNED          # OR: match either status
bzr bug list --status '!CLOSED'                      # NOT: exclude CLOSED
bzr bug list --status NEW --status '!VERIFIED'       # mixed positive and negated
bzr bug list --summary "kernel panic" --product Kernel  # substring on summary
bzr bug list --product Firefox --changed-since 2026-04-01  # filter by date range
```

Filter flags (`--product`, `--component`, `--status`, `--assignee`, `--creator`, `--priority`, `--severity`) are repeatable for OR semantics and support a `!` prefix for negation (NOT).

`--summary` is the structured counterpart to [`bzr bug search`](#bzr-bug-search): it does a substring match against the bug's Summary field across all states (open and closed), whereas `bzr bug search` uses Bugzilla's quicksearch and defaults to open bugs only.

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--product <P>` | No | | Filter by product name (repeatable; `!` prefix to exclude) |
| `--component <C>` | No | | Filter by component name (repeatable; `!` prefix to exclude) |
| `--status <S>` | No | | Filter by status (repeatable; `!` prefix to exclude) |
| `--assignee <A>` | No | | Filter by assignee email (repeatable; `!` prefix to exclude) |
| `--creator <C>` | No | | Filter by bug creator (repeatable; `!` prefix to exclude) |
| `--priority <P>` | No | | Filter by priority (repeatable; `!` prefix to exclude) |
| `--severity <S>` | No | | Filter by severity (repeatable; `!` prefix to exclude) |
| `--id <ID>` | No | | Filter by bug ID (repeatable; `!` negation not supported) |
| `--alias <A>` | No | | Filter by bug alias |
| `--summary <S>` | No | | Substring match on the Summary field (matches all bug states) |
| `--limit <N>` | No | 50 | Max results |
| `--count` | No | | Print only the count of matching bugs — an integer (table) or `{"count": N}` (JSON). Fetches ids only and lifts the row limit, so the count reflects all matches (bounded by the server's max-results setting). Ignores `--fields`, `--limit`, and `--sort`. |
| `--fields <F>` | No | | Comma-separated built-in fields or Bugzilla custom fields named `cf_*` requested from the server; in table output, selects which columns to show (in order). Under `--json`, the object contains only the selected fields (gh-style; `id` is included only when requested). A selection that resolves to no known fields is rejected with exit code 7 rather than emitting an empty object. |
| `--exclude-fields <F>` | No | | Comma-separated fields dropped from the server request; in table output, removes those columns. Under `--json`, the object omits the dropped fields (including custom `cf_*` fields and `id`, when excluded). Excluding every field is rejected with exit code 7 rather than emitting `{}`. |
| `--created-since <DATE>` | No | | Filter to bugs whose `creation_time` is `>= DATE`. See [Date format](#date-format) below. |
| `--changed-since <DATE>` | No | | Filter to bugs whose `last_change_time` is `>= DATE`. See [Date format](#date-format) below. |
| `--sort <FIELD>` | No | | Sort results by `FIELD` (e.g. `last_change_time`, `priority`, `bug_id`). Field-name aliases (`id`, `severity`, `status`, ...) are accepted. See [Result ordering](#result-ordering) below. |
| `--order asc\|desc` | No | `asc` | Sort direction; only meaningful with `--sort`. |

#### Result ordering

`--sort`/`--order` map to Bugzilla's `order` parameter and apply to `bug list`,
`bug search`, `bug my`, and `query run`. A `bug_id` tiebreaker is always
appended so ties resolve deterministically. **Absent `--sort`, results default
to a stable `bug_id` order** so identical runs return rows in the same order —
this means `bug search` no longer relies on Bugzilla's relevance ranking by
default; pass `--sort` to choose a different order. For `bug search --from-url`
and `query run`, an explicit `--sort` overrides any ordering carried by the URL
or saved query; otherwise the saved/URL order is preserved. `query save --sort`
persists an order into the saved query.

#### Date format

`--created-since` and `--changed-since` accept ISO 8601 datetimes (`YYYY-MM-DDTHH:MM:SS`, `YYYY-MM-DDTHH:MM:SSZ`, or `YYYY-MM-DDTHH:MM:SS±HH:MM`) or a bare `YYYY-MM-DD`. Bare dates are treated as `00:00:00 UTC`. Fractional seconds, week dates, and ordinal dates are rejected with exit code 7. The same validator is used by `bzr bug history --since` and `bzr comment list --since`.

#### Field selection and custom fields

`--fields` accepts built-in bug fields and Bugzilla custom fields whose names
start with `cf_`. Custom fields are not fetched by default; request them
explicitly, for example `--fields id,summary,cf_release`. If Bugzilla omits a
requested custom field, it is omitted from JSON output and rendered as an empty
table cell. Unknown non-custom field names warn or fail as described above.

#### Additional field filters (issue #158)

Eight additional field filters are accepted, each repeatable for OR
within a field, AND across fields, with `!`-prefix to invert:

| Flag | Match style | Negation operator |
| --- | --- | --- |
| `--whiteboard` | substring | `notsubstring` |
| `--target-milestone` | exact | `notequals` |
| `--version` | exact | `notequals` |
| `--op-sys` | exact | `notequals` |
| `--platform` | exact | `notequals` |
| `--resolution` | exact (empty matches open) | `notequals` |
| `--qa-contact` | exact | `notequals` |
| `--url` | substring | `notsubstring` |

Examples:

```sh
bzr bug list --whiteboard 'needs-review'
bzr bug list --whiteboard '!wip' --resolution '!FIXED'
bzr bug list --version 9.4 --version 9.5 --op-sys Linux
```

The `--platform` flag matches the Bugzilla `Bug.search` API
parameter name. The output-side bug field is `rep_platform` (and
`bzr bug create` accepts `--rep-platform` for the input side of bug
creation); the search/list filter uses `--platform` to match
upstream Bugzilla and `bzl-search`.

### `bzr bug view`

Display detailed information about one or more bugs.

Under `--json` the returned object is trimmed to the selected fields
(gh-style) on every transport, since trimming happens client-side after
the fetch. On XML-RPC servers, single-bug `bzr bug view` fetches the full
bug regardless of `--fields`/`--exclude-fields`, so there the selection
only controls which detail rows (table) or object keys (JSON) appear, not
what is sent over the wire.

```bash
bzr bug view 12345
bzr bug view 12345 12346 12347
bzr bug view 12345 my-alias 12347 --permissive
bzr bug view 12345 --web
bzr --json bug view 12345
bzr --json bug view 12345 12346 | jq '.bugs[].summary'
bzr bug view my-alias --fields id,summary,assigned_to
```

| Option | Required | Description |
|--------|----------|-------------|
| `<IDS>...` | Yes | One or more bug IDs or aliases. Aliases and numeric IDs may be mixed. |
| `--web` | No | Open each bug's web page (`show_bug.cgi?id=<ID>`) on the active server in the default browser instead of printing its record. Resolves the URL from local config — no network call or authentication, so it works before credentials are set. When stdout is not a terminal or no display is available (headless / SSH without X), the URL is printed to stdout and the command exits 0 instead of opening a browser. `--fields` and `--permissive` are ignored with `--web`. |
| `--permissive` | No | Multi-ID only. Continue past per-bug failures, surfacing them as `Bug #N — UNAVAILABLE` placeholder rows (table) or entries in `failed` (JSON). Exit 0 even if some bugs fail. Has no effect on session-wide failures (transport, auth, security) — those still bail. Setting `--permissive` with a single ID returns input-validation error (exit 7). |
| `--fields <F>` | No | Comma-separated built-in fields or Bugzilla custom fields named `cf_*` requested from the server; in table output, selects which detail rows to show. Under `--json`, the object contains only the selected fields (gh-style; `id` is included only when requested). |
| `--exclude-fields <F>` | No | Comma-separated fields dropped from the server request; in table output, removes those detail rows. Under `--json`, the object omits the dropped fields (including custom `cf_*` fields and `id`, when excluded). |

**Output shapes:**

- **Single-ID, table:** detail block (status, priority, assignee, etc.).
- **Single-ID, `--json`:** bare `Bug` object, trimmed to `--fields`/`--exclude-fields` when given (the full object otherwise). The wrapper shape is unchanged from prior versions.
- **Multi-ID, table:** one detail block per bug in argument order, separated by a `─` divider line. Inaccessible bugs (under `--permissive`) appear as `Bug #N — UNAVAILABLE` blocks.
- **Multi-ID, `--json`:** wrapped object `{"bugs": [...], "failed": [...]}`. The `failed` array is always present (empty when there are no failures) so `jq` consumers can rely on `.bugs[]` regardless of whether `--permissive` was passed.

> **Note:** Under `--json`, `bzr bug view` stays lenient when the field
> selection resolves to nothing known — an unknown/mistyped `--fields`, or an
> `--exclude-fields` covering every field — emitting an empty `{}` object and
> exiting 0 with a one-line stderr warning. The list-style commands
> (`bzr bug list`, `bzr bug my`, `bzr bug search`, `query run`) instead exit 7
> for the same mistake. A `{}` result with a zero exit can therefore mean a
> field name was misspelled; check stderr.

### `bzr bug search`

Search bugs using Bugzilla's quicksearch syntax, or execute a search from a Bugzilla buglist.cgi URL.

```bash
bzr bug search "kernel panic"
bzr bug search "ALL kernel panic"                      # include closed/resolved bugs
bzr bug search "component:NetworkManager priority:high" --limit 10
bzr bug search "memory leak" --fields id,summary
bzr bug search "memory leak" --fields id,summary,cf_release
bzr bug search --from-url "https://bugzilla.example.com/buglist.cgi?product=Firefox&bug_status=NEW"
bzr bug search --from-url "https://bugzilla.example.com/buglist.cgi?product=Firefox&bug_status=NEW" --save-as "my-query"
bzr bug search --from-url "https://bugzilla.example.com/buglist.cgi?known_name=my%20search&product=Firefox" --save-as
```

> **Note:** Bugzilla's quicksearch defaults to OPEN bugs only. Prepend the bare token `ALL` to the query to include closed/resolved bugs. For a Summary-field-only substring match across all bug states with no quicksearch tokenization or status defaults at play, use [`bzr bug list --summary <text>`](#bzr-bug-list); quicksearch additionally searches description and comments, so `ALL <term>` is broader than `--summary <term>` against the same term.

`--from-url` and the positional `<QUERY>` argument are mutually exclusive.

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `<QUERY>` | No* | | Search query (quicksearch syntax) |
| `--from-url <URL>` | No* | | Execute a search from a Bugzilla buglist.cgi URL. Recognized parameters (product, component, status, etc.) are mapped to structured fields; unrecognized parameters (boolean charts, field-change filters) are passed through to the REST API verbatim. |
| `--save-as [NAME]` | No | | Save this URL query for future reuse. If `NAME` is omitted, uses the URL's `known_name` parameter as the query name. Requires `--from-url`. |
| `--limit <N>` | No | 50 | Max results. When `--from-url` is used, the URL's own limit parameter takes precedence unless overridden here. |
| `--count` | No | | Print only the count of matching bugs — an integer (table) or `{"count": N}` (JSON). Counts all matches (bounded by the server's max-results setting). |
| `--fields <F>` | No | | Comma-separated built-in fields or Bugzilla custom fields named `cf_*` requested from the server; in table output, selects which columns to show (in order). Under `--json`, the object contains only the selected fields (gh-style; `id` is included only when requested). A selection that resolves to no known fields is rejected with exit code 7 rather than emitting an empty object. |
| `--exclude-fields <F>` | No | | Comma-separated fields dropped from the server request; in table output, removes those columns. Under `--json`, the object omits the dropped fields (including custom `cf_*` fields and `id`, when excluded). Excluding every field is rejected with exit code 7 rather than emitting `{}`. |

*One of `<QUERY>` or `--from-url` must be provided.

### `bzr bug history`

View the change history of a bug, showing who changed which fields and when.

```bash
bzr bug history 12345
bzr bug history 12345 --since 2025-01-01
bzr --json bug history 12345
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ID>` | Yes | Bug ID |
| `--since <DATE>` | No | Only show changes after this date (ISO 8601) |

### `bzr bug my`

Show bugs related to the authenticated user. Defaults to bugs assigned to you.

```bash
bzr bug my                    # bugs assigned to me
bzr bug my --created          # bugs I created
bzr bug my --cc               # bugs I'm CC'd on
bzr bug my --all              # all of the above
bzr bug my --status NEW --limit 20
bzr bug my --all --status '!CLOSED'         # all non-closed bugs
bzr bug my --status NEW --status ASSIGNED   # OR filter
bzr bug my --status NEW --status '!RESOLVED'  # mixed positive and negated
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--created` | No | | Show bugs I created (instead of assigned) |
| `--cc` | No | | Show bugs I'm CC'd on (instead of assigned) |
| `--all` | No | | Show all bugs related to me (assigned + created + CC'd) |
| `--status <S>` | No | | Filter by status (repeatable; `!` prefix to exclude) |
| `--limit <N>` | No | 50 | Max results per category. With `--all`, each of the three categories (assigned, created, CC'd) is queried separately up to this limit; duplicates across categories are removed. |
| `--count` | No | | Print only the count of distinct matching bugs (deduped across the active categories) — an integer (table) or `{"count": N}` (JSON). |
| `--fields <F>` | No | | Comma-separated built-in fields or Bugzilla custom fields named `cf_*` requested from the server; in table output, selects which columns to show (in order). Under `--json`, the object contains only the selected fields (gh-style; `id` is included only when requested). A selection that resolves to no known fields is rejected with exit code 7 rather than emitting an empty object. |
| `--exclude-fields <F>` | No | | Comma-separated fields dropped from the server request; in table output, removes those columns. Under `--json`, the object omits the dropped fields (including custom `cf_*` fields and `id`, when excluded). Excluding every field is rejected with exit code 7 rather than emitting `{}`. |

### `bzr bug create`

File a new bug.

```bash
bzr bug create --product Fedora --component kernel \
  --summary "Boot failure on 6.x" \
  --description "System hangs at initramfs" \
  --priority high --severity major

# Read the description from a file
bzr bug create --product Fedora --component kernel \
  --summary "Boot failure" \
  --description-file /tmp/desc.txt

# Pipe the description from stdin
echo "long-form description" | bzr bug create \
  --product Fedora --component kernel --summary "Boot failure"

# Compose interactively in $EDITOR (no --summary or --description)
bzr bug create --product Fedora --component kernel

bzr bug create --template security-bug --summary "XSS in login form"
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--product <P>` | Yes* | | Product name |
| `--component <C>` | Yes* | | Component name |
| `--summary <S>` | Yes** | | One-line summary |
| `--version <V>` | No | "unspecified" | Version |
| `--description <D>` | No | | Full description (mutually exclusive with `--description-file`) |
| `--description-file <PATH>` | No | | Read the description from a UTF-8 file (mutually exclusive with `--description`) |
| `--priority <P>` | No | | Priority level |
| `--severity <S>` | No | | Severity level |
| `--assignee <A>` | No | | Assignee email |
| `--op-sys <OS>` | No | | Operating system (required by some Bugzilla installations) |
| `--rep-platform <PLAT>` | No | | Hardware platform (required by some Bugzilla installations) |
| `--blocks <IDs>` | No | | Bug IDs this bug blocks (comma-separated) |
| `--depends-on <IDs>` | No | | Bug IDs this bug depends on (comma-separated) |
| `--alias <A>` | No | | Set an alias for the new bug |
| `--url <U>` | No | | Set the URL field |
| `--whiteboard <W>` | No | | Set the Status Whiteboard |
| `--target-milestone <T>` | No | | Set the Target Milestone |
| `--deadline <DATE>` | No | | Set the deadline (`YYYY-MM-DD`); malformed input exits 7 |
| `--cc <C>` | No | | Add CC entries (comma-separated, repeatable) |
| `--keywords <K>` | No | | Add keywords (comma-separated, repeatable) |
| `--groups <G>` | No | | Add the bug to these groups (comma-separated, repeatable) |
| `--flag <F>` | No | | Set/request a flag using Bugzilla flag syntax (repeatable): `name+`, `name-`, `name?`, `name?(user@example.com)` |
| `--template <T>` | No | | Name of a saved template to use for default field values |

These field flags give `bzr bug create` parity with `bzr bug update` for the subset Bugzilla's `Bug.create` accepts, so a bug and its metadata are filed in a single API call instead of a create-then-update two-step.

*Required unless a template provides the value.
**`--summary` is required unless the editor flow is active. The editor flow opens `$EDITOR` (or `vi` fallback) with a templated buffer when stdin is a TTY and no description source is supplied; the first non-empty line above the buffer's `# ------------------------ >8 ------------------------` sentinel divider becomes the summary, the rest becomes the description.

#### Description source precedence

Highest priority first:

1. `--description "text"` — literal value.
2. `--description-file PATH` — UTF-8 file contents.
3. Piped stdin — when stdin is not a TTY (e.g. `echo body | bzr bug create ...`).
4. `$EDITOR` — when stdin is a TTY and no explicit source is supplied. The buffer is pre-filled with `--summary` (if given) and any saved-template `description` body, followed by a `git commit -v`-style sentinel divider with informational field reminders.

A value of `-` for `--description` or `--description-file` reads the description from stdin (e.g. `... --description -`).

`--description` and `--description-file` are mutually exclusive (clap rejects with exit code 2). An empty piped stdin (when no other source is supplied) aborts with exit code 7.

#### Exit codes (this command)

| Code | Condition |
|------|-----------|
| 0 | Success |
| 2 | Conflicting flags (e.g. `--description` and `--description-file` both set) |
| 4 | Bugzilla API error (e.g. server requires `--op-sys` and it wasn't provided) |
| 7 | Input validation: missing `--summary` outside the editor flow; missing or unreadable `--description-file`; empty stdin without an explicit description; empty editor buffer; `$EDITOR` exited non-zero |
| 9 | Authentication failure |

Agent note: agent workflows should pass `--description` (or `--description-file`) explicitly and supply `--summary`. The `$EDITOR` flow only fires when stdin is a TTY, which is rare in headless / CI invocations.

### `bzr bug clone`

Clone an existing bug, copying its fields into a new bug. Override flags (`--summary`, `--product`, `--component`, `--version`, `--description`, `--priority`, `--severity`, `--assignee`, `--op-sys`, `--rep-platform`) take precedence over values copied from the source.

```bash
bzr bug clone 12345
bzr bug clone 12345 --summary "Variant: different environment"
bzr bug clone 12345 --component NewComponent --add-depends-on
bzr bug clone 12345 --no-comment --no-cc
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ID>` | Yes | Source bug ID or alias |
| `--summary <S>` | No | Override summary (copies from source if omitted) |
| `--product <P>` | No | Override product |
| `--component <C>` | No | Override component |
| `--version <V>` | No | Override version (copies from source if omitted) |
| `--description <D>` | No | Override description (copies comment #0 from source if omitted) |
| `--priority <P>` | No | Override priority |
| `--severity <S>` | No | Override severity |
| `--assignee <A>` | No | Override assignee |
| `--op-sys <OS>` | No | Override operating system |
| `--rep-platform <PLAT>` | No | Override hardware platform |
| `--no-comment` | No | Skip the "Cloned from bug #N" comment |
| `--add-depends-on` | No | Make the new bug depend on the source bug |
| `--add-blocks` | No | Make the new bug block the source bug |
| `--no-cc` | No | Don't copy the CC list from the source bug |
| `--no-keywords` | No | Don't copy keywords from the source bug |

Agent note: cloning without overrides copies metadata from the source bug, which may be broader than an agent intends. For predictable automation, use explicit overrides such as `--summary`, `--component`, `--description`, `--no-cc`, or `--no-keywords`.

### `bzr bug update`

Modify fields on an existing bug. Supports multiple IDs for batch updates.

A comment may be posted atomically with the update via `--comment` or `--comment-file`; this avoids the need for a separate `bzr comment add` call. A value of `-` for either flag reads the comment from stdin.

```bash
bzr bug update 12345 --status ASSIGNED --assignee dev@example.com
bzr bug update 12345 --status RESOLVED --resolution FIXED
bzr bug update 12345 --dupe-of 67890
bzr bug update 12345 --deadline 2026-12-31 --estimated-time 3.5
bzr bug update 12345 --work-time 0.5 --remaining-time 1.25
bzr bug update 12345 --reset-assigned-to --reset-qa-contact
bzr bug update 12345 --flag "review?(alice@example.com)"
bzr bug update 12345 --blocks-add 100,200 --depends-on-add 50
bzr bug update 12345 --keywords-add fix-needed,regression \
    --cc-add alice@example.com
bzr bug update 12345 --see-also-add https://example.com/issue/42 \
    --see-also-add https://other.example/bug/7
bzr bug update 12345 --status RESOLVED --resolution FIXED \
    --comment "Fixed by patch in #200"
bzr bug update 100 200 300 --status RESOLVED --resolution WONTFIX
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ID...>` | Yes | Bug ID(s) — pass multiple for batch updates |
| `--status <S>` | No | New status |
| `--resolution <R>` | No | Resolution (FIXED, WONTFIX, DUPLICATE, etc.) |
| `--dupe-of <ID>` | No | Mark this bug as a duplicate of another bug; Bugzilla sets status/resolution |
| `--alias <ALIAS>` | No | Set this bug's alias; only valid for single-bug updates |
| `--deadline <DATE>` | No | Set deadline date (`YYYY-MM-DD`) |
| `--estimated-time <HOURS>` | No | Set total estimated work time in hours |
| `--remaining-time <HOURS>` | No | Set remaining work time in hours |
| `--work-time <HOURS>` | No | Add work time in hours for this update |
| `--reset-assigned-to` | No | Reset assignee to the component default |
| `--reset-qa-contact` | No | Reset QA contact to the component default |
| `--assignee <A>` | No | Reassign to email |
| `--priority <P>` | No | Set priority |
| `--severity <S>` | No | Set severity |
| `--summary <S>` | No | Update summary text |
| `--whiteboard <W>` | No | Set whiteboard text |
| `--flag <F>` | No | Set flags (repeatable; see [Flag Syntax](#flag-syntax)) |
| `--blocks-add <IDs>` | No | Add bug IDs to the blocks list (comma-separated) |
| `--blocks-remove <IDs>` | No | Remove bug IDs from the blocks list (comma-separated) |
| `--depends-on-add <IDs>` | No | Add bug IDs to the depends-on list (comma-separated) |
| `--depends-on-remove <IDs>` | No | Remove bug IDs from the depends-on list (comma-separated) |
| `--keywords-add <K>` | No | Add keywords (comma-separated) |
| `--keywords-remove <K>` | No | Remove keywords (comma-separated) |
| `--cc-add <U>` | No | Add CC entries (comma-separated; usernames or emails) |
| `--cc-remove <U>` | No | Remove CC entries (comma-separated) |
| `--groups-add <G>` | No | Add groups (comma-separated; requires permission) |
| `--groups-remove <G>` | No | Remove groups (comma-separated; requires permission) |
| `--see-also-add <URL>` | No | Add a see-also URL (repeat for multiple; no comma-list) |
| `--see-also-remove <URL>` | No | Remove a see-also URL (repeat for multiple) |
| `--comment <BODY>` | No | Post a comment atomically with the field changes; `-` reads stdin (mutually exclusive with `--comment-file`) |
| `--comment-file <PATH>` | No | Read the comment body from a UTF-8 file; `-` reads stdin (mutually exclusive with `--comment`; missing or non-UTF-8 paths exit 7) |
| `--comment-private` | No | Mark the comment private (requires `--comment` or `--comment-file`) |

When updating multiple bugs, failures on individual bugs do not abort the batch. A summary is printed showing which bugs succeeded and which failed.

Agent note: before automated status, priority, severity, or resolution changes, validate allowed values with `bzr field list <field>`, for example `bzr field list status` or `bzr field list resolution`.

### `bzr bug resolve` / `close` / `reopen` / `dup`

Convenience verbs — thin sugar over `bzr bug update` for the common state
transitions, so you don't have to spell out `--status`/`--resolution` each
time. Each accepts multiple IDs (batch, except `dup`) and the same
`--comment` / `--comment-file` / `--comment-private` flags as `bug update`,
posting the comment atomically with the change. Batch behavior (per-bug
partial-failure reporting, exit code 11) is inherited from `bug update`.

```bash
bzr bug resolve 12345                       # → update --status RESOLVED --resolution FIXED
bzr bug resolve 12345 12346 --as WONTFIX    # batch, custom resolution
bzr bug close 12345 --comment "Shipped"     # → update --status CLOSED (resolution preserved)
bzr bug close 12345 --as INVALID            # close an open bug with a resolution
bzr bug reopen 12345                         # → update --status REOPENED
bzr bug dup 12345 100                        # → update --dupe-of 100
```

| Verb | Equivalent `update` | Notes |
|------|---------------------|-------|
| `resolve <ID...> [--as <R>]` | `--status RESOLVED --resolution <R>` | `--as` defaults to `FIXED` |
| `close <ID...> [--as <R>]` | `--status CLOSED [--resolution <R>]` | resolution set only when `--as` is given, otherwise the existing one is preserved |
| `reopen <ID...>` | `--status REOPENED` | Bugzilla clears the resolution automatically |
| `dup <ID> <TARGET>` | `--dupe-of <TARGET>` | Bugzilla sets RESOLVED/DUPLICATE automatically |

The status strings (`RESOLVED`, `CLOSED`, `REOPENED`) follow the conventional
Bugzilla workflow; on a server with a customized workflow, an unsupported
transition fails with the same server error `bug update` would return.

---

## `bzr comment` -- Comment Operations

### `bzr comment list`

List all comments on a bug.

```bash
bzr comment list 12345
bzr comment list 12345 --since 2025-06-01
bzr --json comment list 12345
```

| Option | Required | Description |
|--------|----------|-------------|
| `<BUG_ID>` | Yes | Bug ID |
| `--since <DATE>` | No | Only show comments after this date (ISO 8601) |

### `bzr comment add`

Add a comment to a bug. The body is resolved in this order: `--body <TEXT>`, `--body-file <PATH>`, piped stdin, then `$EDITOR` (falls back to `vi`) at a TTY. A value of `-` for `--body` or `--body-file` reads from stdin. `--body` and `--body-file` are mutually exclusive (clap rejects with exit code 2). Add `--private` to mark the comment as visible only to users with elevated permissions on the server.

```bash
bzr comment add 12345 --body "Confirmed on Fedora 42"
bzr comment add 12345                                 # opens editor
echo "Automated comment" | bzr comment add 12345      # reads stdin
echo "Automated comment" | bzr comment add 12345 --body -   # explicit stdin
bzr comment add 12345 --body-file notes.txt           # reads a file
bzr comment add 12345 --body "internal note" --private  # private
```

| Option | Required | Description |
|--------|----------|-------------|
| `<BUG_ID>` | Yes | Bug ID |
| `--body <TEXT>` | No | Comment text; `-` reads stdin. Reads piped stdin or opens `$EDITOR` if both body flags omitted |
| `--body-file <PATH>` | No | Read the comment body from a UTF-8 file; `-` reads stdin (mutually exclusive with `--body`; missing or non-UTF-8 paths exit 7) |
| `--private` | No | Mark the comment as private (visible only to users with elevated permissions) |

Agent note: `bzr comment add <BUG_ID>` without `--body` is not agent-friendly at a TTY because it opens `$EDITOR`. Prefer `bzr comment add <BUG_ID> --body "text"`. If you already have generated text on stdin, `echo "text" | bzr comment add <BUG_ID>` (or `--body -`) is also safe.

### `bzr comment tag`

Add or remove tags on a comment.

```bash
bzr comment tag 98765 --add spam
bzr comment tag 98765 --remove spam
bzr comment tag 98765 --add needs-info --add follow-up
```

| Option | Required | Description |
|--------|----------|-------------|
| `<COMMENT_ID>` | Yes | Comment ID |
| `--add <TAG>` | No | Tags to add (repeatable) |
| `--remove <TAG>` | No | Tags to remove (repeatable) |

### `bzr comment search-tags`

Search for comments by tag.

```bash
bzr comment search-tags spam
bzr --json comment search-tags needs-info
```

| Option | Required | Description |
|--------|----------|-------------|
| `<QUERY>` | Yes | Tag to search for |

---

## `bzr attachment` -- Attachment Operations

### `bzr attachment list`

List all attachments on a bug.

```bash
bzr attachment list 12345
bzr --json attachment list 12345
```

### `bzr attachment view`

Show a single attachment's metadata by attachment ID — summary, bug, file
name, content type, size, flags (patch/obsolete/private), creator, and
timestamps — **without** downloading its bytes. On REST the `data` field is
excluded server-side, so inspecting a large attachment is cheap. The `data`
field is omitted from `--json` output.

```bash
bzr attachment view 9876
bzr --json attachment view 9876 | jq '.summary, .size'
```

### `bzr attachment download`

Download one or more attachments to disk.

**Synopsis:**

```
bzr attachment download <ID>...
bzr attachment download <ID> --out <PATH>
bzr attachment download [<ID>...] --bug <BUG_ID>... [--out-dir <DIR>]
```

**Arguments and flags:**

| flag | description |
|---|---|
| `<ID>` | Attachment ID(s). Repeatable as positional arguments. |
| `--bug <BUG_ID>` | Download every attachment for the given bug. Repeatable. |
| `-o`, `--out <PATH>` | Output file path. Single-attachment shape only; conflicts with `--out-dir` and `--bug`. |
| `--out-dir <DIR>` | Output directory for batch downloads. Default: `./attachments`. Files land at `<out-dir>/<bug-id>/<att-id>.<file_name>`. |

**Examples:**

```bash
# Single attachment, original filename in cwd
bzr attachment download 9876

# Single attachment, custom path
bzr attachment download 9876 --out patch.diff

# Multiple attachment IDs into a directory
bzr attachment download 9876 9877 9878 --out-dir /tmp/patches

# Every attachment of one or more bugs
bzr attachment download --bug 12345 --bug 67890 --out-dir /tmp/all

# Mixed: per-bug + explicit attachment ID
bzr attachment download --bug 12345 9876 --out-dir /tmp/mixed
```

**Output:**

The single-attachment shape emits a one-line `Downloaded attachment #N to PATH (BYTES bytes)` summary or a `DownloadResult` JSON object.

The bulk shapes emit an `AttachmentBatchResult` (table or JSON): per-bug success rows with each saved file, per-attachment rows for positional IDs, and a `Summary: X succeeded, Y failed, Z total bytes` trailer. Bug-level and per-attachment failures are written to stderr in table mode.

**Exit codes:**

- `0` — every target succeeded
- `6` — `--out-dir` could not be created (pre-flight)
- `7` — input validation (no IDs and no `--bug`; `--out` with `--bug` or with multiple IDs; `--out` paired with `--out-dir`)
- `11` — at least one target failed (`BatchPartialFailure`)
- other — see the global exit-code table

**See also:** `bzr attachment list` (discover IDs).

### `bzr attachment upload`

Upload a file as an attachment to a bug. MIME type is auto-detected from the file extension if not specified. Add `--private` to mark the attachment as visible only to users with elevated permissions on the server.

```bash
bzr attachment upload 12345 screenshot.png
bzr attachment upload 12345 data.csv --summary "Performance data" --content-type text/csv
bzr attachment upload 12345 patch.diff --flag "review?(alice@example.com)"
bzr attachment upload 12345 secret.bin --summary "internal trace" --private
bzr attachment upload 12345 fix.patch --comment "see #6789 for context"
bzr attachment upload 12345 patch.diff --comment "sensitive context" --comment-private
bzr attachment upload 12345 fix.patch --patch
```

| Option | Required | Description |
|--------|----------|-------------|
| `<BUG_ID>` | Yes | Bug ID |
| `<FILE>` | Yes | File to upload |
| `--summary <S>` | No | Description of the attachment (default: filename) |
| `--content-type <MIME>` | No | MIME type (auto-detected if omitted; defaults to `text/plain` when `--patch` is set without an explicit type) |
| `--private` / `--no-private` | No | Mark the attachment private (or explicitly public; default public) |
| `--patch` / `--no-patch` | No | Mark the attachment a patch (or non-patch; default non-patch); `--patch` defaults `--content-type` to `text/plain` |
| `--comment <BODY>` | No | Post a comment alongside the attachment in the same API call |
| `--comment-private` | No | Mark the comment posted via `--comment` private. Issues a follow-up `Bug.update` call (two API round-trips). Requires `--comment`. |
| `--flag <F>` | No | Set flags (repeatable; see [Flag Syntax](#flag-syntax)) |

Agent note: for clearer audit trails, agents should usually pass `--summary` explicitly instead of relying on the filename-derived default.

### `bzr attachment update`

Update metadata on an existing attachment.

```bash
bzr attachment update 67890 --summary "Updated patch"
bzr attachment update 67890 --obsolete
bzr attachment update 67890 --no-private
bzr attachment update 67890 --flag "review+(alice@example.com)"
```

Each boolean property is a `--x` / `--no-x` pair: pass `--x` to set it true,
`--no-x` to set it false, or neither to leave it unchanged. (If both are given,
the last one on the command line wins.)

| Option | Required | Description |
|--------|----------|-------------|
| `<ATTACHMENT_ID>` | Yes | Attachment ID |
| `--summary <S>` | No | New summary |
| `--file-name <N>` | No | New file name |
| `--content-type <MIME>` | No | New content type |
| `--obsolete` / `--no-obsolete` | No | Mark obsolete / un-obsolete (unset = unchanged) |
| `--patch` / `--no-patch` | No | Mark as patch / non-patch (unset = unchanged) |
| `--private` / `--no-private` | No | Mark private / public (unset = unchanged) |
| `--flag <F>` | No | Set flags (repeatable; see [Flag Syntax](#flag-syntax)) |

---

## `bzr product` -- Product Operations

### `bzr product list`

List products accessible to the authenticated user.

```bash
bzr product list
bzr product list --type selectable
bzr --json product list
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--type <TYPE>` | No | "accessible" | Product type: `accessible`, `selectable`, or `enterable` |

### `bzr product view`

View product details including components, versions, and milestones.

```bash
bzr product view Fedora
bzr --json product view Fedora
```

### `bzr product create`

Create a new product (requires admin privileges).

```bash
bzr product create --name "New Product" --description "A new product"
bzr product create --name "New Product" --description "Desc" --version "1.0" --is-open true
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--name <N>` | Yes | | Product name |
| `--description <D>` | Yes | | Product description |
| `--version <V>` | No | "unspecified" | Initial version |
| `--is-open <BOOL>` | No | true | Whether the product is open for bugs |

Agent note: this is a write operation with admin impact. For unattended workflows, prefer `--json` plus a preceding `bzr --json product view` or `bzr --json product list` check when you need to avoid duplicate names or confirm current state.

### `bzr product update`

Update an existing product (requires admin privileges).

```bash
bzr product update "My Product" --description "Updated description"
bzr product update "My Product" --is-open false
bzr product update "My Product" --default-milestone "2.0"
```

| Option | Required | Description |
|--------|----------|-------------|
| `<NAME>` | Yes | Product name |
| `--description <D>` | No | New description |
| `--default-milestone <M>` | No | Default milestone |
| `--is-open <BOOL>` | No | Whether the product is open for bugs |

---

## `bzr field` -- Field Value Lookup

### `bzr field list`

List valid values for a bug field (e.g. status, priority, severity, resolution). For status fields, shows allowed state transitions.

Common field name aliases are resolved automatically (matching is case-insensitive):

| You type | API Field Name |
|----------|----------------|
| `file_loc` | `bug_file_loc` |
| `group` | `bug_group` |
| `id` | `bug_id` |
| `severity` | `bug_severity` |
| `status` | `bug_status` |
| `type` | `bug_type` |

Aliases only target built-in `bug_*` fields. Custom fields (which Bugzilla requires to use the `cf_` prefix) are unaffected. Fields without aliases (e.g. `priority`, `resolution`) are passed through as-is. Run `bzr field aliases` to see the full alias list.

```bash
bzr field list status
bzr field list priority
bzr --json field list severity
```

### `bzr field aliases`

Show all available field name aliases and their corresponding API field names.

```bash
bzr field aliases
bzr --json field aliases
```

---

## `bzr user` -- User Operations

### `bzr user search`

Search for users by name or email.

```bash
bzr user search "alice"
bzr user search "alice" --details   # includes groups and login status
bzr --json user search "example.com"
```

| Option | Description |
|--------|-------------|
| `--details` | Show extended details (groups, login status). Only affects table output; JSON always includes all fields. Group visibility depends on caller privileges. |

### `bzr user create`

Create a new user (requires admin privileges).

```bash
bzr user create --email alice@example.com --full-name "Alice Smith"
bzr user create --email bob@example.com --password s3cret
bzr user create --email carol@example.com --login carol   # Bugzilla 5.3+ with use_email_as_login disabled
```

| Option | Required | Description |
|--------|----------|-------------|
| `--email <E>` | Yes | User email |
| `--login <L>` | No | Login name (if different from email). Required on Bugzilla 5.3+ when `use_email_as_login` is disabled |
| `--full-name <N>` | No | Full name |
| `--password <P>` | No | Password (server generates one if omitted) |

> **Note:** On Bugzilla 5.3+ with `use_email_as_login` disabled, the REST API has a conflict with the `login` field. Set `api_mode = "hybrid"` in your server config to use XML-RPC for user creation, which avoids this issue.

Agent note: if the server’s login policy is not known, inspect existing users or server conventions before automating `--login`. On affected Bugzilla 5.3+ setups, prefer `api_mode = "hybrid"` as noted above.

### `bzr user update`

Update an existing user (requires admin privileges).

```bash
bzr user update alice@example.com --real-name "Alice J. Smith"
bzr user update alice@example.com --disable-login true --login-denied-text "Account suspended"
```

| Option | Required | Description |
|--------|----------|-------------|
| `<USER>` | Yes | User ID or login name |
| `--real-name <N>` | No | New real name |
| `--email <E>` | No | New email |
| `--disable-login <BOOL>` | No | Disable login |
| `--login-denied-text <T>` | No | Custom message shown when login is denied |

---

## `bzr group` -- Group Management

### `bzr group add-user`

Add a user to a group.

```bash
bzr group add-user --group testers --user alice@example.com
```

| Option | Required | Description |
|--------|----------|-------------|
| `--group <G>` | Yes | Group name |
| `--user <U>` | Yes | User email or login |

### `bzr group remove-user`

Remove a user from a group.

```bash
bzr group remove-user --group testers --user alice@example.com
```

| Option | Required | Description |
|--------|----------|-------------|
| `--group <G>` | Yes | Group name |
| `--user <U>` | Yes | User email or login |

### `bzr group list-users`

List all users in a group.

```bash
bzr group list-users --group admin
bzr group list-users --group admin --details   # includes groups and login status
bzr --json group list-users --group admin
```

| Option | Description |
|--------|-------------|
| `--group <G>` | **Required.** Group name |
| `--details` | Show extended details (groups, login status). Only affects table output; JSON always includes all fields. Group visibility depends on caller privileges. |

### `bzr group view`

View group details.

```bash
bzr group view admin
bzr --json group view admin
```

### `bzr group create`

Create a new group (requires admin privileges).

```bash
bzr group create --name "qa-team" --description "QA team members"
bzr group create --name "qa-team" --description "QA" --is-active true
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--name <N>` | Yes | | Group name |
| `--description <D>` | Yes | | Group description |
| `--is-active <BOOL>` | No | true | Whether the group is active |

Agent note: this is an admin write. In automation, pair it with a preceding `bzr --json group view <name>` or existing state check when you need idempotent behavior.

### `bzr group update`

Update an existing group (requires admin privileges).

```bash
bzr group update qa-team --description "Updated QA team description"
bzr group update qa-team --is-active false
```

| Option | Required | Description |
|--------|----------|-------------|
| `<GROUP>` | Yes | Group name or ID |
| `--description <D>` | No | New description |
| `--is-active <BOOL>` | No | Whether the group is active |

---

## `bzr whoami`

Show the currently authenticated user.

```bash
bzr whoami
bzr --json whoami
```

`whoami` takes no subcommand. (The former no-op `whoami show` alias was
removed in favor of the single bare form.)

---

## `bzr server` -- Server Diagnostics

### `bzr server info`

Show server version and installed extensions.

```bash
bzr server info
bzr --json server info
```

---

## `bzr classification` -- Classification Operations

### `bzr classification list`

List the server's classifications with their ID, name, description, and
product count. JSON output is the full classification array.

Bugzilla has no bulk classification endpoint, so bzr reads the names from the
`classification` field's legal values and fetches each one's detail.
Classifications are an optional feature; when they are disabled the only
entry is `Unclassified`, and bzr prints a note to stderr.

```bash
bzr classification list
bzr --json classification list | jq '.[].name'
```

### `bzr classification view`

View a classification by name or ID.

```bash
bzr classification view "Unclassified"
bzr --json classification view "Unclassified"
```

---

## `bzr component` -- Component Operations

### `bzr component list`

List a product's components with their ID, name, description, default
assignee, and active flag. Reads the same data as `bzr product view
<product>`; JSON output is the full component array.

```bash
bzr component list --product Fedora
bzr --json component list --product Fedora | jq '.[].name'
```

### `bzr component view`

View a single component by exact name within a product. JSON output is the
`Component` object. Errors if the product has no component with that name.

```bash
bzr component view Fedora kernel
bzr --json component view Fedora kernel
```

### `bzr component create`

Create a new component in a product (requires admin privileges).

```bash
bzr component create --product Fedora --name "new-component" \
  --description "Handles new features" --default-assignee dev@example.com
```

| Option | Required | Description |
|--------|----------|-------------|
| `--product <P>` | Yes | Product name |
| `--name <N>` | Yes | Component name |
| `--description <D>` | Yes | Component description |
| `--default-assignee <E>` | Yes | Default assignee email |

Agent note: this is safer after confirming the product exists with `bzr --json product view <product>` and that the assignee is valid with `bzr --json user search "<email-or-name>"`.

### `bzr component update`

Update an existing component (requires admin privileges).

```bash
bzr component update 42 --name "renamed-component"
bzr component update 42 --default-assignee newdev@example.com
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ID>` | Yes | Component ID |
| `--name <N>` | No | New name |
| `--description <D>` | No | New description |
| `--default-assignee <E>` | No | New default assignee |

---

## `bzr config` -- Configuration Management

Configuration is stored in `~/.config/bzr/config.toml`. Multiple servers can be configured and switched between using aliases.

### `bzr config set-server`

Add or update a named server configuration.

```bash
export REDHAT_BZ_API_KEY=abc123
bzr config set-server redhat --url https://bugzilla.redhat.com --api-key-env REDHAT_BZ_API_KEY --email you@redhat.com
bzr config set-server mozilla --url https://bugzilla.mozilla.org --api-key-env MOZILLA_BZ_API_KEY
bzr config set-server internal --url https://bugzilla.internal --api-key-env INTERNAL_BZ_API_KEY --tls-insecure
bzr config set-server legacy --url https://bugzilla.example.com --api-key abc123
```

The `--email` flag is required for older Bugzilla servers (5.0 or earlier) that don't support the `/rest/whoami` endpoint.

The `--tls-insecure` flag disables TLS certificate verification for the server. Use this for servers with self-signed, expired, or wrong-hostname certificates (e.g. internal Bugzilla instances behind corporate firewalls).

The first server added is automatically set as the default.

| Option | Required | Description |
|--------|----------|-------------|
| `<NAME>` | Yes | Server alias name |
| `--url <URL>` | Yes | Server URL |
| `--api-key <KEY>` | One of `--api-key` / `--api-key-env` | API key value (less secure: can leak via shell history or process args) |
| `--api-key-env <ENV_VAR>` | One of `--api-key` / `--api-key-env` | Environment variable name containing the API key |
| `--email <EMAIL>` | No | Login email (required for Bugzilla 5.0 or earlier) |
| `--auth-method <METHOD>` | No | Override auto-detected auth method (`header` or `query_param`) |
| `--tls-insecure` | No | Disable TLS certificate verification (self-signed, expired, wrong hostname) |

#### TLS Options

| Flag | Description |
|------|-------------|
| `--tls-insecure` | Accept invalid TLS certificates |
| `--tls-ca-cert <PATH>` | Path to PEM CA certificate file |
| `--tls-pin-sha256 <HASH>` | Pin a certificate fingerprint |
| `--tls-pin-now` | Connect and pin the server's current certificate |
| `--tls-pin-clear` | Remove a stored certificate pin |

Agent note: prefer `--api-key-env` in local shells, CI, and agent environments. API keys passed on the command line may end up in shell history or process listings, and inline keys are stored in `config.toml`. Verify the result with `bzr whoami` or `bzr --json config show`.

### `bzr config set-default`

Change which server is used when `--server` is not specified.

```bash
bzr config set-default mozilla
```

### `bzr config remove-server <NAME>`

Remove a server alias from the config. Deletes the `[servers.<NAME>]` block
and, if the server stored its API key in the OS keychain, removes that
keychain entry too (idempotently — a missing entry is not an error). The
server must exist.

Removing the current **default** server is refused while other servers
remain — set a new default first with `bzr config set-default <other>`.
Removing the **only** configured server is allowed and leaves the config
with no default. Emits the standard mutation JSON (`"action": "removed"`)
under `--json`.

```bash
bzr config remove-server staging
bzr --json config remove-server throwaway
```

### `bzr config rename-server <OLD> <NEW>`

Rename a server alias, preserving all of its fields. `<OLD>` must exist and
`<NEW>` must not. If the API key lives in the OS keychain under the default
account (the server name), the stored secret is moved to the new account so
credentials keep working; an explicitly configured `--account` is left as-is.
If `default_server` pointed at `<OLD>`, it is updated to `<NEW>`. Emits the
standard mutation JSON (`"action": "renamed"`, with `previous_name`) under
`--json`.

```bash
bzr config rename-server stage staging
bzr --json config rename-server old-name new-name
```

### `bzr config show`

Display the current configuration (API keys are masked). Supports `--json` for structured output.

```bash
bzr config show
bzr --json config show
```

### `bzr config set-keyring <server> [--service NAME] [--account NAME]`

Store an API key for a previously-configured server in the OS keychain
(macOS Keychain, Windows Credential Manager, or Linux Secret Service).
The key is read from stdin with echo disabled, so it never appears on
the command line or in shell history. After storage, `config.toml` is
rewritten to drop any inline `api_key` / `api_key_env` value and add an
`api_key_keyring` reference.

- `--service NAME` overrides the keyring service name (default: `bzr`).
- `--account NAME` overrides the keyring account name (default: the
  server alias).

Example:

```bash
$ bzr config set-keyring prod
Enter API key for service='bzr' account='prod' (input hidden):
Stored API key for server 'prod' in OS keychain (service=bzr, account=prod)
```

### `bzr config unset-keyring <server>`

Remove a server's API key from the OS keychain and clear the
`api_key_keyring` entry from `config.toml`. The server entry itself is
preserved; re-run `bzr config set-server` or `bzr config set-keyring`
afterward to re-credential it.

Idempotent: missing keychain entries are silently ignored.

### `bzr config migrate-to-keyring <server> [--service NAME] [--account NAME] --yes`

Copy an existing inline or env-backed API key into the OS keychain.

- For **inline** sources, `config.toml` is rewritten: `api_key` is
  dropped and `api_key_keyring` is added.
- For **env** sources, `config.toml` is left unchanged — the env var may
  be shared with other tools. The secret is still stored in the
  keychain so you can later edit `config.toml` manually to switch over.

`--yes` is required to confirm the migration.

---

## Credential storage

`bzr` supports three mutually-exclusive API key sources per server:

| Source | Config field | Typical use |
|---|---|---|
| Inline | `api_key = "..."` | Personal dev machines with hardened file permissions |
| Environment variable | `api_key_env = "BZR_API_KEY"` | Headless servers, CI/CD, containers |
| OS keychain | `api_key_keyring = {}` | Desktop workstations with an unlocked keychain daemon |

Exactly one must be set per server; config validation rejects any combination at startup.

### Headless / CI environments

Keychain access requires an unlocked user keyring daemon, which is
typically not available in headless servers, CI runners, or containers.
Use the environment variable source instead:

```toml
[servers.ci]
url = "https://bugzilla.example.com"
api_key_env = "BZR_API_KEY"
```

Inject the secret at runtime without writing it to disk:

**GitHub Actions:**

```yaml
    - name: Run bzr
      env:
        BZR_API_KEY: ${{ secrets.BZR_API_KEY }}
      run: bzr bug list --status NEW
```

**systemd drop-in:**

```ini
[Service]
EnvironmentFile=/etc/bzr.env    # mode 0600, owner root
```

**Docker:**

```dockerfile
ENV BZR_API_KEY=""
# Inject at runtime: docker run -e BZR_API_KEY=... ...
```

See also: `docs/troubleshooting.md` for platform-specific keychain
troubleshooting.

---

## `bzr template` -- Bug Template Management

Templates store named sets of default field values for bug creation. They are saved in the config file and can be used with `bzr bug create --template`.

### `bzr template save`

Save a named template with default field values.

```bash
bzr template save security-bug --product Security --component Triage --priority P1 --severity critical
bzr template save kernel-bug --product Fedora --component kernel --assignee dev@example.com
```

| Option | Required | Description |
|--------|----------|-------------|
| `<NAME>` | Yes | Template name |
| `--product <P>` | No | Default product |
| `--component <C>` | No | Default component |
| `--version <V>` | No | Default version |
| `--priority <P>` | No | Default priority |
| `--severity <S>` | No | Default severity |
| `--assignee <A>` | No | Default assignee |
| `--op-sys <OS>` | No | Default operating system |
| `--rep-platform <PLAT>` | No | Default hardware platform |
| `--description <D>` | No | Default description |

At least one field must be set.

Agent note: templates are agent-friendly because they remove repeated server-specific defaults from future `bug create` calls. Prefer them when agents repeatedly file similar bugs.

### `bzr template list`

List all saved templates.

```bash
bzr template list
bzr --json template list
```

### `bzr template show`

Show details of a template.

```bash
bzr template show security-bug
bzr --json template show security-bug
```

### `bzr template update`

Edit an existing template in place. A supplied field flag replaces that field;
an omitted flag leaves it unchanged. `--clear <FIELD>` (repeatable; names match
the long flags: `product`, `component`, `version`, `priority`, `severity`,
`assignee`, `op-sys`, `rep-platform`, `description`) resets a field. At least one
change is required, and a fully-cleared template is rejected (exit 7). If a field
is both set and cleared in one call, `--clear` wins.

```bash
bzr template update security-bug --severity blocker
bzr template update security-bug --clear assignee
```

### `bzr template delete`

Delete a saved template.

```bash
bzr template delete security-bug
```

---

## `bzr query` -- Saved Query Management

Manage saved queries — reusable bug searches stored in your config file.

### `bzr query save`

Save a named query with filters.

```bash
# Save a structured list query
bzr query save firefox-new --product Firefox --status NEW --status ASSIGNED --limit 25

# Save a free-text search query
bzr query save crashes --search "crash in tab" --limit 10

# Save with multiple filters
bzr query save my-p1 --assignee me@example.com --priority P1 --status NEW --status ASSIGNED

# Import a query from a Bugzilla URL
bzr query save my-query --from-url "https://bugzilla.example.com/buglist.cgi?product=Firefox&bug_status=NEW"

# Save a date-range query (recent activity)
bzr query save recent-firefox --product Firefox --changed-since 2026-04-01
```

`--from-url` and manual filter flags (`--search`, `--product`, `--component`, etc.) are mutually exclusive.

| Option | Required | Description |
|--------|----------|-------------|
| `<NAME>` | Yes | Query name |
| `--from-url <URL>` | No* | Import query from a Bugzilla buglist.cgi URL. Mutually exclusive with manual filter flags (`--search`, `--product`, `--component`, etc.). |
| `--product <P>` | No* | Filter by product name (repeatable; prefix with `!` to exclude) |
| `--component <C>` | No* | Filter by component name (repeatable; prefix with `!` to exclude) |
| `--status <S>` | No* | Filter by status (repeatable; prefix with `!` to exclude) |
| `--assignee <A>` | No* | Filter by assignee email (repeatable; prefix with `!` to exclude) |
| `--creator <C>` | No* | Filter by bug creator email (repeatable; prefix with `!` to exclude) |
| `--priority <P>` | No* | Filter by priority (repeatable; prefix with `!` to exclude) |
| `--severity <S>` | No* | Filter by severity (repeatable; prefix with `!` to exclude) |
| `--search <Q>` | No* | Free-text search query |
| `--limit <N>` | No | Max results |
| `--fields <F>` | No | Stored with the query (comma-separated built-in fields or Bugzilla custom fields named `cf_*`); at run time sets the fields requested from the server and selects table columns. Under `--json`, the object contains only the selected fields (gh-style; `id` is included only when requested). |
| `--exclude-fields <F>` | No | Stored with the query (comma-separated); at run time drops those fields from the server request and removes table columns. Under `--json`, the object omits the dropped fields (including custom `cf_*` fields and `id`, when excluded). |
| `--created-since <DATE>` | No | Save a `creation_time >= DATE` filter into the query. Same accepted forms as [`bzr bug list --created-since`](#date-format). |
| `--changed-since <DATE>` | No | Save a `last_change_time >= DATE` filter into the query. Same accepted forms as [`bzr bug list --changed-since`](#date-format). |

At least one filter must be set. Use either `--from-url` or one or more manual filter flags.

When `--from-url` is used, `--limit`, `--fields`, and `--exclude-fields` may still be provided and will be stored with the saved query as overrides.

All `bzr bug list` filter flags are also accepted; see
[bug list](#bzr-bug-list) for syntax and semantics, including the
`--whiteboard`, `--target-milestone`, `--version`, `--op-sys`,
`--platform`, `--resolution`, `--qa-contact`, and `--url` filters
added in #158.

Agent note: saved queries are useful for agents because they turn multi-flag searches into stable named workflows. Pair them with `bzr --json query run <name>` for deterministic reuse.

### `bzr query list`

List all saved queries.

```bash
bzr query list
```

### `bzr query show`

Show details of a saved query.

```bash
bzr query show firefox-new
```

For URL-sourced queries (saved with `--from-url`), the output also includes the
original source URL, the associated server name, and a count of raw passthrough
parameters. In JSON format, the full list of raw parameters is included.

### `bzr query update`

Edit an existing saved query in place. A supplied filter flag (repeatable)
replaces that field's saved list; a scalar flag (`--limit`, `--fields`,
`--search`, `--created-since`, ...) replaces that value; an omitted flag leaves
it unchanged. `--clear <FIELD>` (repeatable; names match the long flags, e.g.
`status`, `limit`, `search`, `created-since`, `sort`) resets a saved field. At
least one change is required, and an update that would leave the query with no
filters is rejected (exit 7). Raw passthrough params from a `--from-url` query
are preserved. `--sort` / `--order` set the persisted ordering. If a field is
both set and cleared in one call, `--clear` wins.

```bash
bzr query update firefox-new --status ASSIGNED
bzr query update firefox-new --limit 100 --clear severity
```

### `bzr query delete`

Delete a saved query.

```bash
bzr query delete firefox-new
```

### `bzr query run`

Execute a saved query. Supports runtime overrides for limit, fields, exclude-fields, and server.

```bash
# Run a saved query
bzr query run firefox-new

# Run with a different limit
bzr query run firefox-new --limit 10

# Run with field selection
bzr query run firefox-new --fields id,summary,status
bzr query run firefox-new --fields id,summary,cf_release

# Run against a different server
bzr query run my-query --server other-server --limit 50

# Run with a different date cutoff
bzr query run recent-firefox --changed-since 2026-05-01
```

| Option | Required | Description |
|--------|----------|-------------|
| `<NAME>` | Yes | Query name |
| `--limit <N>` | No | Override the saved limit |
| `--fields <F>` | No | Comma-separated built-in fields or Bugzilla custom fields named `cf_*` requested from the server; in table output, selects which columns to show (in order). Under `--json`, the object contains only the selected fields (gh-style; `id` is included only when requested). |
| `--exclude-fields <F>` | No | Comma-separated fields dropped from the server request; in table output, removes those columns. Under `--json`, the object omits the dropped fields (including custom `cf_*` fields and `id`, when excluded). |
| `--server <NAME>` | No | Override the server to run the query against. Takes precedence over the server stored in the saved query. The global `--server` flag takes precedence over this flag. |
| `--created-since <DATE>` | No | Override the saved `creation_time` filter for this run. Same accepted forms as [`bzr bug list --created-since`](#date-format). |
| `--changed-since <DATE>` | No | Override the saved `last_change_time` filter for this run. Same accepted forms as [`bzr bug list --changed-since`](#date-format). |

All eight `bzr bug list` field filters from #158 are also accepted
as overrides. Passing a flag replaces the saved value for that
field; omitting it keeps the saved value. These overrides apply to
this run only — to change a saved field permanently use
[`bzr query update`](#bzr-query-update) (with `--clear <FIELD>` to
reset one).

---

## `bzr completion` -- Shell Completion

Generate a shell completion script and print it to stdout. The script is
produced from bzr's live clap command tree, so it always matches the
installed binary's subcommands and flags. This command is local: it makes
no network calls and needs no configured server.

```
bzr completion <SHELL>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<SHELL>` | Yes | One of `bash`, `zsh`, `fish`, `powershell`, or `elvish`. |

An unrecognized shell name exits 2 with the list of accepted values.

### Install

| Shell | One-line install |
|-------|------------------|
| bash | `bzr completion bash > ~/.local/share/bash-completion/completions/bzr` |
| zsh | `bzr completion zsh > ~/.zfunc/_bzr` (ensure `~/.zfunc` is on `$fpath`, then restart the shell) |
| fish | `bzr completion fish > ~/.config/fish/completions/bzr.fish` |
| powershell | `bzr completion powershell >> $PROFILE` |

For bash, the target directory must exist and `bash-completion` must be
installed and sourced by your shell startup. For zsh, the file name must be
`_bzr` and live in a directory listed in `$fpath` before `compinit` runs.

---

## Flag Syntax

Flags use the pattern `name[status](requestee)`:

| Syntax | Meaning |
|--------|---------|
| `review?(alice@example.com)` | Request review from alice |
| `review+` | Grant review (no specific user) |
| `review-` | Deny review |
| `needinfo?(bob@example.com)` | Request needinfo from bob |
| `approval+` | Grant approval |

The `--flag` option is available on `bzr bug update`, `bzr attachment upload`, and `bzr attachment update`. It can be repeated to set multiple flags.

---

## JSON Output

### Auto-detection

When stdout is not a TTY (i.e. piped to another program or redirected to a file), bzr automatically outputs JSON. At a TTY, it defaults to table format. Override with `--json`, `--output`, or the `BZR_OUTPUT` env var.

Agent note: rely on explicit `--json` rather than TTY auto-detection when writing skills or scripts. It makes the command behavior stable across terminals, CI, and agent runners.

### List and view commands

All list and view commands support JSON output for scripting and piping to tools like `jq`:

```bash
# Get bug IDs matching a search
bzr --json bug search "memory leak" | jq '.[].id'

# Extract assignee from a bug
bzr --json bug view 12345 | jq -r '.assigned_to'

# List attachment filenames
bzr --json attachment list 12345 | jq -r '.[].file_name'

# Get product component names
bzr --json product view Fedora | jq -r '.components[].name'

# List allowed status transitions from NEW
bzr --json field list status | jq '.[] | select(.name == "NEW") | .can_change_to'

# Get only specific fields from a bug
bzr --json bug view 12345 --fields id,summary,status | jq .

# Check authenticated user
bzr --json whoami | jq -r '.name'

# List server extensions
bzr --json server info | jq -r '.extensions | keys[]'

# View config as JSON
bzr --json config show | jq .
```

### Mutation responses

Create, update, and delete commands return structured JSON with `--json`:

```json
{"id":123,"resource":"bug","action":"created"}
{"id":456,"bug_id":123,"resource":"comment","action":"created"}
{"id":789,"resource":"attachment","action":"updated"}
{"user":"alice","group":"qa","resource":"group_membership","action":"added"}
{"id":67890,"file":"/tmp/patch.diff","size":4096,"resource":"attachment","action":"downloaded"}
```

Template mutations use `name` instead of `id`:

```json
{"name":"security-bug","action":"saved"}
{"name":"security-bug","action":"deleted"}
```

Server config mutations key on `name` and carry `config_file`; `rename-server` also includes `previous_name`:

```json
{"name":"staging","config_file":"~/.config/bzr/config.toml","resource":"server","action":"removed"}
{"name":"new-name","previous_name":"old-name","config_file":"~/.config/bzr/config.toml","resource":"server","action":"renamed"}
```

All mutation responses include `resource` and `action` fields. Most include `id` for the created/updated resource. Note: `comment tag` responses use `comment_id`, not `id`. Membership responses (`group_membership`) have no `id` field. Template responses use `name` instead of `id`. Server config responses (`remove-server`/`rename-server`) key on `name` (and `previous_name` for renames) with no `id`.

### Error output

When `--json` is active, errors are emitted as JSON on stderr:

```json
{"error":{"type":"api","message":"Bugzilla API error: Invalid Bug ID (code 101)","exit_code":4}}
```

---

## Configuration File Format

`~/.config/bzr/config.toml`:

```toml
default_server = "redhat"

[servers.redhat]
url = "https://bugzilla.redhat.com"
api_key_env = "REDHAT_BZ_API_KEY"
email = "you@redhat.com"

[servers.mozilla]
url = "https://bugzilla.mozilla.org"
api_key_env = "MOZILLA_BZ_API_KEY"

[servers.older]
url = "https://bugzilla.example.com"
api_key = "old-server-key"
email = "you@example.com"
api_mode = "hybrid"        # auto-detected: rest, xmlrpc, or hybrid
server_version = "5.0.4"   # auto-detected (absent if version endpoint unavailable)

# Self-hosted with a private CA — pin the CA file so a compromised public
# CA cannot mint a cert for this hostname. tls_insecure is mutually
# exclusive with tls_ca_cert and tls_pin_sha256.
[servers.internal]
url = "https://bugzilla.internal"
api_key_env = "INTERNAL_BZ_API_KEY"
tls_ca_cert = "/etc/pki/internal-ca.pem"

# TOFU pin: leaf SPKI fingerprint captured by `--tls-pin-now`. Triggers
# exit code 13 (PinMismatch / IssuerChanged) on rotation; re-pin with
# `--tls-pin-now` or remove with `--tls-pin-clear`.
[servers.pinned]
url = "https://bugzilla.example.com"
api_key_env = "PINNED_BZ_API_KEY"
tls_pin_sha256 = "sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
tls_pin_issuer = "/CN=example-internal-ca"

# Disable TLS verification entirely (last resort; prefer tls_ca_cert).
[servers.scratch]
url = "https://bugzilla.lab"
api_key_env = "LAB_BZ_API_KEY"
tls_insecure = true

[templates.security-bug]
product = "Security"
component = "Triage"
priority = "P1"
severity = "critical"
```

---

## Authentication

`bzr` authenticates using Bugzilla API keys. Prefer `--api-key-env` so the secret is resolved at runtime rather than stored in `~/.config/bzr/config.toml`. On Unix systems, `bzr` warns if the config directory or config file permissions are broader than owner-only access. On first use, it auto-detects whether your server supports header-based auth (`X-BUGZILLA-API-KEY`) or query parameter auth (`Bugzilla_api_key`), and caches the result.

Detection probes endpoints in order:

1. `rest/whoami` (Bugzilla 5.1+) — tries header auth, then query param
2. `rest/valid_login` (Bugzilla 5.0+, requires `--email`) — tries header auth, then query param
3. If step 2 detects query param, verifies by probing `rest/bug?limit=1` with header auth — if the probe succeeds, prefers header auth (avoids leaking API keys in URLs)

For servers running Bugzilla 5.0 or earlier, provide your `--email` when configuring, as auth detection uses the `/rest/valid_login` endpoint which requires it.

If auto-detection picks the wrong method (e.g. on servers with custom extensions), override it with `--auth-method`:

```bash
bzr config set-server myserver --url https://bugzilla.example.com --api-key-env BZR_API_KEY --auth-method header
```

To generate an API key:

1. Log in to your Bugzilla instance
2. Go to **Preferences > API Keys**
3. Generate a new key
4. Add it with `bzr config set-server --api-key-env <ENV_VAR>` (preferred) or `--api-key <KEY>` (legacy)

---

## API Transport

`bzr` supports three API transport modes: `rest`, `hybrid`, and `xmlrpc`. On first use, it auto-detects the server version and selects the best mode:

| Server Version | Mode | Notes |
|----------------|------|-------|
| < 5.0 | `xmlrpc` | REST API not available |
| 5.0.x | `hybrid` | REST exists but may return empty results for some queries; falls back to XML-RPC |
| >= 5.1 | `rest` | REST API is mature |

The detected mode is cached in the config file alongside the server version. If version detection fails due to a transient error, the mode is not cached and will be re-detected on the next invocation.

Override per-invocation with `--api` (does not modify the cached config value):

```bash
bzr --api xmlrpc bug list --product MyProduct
bzr --api hybrid bug search "crash"
bzr --api rest bug view 12345
```

In `hybrid` mode, `bzr` tries REST first for search/list operations. If REST returns empty results and the query has active filters (product, status, etc.), it retries via XML-RPC. For direct bug lookups (`bug view`), it falls back to XML-RPC on server errors but not on authentication failures.
