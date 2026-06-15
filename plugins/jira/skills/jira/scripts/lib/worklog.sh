#!/usr/bin/env bash
# worklog.sh — Jira worklog operations: add, list, delete. Sourced; depends on
# http.sh (jira_curl/jira_lean) and adf.sh (jira_text_to_adf). The add comment
# body is version-gated (ADF for v3, plain text for v2).

_jira_worklog_path() { printf '/rest/api/%s/issue' "${_JIRA_VER:-3}"; }

# jira_worklog <add|list|delete> ...: dispatch on the first arg.
jira_worklog() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    add)    _jira_worklog_add "$@";;
    list)   _jira_worklog_list "$@";;
    delete) _jira_worklog_delete "$@";;
    *) echo "Usage: jira worklog <add|list|delete> ..." >&2; return 1;;
  esac
}

# _jira_worklog_add <KEY> -t <TIME> [-c COMMENT]: log time against an issue.
_jira_worklog_add() {
  local key="${1:-}"; [[ $# -gt 0 ]] && shift
  local time="" comment="" has_comment=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -t|--time)    time="$2"; shift 2;;
      -c|--comment) comment="$2"; has_comment=1; shift 2;;
      *) echo "worklog add: unknown option '$1'" >&2; return 1;;
    esac
  done
  [[ -z "$key" || -z "$time" ]] && {
    echo "Usage: jira worklog add <KEY> -t <TIME> [-c COMMENT]" >&2
    return 1
  }
  local body
  body=$(jq -nc --arg ts "$time" '{timeSpent:$ts}')
  if [[ -n "$has_comment" ]]; then
    body=$(jq -c --argjson c "$(jira_text_to_adf "$comment")" '.comment=$c' <<<"$body")
  fi
  jira_curl POST "$(_jira_worklog_path)/$key/worklog" "$body" | jira_lean '.'
}

# _jira_worklog_list <KEY>: list worklog entries for an issue.
_jira_worklog_list() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira worklog list <KEY>" >&2; return 1; }
  jira_curl GET "$(_jira_worklog_path)/$key/worklog" \
    | jira_lean '{worklogs:[.worklogs[]|{id, author:.author.displayName, timeSpent, started}]}'
}

# _jira_worklog_delete <KEY> <WORKLOG_ID>: remove one worklog entry.
_jira_worklog_delete() {
  local key="${1:-}" wid="${2:-}"
  [[ -z "$key" || -z "$wid" ]] && { echo "Usage: jira worklog delete <KEY> <WORKLOG_ID>" >&2; return 1; }
  jira_curl DELETE "$(_jira_worklog_path)/$key/worklog/$wid"
  echo "deleted worklog $wid"
}
