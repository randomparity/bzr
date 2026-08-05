# bzr command surface (authored against bzr 0.8.1-dev)

Global (place before the subcommand): `--json` (force JSON; long form
`--output json`), `--output ndjson` (one compact record per line, for `jq -c`),
`--dry-run` (preview a bug mutation, no write), `-y`/`--yes` (skip the batch
confirmation prompt), `--timeout <secs>`, `--retry <n>`, `--config <path>`
(alternate config.toml), and stateless inline server flags:
`--server-url <url>` (credentials optional for public read-only commands),
`--server-api-key-env <env>`, `--server-email <email>`, and one of the inline
TLS trust flags `--server-tls-ca-cert`, `--server-tls-pin-sha256`,
`--server-tls-pin-now`, or `--server-tls-insecure`.
`--help` works on any group.

## bug
Operate on bugs.
- `bzr bug list --product Foo --status NEW`
  - Sort/page: `--sort <field> --order asc|desc`, `--offset N`, `--paginate`
    (fetch all pages), `--count` (return just the match count).
  - Extra filters: `--resolution --version --op-sys --platform --whiteboard
    --target-milestone --qa-contact --url --created-since --changed-since`.
- `bzr bug view 12345 [--json] [--web]`   # `--web` opens the bug in a browser
- `bzr bug search "memory leak" [--json]`
- `bzr bug create --product Foo --component bar --summary "..." --description "..."`
  - Field parity with update in one call: `--alias --url --whiteboard
    --target-milestone --deadline --cc --keywords --groups --flag`.
  - Compound create — file the bug, its first comment, and attachments in one
    operation: `--with-comment <body>` / `--with-comment-file <path|->`,
    `--with-attachment <path>` (repeatable), and `--attachment-description
    <text>` (the Nth applies to the Nth attachment). On a post-create sub-step
    failure it prints the new bug ID and exits 11, so the ID is never lost —
    finish the missing step rather than re-filing. See the `bzr-file-bug` skill.
  - `bzr bug create --from-json <path|->`   # one object = one bug; an array batches
- `bzr bug clone 12345`
  - Override create fields on the clone: `--summary --product --component
    --version --description --priority --severity --assignee --op-sys
    --rep-platform --url --whiteboard --target-milestone --deadline --cc
    --keywords --groups --flag`.
  - Exits 11 when the bug is created but the "Cloned from bug #N" comment fails;
    the new bug ID is still printed.
- `bzr bug update 12345 --status RESOLVED --resolution FIXED --flag "review+(a@b.com)"`
  - `--expect-unchanged-since <last_change_time>`   # abort (exit 14) on mid-air collision
  - `--comment <body>` / `--comment-file <path|->` post a comment atomically with
    the field changes, so no second `comment add` call is needed.
    `--comment-private` marks that comment private; it requires one of the two
    (passing it alone is exit 7).
- `bzr bug resolve 12345 [--as WONTFIX]` (sugar over `update`)
- `bzr bug close 12345 [--status CLOSED]` / `reopen 12345 [--status REOPENED]`
  (default to stock statuses VERIFIED / CONFIRMED) / `dup 12345 100`
- `bzr bug history 12345 [--since 2025-01-01]`
  - Under `--json`/`--output ndjson` this emits **flattened** records: one per
    changed field, with `when`/`who`/`field`/`old_value`/`new_value`/`comment_id`.
    An entry that changed N fields expands to N records sharing `when`/`who`.
    `bzr schema history` is the contract.
- `bzr bug links 12345`   # the bug's relationship graph, one record per related bug
  - Covers all six relationship types: `depends_on`, `blocks`, `dupe_of`,
    `duplicates`, `regressed_by`, `regressions`. Each record carries `id`,
    `relation`, `direction`, `depth`, `summary`, `status`.
  - `--recursive --depth <1..=10>` walks breadth-first and cycle-safe instead of
    one hop; `--relation <type>` restricts traversal and output to one type (an
    unknown value is exit 2). Read-only; works without an API key.
- `bzr bug my [--status \!CLOSED] [--product Foo] [--component Bar]`
  - Supports the shared list filters: `--product --component --priority
    --severity --created-since --changed-since --whiteboard --target-milestone
    --version --op-sys --platform --resolution --qa-contact --url`, plus
    `--count`, `--fields`, sorting, paging, and `--all`/`--created`/`--cc`.
- A `-` value for `--description`/`--description-file`/`--comment-file`/`--from-json`
  reads from stdin.

## comment
- `bzr comment list 12345 [--json]`
- `bzr comment add 12345 --body "I reproduced this on Fedora 42"`
- `bzr comment add 12345 --body-file notes.md` (or `--body-file -` for stdin)
- `bzr comment add 12345 --body "internal note" --private`
  - `--private` restricts the comment to users with elevated permissions
    (`editbugs`, or the server's insider group). It is a bare flag and takes no
    value — there is no value-taking `is-private` form.
  - Omitting it posts a **public** comment. If a private comment is what was
    asked for, never fall back to posting without the flag; that discloses the
    body.
- `bzr comment tag 98765 --add needs-info`
- `bzr comment search-tags <text>`

## attachment
- `bzr attachment list 12345 [--json]`
- `bzr attachment view <id> [--json]`           # one attachment's metadata
- `bzr attachment download 12345`
- `bzr attachment download <id> --out - > attachment.bin`
  - `--out -` streams one attachment's raw bytes to stdout and suppresses result output.
- `bzr attachment upload 12345 patch.diff --flag "review?(a@b.com)" --comment "context"`
  - `--comment <body>` and `--comment-file <path|->` post context with the upload.
  - `--comment-private` marks that comment private; `--patch`/`--no-patch` and
    `--private`/`--no-private` set the booleans.
- `bzr attachment update <id> ...`
  - Boolean grammar: `--patch`/`--no-patch`, `--private`/`--no-private`,
    `--obsolete`/`--no-obsolete` (omit both forms to leave a property unchanged).

## config
- `bzr config set-server my-bz --url https://bugzilla.example.com --api-key-env BZR_API_KEY`
- `bzr config set-server public-bz --url https://bugzilla.example.com`
  - Omit `--api-key*` for public read-only servers.
- TLS trust: `--tls-ca-cert <path>`, `--tls-pin-sha256 <pin>`,
  `--tls-pin-now`, `--tls-insecure`, or `--tls-pin-clear`.
- `bzr config show`
- `bzr config set-default my-bz`
- `bzr config set-keyring my-bz` / `bzr config unset-keyring my-bz`
- `bzr config migrate-to-keyring my-bz --yes`
- `bzr config remove-server my-bz` / `bzr config rename-server old new`

## product
- `bzr product list [--json]`
- `bzr product view Fedora [--json]`   # shows components, versions, milestones
- `bzr product create ...` / `bzr product update ...` (admin)

## field
- `bzr field list status [--json]`     # valid values / transitions
- `bzr field aliases`

## user
- `bzr user search "alice" [--json]`
- `bzr user create ...` / `bzr user update ...`

## group
- `bzr group add-user --group testers --user alice@example.com`
- `bzr group remove-user ...` / `bzr group create ...` / `bzr group update ...`

## whoami
- `bzr whoami [--json]`
  - `--json`/`--output ndjson` add two connection-metadata keys alongside the
    user fields: `server_name` (the resolved server, or the inline one) and
    `auth_mode`. Use them to confirm *which* server a key resolved against
    before a write.

## server
- `bzr server info [--json]`           # version and extensions
- `bzr server capabilities [--json]`   # what the server lets you do
  - Reports API transports, auth modes, status-transition summaries, custom
    field definitions, and the attachment-size limit. Fields a stock server
    does not expose anonymously (e.g. `max_attachment_size`) come back `null`
    rather than failing; `flag_types` is `null` today. Works without an API key.
    `bzr schema server-capabilities` is the contract.

## classification
- `bzr classification list [--json]`          # all classifications
- `bzr classification view <name> [--json]`   # the grouping above products

## component
- `bzr component list --product Fedora [--json]`        # components of a product
- `bzr component view Fedora kernel [--json]`           # one component's detail
- `bzr component create ...` / `bzr component update --product Fedora --component kernel ...`
  (admin only)
- `bzr product view <product>` also lists components inline.

## template
- `bzr template save fedora-kernel --product Fedora --component kernel`
  - Templates can also store `--version --priority --severity --assignee
    --op-sys --rep-platform --description --url --whiteboard
    --target-milestone --deadline --cc --keywords --groups --flag`.
- `bzr template update fedora-kernel --component drm --cc triage@example.com`
  - Merge fields in place; `--clear <field>` unsets a stored field.
- `bzr template list` / `bzr template show <name>` / `bzr template delete <name>`
- Apply at create: `bzr bug create --template fedora-kernel --summary "..."`

## query
- `bzr query save my-open --assignee you@example.com --status NEW --status ASSIGNED`
  - `--sort <field> --order asc|desc` persists the result order with the query.
- `bzr query update my-open --status ASSIGNED --clear assignee`  # edit in place; `--clear <field>` drops a field
- `bzr query update my-open --from-url 'https://bz/buglist.cgi?product=Foo'`
  - Refreshes saved URL-derived filters while allowing stored overrides such as
    `--limit`, `--fields`, dates, and sort order.
- `bzr query run my-open [--json]`
- `bzr query list` / `bzr query show <name>` / `bzr query delete <name>`

## completion
- `bzr completion <bash|zsh|fish|powershell|elvish>`   # prints a completion script (local, no network)

## schema
- `bzr schema`            # list the published JSON Schema names — the authoritative list
- `bzr schema bug`        # print one schema (draft 2020-12) for `--json` output
- `bzr schema bug-create-input`  # `bug create --from-json` payload
- `bzr schema bug-update-input`  # `bug update --from-json` payload
- `bzr schema error`      # common JSON/NDJSON error envelope
- `bzr schema envelope`   # the `{schema_version, data}` wrapper itself
- Also published: `history`, `whoami`, `server-capabilities`,
  `compound-create-result`, and one per resource and result type. Run
  `bzr schema` rather than trusting a list in prose.
