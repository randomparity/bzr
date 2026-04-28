# SonarCloud Quality Gate Policy

This project uses SonarCloud's "new code" quality gate, configured in the
SonarCloud project settings (not in this repo). The gate enforces:

- **New-code line coverage:** ≥85%
- **New-code duplication density:** ≤2%
- **0 new bugs / vulnerabilities / security hotspots**
- **Maintainability rating on new code:** A

Existing files are tracked but not gated retroactively. Files modified by
a PR have their changed lines evaluated as "new code." See
https://sonarcloud.io/project/overview?id=randomparity_bzr for the
current dashboard.

## Known coverage exceptions

- **`src/main.rs`** sits at ~81% line coverage. The `#[tokio::main]
  async fn main()` body and Windows-only platform code (`#[cfg(windows)]
  suppress_stdout`) are unreachable from unit/integration tests. Pushing
  higher requires functional tests that invoke the compiled binary.

- **`src/tls/tofu.rs`** sits at ~88% line coverage. The remaining gaps
  (`prompt_tofu`/`prompt_rotation` interactive-stdin paths and
  `probe_server_cert` post-handshake success path) require a TTY harness
  and a live TLS server, neither of which exists in the project's test
  infrastructure.

- **`src/commands/shared.rs`** sits at ~87% line coverage. The TOFU
  decision arms in `handle_tofu` and the rotation-accepted path in
  `handle_pin_rotation` require interactive stdin. The TLS-error path
  rebuild in `detect_with_tofu_fallback` requires a live TLS server.

These exceptions are intrinsic to the code (security-critical paths that
must run in a real TTY/TLS environment) rather than test-laziness, and
are accepted as the per-file coverage floor for these files.
