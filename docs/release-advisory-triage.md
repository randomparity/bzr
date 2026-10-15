# Release advisory reachability triage

Use this procedure for each open dependency alert during the
[pre-tag security assessment](../RELEASING.md#security-assessment). It supplies
bounded evidence for the release operator's decision; it neither dismisses alerts
nor replaces the three advisory inventories or ADR0022's project assessment.

## Fix the inputs before deciding

Record the release commit, alert ID and affected version range, ecosystem,
manifest path, exact package version and source. Read the advisory's conditions,
including vulnerable features and whether exploitation happens at build time.
Read both manifests and lockfiles at that commit: lockfile membership alone says
nothing about activation. A fixed version may remain in a lockfile after an older
alert was resolved; verify the version range before calling it vulnerable.

Use a clean checkout of that commit. Record Cargo/rustc versions, build host,
Cargo configuration and feature flags. Compare the table below with that commit's
[release workflow](../.github/workflows/release.yml),
[publish workflow](../.github/workflows/publish-crates.yml),
[workspace manifest](../Cargo.toml), [xtask manifest](../xtask/Cargo.toml), and
[Cross configuration](../Cross.toml). Changes to those inputs invalidate an old
conclusion. Do not silently substitute your host target or enable all features.

## Bound the package, feature and target sets

The current binary matrix has exactly these seven targets. Defaults include
keyring; only the two bottle Linux targets additionally enable vendored-keyring.
There is no wasm32 or Intel macOS binary in this set.

| Target | Root/features | Build environment |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | bzr defaults + vendored-keyring | Ubuntu x86_64 |
| aarch64-unknown-linux-gnu | bzr defaults + vendored-keyring | cross on Ubuntu x86_64 |
| powerpc64le-unknown-linux-gnu | bzr defaults | Linux ppc64le container under QEMU |
| s390x-unknown-linux-gnu | bzr defaults | cross on Ubuntu x86_64 |
| aarch64-apple-darwin | bzr defaults | macOS arm64 |
| x86_64-pc-windows-msvc | bzr defaults | Windows x86_64 |
| aarch64-pc-windows-msvc | bzr defaults | Windows x86_64 cross-build |

There are additional execution contexts, with separate conclusions:

- **Manpages:** `xtask`, no default features, on Linux x86_64. Its `bzr` library
  dependency disables defaults. A dependency in this graph can affect the release
  runner or generated manpages without being in the shipped executable.
- **Crates.io publication:** `bzr` defaults on Linux x86_64, without the bottle's
  vendored-keyring flag. This bounds the publication verification build; consumers
  may select other targets/features or resolve other compatible versions.
- **Fuzzing:** `bzr-fuzz` from [fuzz/Cargo.toml](../fuzz/Cargo.toml), with its own
  lockfile and `bzr` dependency without defaults. It is excluded from the root
  workspace. Select the actual fuzz target/toolchain for a fuzz conclusion;
  the example below inspects Linux x86_64. Fuzz targets are not release binaries.

Select one root at a time. `--workspace` can unify features across roots and
changes the question. `--target all` includes unreleased platforms and cannot
establish that a crate reaches a released artifact.

## Run successful producers before testing membership

Run the following block in **Bash 3.2 or newer**, from the checkout root, with
Cargo and awk available. Set `crate` and `version` to the exact lockfile entry
being investigated. The example uses the current quinn-proto version, not the
historical vulnerable 0.11.14. No Rust target installation or compilation is
needed for these graph probes; registry access may still be needed.

Every Cargo graph probe carries `--locked`. A nonzero Cargo result means
**unknown**, including a stale lockfile, missing package, network or tool failure.
Do not drop `--locked` to obtain an answer. The predicate returns **0 present,
1 absent, anything else unknown**. Its positive and negative controls and each
selected root's presence are checked before absence is accepted.

```bash
(
  set -eu
  crate=quinn-proto
  version=0.11.18
  triage_tmp=$(mktemp -d)
  trap 'rm -rf "$triage_tmp"' EXIT

  member() {
    awk -v name="$1" -v version="$2" '
      $1 == name && $2 == "v" version { found = 1 }
      END { exit !found }
    ' "$3"
  }

  printf '%s v%s
' "$crate" "$version" > "$triage_tmp/control"
  member "$crate" "$version" "$triage_tmp/control"
  if member "$crate" 0.0.0-triage-absent "$triage_tmp/control"; then
    echo 'Invalid negative control' >&2
    exit 2
  else
    rc=$?
    [ "$rc" -eq 1 ] || exit "$rc"
  fi

  probe() {
    label=$1 root=$2 manifest=$3 target=$4
    shift 4
    for edges in normal normal,build normal,build,dev; do
      if cargo tree --locked --manifest-path "$manifest" -p "$root" \
          --target "$target" --edges "$edges" --prefix none \
          --format '{p}' --color never "$@" > "$triage_tmp/tree"; then
        :
      else
        rc=$?
        printf '%s %s UNKNOWN: cargo exit %s
' "$label" "$edges" "$rc" >&2
        return "$rc"
      fi
      if awk -v root="$root" '$1 == root { found = 1 } END { exit !found }' \
          "$triage_tmp/tree"; then
        :
      else
        echo "$label UNKNOWN: selected root missing or predicate failed" >&2
        return 2
      fi
      if member "$crate" "$version" "$triage_tmp/tree"; then
        printf '%s %s PRESENT (predicate exit 0)
' "$label" "$edges"
      else
        rc=$?
        case "$rc" in
          1) printf '%s %s ABSENT (predicate exit 1)
' "$label" "$edges" ;;
          *) printf '%s UNKNOWN: predicate exit %s
' "$label" "$rc" >&2
             return "$rc" ;;
        esac
      fi
    done
  }

  for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    probe "binary:$target" bzr Cargo.toml "$target" \
      --features vendored-keyring || exit "$?"
  done
  for target in powerpc64le-unknown-linux-gnu s390x-unknown-linux-gnu \
      aarch64-apple-darwin x86_64-pc-windows-msvc aarch64-pc-windows-msvc; do
    probe "binary:$target" bzr Cargo.toml "$target" || exit "$?"
  done
  probe publish bzr Cargo.toml x86_64-unknown-linux-gnu || exit "$?"
  probe manpages xtask Cargo.toml x86_64-unknown-linux-gnu \
    --no-default-features || exit "$?"
  probe fuzz bzr-fuzz fuzz/Cargo.toml x86_64-unknown-linux-gnu \
    --no-default-features || exit "$?"
)
```

Read the statuses, not stdout emptiness. `grep -q` prints nothing on both a
successful match and a non-match; an inverse Cargo tree can also exit 0 with
“nothing to print.” Neither empty output nor a quiet predicate is an absence
verdict. Never pipe a failed producer into a search and judge only the search's
status. A partial run establishes only its completed rows; retain the statuses
and failure diagnostic in the assessment. The block removes its temporary graph
files; record a positive dependency path with a separate probe before deciding.

## Interpret each profile, then make the release decision

- **normal,build present:** activated in the inspected build graph. Build scripts
  and procedural macros can execute on the build host; dev-only filtering must
  not remove those exposures. `--edges normal` alone excludes build dependencies.
- **normal absent, normal,build present:** a build-edge path exists. It needs a
  build-environment/advisory assessment even if the crate is not runtime code.
- **normal present:** investigate the path before saying runtime. Normal edges
  can include procedural macros; graph membership does not establish code in the
  binary, execution of vulnerable functions, or exploitability.
- **only normal,build,dev present:** development/test exposure in that profile.
  Verify the release jobs do not run the affected dev path before excluding it
  from release execution. A separate tool root such as xtask is not a dev edge
  just because humans call it tooling.
- **absent from the successful profiles:** not activated in those inspected
  graphs. This can explain a lockfile alert without a cargo-deny finding. It
  does not establish absence in uninspected profiles or safety of the repository.

For a present result, repeat that profile's Cargo command with `--locked`, the
same manifest/root/target/features and `--edges normal,build`, replacing flat
formatting with the default tree to trace the path. Add `--invert NAME@VERSION`
only after establishing membership. Inspect enabled features and the advisory's
preconditions; record the path and whether exposure is runtime, build-host,
release-tooling, development-only, or unresolved. Multiple classifications can
apply. Preserve package source identity if the same name/version has two sources.

The [Cargo tree documentation](https://doc.rust-lang.org/cargo/commands/cargo-tree.html)
describes these graphs as approximations of compilation. These local probes do
not reproduce the seven build environments: host-specific build dependencies,
proc macros, compiler/configuration and feature unification can matter. Before a
release all-clear that depends on those distinctions, check matching release
host/toolchain/build evidence. An unresolved discrepancy is **unknown**, not no.
Native/system libraries, container images, separately installed packaging tools,
downloaded scripts and action dependencies are outside this Cargo graph procedure.

Answer the bounded question “does this affected dependency reach this release's
runtime or build/publication path?” **Yes** requires a demonstrated path and
applicable advisory conditions. **No** requires successful evidence for the
relevant profiles and hosts plus exclusion of the advisory conditions there.
Otherwise record **unknown**, the missing evidence and its owner; do not clear
that alert for release on this procedure alone. Dependency reachability and an
operator's decision to proceed are separate recorded judgments.

## Worked example: quinn-proto

At commit `ec167b5163304b3e043f1254333f20174fd6e3e4`, both Cargo.lock and
fuzz/Cargo.lock contain registry quinn-proto **0.11.18**. This is a graph example,
not a claim that the historical alert against 0.11.14 still applies.

On macOS arm64 with Cargo 1.89.0, the block's 27 binary/publish/manpage rows
(9 profiles × 3 edge sets) each produced Cargo exit 0, root-control exit 0,
and quinn-proto predicate exit 1. The root manifest enables reqwest's json and
rustls-tls-native-roots, with defaults disabled; reqwest 0.12.28 makes quinn
optional through http3, which these profiles do not activate. The lock entry
therefore does not imply activation. These are cross-target graph observations,
not seven native builds or a claim about arbitrary consumer features.

The fuzz producer exits **101** because its lockfile needs updating. For example,
the root pins clap 4.6.7 while fuzz/Cargo.lock still pins 4.6.6. The block reports
UNKNOWN and exits 101 without printing a fuzz absence verdict. Installed nightly
Cargo 1.100.0-nightly also refused the locked fuzz probe. Both lockfiles remained
byte-identical. Fuzz activation remains unresolved; repairing that pre-existing
drift is separate dependency work. No Cargo error was reinterpreted as absence.

## GitHub Actions: CI-only is a scope, not an all-clear

For a github-actions alert, Cargo probes answer nothing. Map the affected action
and advisory version range to every pinned use at the release commit, including
reusable workflows and composite actions. A SHA pin identifies code; it does not
by itself show that code is outside the advisory's affected versions.

For each affected use, record:

1. Trigger and reachable inputs: pull requests/forks, push/tag, dispatch, and
   untrusted data reaching scripts or action inputs. Read job conditions too.
2. Effective workflow/job permissions, GITHUB_TOKEN access, environment secrets,
   OIDC/id-token grants and credentials left by checkout or earlier steps.
   `persist-credentials: false` removes one persistence path; it does not remove
   the job token, secrets, or the action's other capabilities.
3. Release influence: source, caches and artifacts consumed by later privileged
   jobs; build/sign/attest/upload paths; crates.io credentials; Homebrew tap
   credentials; and repository-writing post-release work. Trace consumers and
   trust checks rather than assuming a different job is an isolation boundary.

Current release builds have id-token/attestations write, the release job writes
contents, publish-crates receives CARGO_REGISTRY_TOKEN, and Homebrew/post-release
jobs have separate write capabilities. An action need not be in the binary to
compromise a release. Even a read-only CI job can expose supplied secrets or
produce an artifact later trusted by a publishing job.

A defensible “not shipped in the binary” conclusion is narrower than “no release
impact.” Record no release impact only after ruling out the advisory's exploit
conditions, credential exposure and influence over release inputs/outputs for
those uses. Otherwise retain CI/build exposure or unknown for the operator;
changing permissions or dismissing the alert is a separate authorized action.

## Record the decision

In the release-preparation PR, retain the commit and alert/version/source,
manifest, each inspected package/features/target/build-host/toolchain context,
producer and predicate exit statuses, dependency paths, advisory preconditions,
Actions privilege/data-flow evidence when relevant, and the resulting bounded
yes/no/unknown with reasons and unresolved owner. Redact local paths, identities
and secrets from evidence. Refresh the inventories immediately before tagging as
RELEASING requires; graph evidence is neither a repository-wide security warranty
nor a substitute for the project-vulnerability release-note assessment.
