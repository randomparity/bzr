# bzr - Bugzilla CLI

A command-line interface for Bugzilla servers, written in Rust. Inspired by the GitHub CLI (`gh`), `bzr` lets you search, view, create, and update bugs, manage comments and attachments, and switch between multiple Bugzilla instances — all from your terminal.

## Features

- **Bug management** — list, search, view, create, update, and view change history
- **Groups** — list group members, add and remove users from groups
- **Comments** — list and add comments, with `$EDITOR` integration for composing
- **Attachments** — list, download, and upload file attachments with auto-detected MIME types
- **Products** — list accessible products and view details (components, versions, milestones)
- **Fields** — look up valid values for bug fields (status, priority, severity, etc.)
- **Users** — search users by name or email
- **Multi-server** — configure and switch between multiple Bugzilla instances
- **Output formats** — human-readable tables (with colored status) or JSON for scripting
- **Secure auth** — API key sent via `X-BUGZILLA-API-KEY` header, never in query strings

## Installation

```
cargo install --path .
```

Requires Rust 1.70+.

## Quick Start

```bash
# Configure a Bugzilla server (add --email for Bugzilla 5.0 or earlier)
bzr config set-server myserver --url https://bugzilla.example.com --api-key YOUR_API_KEY

# List open bugs in a product
bzr bug list --product MyProduct --status NEW

# View a specific bug
bzr bug view 12345

# View change history for a bug
bzr bug history 12345

# Search for bugs
bzr bug search "crash on startup"

# Add a comment
bzr comment add 12345 --body "I can reproduce this on Fedora 42"

# Download an attachment
bzr attachment download 67890 -o patch.diff

# Upload a file to a bug
bzr attachment upload 12345 screenshot.png --summary "UI glitch screenshot"

# List accessible products
bzr product list

# View product details (components, versions, milestones)
bzr product view MyProduct

# Check valid values for a field
bzr field list status

# Search for users
bzr user search "alice"

# List members of a group
bzr group list-users --group admin

# Add a user to a group
bzr group add-user --group testers --user alice@example.com

# Remove a user from a group
bzr group remove-user --group testers --user alice@example.com
```

## Command Reference

### Command Tree

```
bzr [--server <NAME>] [--output table|json]
├── bug
│   ├── list [--product <P>] [--component <C>] [--status <S>] [--assignee <A>] [--limit <N>]
│   ├── view <ID>
│   ├── search <QUERY> [--limit <N>]
│   ├── history <ID>
│   ├── create --product <P> --component <C> --summary <S> [--version <V>]
│   │          [--description <D>] [--priority <P>] [--severity <S>] [--assignee <A>]
│   └── update <ID> [--status <S>] [--resolution <R>] [--assignee <A>]
│                    [--priority <P>] [--severity <S>] [--summary <S>] [--whiteboard <W>]
├── comment
│   ├── list <BUG_ID>
│   └── add <BUG_ID> [--body <TEXT>]
├── attachment
│   ├── list <BUG_ID>
│   ├── download <ATTACHMENT_ID> [-o <FILE>]
│   └── upload <BUG_ID> <FILE> [--summary <S>] [--content-type <MIME>]
├── product
│   ├── list
│   └── view <NAME>
├── field
│   └── list <FIELD_NAME>
├── user
│   └── search <QUERY>
├── group
│   ├── add-user --group <G> --user <U>
│   ├── remove-user --group <G> --user <U>
│   └── list-users --group <G>
└── config
    ├── set-server <NAME> --url <URL> --api-key <KEY> [--email <EMAIL>]
    ├── set-default <NAME>
    └── show
```

### Global Options

| Option | Description |
|--------|-------------|
| `--server <NAME>` | Use a specific server from config instead of the default |
| `--output <FORMAT>` | Output format: `table` (default) or `json` |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### `bzr bug` — Bug Operations

#### `bzr bug list`

List bugs matching filter criteria.

```bash
bzr bug list --product Fedora --status ASSIGNED --limit 20
bzr bug list --assignee user@example.com
```

| Option | Description |
|--------|-------------|
| `--product <P>` | Filter by product name |
| `--component <C>` | Filter by component name |
| `--status <S>` | Filter by status (NEW, ASSIGNED, RESOLVED, etc.) |
| `--assignee <A>` | Filter by assignee email |
| `--limit <N>` | Max results (default: 50) |

#### `bzr bug view`

Display detailed information about a single bug.

```bash
bzr bug view 12345
bzr bug view 12345 --output json
```

#### `bzr bug history`

View the change history of a bug, showing who changed which fields and when.

```bash
bzr bug history 12345
bzr bug history 12345 --output json
```

#### `bzr bug search`

Full-text search using Bugzilla's quicksearch syntax.

```bash
bzr bug search "kernel panic"
bzr bug search "component:NetworkManager priority:high" --limit 10
```

| Option | Description |
|--------|-------------|
| `--limit <N>` | Max results (default: 50) |

#### `bzr bug create`

File a new bug.

```bash
bzr bug create --product Fedora --component kernel \
  --summary "Boot failure on 6.x" \
  --description "System hangs at initramfs" \
  --priority high --severity major
```

| Option | Required | Description |
|--------|----------|-------------|
| `--product <P>` | Yes | Product name |
| `--component <C>` | Yes | Component name |
| `--summary <S>` | Yes | One-line summary |
| `--version <V>` | No | Version (default: "unspecified") |
| `--description <D>` | No | Full description |
| `--priority <P>` | No | Priority level |
| `--severity <S>` | No | Severity level |
| `--assignee <A>` | No | Assignee email |

#### `bzr bug update`

Modify fields on an existing bug.

```bash
bzr bug update 12345 --status ASSIGNED --assignee dev@example.com
bzr bug update 12345 --status RESOLVED --resolution FIXED
```

| Option | Description |
|--------|-------------|
| `--status <S>` | New status |
| `--resolution <R>` | Resolution (FIXED, WONTFIX, DUPLICATE, etc.) |
| `--assignee <A>` | Reassign to email |
| `--priority <P>` | Set priority |
| `--severity <S>` | Set severity |
| `--summary <S>` | Update summary text |
| `--whiteboard <W>` | Set whiteboard text |

### `bzr comment` — Comment Operations

#### `bzr comment list`

List all comments on a bug.

```bash
bzr comment list 12345
bzr comment list 12345 --output json
```

#### `bzr comment add`

Add a comment to a bug. If `--body` is omitted, opens `$EDITOR` (falls back to `vi`).

```bash
bzr comment add 12345 --body "Confirmed on Fedora 42"
bzr comment add 12345   # opens editor
```

| Option | Description |
|--------|-------------|
| `--body <TEXT>` | Comment text (opens `$EDITOR` if omitted) |

### `bzr attachment` — Attachment Operations

#### `bzr attachment list`

List all attachments on a bug.

```bash
bzr attachment list 12345
```

#### `bzr attachment download`

Download an attachment by its ID. Saves to the original filename by default.

```bash
bzr attachment download 67890
bzr attachment download 67890 -o /tmp/patch.diff
```

| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output file path (default: original filename) |

#### `bzr attachment upload`

Upload a file as an attachment to a bug. MIME type is auto-detected from the file extension if not specified.

```bash
bzr attachment upload 12345 screenshot.png
bzr attachment upload 12345 data.csv --summary "Performance data" --content-type text/csv
```

| Option | Description |
|--------|-------------|
| `--summary <S>` | Description of the attachment (default: filename) |
| `--content-type <MIME>` | MIME type (auto-detected if omitted) |

### `bzr product` — Product Operations

#### `bzr product list`

List all products accessible to the authenticated user.

```bash
bzr product list
bzr product list --output json
```

#### `bzr product view`

View product details including components, versions, and milestones.

```bash
bzr product view Fedora
```

### `bzr field` — Field Value Lookup

#### `bzr field list`

List valid values for a bug field (e.g. status, priority, severity, resolution). For status fields, shows allowed state transitions.

```bash
bzr field list status
bzr field list priority
bzr field list severity --output json
```

### `bzr user` — User Operations

#### `bzr user search`

Search for users by name or email.

```bash
bzr user search "alice"
bzr user search "example.com" --output json
```

### `bzr group` — Group Membership Management

#### `bzr group list-users`

List all users in a group.

```bash
bzr group list-users --group admin
bzr group list-users --group admin --output json
```

| Option | Required | Description |
|--------|----------|-------------|
| `--group <G>` | Yes | Group name |

#### `bzr group add-user`

Add a user to a group.

```bash
bzr group add-user --group testers --user alice@example.com
```

| Option | Required | Description |
|--------|----------|-------------|
| `--group <G>` | Yes | Group name |
| `--user <U>` | Yes | User email or login |

#### `bzr group remove-user`

Remove a user from a group.

```bash
bzr group remove-user --group testers --user alice@example.com
```

| Option | Required | Description |
|--------|----------|-------------|
| `--group <G>` | Yes | Group name |
| `--user <U>` | Yes | User email or login |

### `bzr config` — Configuration Management

Configuration is stored in `~/.config/bzr/config.toml`. Multiple servers can be configured and switched between using aliases.

#### `bzr config set-server`

Add or update a named server configuration.

```bash
bzr config set-server redhat --url https://bugzilla.redhat.com --api-key abc123 --email you@redhat.com
bzr config set-server mozilla --url https://bugzilla.mozilla.org --api-key xyz789
```

The `--email` flag is required for older Bugzilla servers (5.0 and earlier) that don't support the `/rest/whoami` endpoint. It is used during auth detection via `/rest/valid_login`.

The first server added is automatically set as the default.

#### `bzr config set-default`

Change which server is used when `--server` is not specified.

```bash
bzr config set-default mozilla
```

#### `bzr config show`

Display the current configuration (API keys are masked).

```bash
bzr config show
```

## JSON Output

All list and view commands support `--output json` for scripting and piping to tools like `jq`:

```bash
# Get bug IDs matching a search
bzr --output json bug search "memory leak" | jq '.[].id'

# Extract assignee from a bug
bzr --output json bug view 12345 | jq -r '.assigned_to'

# List attachment filenames
bzr --output json attachment list 12345 | jq -r '.[].file_name'

# Get product component names
bzr --output json product view Fedora | jq -r '.components[].name'

# List allowed status transitions from NEW
bzr --output json field list status | jq '.[] | select(.name == "NEW") | .can_change_to'
```

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

## Authentication

`bzr` authenticates using Bugzilla API keys. On first use, it auto-detects whether your server supports header-based auth (`X-BUGZILLA-API-KEY`) or query parameter auth (`Bugzilla_api_key`), and caches the result.

For servers running Bugzilla 5.0 or earlier, provide your `--email` when configuring, as auth detection uses the `/rest/valid_login` endpoint which requires it.

To generate an API key:

1. Log in to your Bugzilla instance
2. Go to **Preferences > API Keys**
3. Generate a new key
4. Add it with `bzr config set-server`

## License

MIT
