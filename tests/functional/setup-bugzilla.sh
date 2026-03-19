#!/bin/bash
# Container lifecycle management for bzr functional tests.
# Usage: setup-bugzilla.sh {build|start|stop|status|reset|logs}
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTAINER_NAME="${BZR_FUNC_CONTAINER:-bzr-func-test}"
IMAGE_NAME="${BZR_FUNC_IMAGE:-localhost/bzr-func-test-bz:latest}"
BZ_PORT="${BZR_FUNC_PORT:-8089}"
HEALTH_TIMEOUT="${BZR_FUNC_TIMEOUT:-90}"

# ── Helpers ──────────────────────────────────────────────────────────

log() { echo "==> $*"; }
err() { echo "ERROR: $*" >&2; }

wait_for_ready() {
    local url="http://localhost:${BZ_PORT}/rest/version"
    log "Waiting for Bugzilla REST API at ${url} (timeout: ${HEALTH_TIMEOUT}s)..."

    for i in $(seq 1 "$HEALTH_TIMEOUT"); do
        if curl -sf "$url" >/dev/null 2>&1; then
            log "Bugzilla ready after ${i}s"
            curl -s "$url" | python3 -m json.tool 2>/dev/null || curl -s "$url"
            return 0
        fi
        sleep 1
    done

    err "Bugzilla did not become ready within ${HEALTH_TIMEOUT}s"
    echo "--- Container logs (last 30 lines) ---"
    podman logs --tail 30 "$CONTAINER_NAME" 2>&1 || true
    return 1
}

container_exists() {
    podman container exists "$CONTAINER_NAME" 2>/dev/null
}

# ── Subcommands ──────────────────────────────────────────────────────

cmd_build() {
    log "Building image ${IMAGE_NAME}..."
    podman build \
        -t "$IMAGE_NAME" \
        -f "${SCRIPT_DIR}/Containerfile" \
        "$SCRIPT_DIR"
    log "Image built successfully."
}

cmd_start() {
    if container_exists; then
        log "Container ${CONTAINER_NAME} already exists. Use 'reset' to restart."
        if podman inspect --format '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null | grep -q true; then
            log "Container is already running."
            wait_for_ready
            return 0
        fi
        log "Container exists but is not running. Removing and restarting..."
        podman rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
    fi

    # Check that the image exists
    if ! podman image exists "$IMAGE_NAME" 2>/dev/null; then
        log "Image not found. Building first..."
        cmd_build
    fi

    log "Starting container ${CONTAINER_NAME} on port ${BZ_PORT}..."
    podman run -d \
        --name "$CONTAINER_NAME" \
        -p "${BZ_PORT}:80" \
        "$IMAGE_NAME"

    wait_for_ready
}

cmd_stop() {
    log "Stopping and removing container ${CONTAINER_NAME}..."
    podman rm -f "$CONTAINER_NAME" 2>/dev/null || true
    log "Container removed."
}

cmd_status() {
    if container_exists; then
        podman inspect --format \
            'Name: {{.Name}}  State: {{.State.Status}}  Running: {{.State.Running}}  Pid: {{.State.Pid}}' \
            "$CONTAINER_NAME"
        # Check REST API
        if curl -sf "http://localhost:${BZ_PORT}/rest/version" >/dev/null 2>&1; then
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
}

cmd_logs() {
    if [[ $# -eq 0 ]]; then
        podman logs --tail 50 "$CONTAINER_NAME" 2>&1
    else
        podman logs "$@" "$CONTAINER_NAME" 2>&1
    fi
}

# ── Main ─────────────────────────────────────────────────────────────

case "${1:-}" in
    build)  cmd_build ;;
    start)  cmd_start ;;
    stop)   cmd_stop ;;
    status) cmd_status ;;
    reset)  cmd_reset ;;
    logs)   shift; cmd_logs "$@" ;;
    *)
        echo "Usage: $0 {build|start|stop|status|reset|logs}"
        exit 1
        ;;
esac
