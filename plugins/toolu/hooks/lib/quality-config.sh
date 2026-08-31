#!/usr/bin/env bash
# Quality-threshold resolver for the rust-quality / ts-quality hooks.
#
# Limits are NOT hardcoded in the quality modules. Each threshold resolves with
# this precedence (first hit wins, always a positive integer):
#   1. Project / user override — `lang.<lang>.<key>` in the merged toolu
#      config (.claude/toolu.config.json), via lib/config.sh.
#   2. Native tool config (TS + maxFileLines only) — the `max-lines` rule from
#      .eslintrc.json / .oxlintrc.json. Flat eslint config (eslint.config.{js,mjs,ts})
#      is JS and NOT parseable here — skipped gracefully.
#   3. Built-in default (DEFAULT_* below).
#
# Every reader is guarded (jq present, file present) and 2>/dev/null — any
# failure falls through to the next layer. Never errors, never blocks a tool.
#
# Source via:  . "$TOOLU_LIB_DIR/quality-config.sh"
# (pulls in config.sh + detect.sh from the same dir).

_QC_LIB_DIR="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}"
# shellcheck source=./config.sh
[ -f "$_QC_LIB_DIR/config.sh" ] && . "$_QC_LIB_DIR/config.sh"
# shellcheck source=./detect.sh
[ -f "$_QC_LIB_DIR/detect.sh" ] && . "$_QC_LIB_DIR/detect.sh"

# Built-in defaults — single source of truth, overridable from the environment
# (tests set these to assert fall-through without touching call sites).
: "${DEFAULT_TS_MAX_FILE_LINES:=300}"
: "${DEFAULT_TS_MAX_FN_LINES:=60}"
: "${DEFAULT_RUST_MAX_FILE_LINES:=500}"
: "${DEFAULT_RUST_MAX_FN_LINES:=50}"
: "${DEFAULT_RUST_MAX_IMPL_LINES:=200}"
: "${DEFAULT_PYTHON_MAX_FILE_LINES:=400}"
: "${DEFAULT_PYTHON_MAX_FN_LINES:=50}"

# One jq filter that normalizes every eslint/oxlint `max-lines` encoding to a
# positive integer (or empty): bare number, ["error", N], ["error", {"max": N}].
# "off" / 0 / negatives / missing all yield empty -> the layer is skipped.
_QC_ESLINT_MAXLINES_FILTER='
  .rules?["max-lines"]?
  | if type=="array" then (
          if (.[0] == "off" or .[0] == 0) then empty   # rule disabled by severity
          else ( .[1] | if type=="object" then .max else . end )
          end )
    elif (type=="number" or type=="string") then .
    else empty end
  | (if type=="string" then (tonumber? // empty) else . end)   # accept "120"
  | if type=="number" and . > 0 then (.|floor|tostring) else empty end
'

# _qc_project_override LANG KEY  ->  echoes a positive int or "".
# Reads the already-merged config cache (config.sh handles user+project merge,
# malformed JSON, and missing jq).
_qc_project_override() {
  local lang="$1" key="$2"
  command -v toolu_load_config >/dev/null 2>&1 || return 0
  toolu_load_config
  [ "${_TOOLU_HAS_JQ:-0}" = "1" ] || return 0
  [ -n "${TOOLU_CFG_JSON:-}" ] || return 0
  jq -r --arg l "$lang" --arg k "$key" '
    ((.lang? // {})[$l]? // {})[$k]?
    | (if type=="string" then (tonumber? // empty) else . end)   # accept "120"
    | if type=="number" and . > 0 then (.|floor|tostring) else empty end
  ' <<< "$TOOLU_CFG_JSON" 2>/dev/null
}

# quality_flag LANG KEY DEFAULT  ->  echoes "true" or "false".
# Boolean reader for `.lang.<lang>.<key>` in the merged config, for keys that
# ARE booleans (e.g. noMocks) — _qc_project_override above filters to positive
# integers and can't carry a boolean, so this is a separate reader rather than
# an extension of it. Only a literal JSON `true`/`false` is honored; anything
# else (key absent, wrong type, malformed config, no jq) falls back to DEFAULT
# unchanged. Never errors, never blocks a tool.
quality_flag() {
  local lang="$1" key="$2" def="$3"
  command -v toolu_load_config >/dev/null 2>&1 || { printf '%s' "$def"; return 0; }
  toolu_load_config
  [ "${_TOOLU_HAS_JQ:-0}" = "1" ] || { printf '%s' "$def"; return 0; }
  [ -n "${TOOLU_CFG_JSON:-}" ] || { printf '%s' "$def"; return 0; }
  local val
  val=$(jq -r --arg l "$lang" --arg k "$key" '
    ((.lang? // {})[$l]? // {})[$k]?
    | if type == "boolean" then tostring else empty end
  ' <<< "$TOOLU_CFG_JSON" 2>/dev/null)
  case "$val" in
    true|false) printf '%s' "$val" ;;
    *)          printf '%s' "$def" ;;
  esac
}

# Memoize the git toplevel for this process. detect_project_root shells out to
# `git rev-parse` every call; the root can't change mid-hook, so cache it once
# (mirrors toolu_load_config's caching). A separate _CACHED flag is used so
# an EMPTY result (non-git cwd) is still treated as cached — keying on
# _QC_PROJECT_ROOT being non-empty would re-shell git on every call there. Tests
# re-source this file (resetting both) between cases.
_QC_PROJECT_ROOT=""
_QC_PROJECT_ROOT_CACHED=0
_qc_project_root() {
  [ "$_QC_PROJECT_ROOT_CACHED" = "1" ] && { printf '%s' "$_QC_PROJECT_ROOT"; return 0; }
  command -v detect_project_root >/dev/null 2>&1 || return 0
  _QC_PROJECT_ROOT=$(detect_project_root)
  _QC_PROJECT_ROOT_CACHED=1
  printf '%s' "$_QC_PROJECT_ROOT"
}

# _qc_native_ts_max_lines  ->  echoes a positive int or "".
# Reads the config of the linter detect_ts_linter reports as ACTIVE, so a repo
# carrying BOTH .eslintrc.json and .oxlintrc.json (e.g. mid-migration) gets the
# limit from the one it actually runs — not whichever file is listed first.
# detect_ts_linter precedence is biome > oxc > eslint; biome's max-lines isn't
# parsed here (no machine-readable rule), so it falls through to the default.
# Limitation: only JSON config forms are parsed — `.eslintrc.yaml/.yml`, flat
# `eslint.config.*`, and `package.json#eslintConfig` fall through to the default.
# detect_ts_linter still reports "eslint" for those, so the over-limit advisory
# stays honest ("limit didn't come from its config").
# Limitation: only the top-level `.rules["max-lines"]` is read. A per-glob
# `overrides[].rules["max-lines"]` is intentionally NOT traversed — picking a
# value without matching the override's `files` glob to the edited file could
# enforce a limit that doesn't apply and wrongly BLOCK an edit. We fall through
# to the (stricter-or-equal) default instead; the over-limit advisory says so.
_qc_native_ts_max_lines() {
  command -v jq >/dev/null 2>&1 || return 0
  local root linter f v
  root=$(_qc_project_root)
  [ -n "$root" ] || return 0
  # Map the active linter to its machine-readable config file.
  linter=""
  command -v detect_ts_linter >/dev/null 2>&1 && linter=$(detect_ts_linter)
  case "$linter" in
    oxc)    f="$root/.oxlintrc.json" ;;
    eslint) f="$root/.eslintrc.json" ;;
    *)      return 0 ;;   # biome / none / unparseable form -> default
  esac
  [ -f "$f" ] || return 0
  v=$(jq -r "$_QC_ESLINT_MAXLINES_FILTER" "$f" 2>/dev/null)
  [ -n "$v" ] && printf '%s' "$v"
}

# quality_threshold LANG KEY DEFAULT  ->  always echoes a positive integer.
quality_threshold() {
  local lang="$1" key="$2" def="$3" v
  v=$(_qc_project_override "$lang" "$key")
  [ -n "$v" ] && { printf '%s' "$v"; return 0; }
  if [ "$lang" = "ts" ] && [ "$key" = "maxFileLines" ]; then
    v=$(_qc_native_ts_max_lines)
    [ -n "$v" ] && { printf '%s' "$v"; return 0; }
  fi
  printf '%s' "$def"
}

# count_code_lines lives in detect.sh (sourced above) — both lang modules
# hard-require detect.sh, so it needs no per-module fallback.

# Echo where ts maxFileLines actually resolved from: override | native | default.
# Lets the lang module word its advisory honestly — claiming "eslint enforces
# this" only when the limit really came from a parsed eslint/oxlint config, not
# when a linter is merely present with a config we can't read (e.g. .eslintrc.cjs).
ts_max_file_lines_source() {
  [ -n "$(_qc_project_override ts maxFileLines)" ] && { printf 'override'; return; }
  [ -n "$(_qc_native_ts_max_lines)" ]             && { printf 'native';   return; }
  printf 'default'
}

# Resolve the ts maxFileLines value AND its source in a single pass — prints
# "<value> <source>". The lang module needs both, and resolving them via two
# separate calls (ts_max_file_lines + ts_max_file_lines_source) repeats the
# override/native lookups; this does each lookup once.
ts_max_file_lines_resolved() {
  local v
  v=$(_qc_project_override ts maxFileLines)
  [ -n "$v" ] && { printf '%s override' "$v"; return; }
  v=$(_qc_native_ts_max_lines)
  [ -n "$v" ] && { printf '%s native' "$v"; return; }
  printf '%s default' "$DEFAULT_TS_MAX_FILE_LINES"
}

# Convenience wrappers the quality modules call.
ts_max_file_lines()   { quality_threshold ts   maxFileLines "$DEFAULT_TS_MAX_FILE_LINES"; }
ts_max_fn_lines()     { quality_threshold ts   maxFnLines   "$DEFAULT_TS_MAX_FN_LINES"; }
rust_max_file_lines() { quality_threshold rust maxFileLines "$DEFAULT_RUST_MAX_FILE_LINES"; }
rust_max_fn_lines()   { quality_threshold rust maxFnLines   "$DEFAULT_RUST_MAX_FN_LINES"; }
rust_max_impl_lines() { quality_threshold rust maxImplLines "$DEFAULT_RUST_MAX_IMPL_LINES"; }
python_max_file_lines() { quality_threshold python maxFileLines "$DEFAULT_PYTHON_MAX_FILE_LINES"; }
python_max_fn_lines()   { quality_threshold python maxFnLines   "$DEFAULT_PYTHON_MAX_FN_LINES"; }
