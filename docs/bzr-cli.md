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
- [template](#bzr-template----bug-template-management)
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
| `--no-color` | Disable colored output. Color is also suppressed automatically when stdout is not a TTY. |
| `--quiet` | Suppress all stdout output (exit code confirms success) |
| `--api <MODE>` | Override API transport: `rest`, `xmlrpc`, or `hybrid`. Auto-detected from server version if not set. |
| `-v, --verbose` | Increase log verbosity (`-v`=info, `-vv`=debug, `-vvv`=trace; `RUST_LOG` overrides) |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `BZR_OUTPUT` | Default output format (`table` or `json`). Overridden by `--output` or `--json`. |
| `NO_COLOR` | Disable colored output (any value). Supported natively by the `colored` crate. |
| `CLICOLOR` | Set to `0` to disable colored output (standard convention respected by the `colored` crate). |
| `CLICOLOR_FORCE` | Set to `1` to force colored output even when stdout is not a TTY. |
| `RUST_LOG` | Override log verbosity (e.g. `bzr=debug`). |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General/unknown error |
| 2 | Resource not found, or CLI usage error (invalid arguments)* |
| 3 | Config or TOML parse error |
| 4 | Bugzilla API or XML-RPC error |
| 5 | HTTP/network error |
| 6 | IO error |
| 7 | Input validation error (e.g. invalid flag syntax, empty comment) |
| 8 | Response deserialization error |
| 9 | Authentication error |
| 10 | Data integrity error (e.g. missing attachment data) |

*Exit code 2 is also produced by clap for invalid or missing arguments before bzr's error handling runs.

## Command Tree

```
bzr [--server <NAME>] [--output table|json] [--json] [--no-color] [--quiet] [--api rest|xmlrpc|hybrid] [-v...]
├── bug
│   ├── list [--product <P>] [--component <C>] [--status <S>] [--assignee <A>]
│   │        [--creator <C>] [--priority <P>] [--severity <S>] [--id <ID>...]
│   │        [--alias <A>] [--limit <N>] [--fields <F>] [--exclude-fields <F>]
│   ├── view <ID> [--fields <F>] [--exclude-fields <F>]
│   ├── search <QUERY> [--limit <N>] [--fields <F>] [--exclude-fields <F>]
│   ├── history <ID> [--since <DATE>]
│   ├── my [--created] [--cc] [--all] [--status <S>] [--limit <N>]
│   ├── create [--template <T>] --product <P> --component <C> --summary <S>
│   │          [--version <V>] [--description <D>] [--priority <P>] [--severity <S>]
│   │          [--assignee <A>] [--blocks <IDs>] [--depends-on <IDs>]
│   ├── clone <ID> [--summary <S>] [--product <P>] [--component <C>] [--version <V>]
│   │              [--description <D>] [--priority <P>] [--severity <S>] [--assignee <A>]
│   │              [--no-comment] [--add-depends-on] [--add-blocks] [--no-cc] [--no-keywords]
│   └── update <ID...> [--status <S>] [--resolution <R>] [--assignee <A>]
│                       [--priority <P>] [--severity <S>] [--summary <S>]
│                       [--whiteboard <W>] [--flag <F>...] [--blocks-add <IDs>]
│                       [--blocks-remove <IDs>] [--depends-on-add <IDs>]
│                       [--depends-on-remove <IDs>]
├── comment
│   ├── list <BUG_ID> [--since <DATE>]
│   ├── add <BUG_ID> [--body <TEXT>]
│   ├── tag <COMMENT_ID> [--add <TAG>...] [--remove <TAG>...]
│   └── search-tags <QUERY>
├── attachment
│   ├── list <BUG_ID>
│   ├── download <ATTACHMENT_ID> [-o <FILE>]
│   ├── upload <BUG_ID> <FILE> [--summary <S>] [--content-type <MIME>] [--flag <F>...]
│   └── update <ATTACHMENT_ID> [--summary <S>] [--file-name <N>] [--content-type <MIME>]
│                               [--obsolete <BOOL>] [--is-patch <BOOL>]
│                               [--is-private <BOOL>] [--flag <F>...]
├── product
│   ├── list [--type <TYPE>]
│   ├── view <NAME>
│   ├── create --name <N> --description <D> [--version <V>] [--is-open <BOOL>]
│   └── update <NAME> [--description <D>] [--default-milestone <M>] [--is-open <BOOL>]
├── field
│   └── list <FIELD_NAME>
├── user
│   ├── search <QUERY> [--details]
│   ├── create --email <E> [--full-name <N>] [--password <P>]
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
│   └── view <NAME>
├── component
│   ├── create --product <P> --name <N> --description <D> --default-assignee <E>
│   └── update <ID> [--name <N>] [--description <D>] [--default-assignee <E>]
├── config
│   ├── set-server <NAME> --url <URL> --api-key <KEY> [--email <EMAIL>] [--auth-method <METHOD>]
│   ├── set-default <NAME>
│   └── show
└── template
    ├── save <NAME> [--product <P>] [--component <C>] [--version <V>] [--priority <P>]
    │               [--severity <S>] [--assignee <A>] [--description <D>]
    ├── list
    ├── show <NAME>
    └── delete <NAME>
```

---

## `bzr bug` -- Bug Operations

### `bzr bug list`

List bugs matching filter criteria.

```bash
bzr bug list --product Fedora --status ASSIGNED --limit 20
bzr bug list --assignee user@example.com
bzr bug list --product Fedora --fields id,summary,status
bzr bug list --id 100 --id 200 --id 300
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--product <P>` | No | | Filter by product name |
| `--component <C>` | No | | Filter by component name |
| `--status <S>` | No | | Filter by status (NEW, ASSIGNED, RESOLVED, etc.) |
| `--assignee <A>` | No | | Filter by assignee email |
| `--creator <C>` | No | | Filter by bug creator |
| `--priority <P>` | No | | Filter by priority |
| `--severity <S>` | No | | Filter by severity |
| `--id <ID>` | No | | Filter by bug ID (repeatable) |
| `--alias <A>` | No | | Filter by bug alias |
| `--limit <N>` | No | 50 | Max results |
| `--fields <F>` | No | | Only return these fields (comma-separated) |
| `--exclude-fields <F>` | No | | Exclude these fields (comma-separated) |

### `bzr bug view`

Display detailed information about a single bug.

```bash
bzr bug view 12345
bzr --json bug view 12345
bzr bug view my-alias --fields id,summary,assigned_to
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ID>` | Yes | Bug ID or alias |
| `--fields <F>` | No | Only return these fields (comma-separated) |
| `--exclude-fields <F>` | No | Exclude these fields (comma-separated) |

### `bzr bug search`

Full-text search using Bugzilla's quicksearch syntax.

```bash
bzr bug search "kernel panic"
bzr bug search "component:NetworkManager priority:high" --limit 10
bzr bug search "memory leak" --fields id,summary
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `<QUERY>` | Yes | | Search query |
| `--limit <N>` | No | 50 | Max results |
| `--fields <F>` | No | | Only return these fields (comma-separated) |
| `--exclude-fields <F>` | No | | Exclude these fields (comma-separated) |

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

### `bzr bug create`

File a new bug.

```bash
bzr bug create --product Fedora --component kernel \
  --summary "Boot failure on 6.x" \
  --description "System hangs at initramfs" \
  --priority high --severity major
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--product <P>` | Yes | | Product name |
| `--component <C>` | Yes | | Component name |
| `--summary <S>` | Yes | | One-line summary |
| `--version <V>` | No | "unspecified" | Version |
| `--description <D>` | No | | Full description |
| `--priority <P>` | No | | Priority level |
| `--severity <S>` | No | | Severity level |
| `--assignee <A>` | No | | Assignee email |
| `--blocks <IDs>` | No | | Bug IDs this bug blocks (comma-separated) |
| `--depends-on <IDs>` | No | | Bug IDs this bug depends on (comma-separated) |
| `--template <T>` | No | | Use a saved template for default field values |

When using `--template`, the template provides defaults for `--product`, `--component`, and other fields. CLI flags override template values. `--summary` is always required.

### `bzr bug clone`

Clone an existing bug, copying its fields into a new bug. All create flags are available as overrides.

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
| `--version <V>` | No | Override version |
| `--description <D>` | No | Override description (copies comment #0 from source if omitted) |
| `--priority <P>` | No | Override priority |
| `--severity <S>` | No | Override severity |
| `--assignee <A>` | No | Override assignee |
| `--no-comment` | No | Skip the "Cloned from bug #N" comment |
| `--add-depends-on` | No | Make the new bug depend on the source bug |
| `--add-blocks` | No | Make the new bug block the source bug |
| `--no-cc` | No | Don't copy the CC list from the source bug |
| `--no-keywords` | No | Don't copy keywords from the source bug |

### `bzr bug my`

Show bugs related to the authenticated user. Defaults to bugs assigned to you.

```bash
bzr bug my                    # bugs assigned to me
bzr bug my --created          # bugs I created
bzr bug my --cc               # bugs I'm CC'd on
bzr bug my --all              # all of the above
bzr bug my --status OPEN --limit 20
```

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--created` | No | | Show bugs I created (instead of assigned) |
| `--cc` | No | | Show bugs I'm CC'd on (instead of assigned) |
| `--all` | No | | Show all bugs related to me (assigned + created + CC'd) |
| `--status <S>` | No | | Filter by status |
| `--limit <N>` | No | 50 | Max results per query |
| `--fields <F>` | No | | Only return these fields (comma-separated) |
| `--exclude-fields <F>` | No | | Exclude these fields (comma-separated) |

### `bzr bug update`

Modify fields on an existing bug. Supports multiple IDs for batch updates.

```bash
bzr bug update 12345 --status ASSIGNED --assignee dev@example.com
bzr bug update 12345 --status RESOLVED --resolution FIXED
bzr bug update 12345 --flag "review?(alice@example.com)"
bzr bug update 12345 --blocks-add 100,200 --depends-on-add 50
bzr bug update 100 200 300 --status RESOLVED --resolution DUPLICATE
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ID...>` | Yes | Bug ID(s) — pass multiple for batch updates |
| `--status <S>` | No | New status |
| `--resolution <R>` | No | Resolution (FIXED, WONTFIX, DUPLICATE, etc.) |
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

When updating multiple bugs, failures on individual bugs do not abort the batch. A summary is printed showing which bugs succeeded and which failed.

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

Add a comment to a bug. If `--body` is omitted: reads from stdin when piped (`echo "text" | bzr comment add 12345`), or opens `$EDITOR` (falls back to `vi`) at a TTY.

```bash
bzr comment add 12345 --body "Confirmed on Fedora 42"
bzr comment add 12345                          # opens editor
echo "Automated comment" | bzr comment add 12345  # reads stdin
```

| Option | Required | Description |
|--------|----------|-------------|
| `<BUG_ID>` | Yes | Bug ID |
| `--body <TEXT>` | No | Comment text (reads stdin or opens `$EDITOR` if omitted) |

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

### `bzr attachment download`

Download an attachment by its ID. Saves to the original filename by default.

```bash
bzr attachment download 67890
bzr attachment download 67890 -o /tmp/patch.diff
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ATTACHMENT_ID>` | Yes | Attachment ID |
| `-o, --out <FILE>` | No | Output file path (default: original filename) |

### `bzr attachment upload`

Upload a file as an attachment to a bug. MIME type is auto-detected from the file extension if not specified.

```bash
bzr attachment upload 12345 screenshot.png
bzr attachment upload 12345 data.csv --summary "Performance data" --content-type text/csv
bzr attachment upload 12345 patch.diff --flag "review?(alice@example.com)"
```

| Option | Required | Description |
|--------|----------|-------------|
| `<BUG_ID>` | Yes | Bug ID |
| `<FILE>` | Yes | File to upload |
| `--summary <S>` | No | Description of the attachment (default: filename) |
| `--content-type <MIME>` | No | MIME type (auto-detected if omitted) |
| `--flag <F>` | No | Set flags (repeatable; see [Flag Syntax](#flag-syntax)) |

### `bzr attachment update`

Update metadata on an existing attachment.

```bash
bzr attachment update 67890 --summary "Updated patch"
bzr attachment update 67890 --obsolete true
bzr attachment update 67890 --flag "review+(alice@example.com)"
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ATTACHMENT_ID>` | Yes | Attachment ID |
| `--summary <S>` | No | New summary |
| `--file-name <N>` | No | New file name |
| `--content-type <MIME>` | No | New content type |
| `--obsolete <BOOL>` | No | Mark as obsolete |
| `--is-patch <BOOL>` | No | Mark as patch |
| `--is-private <BOOL>` | No | Mark as private |
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

```bash
bzr field list status
bzr field list priority
bzr --json field list severity
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
| `--login <L>` | No | Login name (required on Bugzilla 5.3+ when `use_email_as_login` is disabled; set `api_mode = "hybrid"` to use XML-RPC which avoids the REST login field conflict) |
| `--full-name <N>` | No | Full name |
| `--password <P>` | No | Password (server generates one if omitted) |

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

### `bzr classification view`

View a classification by name or ID.

```bash
bzr classification view "Unclassified"
bzr --json classification view "Unclassified"
```

---

## `bzr component` -- Component Operations

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
bzr config set-server redhat --url https://bugzilla.redhat.com --api-key abc123 --email you@redhat.com
bzr config set-server mozilla --url https://bugzilla.mozilla.org --api-key xyz789
```

The `--email` flag is required for older Bugzilla servers (5.0 or earlier) that don't support the `/rest/whoami` endpoint.

The first server added is automatically set as the default.

| Option | Required | Description |
|--------|----------|-------------|
| `<NAME>` | Yes | Server alias name |
| `--url <URL>` | Yes | Server URL |
| `--api-key <KEY>` | Yes | API key |
| `--email <EMAIL>` | No | Login email (required for Bugzilla 5.0 or earlier) |
| `--auth-method <METHOD>` | No | Override auto-detected auth method (`header` or `query_param`) |

### `bzr config set-default`

Change which server is used when `--server` is not specified.

```bash
bzr config set-default mozilla
```

### `bzr config show`

Display the current configuration (API keys are masked). Supports `--json` for structured output.

```bash
bzr config show
bzr --json config show
```

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

### `bzr template delete`

Delete a saved template.

```bash
bzr template delete security-bug
```

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

All mutation responses include `resource` and `action` fields. Most include `id` for the created/updated resource. Note: `comment tag` responses use `comment_id`, not `id`. Membership responses (`group_membership`) have no `id` field.

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
api_key = "your-api-key-here"
email = "you@redhat.com"

[servers.mozilla]
url = "https://bugzilla.mozilla.org"
api_key = "another-api-key"

[servers.older]
url = "https://bugzilla.example.com"
api_key = "old-server-key"
email = "you@example.com"
api_mode = "hybrid"        # auto-detected: rest, xmlrpc, or hybrid
server_version = "5.0.4"   # auto-detected (absent if version endpoint unavailable)

[templates.security-bug]
product = "Security"
component = "Triage"
priority = "P1"
severity = "critical"
```

---

## Authentication

`bzr` authenticates using Bugzilla API keys. On first use, it auto-detects whether your server supports header-based auth (`X-BUGZILLA-API-KEY`) or query parameter auth (`Bugzilla_api_key`), and caches the result.

Detection probes endpoints in order:

1. `rest/whoami` (Bugzilla 5.1+) — tries header auth, then query param
2. `rest/valid_login` (Bugzilla 5.0+, requires `--email`) — tries header auth, then query param
3. If step 2 detects query param, verifies by probing `rest/bug?limit=1` with header auth — if the probe succeeds, prefers header auth (avoids leaking API keys in URLs)

For servers running Bugzilla 5.0 or earlier, provide your `--email` when configuring, as auth detection uses the `/rest/valid_login` endpoint which requires it.

If auto-detection picks the wrong method (e.g. on servers with custom extensions), override it with `--auth-method`:

```bash
bzr config set-server myserver --url https://bugzilla.example.com --api-key KEY --auth-method header
```

To generate an API key:

1. Log in to your Bugzilla instance
2. Go to **Preferences > API Keys**
3. Generate a new key
4. Add it with `bzr config set-server`

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
