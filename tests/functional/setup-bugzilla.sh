#!/bin/bash
# Container lifecycle management for bzr functional tests.
# Usage: setup-bugzilla.sh {build|start|stop|status|reset|logs}
# Set BZR_BZ_VERSION to select Bugzilla version (bz50, bz52). Default: bz50.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ── Container runtime detection ─────────────────────────────────────
if command -v podman &>/dev/null; then
    CONTAINER_RT=podman
elif command -v docker &>/dev/null; then
    CONTAINER_RT=docker
else
    echo "ERROR: Neither podman nor docker found in PATH" >&2
    exit 1
fi

# ── Version-aware defaults ───────────────────────────────────────────
BZ_VERSION="${BZR_BZ_VERSION:-bz50}"

case "$BZ_VERSION" in
bz50)
    DEFAULT_PORT=8089
    DEFAULT_TIMEOUT=90
    ;;
bz52)
    DEFAULT_PORT=8090
    DEFAULT_TIMEOUT=240
    ;;
bz53)
    DEFAULT_PORT=8091
    DEFAULT_TIMEOUT=240
    ;;
*)
    echo "ERROR: Unknown BZR_BZ_VERSION=$BZ_VERSION (expected bz50, bz52, or bz53)" >&2
    exit 1
    ;;
esac

CONTAINER_NAME="${BZR_FUNC_CONTAINER:-bzr-func-test-${BZ_VERSION}}"
IMAGE_NAME="${BZR_FUNC_IMAGE:-localhost/bzr-func-${BZ_VERSION}:latest}"
BZ_PORT="${BZR_FUNC_PORT:-$DEFAULT_PORT}"
HEALTH_TIMEOUT="${BZR_FUNC_TIMEOUT:-$DEFAULT_TIMEOUT}"

# ── Helpers ──────────────────────────────────────────────────────────

log() {
    echo "==> [$BZ_VERSION] $*"
    return 0
}
err() {
    echo "ERROR: [$BZ_VERSION] $*" >&2
    return 0
}

wait_for_ready() {
    local url="http://127.0.0.1:${BZ_PORT}/rest/version"
    log "Waiting for Bugzilla REST API at ${url} (timeout: ${HEALTH_TIMEOUT}s)..."

    for i in $(seq 1 "$HEALTH_TIMEOUT"); do
        if curl -sf "$url" >/dev/null 2>&1; then
            log "Bugzilla ready after ${i}s"
            local body
            body=$(curl -s "$url") || body=""
            printf '%s\n' "$body" | python3 -m json.tool 2>/dev/null || printf '%s\n' "$body"
            return 0
        fi
        sleep 1
    done

    err "Bugzilla did not become ready within ${HEALTH_TIMEOUT}s"
    echo "--- Container logs (last 30 lines) ---"
    $CONTAINER_RT logs --tail 30 "$CONTAINER_NAME" 2>&1 || true
    return 1
}

container_exists() {
    $CONTAINER_RT container inspect "$CONTAINER_NAME" >/dev/null 2>&1
    return $?
}

# ── Subcommands ──────────────────────────────────────────────────────

cmd_build() {
    local containerfile="${SCRIPT_DIR}/versions/${BZ_VERSION}/Containerfile"
    local context="${SCRIPT_DIR}/versions/${BZ_VERSION}"
    if [[ ! -f "$containerfile" ]]; then
        err "Containerfile not found: $containerfile"
        exit 1
    fi
    log "Building image ${IMAGE_NAME}..."
    $CONTAINER_RT build \
        -t "$IMAGE_NAME" \
        -f "$containerfile" \
        "$context"
    log "Image built successfully."
    return 0
}

cmd_start() {
    if container_exists; then
        log "Container ${CONTAINER_NAME} already exists. Use 'reset' to restart."
        if $CONTAINER_RT inspect --format '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null | grep -q true; then
            log "Container is already running."
            wait_for_ready
            return 0
        fi
        log "Container exists but is not running. Removing and restarting..."
        $CONTAINER_RT rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
    fi

    # Check that the image exists
    if ! $CONTAINER_RT image inspect "$IMAGE_NAME" >/dev/null 2>&1; then
        log "Image not found. Building first..."
        cmd_build
    fi

    log "Starting container ${CONTAINER_NAME} on port ${BZ_PORT}..."
    $CONTAINER_RT run -d \
        --name "$CONTAINER_NAME" \
        -p "${BZ_PORT}:80" \
        "$IMAGE_NAME"

    wait_for_ready
}

cmd_stop() {
    log "Stopping and removing container ${CONTAINER_NAME}..."
    $CONTAINER_RT rm -f "$CONTAINER_NAME" 2>/dev/null || true
    log "Container removed."
    return 0
}

cmd_status() {
    if container_exists; then
        $CONTAINER_RT inspect --format \
            'Name: {{.Name}}  State: {{.State.Status}}  Running: {{.State.Running}}  Pid: {{.State.Pid}}' \
            "$CONTAINER_NAME"
        # Check REST API
        if curl -sf "http://127.0.0.1:${BZ_PORT}/rest/version" >/dev/null 2>&1; then
            echo "REST API: reachable"
        else
            echo "REST API: not reachable"
        fi
    else
        echo "Container ${CONTAINER_NAME} does not exist."
        return 1
    fi
}

cmd_reset() {
    cmd_stop
    cmd_start
    return 0
}

cmd_logs() {
    if [[ $# -eq 0 ]]; then
        $CONTAINER_RT logs --tail 50 "$CONTAINER_NAME" 2>&1
    else
        $CONTAINER_RT logs "$@" "$CONTAINER_NAME" 2>&1
    fi
    return 0
}

# ── Main ─────────────────────────────────────────────────────────────

case "${1:-}" in
build) cmd_build ;;
start) cmd_start ;;
stop) cmd_stop ;;
status) cmd_status ;;
reset) cmd_reset ;;
logs)
    shift
    cmd_logs "$@"
    ;;
*)
    echo "Usage: $0 {build|start|stop|status|reset|logs}"
    echo "  Set BZR_BZ_VERSION=bz50|bz52|bz53 (default: bz50)"
    exit 1
    ;;
esac
