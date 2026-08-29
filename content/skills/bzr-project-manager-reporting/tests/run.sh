#!/bin/sh
set -eu

: "${BZR_BIN:?set BZR_BIN to the bzr binary under test}"

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
SKILL="$HERE/../SKILL.md"
SAFETY="$HERE/../reference/artifact-safety.md"
PROMPT="$HERE/fixtures/demo-prompt.txt"
REPORT="$HERE/fixtures/demo-report.md"

run_help() {
  output=$("$BZR_BIN" "$@" --help)
  case $output in *Usage:*) ;; *) exit 1 ;; esac
}

run_help query show pm-status --json
run_help query save pm-status --from-url 'https://bugzilla.example.invalid/buglist.cgi?product=Example'
run_help query run pm-status --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard --paginate --json
run_help bug search --from-url 'https://bugzilla.example.invalid/buglist.cgi?product=Example' --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard --paginate --output ndjson
run_help comment list 1 --json

for format in CSV XLSX HTML Markdown; do grep -Fq "$format" "$SKILL"; done
grep -Fq 'requested capability is unavailable' "$SKILL"
grep -Fq 'mutable current snapshot' "$SKILL"
grep -Fq 'durable update history' "$SKILL"
grep -Fq 'bzr-weekly-status' "$SKILL"
grep -Fq 'bzr-dependency-analysis' "$SKILL"
grep -Fq 'bzr-release-readiness' "$SKILL"

grep -Fq '=, +, -, or @' "$SAFETY"
grep -Fq 'HTTP or HTTPS' "$SAFETY"
grep -Fq 'escape' "$SAFETY"

grep -Fq 'Produce a weekly portfolio report' "$PROMPT"
for section in 'Executive summary' 'Milestone view' 'Needs attention' 'Current updates' 'Limitations' 'Provenance'; do
  grep -Fq "$section" "$REPORT"
done
