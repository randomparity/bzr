# Red-Team Engagement — Surface 2: Concurrency

**Date:** 2026-06-04
**Branch:** `security/red-team-concurrency`
**Status:** approved design

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
     config intact.
  2. **Config edit durability** — a concurrent process's committed change must
     not be silently overwritten by another process's stale write (lost update).
  3. **Liveness** — the persistence mechanism must not be able to wedge other
     `bzr` invocations (e.g., by holding a lock across an interactive prompt).

## Invariant catalog

The tests assert these. Each is written failing-first. Priority reflects the
reconnaissance: the real on-disk sinks (CONC-1, CONC-2) lead; the
single-threaded-runtime paths land as regression guards.

- **CONC-1 (flagship, real defect) — write atomicity.** `Config` persistence is
  atomic: a concurrent reader never observes an empty or partial config, and an
  interrupted/failed write leaves the previous config bytes intact. *Current
  code (`src/config.rs`, `write_private_file`) opens the live file with
  `truncate(true)` and then writes, so a crash or a concurrent read between
  truncate and write yields an empty or garbled TOML.* Fix: write to a temp file
  in the same directory → `fsync` → atomic `rename` over the target, preserving
  `0600` (file) / `0700` (dir) hardening. Anchor: `src/config.rs` `save` /
  `write_to_disk` / `write_private_file`.
- **CONC-2 (real defect) — no lost updates.** Concurrent `bzr` processes that
  mutate config do not silently drop each other's edits. Fix via a
  `Config::update_locked(|cfg| …)` API: acquire an exclusive advisory lock
  (`fs4`) on `~/.config/bzr/config.lock` → **reload the latest config from
  disk** → apply the mutation closure → atomically write (CONC-1) → release.
  Reloading *under the lock* is what actually closes the race: a plain lock
  around only the write still lets two processes load stale state and serialize
  last-writer-wins. Anchors: load→modify→save sites in
  `src/commands/shared.rs` (`persist_detected_settings`, `handle_tofu`,
  `handle_pin_rotation`) and `src/commands/config.rs` (`set-server`,
  `set-default`, `set-keyring`, `unset-keyring`, `tls_pin_clear`).
- **CONC-2b — lock liveness.** The advisory lock is never held across
  interactive I/O. The TOFU / pin-rotation prompts run *outside* the lock and
  only produce the values to persist; the `update_locked` closure is
  non-interactive and applies the delta to a freshly-loaded config. This
  prevents a process parked at a `[y/N/always]` prompt from wedging every other
  `bzr` invocation, and avoids self-deadlock on re-entry.
- **CONC-3 (demoted) — runtime confinement.** The runtime is current-thread with
  no task fan-out, so in-process data races cannot occur. Guard: a test/assertion
  documenting the runtime flavor and the absence of `spawn`/`join` fan-out, so a
  future change to `multi_thread` or an added `tokio::spawn` is flagged for
  re-evaluation. *Expected: holds — regression guard.*
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

1. **CONC-1 probe.** Spawn one `std::thread` that repeatedly reads and parses the
   config file while the main thread performs many `Config::save()` calls. Under
   the current truncate-then-write path, some reads observe an empty/partial file
   and fail to parse. *Expected: reproduces → confirms the corruption defect.*
2. **CONC-2 probe.** Explicitly interleave two stale load→modify→save sequences:
   load A; load B; A sets a pin and saves; B sets `auth_method` and saves. Assert
   A's pin survives. Under the current code B's stale in-memory write overwrites
   the file and A's pin is lost. *Expected: reproduces → confirms lost-update.*

## Method

- **Adversarial / property tests** for CONC-1 and CONC-2 — the hardened versions
  of the two pre-checks above, kept as permanent regression guards.
- **Targeted unit tests** for CONC-2b (lock not held across prompts), CONC-3
  (runtime flavor / no fan-out), CONC-4 (double-init safe), CONC-5 (second
  `set()` ignored).
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
  directory with `0600`, write the full serialized content, `fsync`, then
  `rename` over `config.toml`. `rename` within a directory is atomic on POSIX and
  on Windows (via replace semantics — to be confirmed during implementation). A
  failed write aborts before the rename, leaving the original intact; the temp
  file is cleaned up on failure.
- **Advisory lock + reload (CONC-2).** `Config::update_locked(mutator: impl
  FnOnce(&mut Config) -> Result<()>)`:
  1. open/create `~/.config/bzr/config.lock` (`0600`) and take an exclusive
     `fs4` lock;
  2. reload the current config from disk;
  3. run `mutator` on it (non-interactive);
  4. atomic-write (CONC-1);
  5. drop the lock (released on guard drop).
  Call sites that currently do load→(prompt)→mutate→save are refactored so the
  prompt stays outside and the mutation becomes the closure body.
- **Dependency.** Add `fs4` (the actively-maintained successor to `fs2`) for
  cross-platform advisory locking, pinned to its current stable version (looked
  up at implementation time). Justification: hand-rolling `flock`/`LockFileEx`
  across Unix and Windows is error-prone; `fs4` is a small, focused crate.

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
