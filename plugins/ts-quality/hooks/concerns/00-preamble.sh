#!/usr/bin/env bash
# Post-tool check: TypeScript / TSX quality rules
# Lightweight file-level checks (fast, no external tool invocations).
# Project-agnostic: no-op outside TS projects; package-manager driven.
#
# Inputs (from parent dispatcher post-tools/mod.sh, via `export`):
#   $tool_name     - name of the tool being invoked
#   $input         - raw JSON payload on stdin
#   $PROJECT_ROOT  - repository root

: "${tool_name:=}"
: "${input:=}"
: "${PROJECT_ROOT:=$(pwd)}"

# Core lib comes from the toolu dispatcher via TOOLU_LIB_DIR (set by
# plugins/toolu/hooks/post-tools/mod.sh before registry dispatch). Outside
# that pipeline there is no relative path to it — fail SOFT: a quality check
# must never break a tool call by erroring.
[ -n "${TOOLU_LIB_DIR:-}" ] && [ -f "$TOOLU_LIB_DIR/detect.sh" ] || exit 0
# shellcheck source=../../../toolu/hooks/lib/detect.sh
. "$TOOLU_LIB_DIR/detect.sh"
# Threshold resolver (defaults + project/native overrides). Soft if absent.
# shellcheck source=../../../toolu/hooks/lib/quality-config.sh
[ -f "$TOOLU_LIB_DIR/quality-config.sh" ] && . "$TOOLU_LIB_DIR/quality-config.sh"
# Multi-slot gate writer (entries keyed by file — one hook's failure no longer
# clobbers another's). Soft if absent: fallbacks below keep the legacy
# single-slot behavior when the toolu lib predates gate-file.sh.
# shellcheck source=../../../toolu/hooks/lib/gate-file.sh
[ -f "$TOOLU_LIB_DIR/gate-file.sh" ] && . "$TOOLU_LIB_DIR/gate-file.sh"
command -v gate_record_failure >/dev/null 2>&1 || gate_record_failure() {
  jq -n --arg reason "$4" --arg source "$3" --arg file "$2" --arg violations "$5" \
    --arg updatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{status: "failing", reason: $reason, source: $source, file: $file,
      violations: $violations, updatedAt: $updatedAt}' > "$1"
}
command -v gate_clear_file >/dev/null 2>&1 || gate_clear_file() {
  [ -f "$1" ] || return 0
  local _src _file
  _src=$(jq -r '.source // ""' "$1" 2>/dev/null || echo "")
  _file=$(jq -r '.file // ""' "$1" 2>/dev/null || echo "")
  if [ "$_src" = "$3" ] && [ "$_file" = "$2" ]; then
    jq -n --arg source "$3" --arg updatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
      '{status: "passing", source: $source, updatedAt: $updatedAt}' > "$1"
  fi
}
command -v ts_max_file_lines        >/dev/null 2>&1 || ts_max_file_lines()        { echo "${DEFAULT_TS_MAX_FILE_LINES:-300}"; }
command -v ts_max_fn_lines          >/dev/null 2>&1 || ts_max_fn_lines()          { echo "${DEFAULT_TS_MAX_FN_LINES:-60}"; }
command -v ts_max_file_lines_source >/dev/null 2>&1 || ts_max_file_lines_source() { printf 'default'; }
# count_code_lines comes from detect.sh (sourced above) — no fallback needed.

# Load the merged config ONCE in this shell so TOOLU_CFG_LOADED sticks for
# the threshold lookups below — each runs in a $(...) subshell that inherits it
# and skips re-merging (otherwise every wrapper re-spawns the jq merge).
command -v toolu_load_config >/dev/null 2>&1 && toolu_load_config 2>/dev/null || true

# Exit early if this isn't a TypeScript project.
[ "$(detect_ts)" = "ts" ] || exit 0

# Exit if no package manager is detected — we cannot recommend a typecheck command.
pm="$(detect_node_pm)"
[ -n "$pm" ] || exit 0
command -v "$pm" >/dev/null 2>&1 || exit 0

# Resolve the project's typecheck command per package manager.
typecheck_cmd() {
  case "$1" in
    bun)  echo "bun run typecheck" ;;
    pnpm) echo "pnpm -w typecheck" ;;
    yarn) echo "yarn typecheck" ;;
    npm)  echo "npm run typecheck" ;;
    *)    echo "$1 run typecheck" ;;
  esac
}
TYPECHECK_CMD="$(typecheck_cmd "$pm")"

command -v jq >/dev/null 2>&1 || exit 0

fp_from_input=""
if [[ "$tool_name" == "Write" || "$tool_name" == "Edit" || "$tool_name" == "MultiEdit" ]]; then
  fp_from_input=$(echo "$input" | jq -r '.tool_input.path // .tool_input.file_path // .tool_input.target_file // empty' 2>/dev/null || echo "")
fi
FILE_PATH="${CLAUDE_FILE_PATHS:-$fp_from_input}"

[[ -z "$FILE_PATH" || ! -f "$FILE_PATH" ]] && exit 0
[[ ! "$FILE_PATH" =~ \.(ts|tsx)$ ]] && exit 0

# Skip files inside git linked worktrees — quality state is for the main checkout only.
# One git call, one dirname: --git-dir + --git-common-dir come back on two lines.
_file_dir="$(dirname "$FILE_PATH")"
_file_git_dir=""; _file_common_dir=""
{ IFS= read -r _file_git_dir; IFS= read -r _file_common_dir; } < <(
  git -C "$_file_dir" rev-parse --path-format=absolute --git-dir --git-common-dir 2>/dev/null
)
_file_git_dir="${_file_git_dir%/}"
_file_common_dir="${_file_common_dir%/}"
if [[ -n "$_file_git_dir" && -n "$_file_common_dir" && "$_file_git_dir" != "$_file_common_dir" ]]; then
  exit 0
fi

MESSAGES=""
# Separator is a REAL newline. It used to be a literal backslash-n inside double
# quotes, which jq then encoded as "\\n" — so every multi-line violation reached
# the agent as one run-on line with a visible \n in it.
add_error() {
  MESSAGES="${MESSAGES}${1}"$'\n'
}

