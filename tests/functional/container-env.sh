#!/bin/bash
# Container-lookup helpers shared by the functional-test lifecycle
# scripts and tools/record-demo.sh. Source this file; do not execute
# directly. No source-time side effects (no mktemp, no trap) — callers
# that need those (lib.sh) add them separately.

BZ_VERSION="${BZ_VERSION:-${BZR_BZ_VERSION:-bz50}}"

container_runtime() {
	if command -v podman >/dev/null 2>&1; then
		printf '%s' podman
		return 0
	fi
	if command -v docker >/dev/null 2>&1; then
		printf '%s' docker
		return 0
	fi
	return 1
}

# bugzilla_checkout_id — short numeric id derived from this checkout's
# own absolute path (tests/functional's parent-of-parent), so concurrent
# checkouts (worktrees, clones) get distinct ids without coordinating.
# SCRIPT_DIR must be set by the caller before sourcing this file.
bugzilla_checkout_id() {
	local root
	root=$(cd "$SCRIPT_DIR/../.." && pwd) || return 1
	printf '%s' "$root" | cksum | cut -d' ' -f1
	return 0
}

# bugzilla_container_name — $BZR_FUNC_CONTAINER if set, else a name
# scoped to this checkout and Bugzilla version.
bugzilla_container_name() {
	if [[ -n "${BZR_FUNC_CONTAINER:-}" ]]; then
		printf '%s' "$BZR_FUNC_CONTAINER"
		return 0
	fi
	local id
	id=$(bugzilla_checkout_id) || return 1
	printf '%s' "bzr-func-test-${BZ_VERSION}-${id}"
	return 0
}

# bugzilla_container_port <runtime> <container> — host port published
# for the container's 80/tcp, or non-zero (printing nothing) if the
# runtime reports no mapping (container never started, stopped, or
# removed). Checks output, not exit status: podman exits 0 with empty
# stdout for a stopped container; docker exits 1 with a stderr message
# for the same case. Does not redirect stderr, so a runtime error
# distinct from "not running" still reaches the caller.
bugzilla_container_port() {
	local runtime="$1" container="$2"
	local mapping
	mapping=$("$runtime" port "$container" 80/tcp | head -n1)
	[[ -n "$mapping" ]] || return 1
	printf '%s' "${mapping##*:}"
	return 0
}
