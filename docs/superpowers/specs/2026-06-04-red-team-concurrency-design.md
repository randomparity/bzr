# Red-Team Engagement — Surface 2: Concurrency

**Date:** 2026-06-04
**Branch:** `security/red-team-concurrency`
**Status:** approved design (revised after a hostile `/challenge` review — see
"Challenge-review fixes")

## Goal

Act as an adversary against the concurrency and shared-state surface of `bzr`.
Enumerate the invariants the persisted state relies on, write deterministic
adversarial tests that try to break each one, fix every reproduced break via TDD
(failing-then-passing test), and ship one labeled remediation PR per fixed defect
with the threat model documented.

This is the second of three surfaces. Supply-chain follows as a separate
engagement after a checkpoint.

## Reframe (from reconnaissance)

A surface map of the codebase established two facts that determine where the real
risk lives:

- The async runtime is `#[tokio::main(flavor = "current_thread")]`
  (`src/main.rs`) and there are **no** `tokio::spawn`, `join!`, `try_join!`,
  `select!`, or stream-fan-out call sites anywhere in `src/`. Batch paths
  (`src/commands/attachment.rs` download loops) are sequential `.await` loops.
  Therefore **in-process data races cannot occur** — there is no parallelism at
  runtime.
- The genuinely shared, mutable, adversary-reachable state is the **on-disk
  config file** (`~/.config/bzr/config.toml`), written by multiple code paths
  and potentially by multiple concurrent `bzr` processes. Notably, read-looking
  commands (`bzr bug search`) rewrite config during the TLS TOFU / pin-rotation
  flow (`src/commands/shared.rs`), widening the window for surprising writes.

The engagement therefore **leads with the multi-process / crash-during-write
hazards** and demotes the in-process hypotheses (keyring init, `CertCapture`,
async races) to regression guards and latent-footgun notes.

## Threat model

- **Adversary:** concurrent `bzr` invocations (the user runs two commands at
  once, or a long-running interactive command overlaps a quick one), and the
  abrupt-termination case (crash / `SIGKILL` / power loss) mid-write. A
  malicious server is a secondary actor: it can *trigger* a config write
  (TOFU/pin-rotation) at a time of its choosing, but does not control the
  filesystem.
- **Trust boundary:** the config file is the trusted persisted state. Its
  integrity (never empty/partial) and its freshness (concurrent edits not
  silently lost) are the assets.
- **Assets under attack:**
  1. **Config file integrity** — a reader must never observe an empty or
     truncated/partial TOML, and an interrupted write must leave the previous
     config intact. The power-loss case (the previous config must survive a
     crash *after* the rename) additionally requires a directory `fsync`; see
     the Design section.
  2. **Config edit durability** — a concurrent process's committed change must
     not be silently overwritten by another process's stale write (lost update).
  3. **Liveness** — the persistence mechanism must not be able to wedge other
     `bzr` invocations (e.g., by holding a lock across an interactive prompt).

## Invariant catalog

The tests assert these. Each is written failing-first. Priority reflects the
reconnaissance: the real on-disk sinks (CONC-1, CONC-2) lead; the
single-threaded-runtime paths land as regression guards.

- **CONC-1 (flagship, real defect) — write atomicity.** A concurrent reader
  always observes either the complete old config or the complete new config,
  never an empty/partial one. *Current code (`src/config.rs`,
  `write_private_file`) opens the live file with `truncate(true)` then `write_all`
  (`src/config.rs:337-343`), so a read between truncate and write observes a
  zero-byte file.* **Observable correction (challenge finding):** a zero-byte
  TOML does **not** fail to parse — `servers` is `#[serde(default)]`, `Option`
  fields default to `None`, and `Config::load` even maps a missing file to
  `Ok(Config::default())` (`src/config.rs:249`). So the corruption manifests as a
  reader silently getting a config with **no servers / missing fields**, *not* a
  parse error. The invariant and its test therefore assert *content survival* —
  "a read taken during writes still contains the known server" — never "parsing
  fails." Fix: write to a temp file in the same directory → `fsync(temp)` →
  atomic `rename` over the target → `fsync(dir)`, preserving `0600` (file) /
  `0700` (dir) hardening. Anchor: `src/config.rs` `save` / `write_to_disk` /
  `write_private_file`.
- **CONC-2 (real defect; data-integrity, not exploitable security) — no lost
  updates.** Concurrent `bzr` processes that mutate config do not silently drop
  each other's edits. *Severity calibration (challenge finding):* the threat
  model concedes the malicious server does not control the filesystem and cannot
  force or time a second concurrent invocation, so this is a **robustness /
  data-integrity** defect, not an attacker-exploitable vulnerability — and it is
  the heaviest lift in the engagement (new `fs4` dependency + sole-write-path
  refactor across five files + two-process test). It is therefore gated on an
  explicit cost/benefit check at execution time, and ships **separately** from
  and **after** CONC-1 (see Delivery). Fix via a `Config::update_locked(|cfg| …)`
  API: acquire an exclusive advisory lock
  (`fs4`) on a lockfile in the **resolved config directory** (`config.lock`,
  sibling of `config.toml` — *not* a hardcoded `~/.config/bzr` path, so an
  `XDG_CONFIG_HOME` override still shares one lock) → **reload the latest config
  from disk** → apply the mutation closure → atomically write (CONC-1) → release.
  Reloading *under the lock* is what actually closes the race: a plain lock
  around only the write still lets two processes load stale state and serialize
  last-writer-wins.
  **Complete writer set (challenge finding — the original anchor list was
  incomplete).** Every `config.save()` / `save_without_validation()` caller must
  route through `update_locked`, or the guarantee leaks at the omitted site. The
  full set (verified by grep, 14 sites across five files): `src/commands/shared.rs`
  (`persist_detected_settings`, `handle_tofu`, `handle_pin_rotation`),
  `src/commands/config.rs` (`set-default`, `set-server`, `set-keyring`,
  `unset-keyring`, `tls_pin_clear`, `set-default`-path), `src/commands/template.rs`
  (template add/remove), `src/commands/query.rs` (saved-query add/remove), and
  `src/commands/bug/search.rs` (`--save` query). To make bypass impossible, the
  choke point is **compiler-enforced, not grep-enforced**: `save()` /
  `save_without_validation()` become **private to the `config` module**, so any
  external write path fails to *compile*. `update_locked` (the only `pub` write
  API) is then the sole entry, and all 14 callers migrate to it. A string grep
  (`config.save()`) was rejected as the guard: it is binding-name-fragile
  (`let cfg = …; cfg.save()` evades it) and noisy against `*_tests.rs`; Rust
  visibility gives the guarantee for free.
- **CONC-2b — lock liveness & re-entrancy.** Two distinct properties:
  (a) *Liveness:* the lock is never held across interactive I/O — the TOFU /
  pin-rotation prompts run *outside* the lock and only produce the values to
  persist; the `update_locked` closure is non-interactive and applies the delta
  to a freshly-loaded config. This prevents a process parked at a `[y/N/always]`
  prompt from wedging every other `bzr` invocation.
  (b) *Re-entrancy:* `update_locked` is **explicitly non-reentrant** — `fs4`
  `flock` treats two `open()` descriptions in the same process as independent, so
  a nested `update_locked` (a closure that itself calls `update_locked`) would
  self-deadlock. The design forbids nesting: closures perform only in-memory
  mutation. A debug assertion / test guards against a re-entrant call. (The
  earlier draft asserted re-entrancy was *safe*; that was wrong and is corrected
  here.)
- **CONC-3 (demoted) — runtime confinement.** The runtime is current-thread with
  no task fan-out, so in-process data races cannot occur. *Guard (challenge
  finding — this is not unit-testable: `#[tokio::test]` is current-thread
  regardless of `main.rs`, and "no `spawn`" is a static property):* a CI grep
  (`check-no-spawn`, a `make` target / shell check) fails the build if
  `src/main.rs` stops declaring `flavor = "current_thread"` or if a
  `tokio::spawn`/`join!`/`select!` fan-out appears in `src/`, forcing a
  re-evaluation of the in-process-safety assumption. Not a `#[test]`.
  *Expected: holds — regression guard.*
- **CONC-4 (demoted) — keyring init idempotence.** `ensure_default_store`
  (`src/credentials/keyring.rs`) uses an unguarded check-then-act, but it is
  idempotent and benign under the current-thread runtime. Guard test that a
  double init is safe; record the latent footgun (would need guarding if a
  multi-thread runtime is ever adopted). *Expected: holds — regression guard.*
- **CONC-5 (demoted) — `CertCapture` set-once.** Each probe constructs a fresh
  `OnceLock` (`src/tls/tofu.rs`), so the set-once "first cert wins" semantics
  hold even if rustls invokes the verifier more than once. Guard test that a
  second `set()` is ignored. *Expected: holds — regression guard.*

## Empirical pre-checks (before committing the workflow)

Run these as throwaway tests, record results in the spec's Method section. Both
are deterministic (they use real OS threads / explicit interleaving in the test,
independent of the app's single-threaded runtime).

1. **CONC-1 probe.** Spawn one `std::thread` that repeatedly reads the config via
   `Config::load()` while the main thread performs many `save()` calls. Assert on
   **content survival**, not parse success: a read that succeeds but returns a
   config *missing the known server* is the corruption signal (a zero-byte read
   parses to an empty default `Config` — see CONC-1). Under the current
   truncate-then-write path, some reads return the empty/default config.
   *Expected: reproduces → confirms the corruption defect.*
2. **CONC-2 probe (two parts).** (a) *Logic:* explicitly interleave two stale
   load→modify→save sequences (load A; load B; A sets a pin and saves; B sets
   `auth_method` and saves) and assert A's pin survives — this exercises the
   reload-merge logic but **not** the lock. (b) *Mutual exclusion (challenge
   finding — a single-process interleave never tests the lock, and an in-process
   two-fd `flock` test self-deadlocks):* a **second OS process** (a small test
   helper binary) acquires the lock and signals readiness (writes a ready-file /
   closes a pipe) and waits for a go-ahead before releasing. The parent, once it
   sees readiness, calls `try_lock_exclusive()` and asserts it returns
   `WouldBlock` *while the child holds the lock*, then signals the child to
   release and asserts the lock now succeeds. **No `sleep`-based timing
   assertions** — readiness and release are explicit signals, so the test is
   deterministic, not a timing race. *Expected: (a) reproduces lost-update on
   current code; (b) becomes the regression test for the lock itself.*

## Method

- **Adversarial tests** for CONC-1 (content-survival under concurrent reads) and
  CONC-2 (reload-merge logic + the two-process mutual-exclusion test), kept as
  permanent regression guards.
- **Targeted unit tests** for CONC-2b (closure is non-interactive; a re-entrant
  `update_locked` is rejected, not deadlocked), CONC-4 (double-init safe), CONC-5
  (second `set()` ignored).
- **CI grep** for CONC-3 (`check-no-spawn`) — a build-time check, not a `#[test]`.
  (The anti-bypass guard for CONC-2 is *compiler*-enforced via `save()` privacy,
  not a grep — see CONC-2.)
- **Failing-first applies to the reproduced defects only.** CONC-1 and CONC-2 are
  written failing-first and drive a TDD red→green fix (the fix diff carries the
  test). CONC-4 and CONC-5 are **characterization guards** — the invariant
  already holds and cannot be made to fail without changing the architecture, so
  they are *not* failing-first; they lock in current behavior. CONC-3 is a CI
  check, not a test. (Correcting an earlier blanket "every test is failing-first"
  claim, which was false for the demoted guards.)

## Orchestration

Led in priority order so the real sinks are worked first:

**CONC-1 → CONC-2 → CONC-2b → CONC-3 → CONC-4 → CONC-5.**

Execution method (direct vs. multi-agent worktree workflow) is decided with the
user at execution time, consistent with Surface 1. Confirmed breaks are fixed via
TDD locally; the user confirms before each push.

## Design: atomic write + `update_locked`

- **Atomic write (CONC-1) — two platform paths.** Both `write_private_file`
  impls (unix and non-unix; `src/config.rs:333-349`) are replaced.
  - *Unix:* create `config.toml.<unique>.tmp` in the same directory with mode
    `0600`, write the full serialized content, `fsync(temp)`, `rename` over
    `config.toml`, then **`fsync` the containing directory** so the rename
    survives a crash/power-loss (the temp-file fsync alone does not make the
    directory entry durable). `rename` within a directory is atomic on POSIX.
  - *Non-unix (Windows ships in CI):* the current non-unix branch is a bare
    `fs::write` with **no** permission hardening and no atomicity. Replace with a
    temp-file write followed by an atomic replace via `ReplaceFileW` (or
    `std::fs::rename` with a retry loop, since plain `rename`-over-existing is not
    guaranteed on Windows). There is no portable directory-`fsync` equivalent, so
    **the power-loss/crash-durability guarantee is scoped to POSIX**; on Windows
    CONC-1 guarantees concurrent-reader atomicity (no empty/partial read) but not
    crash-durability. This platform difference is stated, not hand-waved.
  - A failed write aborts before the replace, leaving the original intact; the
    temp file is cleaned up on failure.
- **Advisory lock + reload (CONC-2).** `Config::update_locked(mutator: impl
  FnOnce(&mut Config) -> Result<()>)` becomes the **single write path** (bare
  `save()` is made private to `config`, so no caller can bypass the lock — the
  compiler enforces it):
  1. open/create `config.lock` *in the resolved config directory* (`0600`) and
     take an exclusive `fs4` lock;
  2. reload the current config from disk;
  3. run `mutator` on it (non-interactive; nesting forbidden — a re-entrant call
     is rejected with an error rather than allowed to self-deadlock);
  4. atomic-write (CONC-1);
  5. drop the lock (released on guard drop, and on process exit if the holder
     crashes — `fs4` advisory locks do not leak).
  Call sites that currently do load→(prompt)→mutate→save are refactored so the
  prompt stays outside and the mutation becomes the closure body.
  **Closure precondition (challenge finding).** Because step 2 reloads from disk,
  the closure runs against the *on-disk* config, not the caller's in-memory copy.
  Closures must therefore be **self-contained**: they must not depend on
  unpersisted in-memory state, and must *upsert* (create-if-absent) any server
  they touch rather than `get_mut`-and-assume-present — otherwise a "set field on
  server X" closure silently no-ops when X is not yet on disk. A test covers the
  "configure a server not yet persisted" case to lock this in.
- **Dependency.** Add `fs4` (the actively-maintained successor to `fs2`) for
  cross-platform advisory locking, pinned to its current stable version (looked
  up at implementation time). Justification: hand-rolling `flock`/`LockFileEx`
  across Unix and Windows is error-prone; `fs4` is a small, focused crate.

## Challenge-review fixes

**Round 1** found six material defects; all addressed above:

1. **CONC-1 probe tested the wrong signal.** An empty/zero-byte TOML parses to a
   default `Config` (`servers` is `#[serde(default)]`; `Config::load` maps a
   missing file to `Ok(default)`), so "reads fail to parse" was a false-negative.
   The probe/invariant now assert *content survival* (known server present).
2. **CONC-2 lock was untested / re-entrancy claim was wrong.** A single-process
   interleave never exercises the lock, and an in-process two-fd `flock` test
   self-deadlocks. Added a real **two-process** mutual-exclusion test; made
   `update_locked` explicitly non-reentrant (rejected, not deadlocked).
3. **CONC-2 writer set was incomplete.** Five files / 14 `save()` sites write
   config (not the two originally named). `update_locked` is now the sole write
   path.
4. **Power-loss gap.** The atomic write now `fsync`s the directory after the
   rename (temp-file fsync alone is insufficient for crash durability).
5. **CONC-3 was unfalsifiable as a test.** Reclassified as a `check-no-spawn` CI
   grep, not a `#[test]`.
6. **Lockfile path was hardcoded.** Now derived from the resolved config
   directory so an `XDG_CONFIG_HOME` override still shares one lock.

**Round 2** found six second-order defects; all addressed above:

1. **Flaky lock test.** The mandated two-process test was specified as a sleep
   race ("holds briefly / blocks until release"); rewritten to use a readiness
   signal + `try_lock_exclusive() == WouldBlock` with no timing assertions.
2. **Fragile grep guard.** The `check-no-direct-save` grep was binding-name-fragile
   and test-noisy; replaced by **compiler-enforced** privacy (`save()` is private
   to `config`, so bypass fails to compile).
3. **Reload-under-lock precondition.** Added the requirement that `update_locked`
   closures be self-contained (upsert, not `get_mut`-assume-present) so a
   mutation on a not-yet-persisted server doesn't silently no-op; with a test.
4. **POSIX-only design.** The atomic write now specifies the non-unix/Windows
   path explicitly (`ReplaceFileW`/retry, no perm/atomicity gap) and scopes the
   crash-durability guarantee to POSIX rather than hand-waving Windows.
5. **Severity miscalibration.** CONC-2 reframed as data-integrity/robustness (not
   exploitable security); split into its own non-`security`-labelled PR behind a
   cost/benefit gate, shipping after CONC-1.
6. **Failing-first contradiction.** Scoped the failing-first rule to CONC-1/CONC-2;
   CONC-4/5 are characterization guards, CONC-3 is a CI check.

## Delivery

- New tests land as permanent regression guards even where the invariant holds.
- **Two separate PRs, not one** (severity calibration):
  - **CONC-1** — the crash/concurrent-read corruption fix — is the genuine
    data-loss/security deliverable; labeled `security` + `red-team`, threat model
    in the body, `CHANGELOG.md` **Security** entry. Ships first and independently.
  - **CONC-2** — the lost-update lock (`fs4` + sole-write-path refactor) — ships
    second, only after the cost/benefit gate, labeled `red-team` (and *not*
    `security`, since it is a robustness/data-integrity fix, not an exploitable
    vuln); `CHANGELOG.md` **Fixed** entry. Honest labeling avoids overstating it.
- The demotion guards (CONC-3/4/5) ride along with whichever PR is most relevant.
- Each PR is confirmed with the user before push.
- A checkpoint with the user before moving on to the supply-chain surface.

## Open implementation decisions (surface during the fix)

- **`update_locked` refactor depth** — how much of `connect_and_configure`'s
  in-memory `config` is refreshed after a locked write (the in-memory copy used
  for subsequent client construction goes stale relative to disk; decide whether
  to re-read or to keep using the known-applied values).
- **Lockfile staleness / cleanup** — whether the lockfile is ever removed, and
  how a stale lock (holder crashed) is handled (`fs4` advisory locks release on
  process exit, so a crashed holder does not leak the lock; confirm).
- **Windows atomic-replace mechanism** — the Design section scopes this
  (`ReplaceFileW` or `rename`-with-retry, durability scoped to POSIX); confirm the
  exact API and error/retry behavior during implementation.

## Out of scope (this engagement)

- Supply-chain surface (`deny.toml`, lockfile, CI provenance, XML-RPC parsing as
  malicious input) — separate engagement.
- Adding a multi-threaded runtime or concurrent fetch fan-out — not a current
  feature; CONC-3 only guards against a *future* such change regressing the
  in-process-safety assumption.
- Cross-host filesystem / NFS locking correctness — advisory locks over network
  filesystems are best-effort; out of scope.
