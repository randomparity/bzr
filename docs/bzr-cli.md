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
- [Flag Syntax](#flag-syntax)
- [JSON Output](#json-output)
- [Configuration File Format](#configuration-file-format)
- [Authentication](#authentication)

## Global Options

| Option | Description |
|--------|-------------|
| `--server <NAME>` | Use a specific server from config instead of the default |
| `--output <FORMAT>` | Output format: `table` or `json` |
| `--json` | Shorthand for `--output json` |
| `--no-color` | Disable colored output (also auto-disabled when stdout is not a TTY) |
| `--quiet` | Suppress all stdout output (exit code confirms success) |
| `-v, --verbose` | Increase log verbosity (`-v`=info, `-vv`=debug, `-vvv`=trace; `RUST_LOG` overrides) |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `BZR_OUTPUT` | Default output format (`table` or `json`). Overridden by `--output` or `--json`. |
| `NO_COLOR` | Disable colored output (any value). Supported natively by the `colored` crate. |
| `RUST_LOG` | Override log verbosity (e.g. `bzr=debug`). |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General/unknown error |
| 2 | CLI usage error (invalid arguments) |
| 3 | Config or TOML parse error |
| 4 | Bugzilla API error |
| 5 | HTTP/network error |
| 6 | IO error |

## Command Tree

```
bzr [--server <NAME>] [--output table|json] [--json] [--no-color] [--quiet] [-v...]
├── bug
│   ├── list [--product <P>] [--component <C>] [--status <S>] [--assignee <A>]
│   │        [--creator <C>] [--priority <P>] [--severity <S>] [--id <ID>...]
│   │        [--alias <A>] [--limit <N>] [--fields <F>] [--exclude-fields <F>]
│   ├── view <ID> [--fields <F>] [--exclude-fields <F>]
│   ├── search <QUERY> [--limit <N>] [--fields <F>] [--exclude-fields <F>]
│   ├── history <ID> [--since <DATE>]
│   ├── create --product <P> --component <C> --summary <S> [--version <V>]
│   │          [--description <D>] [--priority <P>] [--severity <S>] [--assignee <A>]
│   └── update <ID> [--status <S>] [--resolution <R>] [--assignee <A>]
│                    [--priority <P>] [--severity <S>] [--summary <S>]
│                    [--whiteboard <W>] [--flag <F>...]
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
│   ├── search <QUERY>
│   ├── create --email <E> [--full-name <N>] [--password <P>]
│   └── update <USER> [--real-name <N>] [--email <E>] [--disable-login <BOOL>]
│                      [--login-denied-text <T>]
├── group
│   ├── add-user --group <G> --user <U>
│   ├── remove-user --group <G> --user <U>
│   ├── list-users --group <G>
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
└── config
    ├── set-server <NAME> --url <URL> --api-key <KEY> [--email <EMAIL>]
    ├── set-default <NAME>
    └── show
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

### `bzr bug update`

Modify fields on an existing bug.

```bash
bzr bug update 12345 --status ASSIGNED --assignee dev@example.com
bzr bug update 12345 --status RESOLVED --resolution FIXED
bzr bug update 12345 --flag "review?(alice@example.com)"
```

| Option | Required | Description |
|--------|----------|-------------|
| `<ID>` | Yes | Bug ID |
| `--status <S>` | No | New status |
| `--resolution <R>` | No | Resolution (FIXED, WONTFIX, DUPLICATE, etc.) |
| `--assignee <A>` | No | Reassign to email |
| `--priority <P>` | No | Set priority |
| `--severity <S>` | No | Set severity |
| `--summary <S>` | No | Update summary text |
| `--whiteboard <W>` | No | Set whiteboard text |
| `--flag <F>` | No | Set flags (repeatable; see [Flag Syntax](#flag-syntax)) |

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
| `-o, --output <FILE>` | No | Output file path (default: original filename) |

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
bzr --json user search "example.com"
```

### `bzr user create`

Create a new user (requires admin privileges).

```bash
bzr user create --email alice@example.com --full-name "Alice Smith"
bzr user create --email bob@example.com --password s3cret
```

| Option | Required | Description |
|--------|----------|-------------|
| `--email <E>` | Yes | User email |
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
bzr --json group list-users --group admin
```

| Option | Required | Description |
|--------|----------|-------------|
| `--group <G>` | Yes | Group name |

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
```

All mutation responses include `resource` and `action` fields. Most include `id` for the created/updated resource.

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
```

---

## Authentication

`bzr` authenticates using Bugzilla API keys. On first use, it auto-detects whether your server supports header-based auth (`X-BUGZILLA-API-KEY`) or query parameter auth (`Bugzilla_api_key`), and caches the result.

For servers running Bugzilla 5.0 or earlier, provide your `--email` when configuring, as auth detection uses the `/rest/valid_login` endpoint which requires it.

To generate an API key:

1. Log in to your Bugzilla instance
2. Go to **Preferences > API Keys**
3. Generate a new key
4. Add it with `bzr config set-server`
