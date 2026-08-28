#!/usr/bin/env bash
set -euo pipefail

# Records the README terminal demo (docs/assets/bzr-demo.gif) against the
# functional-test Bugzilla container, so the demo can be regenerated when
# CLI output changes.
#
# Prereqs:
#   - a FRESH functional container: make functional-stop && make functional-start
#     (the script aborts if the demo product already exists, so bug IDs and
#     comment counters stay clean)
#   - asciinema (>= 3), agg, jq, curl
#   - a release bzr binary: cargo build --release (override with BZR_BIN)
#
# Usage: tools/record-demo.sh
#        tools/record-demo.sh --weekly-status

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BZ_URL=${BZ_URL:-http://127.0.0.1:8089}
# Fixed key baked into the test container image (tests/functional/versions/*).
API_KEY="FuncTest0123456789abcdef0123456789abcdef"
ADMIN_EMAIL="admin@test.bzr"
BZR_BIN=${BZR_BIN:-$REPO_ROOT/target/release/bzr}
OUT_GIF="$REPO_ROOT/docs/assets/bzr-demo.gif"
if [[ "${1:-}" == "--weekly-status" ]]; then
  OUT_GIF="$REPO_ROOT/docs/assets/bzr-weekly-status-demo.gif"
fi

# ── Driver mode: runs inside the asciinema PTY ───────────────────────
# Types each command with a human-ish cadence, then runs it for real.
if [[ "${1:-}" == "--drive" ]]; then
  bug_id=$2
  prompt=$'\e[1;35m❯\e[0m '

  type_run() {
    local cmd=$1
    local pause=${2:-1.8}
    local i
    printf '%b' "$prompt"
    for ((i = 0; i < ${#cmd}; i++)); do
      printf '%s' "${cmd:i:1}"
      sleep 0.018
    done
    sleep 0.4
    printf '\n'
    eval "$cmd"
    sleep "$pause"
  }

  sleep 0.5
  type_run 'bzr bug list --product Nimbus' 2.6
  type_run "bzr bug view $bug_id" 2.4
  type_run "bzr comment add $bug_id --body \"Reproduced on 1.4.2 - parser runs before defaults load.\"" 1.6
  type_run "bzr bug update $bug_id --status IN_PROGRESS --assignee alice@nimbus.example" 1.6
  type_run "bzr --output json bug view $bug_id | jq '.data | {id, status, assigned_to}'" 2.6
  printf '%b' "$prompt"
  sleep 1.6
  printf '\n'
  exit 0
fi

if [[ "${1:-}" == "--drive-weekly-status" ]]; then
  bug_id=$2
  workdir=$3
  prompt=$'\e[1;35m❯\e[0m '
  type_run() {
    local cmd=$1 pause=${2:-1.8} i
    printf '%b' "$prompt"
    for ((i = 0; i < ${#cmd}; i++)); do printf '%s' "${cmd:i:1}"; sleep 0.012; done
    sleep 0.3; printf '\n'; eval "$cmd"; sleep "$pause"
  }
  fields='id,summary,status,resolution,assigned_to,priority,severity,target_milestone,deadline,last_change_time,whiteboard,blocks,depends_on'
  type_run 'bzr skills install --agent codex --project .' 1.2
  type_run "bzr query save core-weekly --product Nimbus --fields $fields" 1.2
  type_run "bzr query run core-weekly --fields $fields --paginate --json > '$workdir/baseline.json'" 1.0
  type_run 'echo "No compatible prior snapshot exists; this report establishes the baseline."' 2.0
  type_run "bzr bug update $bug_id --status IN_PROGRESS --whiteboard 'blocked: parser owner needed'" 1.2
  type_run "bzr query run core-weekly --fields $fields --paginate --json > '$workdir/current.json'" 1.0
  type_run "jq -n --arg id '$bug_id' '{facts:[\"Bug #\(\$id) changed status and whiteboard\"],interpretation:[\"Owner decision needed\"]}'" 2.6
  printf '%b\n' "$prompt"
  exit 0
fi

# ── Orchestrator mode ────────────────────────────────────────────────
for tool in asciinema agg jq curl; do
  command -v "$tool" >/dev/null || {
    echo "ERROR: $tool not found on PATH" >&2
    exit 1
  }
done
[[ -x "$BZR_BIN" ]] || {
  echo "ERROR: $BZR_BIN not found — run: cargo build --release" >&2
  exit 1
}
curl -fsS "$BZ_URL/rest/version" >/dev/null || {
  echo "ERROR: no Bugzilla at $BZ_URL — run: make functional-start" >&2
  exit 1
}

workdir=$(mktemp -d)
trap 'rm -r "$workdir"' EXIT
export BZR_CONFIG="$workdir/config.toml"
export BZR_API_KEY="$API_KEY"
export RUST_LOG=error
PATH="$(dirname "$BZR_BIN"):$PATH"
export PATH

echo "==> Configuring throwaway server profile"
bzr config set-server demo --url "$BZ_URL" --api-key-env BZR_API_KEY \
  --auth-method query_param --email "$ADMIN_EMAIL" >/dev/null

echo "==> Seeding demo data"
if ! bzr product create --name Nimbus \
  --description "Cross-platform file sync client" >/dev/null 2>&1; then
  echo "ERROR: product Nimbus already exists — reset the container first:" >&2
  echo "  make functional-stop && make functional-start" >&2
  exit 1
fi
bzr component create --product Nimbus --name sync-engine \
  --description "Background sync daemon" --default-assignee "$ADMIN_EMAIL" >/dev/null
bzr component create --product Nimbus --name ui \
  --description "Desktop UI" --default-assignee "$ADMIN_EMAIL" >/dev/null
bzr user create --email alice@nimbus.example --full-name "Alice Nguyen" \
  --password "Demo!$(od -An -N4 -tx4 /dev/urandom | tr -d ' ')" >/dev/null

mkbug() {
  bzr --output json bug create --product Nimbus "$@" | jq -r '.data.id'
}
b1=$(mkbug --component sync-engine --summary "Crash on startup when config file is empty" \
  --description "A zero-byte config.toml makes the client panic before defaults are applied." \
  --severity critical --priority Highest --op-sys Linux --rep-platform PC)
b2=$(mkbug --component sync-engine --summary "Memory leak in background sync worker" \
  --description "RSS grows ~40 MB/hour while idle with a watched folder of 10k files." \
  --severity major --priority High --op-sys "Mac OS" --rep-platform PC)
mkbug --component sync-engine --summary "Sync stalls on files larger than 2 GB" \
  --description "Chunked upload never resumes after the first 2 GB part." \
  --severity normal --priority High --op-sys Linux --rep-platform PC >/dev/null
b4=$(mkbug --component ui --summary "Dark mode toggle resets after restart" \
  --description "Theme preference is not persisted across sessions." \
  --severity minor --priority Low --op-sys Windows --rep-platform PC)
mkbug --component ui --summary "Conflict dialog shows timestamps in UTC" \
  --description "Timestamps should use the local timezone." \
  --severity normal --priority Normal --op-sys "Mac OS" --rep-platform PC >/dev/null
bzr --output json bug update "$b2" --status IN_PROGRESS \
  --assignee alice@nimbus.example >/dev/null
bzr --output json bug update "$b4" --status RESOLVED --resolution FIXED >/dev/null

echo "==> Recording session (targets bug $b1)"
driver=(--drive "$b1")
if [[ "${1:-}" == "--weekly-status" ]]; then
  driver=(--drive-weekly-status "$b1" "$workdir")
fi
asciinema rec --headless --window-size 100x30 \
  -c "bash ${BASH_SOURCE[0]} ${driver[*]}" "$workdir/demo.cast"

echo "==> Rendering GIF"
agg --theme dracula --font-size 16 --idle-time-limit 3 \
  "$workdir/demo.cast" "$OUT_GIF"
ls -la "$OUT_GIF"
