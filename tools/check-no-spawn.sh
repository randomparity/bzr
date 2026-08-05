#!/usr/bin/env bash
set -euo pipefail

repo_root=${1:-.}
main="$repo_root/src/main.rs"
clippy_config="$repo_root/clippy.toml"

if [[ ! -f $main ]] ||
  ! rg -q '^#\[tokio::main\(flavor = "current_thread"\)\]$' "$main"; then
  echo "ERROR: src/main.rs no longer declares the current_thread runtime." >&2
  echo "The concurrency engagement assumes no in-process parallelism." >&2
  echo "Re-evaluate the CONC-* invariants before changing the runtime flavor." >&2
  exit 1
fi

semantic_guards=(
  'std::thread::spawn'
  'std::thread::Builder::spawn'
  'std::thread::Builder::spawn_scoped'
  'std::thread::Builder::spawn_unchecked'
  'std::thread::scope'
  'std::thread::Scope::spawn'
  'tokio::runtime::Handle::spawn'
  'tokio::runtime::Handle::spawn_blocking'
  'tokio::runtime::LocalRuntime::spawn_blocking'
  'tokio::runtime::LocalRuntime::spawn_local'
  'tokio::runtime::Runtime::spawn'
  'tokio::runtime::Runtime::spawn_blocking'
  'tokio::task::spawn'
  'tokio::task::spawn_blocking'
  'tokio::task::spawn_local'
  'tokio::task::JoinSet'
  'tokio::task::LocalSet'
  'tokio::join'
  'tokio::select'
  'tokio::try_join'
)

for guard in "${semantic_guards[@]}"; do
  if [[ ! -f $clippy_config ]] || ! rg -Fq "\"$guard\"" "$clippy_config"; then
    echo "ERROR: clippy.toml does not disallow $guard." >&2
    echo "Re-evaluate ADR 0016 before weakening semantic concurrency guards." >&2
    exit 1
  fi
done

fan_out='tokio(::task)?::spawn(_local|_blocking)?[[:space:]]*\('
fan_out+='|tokio::task::(LocalSet|JoinSet)|((tokio::)?(try_)?join|tokio::select)!'
fan_out+='|FuturesUnordered|\.buffered[[:space:]]*\(|\.buffer_unordered[[:space:]]*\('
fan_out+='|\.for_each_concurrent[[:space:]]*\(|(std::)?thread::spawn[[:space:]]*\('

if rg -n --glob '*.rs' --glob '!*_tests.rs' --glob '!test_helpers.rs' \
  "$fan_out" "$repo_root/src"; then
  echo "ERROR: task or thread fan-out found in production Rust." >&2
  echo "Re-evaluate the CONC-* invariants (config writes, redaction context, shared state)." >&2
  exit 1
fi
