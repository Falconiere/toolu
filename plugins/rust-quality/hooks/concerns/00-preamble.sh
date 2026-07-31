#!/usr/bin/env bash
# Post-tool check: Rust quality rules.
# Project-agnostic: no-op outside Rust projects or when cargo is missing.
# Unsafe-block exemptions for FFI crates come from
# $TOOLU_SETTINGS_DIR/rust-unsafe-exemptions.txt.
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
command -v rust_max_file_lines >/dev/null 2>&1 || rust_max_file_lines() { echo "${DEFAULT_RUST_MAX_FILE_LINES:-500}"; }
command -v rust_max_fn_lines   >/dev/null 2>&1 || rust_max_fn_lines()   { echo "${DEFAULT_RUST_MAX_FN_LINES:-50}"; }
command -v rust_max_impl_lines >/dev/null 2>&1 || rust_max_impl_lines() { echo "${DEFAULT_RUST_MAX_IMPL_LINES:-200}"; }
# Boolean flag reader (e.g. noMocks) — falls back to the caller's default when
# quality-config.sh predates this reader.
command -v quality_flag >/dev/null 2>&1 || quality_flag() { printf '%s' "$3"; }
# count_code_lines comes from detect.sh (sourced above) — no fallback needed.

# Load the merged config ONCE so TOOLU_CFG_LOADED sticks for the threshold
# lookups below — each runs in a $(...) subshell that inherits it and skips
# re-merging (otherwise every wrapper re-spawns the jq merge).
command -v toolu_load_config >/dev/null 2>&1 && toolu_load_config 2>/dev/null || true

[ "$(detect_rust)" = "rust" ] || exit 0
command -v cargo >/dev/null 2>&1 || exit 0
command -v jq    >/dev/null 2>&1 || exit 0

TOOLU_SETTINGS_DIR=$(toolu_settings_dir)
EXEMPTIONS_FILE="$TOOLU_SETTINGS_DIR/rust-unsafe-exemptions.txt"

# read_list is sourced from lib/detect.sh.

fp_from_input=""
if [[ "$tool_name" == "Write" || "$tool_name" == "Edit" || "$tool_name" == "MultiEdit" ]]; then
  fp_from_input=$(echo "$input" | jq -r '.tool_input.path // .tool_input.file_path // .tool_input.target_file // empty' 2>/dev/null || echo "")
fi
FILE_PATH="${CLAUDE_FILE_PATHS:-$fp_from_input}"

[[ -z "$FILE_PATH" || ! -f "$FILE_PATH" ]] && exit 0
[[ ! "$FILE_PATH" =~ \.rs$ ]] && exit 0

MESSAGES=""
add_error() {
  MESSAGES="${MESSAGES}${1}"$'\n'
}

