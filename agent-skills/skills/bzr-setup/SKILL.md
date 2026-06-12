---
name: bzr-setup
description: Use when bzr is not yet configured for a Bugzilla server, or when bzr whoami fails — configures a server URL and credentials (env var or OS keychain) and verifies the connection.
---

# Set up bzr for a Bugzilla server

Goal: go from no configuration to a working, authenticated bzr.

## 1. Add the server

Pick a short name and the Bugzilla base URL. Choose a credential source:

Environment variable (good for CI / agents):
```
export BZR_API_KEY=...           # the Bugzilla API key
bzr config set-server my-bz --url https://bugzilla.example.com --api-key-env BZR_API_KEY
```

Or store the key directly (kept in bzr config):
```
bzr config set-server my-bz --url https://bugzilla.example.com --api-key YOUR_API_KEY
```

Or use the OS keychain (macOS Keychain / Secret Service / Windows Credential Manager):
```
bzr config set-server my-bz --url https://bugzilla.example.com
bzr config set-keyring my-bz
```

Set it as default if you have more than one:
```
bzr config set-default my-bz
```

## 2. Verify

```
bzr config show     # confirm the server and credential source
bzr whoami          # confirm authentication works
bzr server info     # confirm the server is reachable and its capabilities
```

If `whoami` fails: re-check the URL, that the API key is valid, and that the
credential source you named actually holds the key (`bzr config show` labels
each server's source).

Once `whoami` succeeds, the other bzr skills (`bzr-file-bug`, `bzr-triage-bug`,
`bzr-search-report`) will work. See `bzr-reference` for the full command surface.
