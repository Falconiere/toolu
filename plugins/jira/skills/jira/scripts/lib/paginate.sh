#!/usr/bin/env bash
# paginate.sh — follow Jira's two pagination models and concatenate items.
#   token  : v3 /search/jql — nextPageToken, no total; POST, carry the token.
#   offset : v2 /search and all Agile endpoints — startAt/maxResults/total.
# Emits a JSON array of every item under ITEMS_KEY across all pages. Sourced;
# depends on jira_curl from http.sh.

# jira_paginate MODE METHOD PATH ITEMS_KEY [BODY]
#   MODE = token | offset ; ITEMS_KEY = issues | values ; BODY = JSON (default {})
jira_paginate() {
  local mode="$1" method="$2" path="$3" items="$4" body="${5:-}"
  [[ -z "$body" ]] && body='{}'
  local acc='[]' page req url max token="" start=0 total got last guard=0
  max=$(jq -r '.maxResults // 50' <<<"$body")

  while (( guard++ < 1000 )); do
    req="$body"; url="$path"
    if [[ "$mode" == "token" ]]; then
      [[ -n "$token" ]] && req=$(jq -c --arg t "$token" '. + {nextPageToken:$t}' <<<"$body")
      page=$(jira_curl "$method" "$url" "$req") || return 1
    elif [[ "$method" == "GET" ]]; then
      local sep="?"; [[ "$url" == *"?"* ]] && sep="&"
      url="${url}${sep}startAt=${start}&maxResults=${max}"
      page=$(jira_curl GET "$url") || return 1
    else
      req=$(jq -c --argjson s "$start" --argjson m "$max" '. + {startAt:$s, maxResults:$m}' <<<"$body")
      page=$(jira_curl "$method" "$url" "$req") || return 1
    fi

    acc=$(jq -c --argjson acc "$acc" --arg k "$items" '$acc + (.[$k] // [])' <<<"$page")

    if [[ "$mode" == "token" ]]; then
      token=$(jq -r '.nextPageToken // empty' <<<"$page")
      last=$(jq -r '.isLast // false' <<<"$page")
      [[ "$last" == "true" || -z "$token" ]] && break
    else
      total=$(jq -r '.total // 0' <<<"$page")
      got=$(jq -r --arg k "$items" '(.[$k] // []) | length' <<<"$page")
      (( got == 0 )) && break
      start=$(( start + got ))
      (( start >= total )) && break
    fi
  done
  printf '%s\n' "$acc"
}
