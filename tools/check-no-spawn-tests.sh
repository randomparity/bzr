#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
CHECKER="$SCRIPT_DIR/check-no-spawn.sh"
FIXTURES=$(mktemp -d)
trap 'rm -r "$FIXTURES"' EXIT

new_fixture() {
  local name=$1
  local root="$FIXTURES/$name"
  mkdir -p "$root/src" "$root/docs"
  printf '#[tokio::main(flavor = "current_thread")]\nasync fn main() {}\n' >"$root/src/main.rs"
  printf 'pub fn safe() {}\n' >"$root/src/lib.rs"
  printf '%s\n' "$root"
}

expect_rejected() {
  local name=$1
  local root=$2
  if bash "$CHECKER" "$root" >/dev/null 2>&1; then
    echo "expected check-no-spawn to reject $name" >&2
    return 1
  fi
}

expect_allowed() {
  local name=$1
  local root=$2
  if ! bash "$CHECKER" "$root" >/dev/null 2>&1; then
    echo "expected check-no-spawn to allow $name" >&2
    return 1
  fi
}

missing_main=$(new_fixture missing-main)
rm "$missing_main/src/main.rs"
expect_rejected "missing current-thread runtime" "$missing_main"

wrong_main=$(new_fixture wrong-main)
printf '#[tokio::main(flavor = "multi_thread")]\nasync fn main() {}\n' >"$wrong_main/src/main.rs"
expect_rejected "multi-thread runtime" "$wrong_main"

forbidden=(
  'tokio::spawn(async {})'
  'tokio::task::spawn(async {})'
  'tokio::task::spawn_local(async {})'
  'tokio::task::spawn_blocking(|| {})'
  'tokio::task::LocalSet::new()'
  'tokio::task::JoinSet::new()'
  'tokio::join!(a, b)'
  'tokio::try_join!(a, b)'
  'tokio::select! { _ = a => {} }'
  'FuturesUnordered::new()'
  'stream.buffered(2)'
  'stream.buffer_unordered(2)'
  'stream.for_each_concurrent(2, f)'
  'std::thread::spawn(|| {})'
  'thread::spawn(|| {})'
)

for index in "${!forbidden[@]}"; do
  root=$(new_fixture "forbidden-$index")
  printf '%s\n' "${forbidden[$index]}" >"$root/src/lib.rs"
  expect_rejected "${forbidden[$index]}" "$root"
done

ignored=$(new_fixture ignored-paths)
{
  printf '%s\n' "${forbidden[@]}"
} >"$ignored/src/lib_tests.rs"
{
  printf '%s\n' "${forbidden[@]}"
} >"$ignored/src/test_helpers.rs"
{
  printf '%s\n' "${forbidden[@]}"
} >"$ignored/docs/example.md"
expect_allowed "test and documentation examples" "$ignored"
