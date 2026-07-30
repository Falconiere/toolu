#!/usr/bin/env bash
# Statusline — the toolu statusline.
# Reads the Claude Code statusline JSON on stdin and prints a single status line:
#   model | effort | ctx | <gate> | folder | branch [↑↓] [dirty] | <comemory> | <caveman>
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

# --- Quality gate (toolu): red marker only when failing ---
# Resolve the gate file at the git root (where the rust-quality / ts-quality hooks write it
# via $PROJECT_ROOT), not at $cwd — a subdir-launched session or worktree has
# cwd != project root, which would silently miss the marker.
gate_seg=""
if [ -n "$cwd" ]; then
  _gate_root=$(git -C "$cwd" --no-optional-locks rev-parse --show-toplevel 2>/dev/null)
  gate_file="${_gate_root:-$cwd}/.claude/tmp/quality-gate-status.json"
  if [ -f "$gate_file" ]; then
    gate_status=$(jq -r '.status // ""' "$gate_file" 2>/dev/null)
    [ "$gate_status" = "failing" ] && gate_seg="${BOLD}${RED}✗ gate:failing${RESET}"
  fi
fi

# --- Git branch + folder + working tree status + ahead/behind ---
branch=""; folder=""; _git_seg=""; _ab_seg=""; _first=true
if [ -n "$cwd" ] && [ -d "$cwd" ]; then
  branch=$(git -C "$cwd" --no-optional-locks symbolic-ref --short HEAD 2>/dev/null)
  folder=$(basename "$cwd")
  # One git status call yields file counts + ahead/behind; parse the branch
  # header for ahead/behind, then count staged/unstaged/untracked from the
  # remaining status lines.
  _staged=0; _unstaged=0; _untracked=0; _ahead=0; _behind=0
  while IFS= read -r _line; do
    if [ "$_first" = true ]; then
      _first=false
      case "$_line" in
        "## "*)
          _rest="${_line:3}"
          case "$_rest" in
            *ahead*|*behind*)
              _ab="${_rest#*[[]}"; _ab="${_ab%]}"
              case "$_ab" in
                *ahead*) _ahead_tmp="${_ab#*ahead }"; _ahead="${_ahead_tmp%%,*}"; _ahead="${_ahead%% *}" ;;
              esac
              case "$_ab" in
                *behind*) _behind_tmp="${_ab#*behind }"; _behind="${_behind_tmp%%,*}"; _behind="${_behind%% *}" ;;
              esac
              ;;
          esac
          ;;
      esac
      continue
    fi
    [ ${#_line} -lt 2 ] && continue
    case "${_line:0:2}" in
      "??") _untracked=$(( _untracked + 1 )) ;;
      "!!") ;;
      *)
        [ "${_line:0:1}" != " " ] && _staged=$(( _staged + 1 ))
        [ "${_line:1:1}" != " " ] && _unstaged=$(( _unstaged + 1 ))
        ;;
    esac
  done < <(git -C "$cwd" --no-optional-locks status --porcelain --branch 2>/dev/null)
  # Build ahead/behind segment
  _ab_txt=""
  [ "${_ahead:-0}" -gt 0 ] 2>/dev/null && _ab_txt="${_ab_txt}↑${_ahead}"
  [ "${_behind:-0}" -gt 0 ] 2>/dev/null && _ab_txt="${_ab_txt}↓${_behind}"
  [ -n "$_ab_txt" ] && _ab_seg="${DIM}${_ab_txt}${RESET}"
  # Build dirty-status segment
  _parts=""
  [ "${_staged:-0}" -gt 0 ]   && _parts="${_parts}+${_staged} "
  [ "${_unstaged:-0}" -gt 0 ] && _parts="${_parts}~${_unstaged} "
  [ "${_untracked:-0}" -gt 0 ] && _parts="${_parts}?${_untracked} "
  _parts="${_parts%" "}"
  [ -n "$_parts" ] && _git_seg="${YELLOW}[${_parts}]${RESET}"
fi

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
CFG="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
comemory_seg=""
if [ -n "$cwd" ]; then
  _ck=$(git -C "$cwd" --no-optional-locks rev-parse --git-common-dir 2>/dev/null)
  if [ -n "$_ck" ]; then
    case "$_ck" in
      /*) ;;
      *) _ck=$(cd "$cwd" 2>/dev/null && cd "$_ck" 2>/dev/null && pwd) ;;
    esac
    if [ -n "$_ck" ]; then  # absolutize may fail; never build a key from an empty path
      _cfile="${CFG}/comemory-status/$(basename "$(dirname "$_ck")").json"
      if [ -f "$_cfile" ]; then
        _cn=$(jq -r '.count // empty' "$_cfile" 2>/dev/null)
        [ -n "$_cn" ] && comemory_seg="${BOLD}${GREEN}[COMEMORY:${_cn}]${RESET}"
      fi
    fi
  fi
fi

# --- Assemble ---
sep="${DIM} | ${RESET}"
line="${CYAN}${model}${RESET}"
[ -n "$effort" ] && [ "$effort" != "null" ] && line="${line}${sep}${YELLOW}effort:${effort}${RESET}"
line="${line}${sep}${MAGENTA}ctx:${tokens_seg}${RESET}"
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
