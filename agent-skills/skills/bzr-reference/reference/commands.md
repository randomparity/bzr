# bzr command surface (authored against bzr 0.4.4)

Global: `--json` (force JSON; long form `--output json`), `--help` on any group.

## bug
Operate on bugs.
- `bzr bug list --product Foo --status NEW`
- `bzr bug view 12345 [--json]`
- `bzr bug search "memory leak" [--json]`
- `bzr bug create --product Foo --component bar --summary "..." --description "..."`
- `bzr bug clone 12345`
- `bzr bug update 12345 --status RESOLVED --resolution FIXED --flag "review+(a@b.com)"`
- `bzr bug history 12345 [--since 2025-01-01]`
- `bzr bug my [--status \!CLOSED]`

## comment
- `bzr comment list 12345 [--json]`
- `bzr comment add 12345 --body "I reproduced this on Fedora 42"`
- `bzr comment tag 98765 --add needs-info`
- `bzr comment search-tags <text>`

## attachment
- `bzr attachment list 12345 [--json]`
- `bzr attachment download 12345`
- `bzr attachment upload 12345 patch.diff --flag "review?(a@b.com)"`
- `bzr attachment update <id> ...`

## config
- `bzr config set-server my-bz --url https://bugzilla.example.com --api-key-env BZR_API_KEY`
- `bzr config show`
- `bzr config set-default my-bz`
- `bzr config set-keyring my-bz` / `bzr config unset-keyring my-bz`
- `bzr config migrate-to-keyring my-bz --yes`

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
- `bzr classification view <name> [--json]`   # the grouping above products

## component
- `bzr component create ...` / `bzr component update ...` (admin only)
- To LIST components, use `bzr product view <product>`.

## template
- `bzr template save fedora-kernel --product Fedora --component kernel`
- `bzr template list` / `bzr template show <name>` / `bzr template delete <name>`
- Apply at create: `bzr bug create --template fedora-kernel --summary "..."`

## query
- `bzr query save my-open --assignee you@example.com --status NEW --status ASSIGNED`
- `bzr query run my-open [--json]`
- `bzr query list` / `bzr query show <name>` / `bzr query delete <name>`
