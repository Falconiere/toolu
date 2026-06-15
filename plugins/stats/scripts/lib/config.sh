#!/usr/bin/env bash
# config.sh — load optional user settings for the stats report from a flat
# key=value file at ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/stats.conf.
#
# Only a fixed whitelist of keys is honored; unknown keys and comment (#) lines
# are ignored, malformed lines are skipped. Parsing is pure parameter expansion
# with NO eval and no command substitution, so a value like `$(rm -rf x)` is
# stored as the literal string and never executed. A key is applied only when
# its STATS_* variable is unset, so an already-exported env var or a CLI flag
# (parsed after this runs) always wins over the file.
#
# Keys: plan (pro|max5|max20), billing (api|subscription|both),
# weekly_limit_tokens, window_limit_tokens.
set -u

# stats_load_config -> exports STATS_PLAN / STATS_BILLING / STATS_WEEKLY_LIMIT /
# STATS_WINDOW_LIMIT from the config file for any not already set. No-op when the
# file is absent. Always returns 0 (a bad config must never break the report).
stats_load_config() {
  local cfg="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/stats.conf" line key val
  [ -f "$cfg" ] || return 0
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%%#*}"                              # drop trailing comment
    case "$line" in *=*) : ;; *) continue ;; esac    # require key=value
    key="${line%%=*}"; val="${line#*=}"
    key="${key//[[:space:]]/}"                       # keys carry no spaces
    val="${val#"${val%%[![:space:]]*}"}"             # ltrim value
    val="${val%"${val##*[![:space:]]}"}"             # rtrim value
    case "$key" in
      plan)                [ -n "${STATS_PLAN:-}" ]         || export STATS_PLAN="$val" ;;
      billing)             [ -n "${STATS_BILLING:-}" ]      || export STATS_BILLING="$val" ;;
      weekly_limit_tokens) [ -n "${STATS_WEEKLY_LIMIT:-}" ] || export STATS_WEEKLY_LIMIT="$val" ;;
      window_limit_tokens) [ -n "${STATS_WINDOW_LIMIT:-}" ] || export STATS_WINDOW_LIMIT="$val" ;;
      *) : ;;                                         # ignore unknown keys
    esac
  done < "$cfg"
  return 0
}
