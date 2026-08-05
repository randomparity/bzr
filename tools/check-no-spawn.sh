#!/usr/bin/env bash
set -euo pipefail

repo_root=${1:-.}
main="$repo_root/src/main.rs"

if [[ ! -f $main ]] || ! rg -q 'flavor = "current_thread"' "$main"; then
  echo "ERROR: src/main.rs no longer declares the current_thread runtime." >&2
  echo "The concurrency engagement assumes no in-process parallelism." >&2
  echo "Re-evaluate the CONC-* invariants before changing the runtime flavor." >&2
  exit 1
fi

fan_out='tokio(::task)?::spawn(_local|_blocking)?[[:space:]]*\(|tokio::task::(LocalSet|JoinSet)|((tokio::)?(try_)?join|tokio::select)!|FuturesUnordered|\.buffered[[:space:]]*\(|\.buffer_unordered[[:space:]]*\(|\.for_each_concurrent[[:space:]]*\(|(std::)?thread::spawn[[:space:]]*\('

if rg -n --glob '*.rs' --glob '!*_tests.rs' --glob '!test_helpers.rs' "$fan_out" "$repo_root/src"; then
  echo "ERROR: task or thread fan-out found in production Rust." >&2
  echo "Re-evaluate the CONC-* invariants (config writes, redaction context, shared state)." >&2
  exit 1
fi
