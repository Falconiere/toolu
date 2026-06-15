#!/usr/bin/env bash
# raw.sh — escape hatch: issue an arbitrary authenticated request to any Jira
# REST path, reaching endpoints the ergonomic families do not wrap. Sourced;
# depends on jira_curl/jira_lean from http.sh.

# jira_raw METHOD PATH [JSON_BODY]: METHOD in GET|POST|PUT|DELETE; PATH is
# relative to JIRA_BASE_URL. Prints the response body.
jira_raw() {
  if [[ $# -lt 2 ]]; then
    echo "Usage: jira raw <GET|POST|PUT|DELETE> <path> [json_body]" >&2
    return 1
  fi
  local method="$1" path="$2" body="${3:-}"
  jira_curl "$method" "$path" "$body" | jira_lean '.'
}
