#!/usr/bin/env bash
# user.sh — Jira user operations: whoami, search, get. Sourced; depends on
# http.sh (jira_curl/jira_lean). v3 keys users by accountId, v2 by username;
# the user search/lookup query param is version-gated accordingly.

jira_user() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    whoami) _jira_user_whoami "$@";;
    search) _jira_user_search "$@";;
    get)    _jira_user_get "$@";;
    *) echo "Usage: jira user <whoami|search|get> ..." >&2; return 1;;
  esac
}

# whoami: GET the current user, leaned to identity essentials.
_jira_user_whoami() {
  jira_curl GET "/rest/api/${_JIRA_VER:-3}/myself" \
    | jira_lean '{accountId, displayName, emailAddress}'
}

# search -q <QUERY>: find users; v3 keys on ?query, v2 on ?username.
_jira_user_search() {
  local query=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -q|--query) query="$2"; shift 2;;
      *) echo "user search: unknown option '$1'" >&2; return 1;;
    esac
  done
  [[ -z "$query" ]] && { echo "Usage: jira user search -q <QUERY>" >&2; return 1; }
  local param="query"
  [[ "${_JIRA_VER:-3}" == "2" ]] && param="username"
  jira_curl GET "/rest/api/${_JIRA_VER:-3}/user/search?${param}=$(jira_urlencode "$query")" \
    | jira_lean '{users:[.[]|{accountId, displayName}]}'
}

# get <ACCOUNT_ID>: GET one user; v3 keys on ?accountId, v2 on ?username.
_jira_user_get() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira user get <ACCOUNT_ID>" >&2; return 1; }
  local param="accountId"
  [[ "${_JIRA_VER:-3}" == "2" ]] && param="username"
  jira_curl GET "/rest/api/${_JIRA_VER:-3}/user?${param}=$(jira_urlencode "$id")" | jira_lean '.'
}
