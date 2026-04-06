# Troubleshooting

## Keyring / credential storage

`bzr` uses the OS-native credential store when a server is configured
with `api_key_keyring`. This section covers common failure modes per
platform.

### Linux (Secret Service)

`bzr` talks to a running Secret Service daemon over D-Bus — typically
`gnome-keyring-daemon` (GNOME, XFCE) or `kwalletd6` (KDE). Headless
systems and most containers do not have one.

Check daemon status:

```bash
systemctl --user status gnome-keyring-daemon
# or
systemctl --user status kwalletd6
```

Probe the Secret Service with `secret-tool`:

```bash
secret-tool store --label="bzr test" service bzr-test account probe
secret-tool lookup service bzr-test account probe
secret-tool clear service bzr-test account probe
```

If `secret-tool` fails with "Cannot autolaunch D-Bus without X11
$DISPLAY", you are in a headless session. Use `api_key_env` instead.

If the daemon is running but the keyring is locked, unlock it via your
desktop's keyring application, then retry `bzr`.

### macOS

Keychain entries live in your login keychain. Inspect them via
**Keychain Access.app** under the `bzr` service name (or whatever
custom `--service` you chose).

macOS may prompt the first time `bzr` reads an entry. Choose "Always
Allow" to suppress subsequent prompts for the same binary path.

To remove an entry manually:

```bash
security delete-generic-password -s bzr -a <account>
```

### Windows

Credentials are stored in **Windows Credential Manager** under
`Generic Credentials`. List them from the command line:

```cmd
cmdkey /list
```

To delete an entry manually:

```cmd
cmdkey /delete:bzr:<account>
```

### Error: "OS keychain unavailable"

This indicates the platform backend is reachable but returned an error.
Common causes:

- **Linux:** Secret Service daemon crashed or never started. Restart
  your desktop session or start the daemon explicitly.
- **macOS:** user denied the Keychain access prompt. Retry and click
  "Always Allow".
- **Windows:** Credential Manager service is disabled. Start it via
  `services.msc` (Credential Manager).

If you cannot resolve the keychain issue, fall back to `api_key_env`:

```bash
bzr config set-server <name> --url <url> --api-key-env BZR_API_KEY
export BZR_API_KEY=<your-key>
```

### Error: "OS keychain locked or inaccessible"

This is a distinct failure mode (keyring v3's `NoStorageAccess`) meaning
the store is present but the process can't access it — typically
because the Linux keyring is locked, or the macOS Keychain has been
locked manually.

Unlock the keyring and retry. On Linux:

```bash
# Unlock the login keyring (GNOME)
gnome-keyring-daemon --unlock
```

On macOS, use Keychain Access.app to unlock the login keychain.

### Error: "no API key found in OS keychain"

The server's `api_key_keyring` reference points at an entry that
doesn't exist. Create it with:

```bash
bzr config set-keyring <server>
```

Or migrate an existing credential into the keychain:

```bash
bzr config migrate-to-keyring <server> --yes
```

### Error: "this bzr build was compiled without keyring support"

You installed a `bzr` build that was compiled with
`--no-default-features`, which strips the `keyring` Cargo feature.
The feature is on by default, so a standard install restores it:

```bash
cargo install bzr --locked
# or, from a local source checkout:
cargo install --path . --locked
```

If you need to run on a system that can't build `libdbus-1` (headless
Linux without a Secret Service daemon, minimal containers), keep the
stripped build and switch the affected server to `api_key_env`
instead:

```bash
export BZR_API_KEY=YOUR_API_KEY
bzr config set-server <server> --api-key-env BZR_API_KEY
```
