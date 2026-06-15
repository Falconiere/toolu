#!/usr/bin/env bash
# board.sh — Jira Agile board operations: list boards, fetch one, list a board's
# issues. Sourced; depends on http.sh (jira_curl/jira_lean). Agile endpoints are
# always under /rest/agile/1.0 regardless of the configured REST API version.

# jira_board <list|get|issues> ...: dispatch on the first arg (sub-action).
jira_board() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    list)   _jira_board_list "$@";;
    get)    _jira_board_get "$@";;
    issues) _jira_board_issues "$@";;
    *) echo "Usage: jira board <list|get|issues> ..." >&2; return 1;;
  esac
}

# _jira_board_list [-p PROJECT]: GET /rest/agile/1.0/board, optionally scoped to
# a project key/id via ?projectKeyOrId=PROJECT.
_jira_board_list() {
  local project=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -p|--project) project="$2"; shift 2;;
      *) echo "board list: unknown option '$1'" >&2; return 1;;
    esac
  done
  local path="/rest/agile/1.0/board"
  [[ -n "$project" ]] && path="${path}?projectKeyOrId=${project}"
  jira_curl GET "$path" | jira_lean '{values: [.values[] | {id, name, type}]}'
}

# _jira_board_get <ID>: GET /rest/agile/1.0/board/<ID>.
_jira_board_get() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira board get <ID>" >&2; return 1; }
  jira_curl GET "/rest/agile/1.0/board/$id" | jira_lean '.'
}

# _jira_board_issues <ID> [-q JQL] [-n MAX]: GET /rest/agile/1.0/board/<ID>/issue,
# appending ?jql=...&maxResults=... when those filters are supplied.
_jira_board_issues() {
  local id="${1:-}"; [[ $# -gt 0 ]] && shift
  [[ -z "$id" ]] && { echo "Usage: jira board issues <ID> [-q JQL] [-n MAX]" >&2; return 1; }
  local jql="" max=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -q|--query) jql="$2"; shift 2;;
      -n|--num)   max="$2"; shift 2;;
      *) echo "board issues: unknown option '$1'" >&2; return 1;;
    esac
  done
  local path="/rest/agile/1.0/board/$id/issue" query=""
  [[ -n "$jql" ]] && query="${query}&jql=$(jira_urlencode "$jql")"
  [[ -n "$max" ]] && query="${query}&maxResults=${max}"
  [[ -n "$query" ]] && path="${path}?${query#&}"
  jira_curl GET "$path" \
    | jira_lean '{issues: [.issues[] | {key, summary: .fields.summary, status: .fields.status.name}]}'
}
