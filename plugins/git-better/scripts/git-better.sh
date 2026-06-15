#!/usr/bin/env bash
# git-better (`gb`) — token-lean git read commands.
#
# Bare invocations apply a lean default (compact, lockfiles excluded, color off);
# the moment any argument is present the command is forwarded verbatim to git with
# color disabled, so power use (`gb diff --cached`, ranges, paths) still works.
# Read ops `exec` git so its exit code passes straight through.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NOCOLOR=(-c color.ui=false)

usage() {
  cat <<'USAGE'
Usage: gb <subcommand> [args...]

  status [git-status-args...]   git status -sb (+args)              — short, branch, no color
  diff   [git-diff-args...]     bare: --stat, lockfiles excluded    — any arg → verbatim
         [--full]               --full: full hunks, all files
  log    [git-log-args...]      git log --oneline -n 20 (+args override)
  show   [git-show-args...]     bare: show --stat HEAD              — any arg → verbatim
         [--full]               --full: full hunks
  conventions [--json] [--refresh] [--save-prose <file>]
                                repo convention profile (cached)

Lean by default; pass any git flag to override.
USAGE
}

subcmd="${1:-}"
shift 2>/dev/null || true

case "$subcmd" in
  status)
    exec git "${NOCOLOR[@]}" status -sb "$@"
    ;;
  diff)
    if [ "$#" -eq 0 ]; then
      # Bare: scope-first stat, excluding lockfiles (the `*.lock` pathspec also
      # matches bun.lock). A positive pathspec `.` is required alongside excludes.
      exec git "${NOCOLOR[@]}" diff --stat -- . \
        ':(exclude)*.lock' ':(exclude)*-lock.json' ':(exclude)*.lockb' ':(exclude)*.sum'
    fi
    [ "$1" = "--full" ] && shift   # sugar: full hunks, all files
    exec git "${NOCOLOR[@]}" diff "$@"
    ;;
  log)
    exec git "${NOCOLOR[@]}" log --oneline -n 20 "$@"
    ;;
  show)
    if [ "$#" -eq 0 ]; then
      exec git "${NOCOLOR[@]}" show --stat HEAD
    fi
    [ "$1" = "--full" ] && shift
    exec git "${NOCOLOR[@]}" show "$@"
    ;;
  conventions)
    exec bash "$SCRIPT_DIR/lib/conventions-cache.sh" "$@"
    ;;
  ""|-h|--help|help)
    usage
    [ -z "$subcmd" ] && exit 1 || exit 0
    ;;
  *)
    echo "gb: unknown subcommand '$subcmd'" >&2
    usage >&2
    exit 1
    ;;
esac
