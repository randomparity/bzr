# bzr command surface (authored against bzr 0.5.0)

Global (place before the subcommand): `--json` (force JSON; long form
`--output json`), `--output ndjson` (one compact record per line, for `jq -c`),
`--dry-run` (preview a bug mutation, no write), `-y`/`--yes` (skip the batch
confirmation prompt), `--timeout <secs>`, `--retry <n>`, `--config <path>`
(alternate config.toml), and the stateless inline server trio
`--server-url <url>` / `--server-api-key-env <env>` / `--server-email <email>`.
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
  - `bzr bug create --from-json <path|->`   # one object = one bug; an array batches
- `bzr bug clone 12345`
- `bzr bug update 12345 --status RESOLVED --resolution FIXED --flag "review+(a@b.com)"`
  - `--expect-unchanged-since <last_change_time>`   # abort (exit 14) on mid-air collision
- `bzr bug resolve 12345 [--as WONTFIX]` (sugar over `update`)
- `bzr bug close 12345 [--status CLOSED]` / `reopen 12345 [--status REOPENED]`
  (default to stock statuses VERIFIED / CONFIRMED) / `dup 12345 100`
- `bzr bug history 12345 [--since 2025-01-01]`
- `bzr bug my [--status \!CLOSED]`
- A `-` value for `--description`/`--description-file`/`--comment-file`/`--from-json`
  reads from stdin.

## comment
- `bzr comment list 12345 [--json]`
- `bzr comment add 12345 --body "I reproduced this on Fedora 42"`
- `bzr comment add 12345 --body-file notes.md` (or `--body-file -` for stdin)
- `bzr comment tag 98765 --add needs-info`
- `bzr comment search-tags <text>`

## attachment
- `bzr attachment list 12345 [--json]`
- `bzr attachment view <id> [--json]`           # one attachment's metadata
- `bzr attachment download 12345`
- `bzr attachment upload 12345 patch.diff --flag "review?(a@b.com)" --comment "context"`
  - `--comment-private` marks that comment private; `--patch`/`--no-patch` and
    `--private`/`--no-private` set the booleans.
- `bzr attachment update <id> ...`
  - Boolean grammar: `--patch`/`--no-patch`, `--private`/`--no-private`,
    `--obsolete`/`--no-obsolete` (omit both forms to leave a property unchanged).

## config
- `bzr config set-server my-bz --url https://bugzilla.example.com --api-key-env BZR_API_KEY`
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

## server
- `bzr server info [--json]`           # version, extensions, capabilities

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
- `bzr template update fedora-kernel --component drm`   # merge fields in place
- `bzr template list` / `bzr template show <name>` / `bzr template delete <name>`
- Apply at create: `bzr bug create --template fedora-kernel --summary "..."`

## query
- `bzr query save my-open --assignee you@example.com --status NEW --status ASSIGNED`
  - `--sort <field> --order asc|desc` persists the result order with the query.
- `bzr query update my-open --status ASSIGNED --clear assignee`  # edit in place; `--clear <field>` drops a field
- `bzr query run my-open [--json]`
- `bzr query list` / `bzr query show <name>` / `bzr query delete <name>`

## completion
- `bzr completion <bash|zsh|fish|powershell|elvish>`   # prints a completion script (local, no network)

## schema
- `bzr schema`            # list the published JSON Schema names
- `bzr schema bug`        # print one schema (draft 2020-12) for `--json` output
- `bzr schema bug-create-input`  # `bug create --from-json` payload
- `bzr schema bug-update-input`  # `bug update --from-json` payload
