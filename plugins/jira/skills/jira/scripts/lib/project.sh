#!/usr/bin/env bash
# project.sh — Jira project operations: list, get, versions, components. Sourced;
# depends on http.sh (jira_curl/jira_lean). All actions are read-only GETs.

_jira_project_path() { printf '/rest/api/%s/project' "${_JIRA_VER:-3}"; }

jira_project() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    list)       _jira_project_list "$@";;
    get)        _jira_project_get "$@";;
    versions)   _jira_project_versions "$@";;
    components) _jira_project_components "$@";;
    *) echo "Usage: jira project <list|get|versions|components> ..." >&2; return 1;;
  esac
}

# list: GET the project collection (a top-level array), leaned to key/name/id.
_jira_project_list() {
  jira_curl GET "$(_jira_project_path)" | jira_lean '{projects:[.[]|{key, name, id}]}'
}

# get <KEY>: GET one project's full detail.
_jira_project_get() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira project get <KEY>" >&2; return 1; }
  jira_curl GET "$(_jira_project_path)/$key" | jira_lean '.'
}

# versions <KEY>: GET the project's released/unreleased versions.
_jira_project_versions() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira project versions <KEY>" >&2; return 1; }
  jira_curl GET "$(_jira_project_path)/$key/versions" | jira_lean '.'
}

# components <KEY>: GET the project's components.
_jira_project_components() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira project components <KEY>" >&2; return 1; }
  jira_curl GET "$(_jira_project_path)/$key/components" | jira_lean '.'
}
