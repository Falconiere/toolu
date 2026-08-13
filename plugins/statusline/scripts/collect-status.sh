#!/usr/bin/env bash
# Collect host-neutral project status as JSON for renderers and explicit skills.
set -euo pipefail

command -v jq >/dev/null 2>&1 || {
  printf '%s\n' 'collect-status: jq is required' >&2
  exit 1
}

cwd="${1:-$PWD}"
if [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || {
  [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]
}; then
  host=codex
  config_root="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}"
  project_state=.codex
else
  host=claude
  config_root="${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}"
  project_state=.claude
fi

repo_root=""
branch=""
folder=""
ahead=0
behind=0
staged=0
unstaged=0
untracked=0
if [ -d "$cwd" ]; then
  folder=$(basename "$cwd")
  repo_root=$(git -C "$cwd" --no-optional-locks rev-parse --show-toplevel 2>/dev/null || true)
fi

if [ -n "$repo_root" ]; then
  branch=$(git -C "$cwd" --no-optional-locks symbolic-ref --short HEAD 2>/dev/null || true)
  first=1
  while IFS= read -r line; do
    if [ "$first" -eq 1 ]; then
      first=0
      case "$line" in
        "## "*)
          rest="${line:3}"
          case "$rest" in
            *ahead*)
              value="${rest#*ahead }"
              ahead="${value%%,*}"
              ahead="${ahead%% *}"
              ahead="${ahead%]}"
              ;;
          esac
          case "$rest" in
            *behind*)
              value="${rest#*behind }"
              behind="${value%%,*}"
              behind="${behind%% *}"
              behind="${behind%]}"
              ;;
          esac
          ;;
      esac
      continue
    fi
    [ "${#line}" -ge 2 ] || continue
    case "${line:0:2}" in
      "??") untracked=$((untracked + 1)) ;;
      "!!") ;;
      *)
        [ "${line:0:1}" = " " ] || staged=$((staged + 1))
        [ "${line:1:1}" = " " ] || unstaged=$((unstaged + 1))
        ;;
    esac
  done < <(git -C "$cwd" --no-optional-locks status --porcelain --branch 2>/dev/null || true)
fi

gate_status=""
gate_reason=""
gate_file="${repo_root:-$cwd}/$project_state/tmp/quality-gate-status.json"
if [ -f "$gate_file" ]; then
  gate_status=$(jq -r '.status | strings // empty' "$gate_file" 2>/dev/null || true)
  gate_reason=$(jq -r '.reason | strings // empty' "$gate_file" 2>/dev/null || true)
fi

comemory_count=""
if [ -n "$repo_root" ]; then
  common_dir=$(git -C "$cwd" --no-optional-locks rev-parse --git-common-dir 2>/dev/null || true)
  if [ -n "$common_dir" ]; then
    case "$common_dir" in
      /*) ;;
      *) common_dir=$(cd "$cwd" 2>/dev/null && cd "$common_dir" 2>/dev/null && pwd) || common_dir="" ;;
    esac
    if [ -n "$common_dir" ]; then
      marker="$config_root/comemory-status/$(basename "$(dirname "$common_dir")").json"
      if [ -f "$marker" ]; then
        comemory_count=$(jq -r '.count | numbers // empty' "$marker" 2>/dev/null || true)
      fi
    fi
  fi
fi

jq -cn \
  --arg host "$host" \
  --arg cwd "$cwd" \
  --arg repo_root "$repo_root" \
  --arg folder "$folder" \
  --arg branch "$branch" \
  --arg gate_status "$gate_status" \
  --arg gate_reason "$gate_reason" \
  --arg comemory_count "$comemory_count" \
  --argjson ahead "${ahead:-0}" \
  --argjson behind "${behind:-0}" \
  --argjson staged "$staged" \
  --argjson unstaged "$unstaged" \
  --argjson untracked "$untracked" \
  '{host:$host,cwd:$cwd,repo_root:$repo_root,folder:$folder,branch:$branch,
    ahead:$ahead,behind:$behind,
    working_tree:{staged:$staged,unstaged:$unstaged,untracked:$untracked},
    gate:{status:$gate_status,reason:$gate_reason},
    comemory_count:(if $comemory_count == "" then null else ($comemory_count | tonumber) end)}'
