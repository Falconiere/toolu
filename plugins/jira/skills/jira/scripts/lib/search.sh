#!/usr/bin/env bash
# search.sh — JQL search. v3 (Cloud) POSTs to /rest/api/3/search/jql with
# nextPageToken paging; v2 (Server/DC) POSTs to /rest/api/2/search with startAt
# paging. Sourced; depends on http.sh and paginate.sh.

# jira_search -q <JQL> [-n MAX] [--all] [--fields f1,f2]
jira_search() {
  local jql="" max=50 all="false" fields=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -q|--query)  jql="$2"; shift 2;;
      -n|--num)    max="$2"; shift 2;;
      --all)       all="true"; shift;;
      --fields)    fields="$2"; shift 2;;
      *) if [[ -z "$jql" ]]; then jql="$1"; shift
         else echo "search: unknown option '$1'" >&2; return 1; fi;;
    esac
  done
  [[ -z "$jql" ]] && { echo "Usage: jira search -q <JQL> [-n MAX] [--all] [--fields f1,f2]" >&2; return 1; }

  local fields_json="null"
  [[ -n "$fields" ]] && fields_json=$(jq -nc --arg f "$fields" '$f|split(",")')

  local body
  body=$(jq -nc --arg q "$jql" --argjson n "$max" --argjson f "$fields_json" \
    '{jql:$q, maxResults:$n} | if $f then .fields=$f else . end')

  local path mode
  if [[ "${_JIRA_VER:-3}" == "2" ]]; then
    path="/rest/api/2/search"; mode="offset"
  else
    path="/rest/api/3/search/jql"; mode="token"
  fi

  if [[ "$all" == "true" ]]; then
    jira_paginate "$mode" POST "$path" issues "$body" | jira_lean '.'
  else
    jira_curl POST "$path" "$body" \
      | jira_lean '{issues: [.issues[] | {key, summary: .fields.summary, status: .fields.status.name}]}'
  fi
}
