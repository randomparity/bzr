# 0002 — Per-test config-path isolation replaces the shared env lock

- Status: Accepted
- Date: 2026-06-23
- Issue: #421

## Context

Async tests that exercise config loading, server connection, and auth/API
detection point `bzr` at a throwaway config by mutating the process-global
`XDG_CONFIG_HOME` environment variable. Because that variable is process-global
and is read *lazily* inside `connect_and_configure` / `dispatch` (not at test
setup time), every such test must:

1. Serialize behind a single `tokio::sync::Mutex` (`ENV_LOCK`) so concurrent
   tests do not clobber each other's `XDG_CONFIG_HOME`, and
2. Hold the guard across **all** of its `.await` points — including the mock
   network round-trips — because the env var must still be set when the command
   reads it deep inside the call stack.

Desloppify flags these tests as "async lock guard held across await points."
The guard-across-await pattern is not itself a `tokio::sync::Mutex` misuse (that
type is designed to be held across awaits), but it does make the whole
env-touching test suite a single global critical section: tests cannot run in
parallel, and the breadth of the lock hides which state it actually protects.

The production code already supports an alternative: every config-loading and
config-persisting call site threads `CommandContext::config_path_override`
(surfaced from the `--config` global flag). `Config::path_at` resolves with the
precedence `explicit override > BZR_CONFIG > XDG_CONFIG_HOME`, so an explicit
path short-circuits before any environment variable is consulted.

## Decision

Tests select their throwaway config by passing an **explicit per-test config
path** — via `CommandContext::with_config_path_override` for `execute`-level
tests, or the `--config <path>` flag for `dispatch`-level tests — instead of
mutating `XDG_CONFIG_HOME`. Such tests acquire no shared lock and run in
parallel.

`ENV_LOCK` is retained for, and only for, tests that genuinely mutate
process-global environment state that command resolution must observe:

- tests that `set_var`/`remove_var` an **API-key environment variable** named by
  `api_key_env` / an inline `--server-api-key-env`,
- the test that deliberately verifies `--config` overrides `XDG_CONFIG_HOME`
  precedence (it must set both), and
- tests that trigger **libc name resolution** of a hostname (the self-signed TLS
  tests connect to `https://localhost:{port}` to match the cert SAN;
  `getaddrinfo("localhost")` reads env outside Rust's serialized env lock, so the
  lock keeps that C-side `getenv` from racing a concurrent `set_var`). Tests that
  connect only to numeric `127.0.0.1` URLs take glibc's numeric fast path, read
  no env via libc, and need no lock.

Each retained `ENV_LOCK` acquisition keeps a comment stating which
process-global variable it protects and why the explicit-path approach does not
apply.

This is scoped to the four files named in issue #421
(`src/test_helpers.rs`, `src/lib_tests.rs`,
`src/commands/runtime/shared/mod_tests.rs`, `src/commands/field_tests.rs`). The
shared `setup_test_env` / `setup_empty_config_env` helpers stay in place,
unchanged, for the ~100 call sites in other test files that are out of scope.

## Consequences

- Migrated tests no longer serialize behind `ENV_LOCK`; they hold no guard
  across awaits, so the desloppify finding clears for them and they can run
  concurrently.
- The invariant guarded by `ENV_LOCK` narrows from "any test that wants an
  isolated config" to "a test that mutates a process-global env var." The lock's
  purpose becomes legible at every remaining call site.
- A new lightweight helper writes a config file to a temp dir and returns its
  path (no env mutation), centralizing the directory/permissions boilerplate the
  per-file writers previously duplicated.
- Test behavior is unchanged: same configs, same mocks, same assertions. Only
  the config-selection mechanism (explicit path vs. env var) changes.

## Considered & rejected

- **Narrow the lock to wrap only the `set_var` call.** Not feasible: the env var
  is read lazily inside the command under test, long after setup returns, so the
  guard must remain held until the command finishes reading config. Narrowing the
  guard to the mutation alone would reintroduce the race the lock exists to
  prevent.
- **Replace `ENV_LOCK` with a per-variable lock registry.** More machinery than
  the problem warrants; the handful of genuine env-mutating tests are cheap to
  serialize behind one mutex, and a registry would not remove the
  guard-across-await pattern for them.
- **Switch `ENV_LOCK` to a `std::sync::Mutex`.** Does not address the finding —
  the guard would still span awaits (and a std mutex held across an await is a
  genuine clippy `await_holding_lock` violation, which is worse).
- **Migrate all ~100 `setup_test_env` call sites.** Out of scope for #421 and a
  much larger, riskier change; the four flagged files capture the tests the audit
  identified.

## Amendment — 2026-09-15 (issue #831, epic #818)

### Why this was reopened

This ADR scoped itself to four files and left `setup_test_env` /
`setup_empty_config_env` in place for the call sites it declared out of scope.
Epic #818 migrated those. Issue #831 asked whether the env-mutating sites this
ADR retained need something stronger than `ENV_LOCK` now that a large lock-free
population runs beside them.

### The premise the question rested on does not hold

#831 argued that `tempfile::TempDir::new` reads `TMPDIR` and therefore races
every retained `set_var`. It does not. On unix `std::env::var_os` takes the read
half of an `RwLock` whose write half `std::env::set_var` takes
(`std/src/sys/env/unix.rs`); `std::env::temp_dir` is a `var_os("TMPDIR")` call
(`std/src/sys/pal/unix/os.rs`); and `tempfile::env::temp_dir` delegates to it
(`tempfile/src/env.rs`). `set_var`'s own safety contract (`std/src/env.rs`)
scopes its requirement to readers reaching the environment through "functions or
global variables **other than** the ones in this module". A `tempfile` temp-dir
creation is not such a reader.

Neither is `std::process::Command::spawn`: std takes the same env read lock
around fork/exec and around `posix_spawn`
(`std/src/sys/process/unix/unix.rs`).

What genuinely races is a reader called from C, which std names explicitly:
`getaddrinfo` via `ToSocketAddrs`. This ADR already identified it.

These citations are to pinned dependencies — the toolchain in
`rust-toolchain.toml` and the exact `tempfile` version in `Cargo.toml`. Bumping
either is what invalidates this section, not a change to the test tree.

### Decision

1. **Stop retaining sites that were never retained for a reason.** Most of the
   `set_var("XDG_CONFIG_HOME")` sites set a variable nothing in the test then
   reads: each already selects its config by explicit path, and
   `Config::path_at` returns an explicit override before consulting any
   variable. Those `set_var`s and their `ENV_LOCK` acquisitions are deleted,
   leaving only the tests whose subject is environment-based resolution.
   Shrinking the writer population is the only mitigation here that is both free
   and real.

2. **Take `tempfile` out of the per-creation env-reading population.**
   `test_helpers::init_temp_root` installs `tempfile::env::override_temp_dir`
   once per test process, so a temp directory resolves its root without
   consulting the environment. This is defence in depth — it removes a
   dependency on a std implementation detail — not the repair of a live race,
   which the section above shows does not exist.

3. **Every surviving `set_var` carries a SAFETY comment stating what the lock
   actually buys.** The previous wording — "tests are serialized via ENV_LOCK;
   no other threads read this var concurrently" — was false when written.

### What `ENV_LOCK` is retained for

Retained categories, superseding the three this ADR originally listed:

- a test that mutates a process-global variable **the command under test must
  observe** — an API-key variable named by `api_key_env` /
  `--server-api-key-env`, a `BZR_*_TEST_*` hook, `EDITOR`;
- a test whose subject **is** environment-based resolution — the
  `Config::path_at` precedence tests, the `DISPLAY`/`WAYLAND_DISPLAY` test;
- a test that triggers **libc name resolution** of a hostname, because
  `getaddrinfo` reads the environment outside std's lock. A test that connects
  only to a numeric address takes the numeric fast path and needs no lock.

This amendment publishes **no count of the retained set, deliberately.** A
count inside an ADR is a measurement with a shelf life embedded in an artifact
that has none, and this campaign has now had three measured claims falsified by
a sibling merge with no diff to signal it. Worse, the obvious predicate —
`git grep 'ENV_LOCK\.\(blocking_\)\?lock()' -- src tests` — sees explicit
acquisitions only. A test can hold the lock *implicitly* through a helper that
takes it on the caller's behalf, so converting such a helper into explicit
per-test acquisitions makes the count rise while the risk falls. A reader given
only the number draws the opposite conclusion from the truth.

To re-derive the set at any commit: run that predicate, add the helpers that
take the lock on a caller's behalf, and classify every hit against the three
categories above. **A hit matching none of them is a defect, not a retention.**
Put the resulting enumeration in a pull-request body, where a stale number is
harmless.

### What `ENV_LOCK` does and does not do

It **does**:

- order writers that take it against each other, so two tests cannot clobber
  each other's value of the same variable;
- order a writer against a **libc-side** env reader in a test that also takes
  the lock.

It **does not**:

- order a writer against a lock-free test. It never did. What makes a migrated
  test safe is not the lock; it is that its env reads all go through
  `std::env`, which carries its own ordering, and that it reads no variable any
  writer writes.
- make `set_var` sound in general. It stays `unsafe` for the reason std gives:
  no library advertises which of its functions call libc `getenv`, and the set
  can change in a patch release. The retained writers are a bet that nothing in
  this dependency tree does so off the lock — re-validated by reading the tree,
  not by a green suite.
- prevent **semantic** cross-talk, which is a different failure from a data
  race. A writer that leaves `XDG_CONFIG_HOME` set changes what
  `Config::path_at(None)` resolves to for every later test in the same binary,
  lock or no lock.

### Residual risks

- **The `getaddrinfo` pairing is enforced by nothing.** It is ordered only
  because both participants happen to take `ENV_LOCK`. A future test that
  resolves a hostname without taking it reintroduces the race silently; no lint,
  guardrail or CI job detects that.
- **A green `make test` is not evidence.** Env data races and the stdin
  contention characterised under #832 are probabilistic and per-run — which test
  loses varies between runs. Treat a green suite as the absence of a signal, not
  the presence of a proof. Nothing in this amendment is justified by a passing
  run.
- **`override_temp_dir` installs on first use, not at process start.** It is a
  one-shot `OnceCell`, and a Rust test binary has no pre-`main` hook without a
  new dependency. Most test files construct a `tempfile::TempDir` directly
  rather than through a shared helper, so one of those can win the race to the
  first creation and perform one `TMPDIR` read. Closing that completely would
  need that dependency, or converting every direct construction site to an `_in`
  variant. Neither is warranted for a read that is already ordered.
- **`config_path_resolution_precedence` leaves `XDG_CONFIG_HOME` pointing at an
  absolute throwaway path** when it finishes, restoring only `BZR_CONFIG`. That
  residue is accidental but currently load-bearing, and tidying it would make
  the suite less safe: see the next point.
- **A `Config::path_at(None)` reader with no writer left in the process resolves
  the developer's real config** — `~/Library/Application Support/bzr/config.toml`
  on macOS, *not* `~/.config/bzr`, so checking `~/.config/bzr` proves nothing.
  Issue #835 removes the last helpers that point `XDG_CONFIG_HOME` somewhere
  harmless; it must remove the `path_at(None)` readers in the same change.

### Considered & rejected (for #831)

- **Move the residual env-mutating tests into their own test binaries**, as
  `tests/conc2_lock.rs` does. Declined: once the population is this small, a
  whole extra binary per test costs more build complexity than the ordering it
  buys, and it does not address `getaddrinfo` either.
- **Keep `ENV_LOCK` and document the gap only.** Declined: it leaves in place
  writers whose only effect is to widen the surface being documented.
