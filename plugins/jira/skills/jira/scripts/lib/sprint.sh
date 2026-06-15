#!/usr/bin/env bash
# sprint.sh — Jira Agile sprint operations: list (per board), get, issues,
# create, move issues, start, complete. Sourced; depends on http.sh
# (jira_curl/jira_lean). Agile endpoints always use /rest/agile/1.0 regardless
# of API version; start/complete mutate the live sprint state.

# jira_sprint <list|get|issues|create|move|start|complete> ...: dispatch a
# sprint sub-action; unknown/missing prints usage to stderr and returns 1.
jira_sprint() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    list)     _jira_sprint_list "$@";;
    get)      _jira_sprint_get "$@";;
    issues)   _jira_sprint_issues "$@";;
    create)   _jira_sprint_create "$@";;
    move)     _jira_sprint_move "$@";;
    start)    _jira_sprint_start "$@";;
    complete) _jira_sprint_complete "$@";;
    *) echo "Usage: jira sprint <list|get|issues|create|move|start|complete> ..." >&2; return 1;;
  esac
}

# _jira_sprint_list BOARD_ID: list the sprints on a board.
_jira_sprint_list() {
  local board="${1:-}"
  [[ -z "$board" ]] && { echo "Usage: jira sprint list <BOARD_ID>" >&2; return 1; }
  jira_curl GET "/rest/agile/1.0/board/$board/sprint" \
    | jira_lean '{values:[.values[]|{id,name,state}]}'
}

# _jira_sprint_get ID: fetch one sprint.
_jira_sprint_get() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira sprint get <ID>" >&2; return 1; }
  jira_curl GET "/rest/agile/1.0/sprint/$id" | jira_lean '.'
}

# _jira_sprint_issues ID: list the issues in a sprint.
_jira_sprint_issues() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira sprint issues <ID>" >&2; return 1; }
  jira_curl GET "/rest/agile/1.0/sprint/$id/issue" \
    | jira_lean '{issues:[.issues[]|{key, summary:.fields.summary, status:.fields.status.name}]}'
}

# _jira_sprint_create BOARD_ID -n NAME: create a sprint on a board.
_jira_sprint_create() {
  local board="${1:-}"; [[ $# -gt 0 ]] && shift
  local name=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -n|--name) name="$2"; shift 2;;
      *) echo "sprint create: unknown option '$1'" >&2; return 1;;
    esac
  done
  [[ -z "$board" || -z "$name" ]] && {
    echo "Usage: jira sprint create <BOARD_ID> -n <NAME>" >&2
    return 1
  }
  local body
  body=$(jq -nc --argjson b "$board" --arg n "$name" '{originBoardId:$b, name:$n}')
  jira_curl POST "/rest/agile/1.0/sprint" "$body" | jira_lean '.'
}

# _jira_sprint_move SPRINT_ID KEY...: move one or more issues into a sprint.
_jira_sprint_move() {
  local id="${1:-}"; [[ $# -gt 0 ]] && shift
  [[ -z "$id" || $# -eq 0 ]] && { echo "Usage: jira sprint move <SPRINT_ID> <KEY...>" >&2; return 1; }
  local body
  body=$(jq -nc '{issues:$ARGS.positional}' --args "$@")
  jira_curl POST "/rest/agile/1.0/sprint/$id/issue" "$body" | jira_lean '.'
}

# _jira_sprint_start ID: start a sprint (set state active).
_jira_sprint_start() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira sprint start <ID>" >&2; return 1; }
  jira_curl POST "/rest/agile/1.0/sprint/$id" '{"state":"active"}'
  echo "started sprint $id"
}

# _jira_sprint_complete ID: complete a sprint (set state closed).
_jira_sprint_complete() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira sprint complete <ID>" >&2; return 1; }
  jira_curl POST "/rest/agile/1.0/sprint/$id" '{"state":"closed"}'
  echo "completed sprint $id"
}
