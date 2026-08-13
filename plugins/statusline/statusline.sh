#!/usr/bin/env bash
# Statusline — the toolu statusline.
# Reads the Claude Code statusline JSON on stdin and prints a single status line:
#   model | effort | ctx | <email domain> | <gate> | folder | branch [↑↓] [dirty] | <comemory> | <caveman>
# The signature segment is the quality-gate marker: when this project's
# PostToolUse gate is failing, it shows a loud red marker so you can't miss it.
# (Lights up only when a gate writer — e.g. rust-quality/ts-quality/toolu — is present.)
#
# Wire it up (settings.json) after the SessionStart hook has symlinked it to a
# stable path:
#   "statusLine": { "type": "command",
#                   "command": "bash ~/.claude/statusline/statusline.sh" }
#
# Every field is read defensively — the schema marks effort/used_percentage/etc.
# as absent before the first API call, after /compact, or on models that lack
# them. Missing jq degrades to a minimal line rather than erroring.

input=$(cat 2>/dev/null || echo '{}')

# --- Colors (ANSI) ---
CYAN=$'\033[36m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'
MAGENTA=$'\033[35m'; BLUE=$'\033[34m'; RED=$'\033[31m'
DIM=$'\033[2m'; BOLD=$'\033[1m'; RESET=$'\033[0m'

# Without jq we cannot parse the payload — emit nothing rather than garbage.
command -v jq >/dev/null 2>&1 || { printf 'toolu'; exit 0; }

# One jq pass extracts every field — a statusline renders on every prompt, so
# six separate jq spawns would be wasteful. One value per line (not @tsv: tab is
# IFS-whitespace, so `read` would collapse empty fields and shift everything);
# the per-field `read` preserves empty lines. bash-3.2-safe (no mapfile).
{
  IFS= read -r model
  IFS= read -r effort
  IFS= read -r cwd
  IFS= read -r ctx_size
  IFS= read -r ctx_used
  IFS= read -r ctx_pct
} < <(printf '%s' "$input" | jq -r '
  (.model.display_name // "Claude"),
  (.effort.level // ""),
  (.workspace.current_dir // .cwd // ""),
  (.context_window.context_window_size // 0),
  (.context_window.total_input_tokens // 0),
  (.context_window.used_percentage // "")' 2>/dev/null)
[ -n "$model" ] || model="Claude"
[ -n "$ctx_size" ] || ctx_size=0
[ -n "$ctx_used" ] || ctx_used=0

# Resolve the real plugin directory even when this renderer was invoked through
# the stable SessionStart symlink. Shared project status lives in the collector;
# this file only adds Claude's transient model/context/account presentation.
_self="${BASH_SOURCE[0]}"
case "$_self" in */*) ;; *) _self="./$_self" ;; esac
_hops=0
while [ -L "$_self" ] && [ "$_hops" -lt 40 ]; do
  _hops=$((_hops + 1))
  _link=$(readlink "$_self")
  case "$_link" in
    /*) _self="$_link" ;;
    *) _self="${_self%/*}/$_link" ;;
  esac
done
_plugin_dir=$(cd "${_self%/*}" 2>/dev/null && pwd) || _plugin_dir=""
_collector="${_plugin_dir:+$_plugin_dir/}scripts/collect-status.sh"
if [ -n "$cwd" ] && [ -f "$_collector" ]; then
  project_status=$(bash "$_collector" "$cwd" 2>/dev/null || true)
else
  project_status=""
fi
if ! jq -e 'type == "object"' <<<"$project_status" >/dev/null 2>&1; then
  project_status='{"repo_root":"","folder":"","branch":"","ahead":0,"behind":0,"working_tree":{"staged":0,"unstaged":0,"untracked":0},"gate":{"status":"","reason":""},"comemory_count":null}'
fi

format_tokens() {
  local n="$1"
  # Guard non-numeric input (a future schema change could emit a string) so the
  # arithmetic and printf below stay safe on the per-render hot path.
  [[ "$n" =~ ^[0-9]+$ ]] || { printf '0'; return; }
  if [ "$n" -ge 1000000 ]; then
    # M-tier with one decimal, integer-only (no float arithmetic in bash): 13779513 -> 13.7M
    printf '%d.%dM' "$(( n / 1000000 ))" "$(( (n % 1000000) / 100000 ))"
  elif [ "$n" -ge 1000 ]; then printf '%dk' "$(( n / 1000 ))"; else printf '%d' "$n"; fi
}
ctx_used_fmt=$(format_tokens "$ctx_used")
ctx_size_fmt=$(format_tokens "$ctx_size")
if [[ "$ctx_pct" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
  tokens_seg="${ctx_used_fmt}/${ctx_size_fmt} ($(printf '%.0f%%' "$ctx_pct"))"
else
  tokens_seg="${ctx_used_fmt}/${ctx_size_fmt}"
fi

# --- Account (Claude login email domain) ---
# Reads the OAuth account email Claude Code itself stores in ~/.claude.json
# (or $CLAUDE_CONFIG_DIR/.claude.json when that's set — no fallback to $HOME
# in that case, so a custom config dir never leaks another account's email).
# Only the domain is shown: enough to tell accounts apart across sessions
# without printing the full address to the terminal.
account_seg=""
if [ -n "${CLAUDE_CONFIG_DIR:-}" ]; then
  _acct_file="${CLAUDE_CONFIG_DIR}/.claude.json"
else
  _acct_file="$HOME/.claude.json"
fi
if [ -f "$_acct_file" ]; then
  _acct_email=$(jq -r '.oauthAccount.emailAddress // empty' "$_acct_file" 2>/dev/null)
  [[ "$_acct_email" == *@* ]] && account_seg="${GREEN}${_acct_email#*@}${RESET}"
fi

# --- Quality gate (toolu): red marker only when failing ---
# Resolve the gate file at the git root (where the rust-quality / ts-quality hooks write it
# via $PROJECT_ROOT), not at $cwd — a subdir-launched session or worktree has
# cwd != project root, which would silently miss the marker.
gate_seg=""
gate_status=$(jq -r '.gate.status // ""' <<<"$project_status")
[ "$gate_status" = "failing" ] && gate_seg="${BOLD}${RED}✗ gate:failing${RESET}"

# --- Git branch + folder + working tree status + ahead/behind ---
branch=$(jq -r '.branch // ""' <<<"$project_status")
folder=$(jq -r '.folder // ""' <<<"$project_status")
_repo_root=$(jq -r '.repo_root // ""' <<<"$project_status")
_ahead=$(jq -r '.ahead // 0' <<<"$project_status")
_behind=$(jq -r '.behind // 0' <<<"$project_status")
_staged=$(jq -r '.working_tree.staged // 0' <<<"$project_status")
_unstaged=$(jq -r '.working_tree.unstaged // 0' <<<"$project_status")
_untracked=$(jq -r '.working_tree.untracked // 0' <<<"$project_status")
_first=true
[ -z "$_repo_root" ] || _first=false
_ab_txt=""
[ "$_ahead" -gt 0 ] && _ab_txt="${_ab_txt}↑${_ahead}"
[ "$_behind" -gt 0 ] && _ab_txt="${_ab_txt}↓${_behind}"
_ab_seg=""
[ -n "$_ab_txt" ] && _ab_seg="${DIM}${_ab_txt}${RESET}"
_parts=""
[ "$_staged" -gt 0 ] && _parts="${_parts}+${_staged} "
[ "$_unstaged" -gt 0 ] && _parts="${_parts}~${_unstaged} "
[ "$_untracked" -gt 0 ] && _parts="${_parts}?${_untracked} "
_parts="${_parts%" "}"
_git_seg=""
[ -n "$_parts" ] && _git_seg="${YELLOW}[${_parts}]${RESET}"

# --- Caveman mode (lights up when the caveman plugin is installed) ---
# Read the flag file written by caveman-activate; refuse symlinks, cap the read,
# strip to a safe charset, whitelist known modes.
caveman_seg=""
caveman_flag="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/.caveman-active"
if [ -f "$caveman_flag" ] && [ ! -L "$caveman_flag" ]; then
  caveman_mode=$(head -c 64 "$caveman_flag" 2>/dev/null | tr -d '\n\r' | tr '[:upper:]' '[:lower:]' | tr -cd 'a-z0-9-')
  case "$caveman_mode" in
    off) ;;
    "") caveman_seg="${BOLD}${GREEN}[CAVEMAN]${RESET}" ;;
    full|lite|ultra|wenyan-lite|wenyan|wenyan-full|wenyan-ultra|commit|review|compress)
      caveman_seg="${BOLD}${GREEN}[CAVEMAN:$(printf '%s' "$caveman_mode" | tr '[:lower:]' '[:upper:]')]${RESET}" ;;
  esac
fi

# --- Comemory memory count ([COMEMORY:N]): per-project, main-repo scoped ---
# Read the marker written by comemory's comemory-status SessionStart hook.
# The key derivation MUST match that hook (git-common-dir → main-repo basename)
# so a worktree resolves to the same scope as its main checkout.
comemory_seg=""
_cn=$(jq -r '.comemory_count // empty' <<<"$project_status")
[ -n "$_cn" ] && comemory_seg="${BOLD}${GREEN}[COMEMORY:${_cn}]${RESET}"

# --- Assemble ---
sep="${DIM} | ${RESET}"
line="${CYAN}${model}${RESET}"
[ -n "$effort" ] && [ "$effort" != "null" ] && line="${line}${sep}${YELLOW}effort:${effort}${RESET}"
line="${line}${sep}${MAGENTA}ctx:${tokens_seg}${RESET}"
[ -n "$account_seg" ] && line="${line}${sep}${account_seg}"
[ -n "$gate_seg" ] && line="${line}${sep}${gate_seg}"
[ -n "$folder" ] && line="${line}${sep}${BOLD}${folder}${RESET}"
[ -n "$branch" ] && line="${line}${sep}${BLUE}${branch}${RESET}"
if [ "$_first" = false ]; then
  [ -n "$_ab_seg" ] && line="${line}${_ab_seg}"
  [ -n "$_git_seg" ] && line="${line}${_git_seg}"
fi
[ -n "$comemory_seg" ] && line="${line}${sep}${comemory_seg}"
[ -n "$caveman_seg" ] && line="${line}${sep}${caveman_seg}"

printf '%s' "$line"
