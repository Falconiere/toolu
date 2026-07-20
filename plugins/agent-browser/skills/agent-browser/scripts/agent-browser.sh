#!/usr/bin/env bash
set -euo pipefail

# agent-browser wrapper — bakes token-lean defaults onto the read-heavy command
# family so the agent spends context on the accessibility tree, not raw HTML or
# screenshots. Injection is fail-open via a STATIC per-subcommand allow-map: a
# default flag is added ONLY where that subcommand accepts it, so shaping can
# never turn a working call into a usage error; every other subcommand passes
# through untouched. A leading `--raw` bypasses all shaping. The binary is never
# auto-installed — a missing one prints an install guide and exits non-zero.
#
# $AGENT_BROWSER_BIN overrides the binary (absolute path or PATH name), for
# pointing at a custom build or for tests.

BIN="${AGENT_BROWSER_BIN:-agent-browser}"
DEFAULT_MAX_OUTPUT=4000

install_guide() {
  echo "agent-browser not found — install: npm i -g agent-browser && agent-browser install  (or: brew install agent-browser / cargo install agent-browser)" >&2
}

require_bin() {
  command -v "$BIN" >/dev/null 2>&1 || { install_guide; exit 127; }
}

# Exact-token or --opt=value match against the remaining args.
has_flag() {
  local needle="$1"; shift
  local a
  for a in "$@"; do
    case "$a" in
      "$needle" | "$needle="*) return 0 ;;
    esac
  done
  return 1
}

# --raw: total bypass, no shaping.
if [ "${1:-}" = "--raw" ]; then
  shift
  require_bin
  exec "$BIN" "$@"
fi

require_bin

# No subcommand — let agent-browser print its own help/usage.
if [ "$#" -eq 0 ]; then
  exec "$BIN"
fi

cmd="$1"; shift   # remaining flags/args stay in "$@"

inject=()
case "$cmd" in
  snapshot)
    # -i (interactive-only compact tree) unless the caller already scoped it.
    if ! has_flag -i "$@" && ! has_flag -a "$@" \
       && ! has_flag -s "$@" && ! has_flag -d "$@"; then
      inject+=(-i)
    fi
    has_flag --json "$@"               || inject+=(--json)
    has_flag --max-output "$@"         || inject+=(--max-output "$DEFAULT_MAX_OUTPUT")
    has_flag --content-boundaries "$@" || inject+=(--content-boundaries)
    ;;
  get)
    has_flag --json "$@"               || inject+=(--json)
    has_flag --max-output "$@"         || inject+=(--max-output "$DEFAULT_MAX_OUTPUT")
    has_flag --content-boundaries "$@" || inject+=(--content-boundaries)
    ;;
  find | diff)
    has_flag --json "$@"       || inject+=(--json)
    has_flag --max-output "$@" || inject+=(--max-output "$DEFAULT_MAX_OUTPUT")
    ;;
  *)
    : # unknown/binary-output subcommand — inject nothing (fail-open passthrough)
    ;;
esac

exec "$BIN" "$cmd" "${inject[@]+"${inject[@]}"}" "$@"
