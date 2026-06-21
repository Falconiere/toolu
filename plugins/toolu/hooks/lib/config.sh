#!/usr/bin/env bash
# Toolu runtime config loader.
#
# Reads two layers of JSON config and merges them (project override wins):
#   1. <agent-config-dir>/toolu.config.json
#   2. <project-root>/<project-config-dir>/toolu.config.json
#
# Defaults are Claude Code-compatible (~/.claude + .claude/), but callers can
# point them at pi-style roots via TOOLU_CONFIG_DIR and
# TOOLU_PROJECT_CONFIG_DIRNAME.
#
# Public API:
#   toolu_load_config          - load + cache the merged config
#   toolu_enabled CAT NAME     - 0 if enabled (default), 1 if disabled
#   toolu_comemory_state       - print 'available' | 'missing' | 'disabled'
#   toolu_config_exists        - 0 if any config file is on disk (cheap stat-only check)
#
# Defaults: missing key = enabled. Malformed JSON or missing jq = all enabled
# with a single stderr warning.

# Merged config is held in memory, not a temp file: hooks are short-lived
# processes, and a per-PID cache file under $TMPDIR was never cleaned up —
# one leaked file per hook invocation. $(...) subshells inherit the variable
# exactly like they inherited the file path, so load-once semantics hold.
TOOLU_CFG_JSON='{}'
TOOLU_CFG_LOADED=0
_TOOLU_HAS_JQ=""

_toolu_warn() {
  printf 'toolu-config: %s\n' "$1" >&2
}

_toolu_agent_dir() {
  if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
    printf '%s' "$TOOLU_CONFIG_DIR"
  elif [ -n "${CLAUDE_CONFIG_DIR:-}" ]; then
    printf '%s' "$CLAUDE_CONFIG_DIR"
  elif [ -n "${PI_CODING_AGENT_DIR:-}" ]; then
    printf '%s' "$PI_CODING_AGENT_DIR"
  else
    printf '%s/.claude' "$HOME"
  fi
}

_toolu_project_cfg_dirname() {
  printf '%s' "${TOOLU_PROJECT_CONFIG_DIRNAME:-.claude}"
}

_toolu_user_cfg() {
  local agent_dir
  agent_dir=$(_toolu_agent_dir)
  if [ "$agent_dir" = "$HOME/.claude" ]; then
    printf '%s/.claude/toolu.config.json' "$HOME"
  else
    printf '%s/toolu.config.json' "$agent_dir"
  fi
}

# Mirrored by the setup_done marker writer in plugins/comemory/scripts/setup.sh
# (cross-plugin sourcing is barred; parity enforced by path-parity.bats).
_toolu_project_cfg() {
  local root="${TOOLU_PROJECT_DIR:-${CLAUDE_PROJECT_DIR:-}}"
  [ -z "$root" ] && root=$(git rev-parse --show-toplevel 2>/dev/null || true)
  [ -z "$root" ] && return 0
  printf '%s/%s/toolu.config.json' "$root" "$(_toolu_project_cfg_dirname)"
}

toolu_load_config() {
  [ "$TOOLU_CFG_LOADED" = "1" ] && return 0
  TOOLU_CFG_LOADED=1

  if command -v jq >/dev/null 2>&1; then
    _TOOLU_HAS_JQ=1
  else
    _TOOLU_HAS_JQ=0
    _toolu_warn "jq missing; all components enabled"
    TOOLU_CFG_JSON='{}'
    return 0
  fi

  local user_cfg project_cfg user_json='{}' project_json='{}'
  user_cfg=$(_toolu_user_cfg)
  project_cfg=$(_toolu_project_cfg)

  if [ -f "$user_cfg" ]; then
    if ! user_json=$(jq -e . "$user_cfg" 2>/dev/null); then
      _toolu_warn "malformed JSON in $user_cfg; ignoring"
      user_json='{}'
    fi
  fi

  if [ -n "$project_cfg" ] && [ -f "$project_cfg" ]; then
    if ! project_json=$(jq -e . "$project_cfg" 2>/dev/null); then
      _toolu_warn "malformed JSON in $project_cfg; ignoring"
      project_json='{}'
    fi
  fi

  TOOLU_CFG_JSON=$(jq -cn --argjson u "$user_json" --argjson p "$project_json" \
    '$u * $p' 2>/dev/null) || TOOLU_CFG_JSON='{}'
}

toolu_enabled() {
  local category="$1" name="$2"
  toolu_load_config
  [ "$_TOOLU_HAS_JQ" = "1" ] || return 0
  local val
  val=$(jq -r --arg c "$category" --arg n "$name" \
    'if (.[$c]? // {}) | has($n) then .[$c][$n] | tostring else "missing" end' \
    <<< "$TOOLU_CFG_JSON" 2>/dev/null)
  [ "$val" = "false" ] && return 1
  return 0
}

# Default-OFF inverse of toolu_enabled: 0 only when .<cat>.<name> is boolean
# `true`; absent/false/non-true/malformed/no-jq -> 1.
toolu_flag_true() {
  local category="$1" name="$2"
  toolu_load_config
  [ "$_TOOLU_HAS_JQ" = "1" ] || return 1
  local val
  val=$(jq -r --arg c "$category" --arg n "$name" \
    'if (.[$c]? // {}) | has($n) then (.[$c][$n] == true) else false end' \
    <<< "$TOOLU_CFG_JSON" 2>/dev/null)
  [ "$val" = "true" ] && return 0
  return 1
}

# Like toolu_enabled but DEFAULT OFF: returns 0 only when the key is
# explicitly `true`. For opt-in components (e.g. the session-end comemory
# reminder) where the default-enabled opt-out semantics are wrong. Missing jq or
# a missing/non-true value -> 1 (disabled).
toolu_enabled_explicit() {
  local category="$1" name="$2"
  toolu_load_config
  [ "$_TOOLU_HAS_JQ" = "1" ] || return 1
  local val
  val=$(jq -r --arg c "$category" --arg n "$name" \
    '(.[$c]? // {})[$n]? | tostring' <<< "$TOOLU_CFG_JSON" 2>/dev/null)
  [ "$val" = "true" ] && return 0
  return 1
}

# Return 0 if any config file is on disk. Stat-only; no jq, no load. Used by
# hot-path hooks (e.g. mcp-blocker) to skip config-load entirely when no
# user has opted in.
toolu_config_exists() {
  local user_cfg project_cfg
  user_cfg=$(_toolu_user_cfg)
  [ -f "$user_cfg" ] && return 0
  project_cfg=$(_toolu_project_cfg)
  [ -n "$project_cfg" ] && [ -f "$project_cfg" ] && return 0
  return 1
}

# Print comemory availability for hook reminder text:
#   'available' — CLI installed AND skills.comemory != false
#   'missing'   — CLI absent AND skills.comemory != false (emit install hint)
#   'disabled'  — skills.comemory == false (silent; user opted out)
#
# Centralizes the tri-state so pre-compact / session-end / user-prompt-submit
# do not each hand-roll the branching.
toolu_comemory_state() {
  if ! toolu_enabled skills comemory; then
    printf 'disabled'
    return 0
  fi
  if command -v comemory >/dev/null 2>&1; then
    printf 'available'
  else
    printf 'missing'
  fi
}

