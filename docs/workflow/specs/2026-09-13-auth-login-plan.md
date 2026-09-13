# Auth login implementation plan

1. Add the `auth` CLI command and dispatch/capability routing. Accept password
   through a required flag and `--restrict-login`; retain no credential in CLI
   output.
2. Add REST and XML-RPC login/logout client operations. Keep mode selection at
   the client boundary and use the existing hybrid fallback convention.
3. Add an auth command handler that loads a named server, builds an
   unauthenticated client from its TLS and API settings, then atomically writes
   or clears only `ServerConfig::token` after the server operation succeeds.
4. Add sibling unit tests for CLI parsing, transport payloads, persistence
   ordering, and error preservation. Extend auth comparison and functional
   phases, CLI reference, and parity report.
5. Run formatting, clippy, focused tests, lint, the quiet test suite, and the
   functional proof; review and simplify the final diff before publishing.
