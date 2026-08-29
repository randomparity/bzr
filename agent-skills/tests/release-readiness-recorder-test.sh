#!/usr/bin/env bash
set -euo pipefail

test_root=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(cd "$test_root/../.." && pwd -P)
work=$(mktemp -d)
trap 'rm -r "$work"' EXIT

sandbox="$work/repo"
stub_bin="$work/bin"
mkdir -p "$sandbox/tools" "$sandbox/docs/assets" "$stub_bin"
cp -p "$repo_root/tools/record-demo.sh" "$sandbox/tools/record-demo.sh"
cp -p "$repo_root/tools/run-release-readiness-demo.sh" \
  "$sandbox/tools/run-release-readiness-demo.sh"

tracked_cast="$sandbox/docs/assets/bzr-release-readiness-demo.cast"
tracked_gif="$sandbox/docs/assets/bzr-release-readiness-demo.gif"
printf 'published-cast\n' >"$tracked_cast"
printf 'published-gif\n' >"$tracked_gif"
cp "$tracked_cast" "$work/cast.before"
cp "$tracked_gif" "$work/gif.before"

cat >"$stub_bin/bzr" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ " $* " == *' bug list '* ]]; then
  printf '%s\n' '{"data":[{"id":3,"product":"ReleaseDemo","whiteboard":"bzr-release-readiness-demo-v1 release-blocker"}]}'
fi
EOF
cat >"$stub_bin/curl" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat >"$stub_bin/asciinema" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
for output; do :; done
printf '%s\n' "$FAKE_CAST_CONTENT" >"$output"
EOF
cat >"$stub_bin/agg" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
args=("$@")
input=${args[${#args[@]}-2]}
output=${args[${#args[@]}-1]}
printf '%s\n%s\n' "$input" "$output" >"$AGG_CALLED"
printf 'rendered-gif\n' >"$output"
[[ ${FAKE_AGG_FAIL:-0} -eq 0 ]]
EOF
chmod +x "$stub_bin/bzr" "$stub_bin/curl" "$stub_bin/asciinema" "$stub_bin/agg"

common_env=(
  PATH="$stub_bin:$PATH"
  BZR_BIN="$stub_bin/bzr"
  AGG_CALLED="$work/agg-called"
)

if env "${common_env[@]}" FAKE_CAST_CONTENT='http://127.0.0.1:8089' \
  bash "$sandbox/tools/record-demo.sh" release-readiness \
  >"$work/leak.stdout" 2>"$work/leak.stderr"; then
  printf 'recorder privacy failure: leaking cast was accepted\n' >&2
  exit 1
fi
cmp "$work/cast.before" "$tracked_cast" || {
  printf 'recorder privacy failure: unverified cast replaced the published cast\n' >&2
  exit 1
}
cmp "$work/gif.before" "$tracked_gif" || {
  printf 'recorder privacy failure: rejected cast replaced the published GIF\n' >&2
  exit 1
}
[[ ! -e $work/agg-called ]] || {
  printf 'recorder privacy failure: rejected cast reached the renderer\n' >&2
  exit 1
}

if env "${common_env[@]}" FAKE_CAST_CONTENT='verified-cast' FAKE_AGG_FAIL=1 \
  bash "$sandbox/tools/record-demo.sh" release-readiness \
  >"$work/render-failure.stdout" 2>"$work/render-failure.stderr"; then
  printf 'recorder publication failure: failing renderer was accepted\n' >&2
  exit 1
fi
cmp "$work/cast.before" "$tracked_cast" || {
  printf 'recorder publication failure: failed render replaced the published cast\n' >&2
  exit 1
}
cmp "$work/gif.before" "$tracked_gif" || {
  printf 'recorder publication failure: failed render replaced the published GIF\n' >&2
  exit 1
}

env "${common_env[@]}" FAKE_CAST_CONTENT='verified-cast' \
  bash "$sandbox/tools/record-demo.sh" release-readiness \
  >"$work/clean.stdout" 2>"$work/clean.stderr"
grep -Fxq 'verified-cast' "$tracked_cast"
grep -Fxq 'rendered-gif' "$tracked_gif"
if grep -Fq "$sandbox/docs/assets/" "$work/agg-called"; then
  printf 'recorder publication failure: renderer used a published asset path\n' >&2
  exit 1
fi

printf 'release-readiness recorder privacy: ok\n'
