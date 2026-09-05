# Auth, configuration, and TLS comparison design

## Scope

Issue #669 adds the R11 comparison phase for python-bugzilla 3.3.0 and bzr against the existing
Bugzilla 5.0, 5.2, and 5.3 containers. It changes functional comparison fixtures and parity
documentation only. It does not add production authentication, configuration-import, Bearer, or
client-certificate behavior to bzr.

The design extends [ADR 0044](../../adr/0044-python-bugzilla-comparison-sidecar.md), follows
[ADR 0050](../../adr/0050-run-comparison-proxies-in-sidecar-namespace.md), and uses
[ADR 0051](../../adr/0051-share-adapter-with-bounded-local-proofs.md) for its library-only proof.
The operator selected proxy processes inside the existing sidecar's shared Bugzilla network
namespace.

## Architecture

Add one ordered comparison phase, `compare/06-auth-config-tls.sh`. The host `bzr` process continues
to reach Bugzilla and host-side proxies through runtime-assigned loopback ports. Python-bugzilla
runs in the existing sidecar and reaches Bugzilla or namespace-local proxy processes on
`127.0.0.1` fixed internal ports. Both paths target the same Bugzilla container.

`run-compare.sh` stages the two proxy programs in the existing private exchange directory before
starting the sidecar. `lib.sh` owns fixed-purpose helpers to add the Red Hat alias, start a proxy,
check readiness from inside the sidecar, read sanitized evidence, and stop a namespace proxy. Each
start creates a fresh private evidence log after proving no prior PID remains; assertions consume
that exact path and require one current credential-kind record. The runner's EXIT trap remains the
final cleanup boundary.

The Red Hat proxy gains a request transform selected by a `bearer-auth` fixture mode. It accepts a
non-empty `Authorization: Bearer <value>`, removes that header, forwards the same value as
`X-BUGZILLA-API-KEY`, and records only `auth-kind bearer count=1`. It also records query-parameter
and API-key-header credential kinds without values, so the phase can prove version-specific client
choices. Existing response rewrite hooks and modes remain unchanged.

## Comparison contracts

### API-key placement

For each supported Bugzilla version, the phase sends an authenticated identity request through a
credential-observing proxy for each client. The proxy log is the assertion surface: it must contain
exactly the expected `query`, `header`, or translated `bearer` kind and must not contain the API-key
value. The expected query/header split is derived from the live clients and fixed by fixture tests
before the real multi-version run is accepted.

### Token login and cache

The shared python-bugzilla adapter gains `login`, `cached_auth`, and `logout` operations because the
pinned CLI exposes login but not logout. Each consumes a private mode-0600 JSON request file rather
than argv or stdin and emits only booleans and non-secret identifiers. The phase proves a restricted
login succeeds, a token cache file is created in the sidecar home, a later adapter invocation reuses
the token after the password file is removed, and the library logout operation invalidates it. The
equivalent bzr surface is checked as a controlled absence and recorded against #676 (token
transport) and #681 (login/logout commands). Unexpected python-bugzilla failure is an ordinary
failure, not a gap.

### bugzillarc

The phase writes a private staged fixture to `/work` and copies it to the disposable sidecar's real
`/etc/bugzillarc`, then creates `~/.bugzillarc` and
`~/.config/python-bugzilla/bugzillarc` under its isolated home. Distinct non-secret sentinel values
prove later-file precedence, `[DEFAULT] url`, and URL-substring section selection. A fault fixture
omitting the `/etc` layer must fail the three-layer proof. bzr's missing import command is a
controlled parser gap against #682. The fixture never edits the host's `/etc` or home; sidecar
removal discards the system-level fixture.

### TLS verification

The existing host TLS fixture generates its short-lived CA, leaf certificate, and key beneath the
comparison exchange directory so the sidecar can read the same material at `/work`. A second
instance of `tls-proxy.py` listens on sidecar loopback and forwards to port 80. Python-bugzilla must
fail verification without `--nosslverify` and succeed with it. Existing bzr TLS functional coverage
remains the bzr-side proof. For the client-certificate gap, the shared adapter performs a local,
network-free positive control: it constructs the pinned client from a private dummy certificate
path and reports whether its requests session received that exact path, without returning path or
certificate content. That surface proof is paired with bzr's exact parser/config absence and #677.
Mutual TLS and a claim that the certificate authenticates remain excluded.

### Red Hat Bearer path

The sidecar receives an idempotent `bugzilla.redhat.com` loopback alias. Python-bugzilla calls the
namespace Red Hat proxy by that hostname, causing its `.redhat.com` branch to send Bearer auth. The
proxy translates the credential for the stock Bugzilla backend and logs only the credential kind.
The equivalent bzr parser/transport absence is a controlled gap against #678.

### Gap discipline and parity report

Every expected gap is eligible only after its python-bugzilla positive control succeeds and the bzr
absence matches an exact parser or transport observation. The #677 positive control is explicitly a
local client-configuration proof, and its parity row says `surface gap` rather than implying a live
mutual-TLS exchange. The phase records #676, #681, #682, #677, and #678 separately.
`docs/dev/python-bugzilla-parity.md` adds one row per stable comparison test ID; no row claims parity
without a live assertion.

## Failure handling and cleanup

- Invalid proxy kind, port, PID record, or path fails before a container command is run.
- A start refuses a live prior PID, replaces stale PID state, and creates a new empty evidence log;
  the returned log path is the only evidence source for that start.
- Namespace readiness has a bounded retry count and reports the proxy's sanitized log on failure.
- Proxy stop is idempotent. A missing process is accepted only after the sidecar confirms it is not
  running; a malformed PID record is a failure.
- Runner cleanup stops namespace proxies before removing the sidecar. Sidecar removal remains the
  fallback when a phase exits early.
- Every test resets cache/config files it owns before its positive control, so the persistent named
  home cannot make a fresh run pass from stale state.
- Raw passwords, tokens, API keys, certificate paths, and Authorization values never enter process
  argv, test names, proxy evidence, terminal output, or the parity report.

## Threat model

### Boundaries and actors

- The local operator or CI job controls environment overrides and invokes Docker or Podman.
- The disposable Bugzilla server consumes test credentials and request bodies from both clients.
- Python-bugzilla consumes mode-0600 adapter request files, generated config files, passwords, API
  keys, tokens, URLs, and TLS material inside its sidecar.
- The Red Hat proxy consumes untrusted HTTP headers, query strings, and bodies before forwarding.
- Proxy scripts and TLS material cross from the host runner into the sidecar through `/work`.

This design adds namespace-local proxy entry points and a Bearer-to-API-key translation. It widens
the existing Red Hat proxy from response transformation to bounded request credential handling.

### Controls

- Namespace proxies bind `127.0.0.1`, accept only validated numeric fixed fixture ports, and forward
  only to the fixed Bugzilla loopback endpoint.
- The Red Hat proxy keeps the existing one-MiB request-body bound and hop-by-hop header filtering.
- Bearer translation requires the explicit fixture mode and a non-empty single Authorization value;
  the original Authorization header is not forwarded alongside the API-key header.
- Evidence records credential kinds and counts only. Fixture tests search evidence and failure
  output for the known secret sentinels and fail on disclosure.
- Login, cached-auth, logout, and client-certificate adapter operations read secrets and paths from
  private request files and serialize only bounded non-secret facts.
- Staged files stay beneath the mode-0700 comparison directory; secret-bearing files remain mode
  0600. Paths are fixed by the runner rather than derived from server input.
- Container commands pass data as quoted argv or positional shell parameters; no fixture value is
  evaluated as shell source.
- Cleanup targets only the checkout/version-scoped sidecar and PID files created by this run.

### Out of scope

The disposable Bugzilla service is not isolated from other processes already sharing its network
namespace. The tests do not establish whether a production Red Hat server still requires Bearer
auth, and therefore retain #678 as a gap. They do not implement mutual TLS, protect deliberately
malicious local operators, or turn disposable test credentials into production secrets.

## Verification

- Proxy self-tests prove Bearer translation, header removal, empty/malformed rejection, credential-
  kind evidence, secret redaction, and preservation of existing response modes.
- Sidecar fixture tests prove staging, fixed namespace endpoints, alias installation, bounded
  readiness, fresh per-start evidence, idempotent stop, cleanup after failure, and rejection of
  malformed inputs.
- Adapter fixtures prove private-file token lifecycle operations, password-free cache reuse,
  library logout, and the network-free client-certificate surface observation.
- Phase fixture tests prove each positive-control-before-gap transition and stale-gap behavior.
- `make check-functional-test-ids` validates the new phase and evidence IDs.
- `make lint` and `make test` keep repository guardrails green.
- `make functional-compare-all` proves the comparison across bz50, bz52, and bz53.
- `make functional-test-all` proves existing host proxy and functional behavior remains green.
