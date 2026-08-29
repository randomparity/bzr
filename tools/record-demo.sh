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
#        tools/record-demo.sh dependency-analysis
#        tools/record-demo.sh release-readiness

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

# ── Weekly-status driver mode: runs inside the asciinema PTY ────────
if [[ "${1:-}" == "--drive-weekly-status" ]]; then
  bug_id=$2
  workdir=$3
  prompt=$'\e[1;35m❯\e[0m '
  type_run() {
    local cmd=$1 pause=${2:-1.8} i
    printf '%b' "$prompt"
    for ((i = 0; i < ${#cmd}; i++)); do
      printf '%s' "${cmd:i:1}"
      sleep 0.012
    done
    sleep 0.3
    printf '\n'
    eval "$cmd"
    sleep "$pause"
  }
  fields='id,summary,status,resolution,assigned_to,priority,severity,target_milestone,deadline,last_change_time,whiteboard,blocks,depends_on'
  type_run 'bzr skills install --agent codex --project .' 1.2
  type_run "bzr query save core-weekly --product Nimbus --fields $fields" 1.2
  type_run "tools/run-weekly-status-demo.sh '$bug_id' '$workdir' '.agents/skills/bzr-weekly-status'" 3.2
  printf '%b\n' "$prompt"
  exit 0
fi

# ── Dependency-analysis driver mode: runs inside the asciinema PTY ──
if [[ "${1:-}" == "--drive-dependency-analysis" ]]; then
  : "${DEPENDENCY_DEMO_PROJECT:?}"
  : "${DEPENDENCY_DEMO_COLLECTOR:?}"
  : "${DEPENDENCY_DEMO_ANALYZER:?}"
  : "${DEPENDENCY_DEMO_RENDERER:?}"
  : "${DEPENDENCY_DEMO_POLICY:?}"
  : "${DEPENDENCY_DEMO_COLLECTION:?}"
  : "${DEPENDENCY_DEMO_ANALYSIS:?}"
  : "${DEPENDENCY_DEMO_REPORT:?}"
  : "${DEPENDENCY_DEMO_DIAGRAM:?}"
  : "${DEPENDENCY_DEMO_ROOT:?}"

  cd "$DEPENDENCY_DEMO_PROJECT"
  prompt=$'\e[1;35m❯\e[0m '

  type_run() {
    local display=$1
    local pause=$2
    local i
    shift 2
    printf '%b' "$prompt"
    for ((i = 0; i < ${#display}; i++)); do
      printf '%s' "${display:i:1}"
      sleep 0.012
    done
    sleep 0.3
    printf '\n'
    "$@"
    sleep "$pause"
  }

  collector_display=${DEPENDENCY_DEMO_COLLECTOR#"$DEPENDENCY_DEMO_PROJECT"/}
  analyzer_display=${DEPENDENCY_DEMO_ANALYZER#"$DEPENDENCY_DEMO_PROJECT"/}
  renderer_display=${DEPENDENCY_DEMO_RENDERER#"$DEPENDENCY_DEMO_PROJECT"/}
  policy_display=${DEPENDENCY_DEMO_POLICY#"$DEPENDENCY_DEMO_PROJECT"/}
  collection_display=${DEPENDENCY_DEMO_COLLECTION#"$DEPENDENCY_DEMO_PROJECT"/}
  analysis_display=${DEPENDENCY_DEMO_ANALYSIS#"$DEPENDENCY_DEMO_PROJECT"/}
  report_display=${DEPENDENCY_DEMO_REPORT#"$DEPENDENCY_DEMO_PROJECT"/}
  diagram_display=${DEPENDENCY_DEMO_DIAGRAM#"$DEPENDENCY_DEMO_PROJECT"/}
  renderer_command="python3 $renderer_display --input $analysis_display"

  sleep 0.5
  printf 'Read-only dependency analysis for Bugzilla #%s\n\n' "$DEPENDENCY_DEMO_ROOT"
  type_run \
    "python3 $collector_display --policy $policy_display --output $collection_display" 1.4 \
    python3 "$DEPENDENCY_DEMO_COLLECTOR" --policy "$DEPENDENCY_DEMO_POLICY" \
    --output "$DEPENDENCY_DEMO_COLLECTION"
  type_run \
    "python3 $analyzer_display --input $collection_display --output $analysis_display" 1.2 \
    python3 "$DEPENDENCY_DEMO_ANALYZER" --input "$DEPENDENCY_DEMO_COLLECTION" \
    --output "$DEPENDENCY_DEMO_ANALYSIS"
  type_run \
    "$renderer_command --format markdown --output $report_display" 1.2 \
    python3 "$DEPENDENCY_DEMO_RENDERER" --input "$DEPENDENCY_DEMO_ANALYSIS" \
    --format markdown --output "$DEPENDENCY_DEMO_REPORT"
  type_run \
    "$renderer_command --format mermaid --output $diagram_display" 1.2 \
    python3 "$DEPENDENCY_DEMO_RENDERER" --input "$DEPENDENCY_DEMO_ANALYSIS" \
    --format mermaid --output "$DEPENDENCY_DEMO_DIAGRAM"
  type_run "sed -n '1,44p' $report_display" 2.8 \
    sed -n '1,44p' "$DEPENDENCY_DEMO_REPORT"
  printf '%b' "$prompt"
  sleep 1.6
  printf '\n'
  exit 0
fi

# ── Release-readiness driver mode: runs inside the asciinema PTY ────────
if [[ "${1:-}" == "--drive-release-readiness" ]]; then
  : "${RELEASE_READINESS_DEMO_HELPER:?}"
  : "${RELEASE_READINESS_DEMO_MARKER:?}"
  : "${RELEASE_READINESS_DEMO_REPORT:?}"
  : "${RELEASE_READINESS_DEMO_SERVER:?}"

  prompt=$'\e[1;35m❯\e[0m '
  request='Analyze the latest release candidate in Bugzilla. Keep the review read-only. Treat RESOLVED and CLOSED as complete, Highest priority or release-blocker as blocking, and 30 days as stale. Use UTC, include dependency risks, distinguish facts from assumptions and assessments, and return a PM-ready Markdown report.'

  printf '%b' "$prompt"
  for ((i = 0; i < ${#request}; i++)); do
    printf '%s' "${request:i:1}"
    sleep 0.006
  done
  sleep 0.5
  printf '\n\n'
  BZR_BIN="$BZR_BIN" "$RELEASE_READINESS_DEMO_HELPER" \
    "$RELEASE_READINESS_DEMO_SERVER" "$RELEASE_READINESS_DEMO_MARKER" \
    "$RELEASE_READINESS_DEMO_REPORT" >/dev/null 2>&1
  sed -n '1,$p' "$RELEASE_READINESS_DEMO_REPORT"
  sleep 3
  printf '\n%b\n' "$prompt"
  exit 0
fi

# Release-readiness orchestrator mode.
if [[ "${1:-}" == "release-readiness" ]]; then
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
  release_helper="$REPO_ROOT/tools/run-release-readiness-demo.sh"
  [[ -x "$release_helper" ]] || {
    echo "ERROR: release-readiness demo helper is not executable: $release_helper" >&2
    exit 1
  }
  curl -fsS "$BZ_URL/rest/version" >/dev/null || {
    echo "ERROR: no Bugzilla at $BZ_URL — run: make functional-start" >&2
    exit 1
  }

  release_workdir=$(mktemp -d)
  release_workdir=$(cd "$release_workdir" && pwd -P)
  trap 'rm -r "$release_workdir"' EXIT
  export BZR_CONFIG="$release_workdir/config.toml"
  export RUST_LOG=error

  echo "==> Configuring a throwaway read-only profile"
  "$BZR_BIN" config set-server demo --url "$BZ_URL" >/dev/null
  release_marker="bzr-release-readiness-demo-v1"
  release_matches=$("$BZR_BIN" --server demo --json bug list \
    --whiteboard "$release_marker" --fields id,product,whiteboard \
    --limit 100 --paginate --sort bug_id --order asc)
  release_root=$(jq -er --arg marker "$release_marker" '
    [.data[] | select(.whiteboard == ($marker + " release-blocker")) | .id] |
    max
  ' <<<"$release_matches") || {
    echo "ERROR: release-readiness demo fixture not found." >&2
    echo "  Run: make functional-test" >&2
    echo "  Then rerun: tools/record-demo.sh release-readiness" >&2
    exit 1
  }
  release_product=$(jq -er --argjson root "$release_root" '
    .data[] | select(.id == $root) | .product
  ' <<<"$release_matches")

  echo "==> Preparing hidden read-only demo inputs"
  "$BZR_BIN" query save release-readiness-demo --product "$release_product" \
    --limit 1 --sort last_change_time --order desc >/dev/null
  release_url="${BZ_URL}/buglist.cgi?product=${release_product}&query_format=advanced&limit=1&order=changeddate%20DESC"
  "$BZR_BIN" query save release-readiness-demo-url --from-url "$release_url" \
    --limit 1 >/dev/null 2>&1

  export RELEASE_READINESS_DEMO_HELPER="$release_helper"
  export RELEASE_READINESS_DEMO_MARKER="$release_marker"
  export RELEASE_READINESS_DEMO_REPORT="$release_workdir/release-readiness.md"
  export RELEASE_READINESS_DEMO_SERVER=demo
  release_pending_cast="$release_workdir/bzr-release-readiness-demo.cast"
  release_cast="$REPO_ROOT/docs/assets/bzr-release-readiness-demo.cast"
  release_gif="$REPO_ROOT/docs/assets/bzr-release-readiness-demo.gif"

  echo "==> Recording release-readiness analysis (root bug $release_root)"
  (
    cd "$REPO_ROOT"
    asciinema rec --headless --return --overwrite --window-size 110x38 \
      -c "bash tools/record-demo.sh --drive-release-readiness" "$release_pending_cast"
  )

  echo "==> Inspecting cast for credentials and private host data"
  if grep -Fq "$BZ_URL" "$release_pending_cast" ||
    grep -Fq "$release_workdir" "$release_pending_cast" ||
    grep -Fq "$REPO_ROOT" "$release_pending_cast" ||
    grep -Fq "$API_KEY" "$release_pending_cast" ||
    grep -Fq "BZR_API_KEY" "$release_pending_cast"; then
    echo "ERROR: release-readiness cast contains private recording data" >&2
    exit 1
  fi
  mv "$release_pending_cast" "$release_cast"

  echo "==> Rendering release-readiness GIF"
  agg --theme dracula --font-size 16 --idle-time-limit 3 \
    "$release_cast" "$release_gif"
  ls -la "$release_cast" "$release_gif"
  exit 0
fi

# Dependency-analysis orchestrator mode.
if [[ "${1:-}" == "dependency-analysis" ]]; then
  for tool in asciinema agg jq curl python3; do
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

  dependency_workdir=$(mktemp -d)
  dependency_workdir=$(cd "$dependency_workdir" && pwd -P)
  trap 'rm -r "$dependency_workdir"' EXIT
  dependency_project="$dependency_workdir/project"
  mkdir "$dependency_project"
  export BZR_CONFIG="$dependency_workdir/config.toml"
  export RUST_LOG=error
  PATH="$(dirname "$BZR_BIN"):$PATH"
  export PATH

  echo "==> Installing the dependency-analysis skill into a throwaway project"
  bzr skills install --agent codex --project "$dependency_project" >/dev/null
  dependency_skill_root="$dependency_project/.agents/skills/bzr-dependency-analysis"
  dependency_skill_root=$(cd "$dependency_skill_root" && pwd -P)

  resolve_installed_helper() {
    local relative=$1
    local helper
    helper=$(cd "$(dirname "$dependency_skill_root/$relative")" && pwd -P)/$(basename "$relative")
    case "$helper" in
    "$dependency_skill_root"/*) ;;
    *)
      echo "ERROR: installed helper escaped $dependency_skill_root: $helper" >&2
      exit 1
      ;;
    esac
    [[ -f "$helper" && ! -L "$helper" ]] || {
      echo "ERROR: installed helper not found: $helper" >&2
      exit 1
    }
    printf '%s\n' "$helper"
  }

  dependency_collector=$(resolve_installed_helper scripts/collect.py)
  dependency_analyzer=$(resolve_installed_helper scripts/analyze.py)
  dependency_renderer=$(resolve_installed_helper scripts/render.py)

  echo "==> Discovering the pre-provisioned dependency graph (read-only)"
  bzr config set-server demo --url "$BZ_URL" >/dev/null
  dependency_marker="bzr-dependency-analysis-demo-v1"
  dependency_matches=$(bzr --server demo --output json bug list \
    --whiteboard "$dependency_marker" --fields id,whiteboard \
    --limit 100 --paginate --sort bug_id --order asc)
  dependency_root=$(jq -r --arg marker "$dependency_marker" \
    '[.data[] | select(.whiteboard == $marker) | .id] | max // empty' \
    <<<"$dependency_matches")
  if [[ -z "$dependency_root" ]]; then
    echo "ERROR: dependency-analysis demo fixture not found." >&2
    echo "  Run: make functional-test" >&2
    echo "  Then rerun: tools/record-demo.sh dependency-analysis" >&2
    exit 1
  fi
  dependency_root_detail=$(bzr --server demo --output json bug view \
    "$dependency_root" --fields assigned_to)
  dependency_default_assignee=$(jq -r '.data.assigned_to // empty' \
    <<<"$dependency_root_detail")
  if [[ -z "$dependency_default_assignee" ]]; then
    echo "ERROR: dependency-analysis demo root has no default assignee login" >&2
    exit 1
  fi

  dependency_policy="$dependency_project/dependency-policy.json"
  dependency_collection="$dependency_project/dependency-collection.json"
  dependency_analysis="$dependency_project/dependency-analysis.json"
  dependency_report="$dependency_project/dependency-report.md"
  dependency_diagram="$dependency_project/dependency-graph.mmd"
  jq -n --arg bzr "$BZR_BIN" --argjson root "$dependency_root" \
    --arg default_assignee "$dependency_default_assignee" '{
    bounds: {max_depth: 5, max_nodes: 20, max_relationships: 40},
    bzr: $bzr,
    direction: "both",
    resolved_mode: "include-no-traverse",
    resolved_statuses: ["RESOLVED"],
    restriction: null,
    scopes: [{ids: [$root], kind: "bug-ids", server: "demo"}],
    servers: ["demo"],
    stale_after_days: 14,
    unassigned_assignees: {demo: [$default_assignee]}
  }' >"$dependency_policy"

  export DEPENDENCY_DEMO_PROJECT="$dependency_project"
  export DEPENDENCY_DEMO_COLLECTOR="$dependency_collector"
  export DEPENDENCY_DEMO_ANALYZER="$dependency_analyzer"
  export DEPENDENCY_DEMO_RENDERER="$dependency_renderer"
  export DEPENDENCY_DEMO_POLICY="$dependency_policy"
  export DEPENDENCY_DEMO_COLLECTION="$dependency_collection"
  export DEPENDENCY_DEMO_ANALYSIS="$dependency_analysis"
  export DEPENDENCY_DEMO_REPORT="$dependency_report"
  export DEPENDENCY_DEMO_DIAGRAM="$dependency_diagram"
  export DEPENDENCY_DEMO_ROOT="$dependency_root"

  dependency_cast="$REPO_ROOT/docs/assets/bzr-dependency-analysis-demo.cast"
  dependency_gif="$REPO_ROOT/docs/assets/bzr-dependency-analysis-demo.gif"
  echo "==> Recording dependency analysis (root bug $dependency_root)"
  (
    cd "$REPO_ROOT"
    asciinema rec --headless --return --overwrite --window-size 100x32 \
      -c "bash tools/record-demo.sh --drive-dependency-analysis" "$dependency_cast"
  )

  echo "==> Inspecting cast for credentials and private host data"
  if grep -Fq "$BZ_URL" "$dependency_cast" ||
    grep -Fq "$dependency_workdir" "$dependency_cast" ||
    grep -Fq "$REPO_ROOT" "$dependency_cast" ||
    grep -Fq "$API_KEY" "$dependency_cast" ||
    grep -Fq "BZR_API_KEY" "$dependency_cast"; then
    echo "ERROR: dependency-analysis cast contains private recording data" >&2
    exit 1
  fi

  echo "==> Rendering dependency-analysis GIF"
  agg --theme dracula --font-size 16 --idle-time-limit 3 \
    "$dependency_cast" "$dependency_gif"
  ls -la "$dependency_cast" "$dependency_gif"
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
