# Functional runtime environment — issue #934

## Problem
The runner isolates bzr with XDG_CONFIG_HOME, hiding macOS Podman connection
configuration from SQL and CGI fixture calls. A live connection probe succeeds
with the original environment and exits 125 with isolated XDG configuration.

## Scope
C1–C3 refer to the frozen WORK:SCOPE on #934 (q934-825fccf7).
Extend the existing cleanup restoration pattern: run-tests.sh snapshots both
HOME and XDG_CONFIG_HOME values and set/unset state before isolation.
lib.sh owns one subshell helper restoring that snapshot for a supplied command.
Migrate run_bugzilla_sql_file, CGI copy/exec in phases/09c-bug-links.sh, and
runner cleanup to that helper. Remove cleanup's duplicate inline restoration.
Commands retain their arguments, stdin, output and status; parent bzr isolation
survives successful and failing runtime calls. Other lib.sh consumers without
a runner snapshot retain their current environment. No ownership transition
beyond consolidating existing restoration policy; no compatibility facade.
Exclusions approved by the operator: shipped bzr config/auth behavior is outside
#934; provisioning redesign and unrelated harness cleanup belong to separate work.
Operator approved this design in the active quest conversation.

### Failure model
- Actors and deployments: local/CI functional-test operators, Bash 3.2+,
  macOS Podman and existing Docker/Podman harness deployments.
- Invariants/assets: bzr config isolation; original runtime HOME/XDG set/unset
  state; fixture commands and cleanup retain status, streams and arguments (C1–C2).
- Accepted failure classes: unavailable runtime/service remains an ordinary
  command failure; this change does not provision or repair the operator's runtime.
- Covered elsewhere: existing phases own Bugzilla assertion correctness;
  existing setup-bugzilla.sh owns container lifecycle/provisioning.
- Threat boundary: trusted harness passes quoted command arguments and saved
  environment values to the existing runtime; no new untrusted entry point.
  Use argument forwarding without eval and a subshell to contain environment changes.
  Hostile local modifications to the harness are outside this trusted-operator model.

## Success
C1: SQL and CGI fixture runtime calls and cleanup use original HOME/XDG state,
while bzr retains isolated configuration; existing assertions are unchanged.
C2: deterministic checks cover originally set, empty and unset values, changed
parent values, arguments/stdin, nonzero exits and parent environment preservation.
C3: the default real-server functional suite completes successfully on Podman.

## Validation
- focused-test: tools/functional-runtime-env-tests.sh; use a recording runtime
  double through SQL, CGI phase and runner cleanup boundaries. Red: isolated
  HOME/XDG reaches the double or unset becomes set; green command:
  bash tools/functional-runtime-env-tests.sh. Include unsnapshotted lib consumer.
- focused-test: same script checks parent isolation, argument/stdin forwarding
  and failure status; red: any observation differs; same green command.
- Wire regression script into make check-shell; run that gate and make lint.
- task-test-not-applicable: this human-readable design has no executable consumer;
  independently review its criteria/scope instead of testing prose.
- Integration: make test and make functional-test; the latter builds HEAD's
  binary in this checkout and proves fixture setup plus cleanup against Podman.
