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
- **CONC-2 (real defect) — no lost updates.** Concurrent `bzr` processes that
  mutate config do not silently drop each other's edits. Fix via a
  `Config::update_locked(|cfg| …)` API: acquire an exclusive advisory lock
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
  `src/commands/bug/search.rs` (`--save` query). To make bypass impossible rather
  than relying on enumeration staying correct, the **lock + reload + atomic write
  become the only write path**: `save()`/`save_without_validation()` are removed
  (or made private to `config`), all callers migrate to `update_locked`, and a CI
  guard (`check-no-direct-save`, a grep) fails the build if a `config.save()`
  reappears outside `config.rs`.
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
   two-fd `flock` test self-deadlocks):* spawn a **second OS process** (a small
   test helper binary, or a forked `Command`) that takes the lock and holds it
   briefly, and assert the parent's `update_locked` blocks until release rather
   than interleaving. *Expected: (a) reproduces lost-update on current code; (b)
   becomes the regression test for the lock itself.*

## Method

- **Adversarial tests** for CONC-1 (content-survival under concurrent reads) and
  CONC-2 (reload-merge logic + the two-process mutual-exclusion test), kept as
  permanent regression guards.
- **Targeted unit tests** for CONC-2b (closure is non-interactive; a re-entrant
  `update_locked` is rejected, not deadlocked), CONC-4 (double-init safe), CONC-5
  (second `set()` ignored).
- **CI grep** for CONC-3 (`check-no-spawn`) and the `check-no-direct-save` guard
  — build-time checks, not `#[test]`s.
- Every test is written failing-first against current code. A test that
  reproduces a real break drives a TDD fix (red → green); the fix diff carries
  that test. Per the pre-checks, CONC-1 and CONC-2 are expected to reproduce
  breaks.

## Orchestration

Led in priority order so the real sinks are worked first:

**CONC-1 → CONC-2 → CONC-2b → CONC-3 → CONC-4 → CONC-5.**

Execution method (direct vs. multi-agent worktree workflow) is decided with the
user at execution time, consistent with Surface 1. Confirmed breaks are fixed via
TDD locally; the user confirms before each push.

## Design: atomic write + `update_locked`

- **Atomic write (CONC-1).** Replace the in-place truncate-write in
  `write_private_file` with: create `config.toml.<unique>.tmp` in the same
  directory with `0600`, write the full serialized content, `fsync(temp)`,
  `rename` over `config.toml`, then **`fsync` the containing directory** so the
  rename survives a crash/power-loss (the temp-file fsync alone does not make the
  directory entry durable). `rename` within a directory is atomic on POSIX; on
  Windows plain `rename` over an existing file is *not* guaranteed and may need
  `ReplaceFileW` or a retry loop (tracked as an open decision). A failed write
  aborts before the rename, leaving the original intact; the temp file is cleaned
  up on failure.
- **Advisory lock + reload (CONC-2).** `Config::update_locked(mutator: impl
  FnOnce(&mut Config) -> Result<()>)` becomes the **single write path** (bare
  `save()` is removed/privatised so no caller can bypass the lock):
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
- **Dependency.** Add `fs4` (the actively-maintained successor to `fs2`) for
  cross-platform advisory locking, pinned to its current stable version (looked
  up at implementation time). Justification: hand-rolling `flock`/`LockFileEx`
  across Unix and Windows is error-prone; `fs4` is a small, focused crate.

## Challenge-review fixes

A hostile `/challenge` review found six material defects in the first draft; all
are addressed above:

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
   path, `save()` is removed/privatised, and a `check-no-direct-save` CI guard
   prevents regressions.
4. **Power-loss gap.** The atomic write now `fsync`s the directory after the
   rename (temp-file fsync alone is insufficient for crash durability).
5. **CONC-3 was unfalsifiable as a test.** Reclassified as a `check-no-spawn` CI
   grep, not a `#[test]`.
6. **Lockfile path was hardcoded.** Now derived from the resolved config
   directory so an `XDG_CONFIG_HOME` override still shares one lock.

## Delivery

- New tests land as permanent regression guards even where the invariant holds.
- One branch + PR per *fixed* defect, labeled `security` and `red-team`, with the
  threat model in the body. Each PR is confirmed with the user before push.
- A `CHANGELOG.md` Security/Fixed entry is added as the work lands.
- A checkpoint with the user before moving on to the supply-chain surface.

## Open implementation decisions (surface during the fix)

- **`update_locked` refactor depth** — how much of `connect_and_configure`'s
  in-memory `config` is refreshed after a locked write (the in-memory copy used
  for subsequent client construction goes stale relative to disk; decide whether
  to re-read or to keep using the known-applied values).
- **Lockfile staleness / cleanup** — whether the lockfile is ever removed, and
  how a stale lock (holder crashed) is handled (`fs4` advisory locks release on
  process exit, so a crashed holder does not leak the lock; confirm).
- **Windows `rename`-over-existing semantics** — confirm the atomic-replace path
  on Windows (may need `ReplaceFile`/retry rather than plain `rename`).

## Out of scope (this engagement)

- Supply-chain surface (`deny.toml`, lockfile, CI provenance, XML-RPC parsing as
  malicious input) — separate engagement.
- Adding a multi-threaded runtime or concurrent fetch fan-out — not a current
  feature; CONC-3 only guards against a *future* such change regressing the
  in-process-safety assumption.
- Cross-host filesystem / NFS locking correctness — advisory locks over network
  filesystems are best-effort; out of scope.
