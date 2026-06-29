#!/bin/sh
# bzr-skill installer. Copies skill folders into agent skill directories.
set -eu

# shellcheck disable=SC1007  # CDPATH= is intentional: zero it before cd
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SKILLS_SRC="$SCRIPT_DIR/skills"
VERSION_FILE="$SCRIPT_DIR/VERSION"
SENTINEL=".bzr-skill-managed"
SKILL_NAMES="bzr-reference bzr-setup bzr-file-bug bzr-triage-bug bzr-search-report bzr-bulk-triage"
DEST_ROOT="${BZR_SKILL_DEST_ROOT:-$HOME}"
AGENTS_DIR="$DEST_ROOT/.agents/skills"
CLAUDE_DIR="$DEST_ROOT/.claude/skills"
TMP_FETCH_DIR=""
SOURCE_COMMIT_OVERRIDE=""

AGENT=""
DRY_RUN=0
FORCE=0
ACTION="install"

usage() {
  cat <<EOF
bzr-skill installer

Usage: install.sh [--agent <target>] [--dry-run] [--force] [--uninstall] [--list] [--help]

Targets (--agent):
  standard | bob | codex   -> $AGENTS_DIR
  claude                   -> $CLAUDE_DIR
  all                      -> both

Options:
  --dry-run    show the plan without writing
  --force      overwrite a foreign same-named folder (never overrides symlink guard)
  --uninstall  remove skill folders this installer owns
  --list       show which bzr skills are installed where (and stale ones)
  --help       this message

Env:
  BZR_SKILL_DEST_ROOT   destination root (default: \$HOME); used by tests.
EOF
}

die() {
  printf 'install: %s\n' "$1" >&2
  exit 1
}

source_version() {
  [ -f "$VERSION_FILE" ] && cat "$VERSION_FILE" || echo "unknown"
}

source_commit() {
  if [ -n "$SOURCE_COMMIT_OVERRIDE" ]; then
    printf '%s\n' "$SOURCE_COMMIT_OVERRIDE"
  else
    git -C "$SCRIPT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown"
  fi
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "remote install needs '$1' but it is not on PATH"
}

# $1 = url-or-path, $2 = destination file. Download (http/https) or copy (file://, path).
fetch_tarball() {
  case "$1" in
  http://* | https://*)
    if command -v curl >/dev/null 2>&1; then
      curl -fsSL "$1" -o "$2" || die "download failed: $1"
    elif command -v wget >/dev/null 2>&1; then
      wget -q -O "$2" "$1" || die "download failed: $1"
    else
      die "remote install needs 'curl' or 'wget' but neither is on PATH"
    fi
    ;;
  file://*)
    src=${1#file://}
    [ -f "$src" ] || die "tarball not found: $src"
    cp -- "$src" "$2" || die "cannot copy tarball: $src"
    ;;
  *)
    [ -f "$1" ] || die "tarball not found: $1"
    cp -- "$1" "$2" || die "cannot copy tarball: $1"
    ;;
  esac
}

# Decide local vs remote; in remote mode download+extract and repoint globals.
resolve_skills_src() {
  ref="${BZR_SKILL_REF:-main}"
  # Default URL is the only path with no hermetic test (it would hit the network);
  # the forced-remote tests exercise the same download/extract code via a fixture.
  url="${BZR_SKILL_TARBALL_URL:-https://codeload.github.com/randomparity/bzr/tar.gz/$ref}"

  if [ -z "${BZR_SKILL_TARBALL_URL:-}" ] && [ -z "${BZR_SKILL_REF:-}" ]; then
    [ -d "$SKILLS_SRC/bzr-reference" ] && return 0
  fi

  need_cmd tar
  need_cmd mktemp
  TMP_FETCH_DIR=$(mktemp -d) || die "cannot create temp dir"
  tarball="$TMP_FETCH_DIR/skills.tgz"
  fetch_tarball "$url" "$tarball"
  (cd "$TMP_FETCH_DIR" && tar xzf "$tarball") || die "cannot extract tarball: $url"

  found=""
  for d in "$TMP_FETCH_DIR"/*/agent-skills/skills; do
    [ -d "$d/bzr-reference" ] || continue
    found="$d"
    break
  done
  [ -n "$found" ] || die "downloaded tarball has no agent-skills/skills/bzr-reference"
  SKILLS_SRC="$found"
  VERSION_FILE="$found/../VERSION"
  if [ -n "${BZR_SKILL_REF:-}" ]; then
    SOURCE_COMMIT_OVERRIDE="remote:$BZR_SKILL_REF"
  elif [ -n "${BZR_SKILL_TARBALL_URL:-}" ]; then
    SOURCE_COMMIT_OVERRIDE="remote:url"
  else
    SOURCE_COMMIT_OVERRIDE="remote:$ref"
  fi
}

# Consolidated cleanup: remove the fetch workdir and release both locks.
cleanup() {
  [ -n "$TMP_FETCH_DIR" ] && rm -rf "$TMP_FETCH_DIR"
  release_lock "$AGENTS_DIR"
  release_lock "$CLAUDE_DIR"
}

# Echo the destination dir(s) for the chosen agent, one per line.
resolve_destinations() {
  case "$1" in
  standard | bob | codex) printf '%s\n' "$AGENTS_DIR" ;;
  claude) printf '%s\n' "$CLAUDE_DIR" ;;
  all) printf '%s\n%s\n' "$AGENTS_DIR" "$CLAUDE_DIR" ;;
  *) return 1 ;;
  esac
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
    --agent)
      [ "$#" -ge 2 ] || die "--agent needs a value"
      AGENT="$2"
      shift 2
      ;;
    --agent=*)
      AGENT="${1#--agent=}"
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    --uninstall)
      ACTION="uninstall"
      shift
      ;;
    --list)
      ACTION="list"
      shift
      ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1" ;;
    esac
  done
}

# Prompt for a target when none was given and we have a terminal.
# Non-interactive callers (CI, tests) must pass --agent and never reach this.
prompt_agent() {
  if [ ! -t 0 ]; then
    die "no --agent given and no terminal to prompt (try --help)"
  fi
  printf 'Install bzr skills for which agent?\n' >&2
  printf '  1) standard / bob / codex  (%s)\n' "$AGENTS_DIR" >&2
  printf '  2) claude                  (%s)\n' "$CLAUDE_DIR" >&2
  printf '  3) all (both)\n' >&2
  printf 'Choose [1-3]: ' >&2
  read -r choice
  case "$choice" in
  1) AGENT="standard" ;;
  2) AGENT="claude" ;;
  3) AGENT="all" ;;
  *) die "invalid choice: $choice" ;;
  esac
}

probe_bzr() {
  if ! command -v bzr >/dev/null 2>&1; then
    printf 'install: note: bzr not found on PATH. The skills call the bzr binary;\n' >&2
    printf '         install it from https://github.com/randomparity/bzr\n' >&2
  fi
}

ensure_dir() {
  [ -d "$1" ] || mkdir -p "$1" || die "cannot create $1"
}

is_owned() {
  # $1 = folder path. Owned iff it contains the sentinel.
  [ -f "$1/$SENTINEL" ]
}

write_sentinel() {
  # $1 = folder, $2 = skill name
  cat >"$1/$SENTINEL" <<EOF
managed-by: bzr-skill
installed-skill: $2
source-version: $(source_version)
source-commit: $(source_commit)
EOF
}

# Install one skill folder into one destination. Returns non-zero on refusal/error.
install_skill() {
  skill="$1"
  dest="$2"
  src="$SKILLS_SRC/$skill"
  target="$dest/$skill"
  [ -d "$src" ] || die "source skill missing: $src"

  if [ -L "$target" ]; then
    printf 'install: REFUSE %s: destination is a symlink (unconditional guard)\n' "$target" >&2
    return 1
  fi
  if [ -e "$target" ] && ! is_owned "$target"; then
    if [ "$FORCE" -ne 1 ]; then
      printf 'install: REFUSE %s: foreign folder (no sentinel); use --force to overwrite\n' "$target" >&2
      return 1
    fi
  fi

  stage="$dest/.bzr-skill.stage.$skill.$$"
  rm -rf "$stage"
  cp -R "$src" "$stage"
  write_sentinel "$stage" "$skill"

  # Verify the staged copy BEFORE touching the original; abort if incomplete.
  if [ ! -f "$stage/SKILL.md" ]; then
    rm -rf "$stage"
    printf 'install: ERROR staged copy for %s missing SKILL.md; aborting (original untouched)\n' "$skill" >&2
    return 1
  fi

  if [ -e "$target" ]; then
    aside="$dest/.bzr-skill.old.$skill.$$"
    rm -rf "$aside"
    mv "$target" "$aside"
    if mv "$stage" "$target"; then
      rm -rf "$aside"
    else
      mv "$aside" "$target"
      rm -rf "$stage"
      printf 'install: ERROR replacing %s; restored original\n' "$target" >&2
      return 1
    fi
  else
    mv "$stage" "$target"
  fi
  return 0
}

reject_symlink_path() {
  # $1 = a destination dir under DEST_ROOT. Die if it, or any ancestor down
  # from DEST_ROOT, is a symlink (home-directory-escape guard).
  # Uses parameter expansion instead of dirname to avoid external command dependency.
  d="$1"
  while [ -n "$d" ] && [ "$d" != "$DEST_ROOT" ] && [ "$d" != "/" ]; do
    if [ -L "$d" ]; then
      die "refusing $1: ancestor $d is a symlink (escape guard)"
    fi
    # Strip the last path component (portable replacement for dirname).
    case "$d" in
    */*) d="${d%/*}" ;;
    *) d="" ;;
    esac
  done
}

acquire_lock() {
  # $1 = destination dir. Lock dir: $1/.bzr-skill.lock
  lockdir="$1/.bzr-skill.lock"
  ensure_dir "$1"
  if mkdir "$lockdir" 2>/dev/null; then
    printf '%s\n' "$$" >"$lockdir/pid"
    return 0
  fi
  # Lock exists: check holder liveness. An empty pid is treated as live (fail-closed).
  holder=$(cat "$lockdir/pid" 2>/dev/null || echo "")
  if [ -z "$holder" ] || kill -0 "$holder" 2>/dev/null; then
    die "destination locked ($lockdir); remove it if stale"
  fi
  # Stale: non-empty holder pid is dead — break and retake.
  rm -rf "$lockdir"
  mkdir "$lockdir" 2>/dev/null || die "could not acquire lock $lockdir"
  printf '%s\n' "$$" >"$lockdir/pid"
}

release_lock() {
  rm -rf "$1/.bzr-skill.lock"
}

do_install() {
  rc=0
  for dest in "$@"; do reject_symlink_path "$dest"; done
  if [ "$DRY_RUN" -ne 1 ]; then
    for dest in "$@"; do acquire_lock "$dest"; done
  fi
  for dest in "$@"; do
    if [ "$DRY_RUN" -eq 1 ]; then
      printf 'plan: install into %s\n' "$dest"
      for s in $SKILL_NAMES; do printf '  - %s\n' "$s"; done
      continue
    fi
    ensure_dir "$dest"
    for s in $SKILL_NAMES; do
      if install_skill "$s" "$dest"; then
        printf 'install: ok   %s -> %s\n' "$s" "$dest"
      else
        printf 'install: failed %s -> %s\n' "$s" "$dest" >&2
        rc=1
      fi
    done
  done
  return "$rc"
}

uninstall_skill() {
  skill="$1"
  dest="$2"
  target="$dest/$skill"
  if [ -L "$target" ]; then
    printf 'uninstall: skip %s: symlink, not touching\n' "$target" >&2
    return 0
  fi
  if [ ! -e "$target" ]; then
    return 0
  fi
  if ! is_owned "$target"; then
    printf 'uninstall: skip %s: foreign folder (no sentinel)\n' "$target"
    return 0
  fi
  aside="$dest/.bzr-skill.rm.$skill.$$"
  mv "$target" "$aside" || {
    printf 'uninstall: ERROR moving %s\n' "$target" >&2
    return 1
  }
  rm -rf "$aside"
  printf 'uninstall: removed %s\n' "$target"
}

do_uninstall() {
  rc=0
  for dest in "$@"; do reject_symlink_path "$dest"; done
  for dest in "$@"; do
    [ -d "$dest" ] || continue
    acquire_lock "$dest"
  done
  for dest in "$@"; do
    for s in $SKILL_NAMES; do
      uninstall_skill "$s" "$dest" || rc=1
    done
  done
  return "$rc"
}

sentinel_version() {
  # $1 = folder. Echo the recorded source-version or empty.
  [ -f "$1/$SENTINEL" ] || return 0
  awk -F': ' '/^source-version:/ {print $2; exit}' "$1/$SENTINEL"
}

do_list() {
  for dest in "$@"; do reject_symlink_path "$dest"; done
  cur=$(source_version)
  for dest in "$@"; do
    printf 'destination: %s\n' "$dest"
    for s in $SKILL_NAMES; do
      target="$dest/$s"
      if [ -L "$target" ]; then
        printf '  %-18s symlink (shadowed)\n' "$s"
      elif [ ! -e "$target" ]; then
        printf '  %-18s absent\n' "$s"
      elif is_owned "$target"; then
        iv=$(sentinel_version "$target")
        if [ "$iv" = "$cur" ]; then
          printf '  %-18s present (%s)\n' "$s" "$iv"
        else
          printf '  %-18s present, stale (installed %s, source %s) -- re-run install.sh\n' \
            "$s" "$iv" "$cur"
        fi
      else
        printf '  %-18s shadowed (foreign folder)\n' "$s"
      fi
    done
  done
}

main() {
  parse_args "$@"
  case "$ACTION" in
  install)
    [ -n "$AGENT" ] || prompt_agent
    dests=$(resolve_destinations "$AGENT") || die "unknown agent: $AGENT"
    if [ "$DRY_RUN" -ne 1 ]; then
      probe_bzr
      trap cleanup EXIT INT TERM
      resolve_skills_src
    fi
    ;;
  uninstall)
    [ -n "$AGENT" ] || die "no --agent given (try --help)"
    dests=$(resolve_destinations "$AGENT") || die "unknown agent: $AGENT"
    trap cleanup EXIT INT TERM
    ;;
  list)
    [ -n "$AGENT" ] || AGENT="all"
    dests=$(resolve_destinations "$AGENT") || die "unknown agent: $AGENT"
    ;;
  *) die "action $ACTION not implemented yet" ;;
  esac
  # Split the newline-separated dest list on newlines only, preserving spaces in paths.
  oldifs=$IFS
  IFS='
'
  # shellcheck disable=SC2086
  set -- $dests
  IFS=$oldifs
  case "$ACTION" in
  install) do_install "$@" ;;
  uninstall) do_uninstall "$@" ;;
  list) do_list "$@" ;;
  esac
}

main "$@"
