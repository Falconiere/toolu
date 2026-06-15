#!/usr/bin/env bash
# issue.sh — Jira issue operations: get, create, update, delete, comment,
# transition(s), assign. Sourced; depends on http.sh (jira_curl/jira_lean) and
# adf.sh (jira_text_to_adf). Comment/description bodies are version-gated (ADF
# for v3, plain text for v2); assign/transition mutate live tickets.

_jira_issue_path() { printf '/rest/api/%s/issue' "${_JIRA_VER:-3}"; }

# Lean projection for one issue: keep the useful fields, drop null/empty ones.
_JIRA_ISSUE_LEAN='{key,
  summary: .fields.summary, status: .fields.status.name,
  assignee: .fields.assignee.displayName, reporter: .fields.reporter.displayName,
  priority: .fields.priority.name, type: .fields.issuetype.name,
  project: .fields.project.key, created: .fields.created, updated: .fields.updated,
  labels: .fields.labels}
  | with_entries(select(.value != null and .value != [] and .value != ""))'

# _jira_assignee_obj ID: compact JSON for an assignee value; "-" unassigns.
# v3 uses accountId, v2 uses name.
_jira_assignee_obj() {
  local id="$1" field="accountId"
  [[ "${_JIRA_VER:-3}" == "2" ]] && field="name"
  if [[ "$id" == "-" ]]; then
    jq -nc --arg f "$field" '{($f): null}'
  else
    jq -nc --arg f "$field" --arg i "$id" '{($f): $i}'
  fi
}

jira_issue() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    get)         _jira_issue_get "$@";;
    create)      _jira_issue_create "$@";;
    update)      _jira_issue_update "$@";;
    delete)      _jira_issue_delete "$@";;
    comment)     _jira_issue_comment "$@";;
    transitions) _jira_issue_transitions "$@";;
    transition)  _jira_issue_transition "$@";;
    assign)      _jira_issue_assign "$@";;
    *) echo "Usage: jira issue <get|create|update|delete|comment|transition|transitions|assign> ..." >&2; return 1;;
  esac
}

_jira_issue_get() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira issue get <KEY>" >&2; return 1; }
  jira_curl GET "$(_jira_issue_path)/$key" | jira_lean "$_JIRA_ISSUE_LEAN"
}

_jira_issue_create() {
  local project="" itype="" summary="" desc="" assignee=""
  local fields=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -p|--project) project="$2"; shift 2;;
      -t|--type)    itype="$2"; shift 2;;
      -s|--summary) summary="$2"; shift 2;;
      -d|--desc)    desc="$2"; shift 2;;
      --assignee)   assignee="$2"; shift 2;;
      -f|--field)   fields+=("$2"); shift 2;;
      *) echo "issue create: unknown option '$1'" >&2; return 1;;
    esac
  done
  [[ -z "$project" || -z "$itype" || -z "$summary" ]] && {
    echo "Usage: jira issue create -p <PROJECT> -t <TYPE> -s <SUMMARY> [-d DESC] [--assignee ID] [-f field=val]..." >&2
    return 1
  }
  local body
  body=$(jq -nc --arg p "$project" --arg t "$itype" --arg s "$summary" \
    '{fields:{project:{key:$p}, issuetype:{name:$t}, summary:$s}}')
  if [[ -n "$desc" ]]; then
    body=$(jq -c --argjson d "$(jira_text_to_adf "$desc")" '.fields.description=$d' <<<"$body")
  fi
  if [[ -n "$assignee" ]]; then
    body=$(jq -c --argjson a "$(_jira_assignee_obj "$assignee")" '.fields.assignee=$a' <<<"$body")
  fi
  local kv k v
  for kv in ${fields[@]+"${fields[@]}"}; do
    k="${kv%%=*}"; v="${kv#*=}"
    body=$(jq -c --arg k "$k" --arg v "$v" '.fields[$k]=$v' <<<"$body")
  done
  jira_curl POST "$(_jira_issue_path)" "$body" | jira_lean '.'
}

_jira_issue_update() {
  local key="${1:-}"; [[ $# -gt 0 ]] && shift
  [[ -z "$key" ]] && { echo "Usage: jira issue update <KEY> [-s SUMMARY] [-d DESC] [-f field=val]..." >&2; return 1; }
  local summary="" desc="" fields=() body='{"fields":{}}'
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -s|--summary) summary="$2"; shift 2;;
      -d|--desc)    desc="$2"; shift 2;;
      -f|--field)   fields+=("$2"); shift 2;;
      *) echo "issue update: unknown option '$1'" >&2; return 1;;
    esac
  done
  [[ -n "$summary" ]] && body=$(jq -c --arg s "$summary" '.fields.summary=$s' <<<"$body")
  [[ -n "$desc" ]] && body=$(jq -c --argjson d "$(jira_text_to_adf "$desc")" '.fields.description=$d' <<<"$body")
  local kv k v
  for kv in ${fields[@]+"${fields[@]}"}; do
    k="${kv%%=*}"; v="${kv#*=}"
    body=$(jq -c --arg k "$k" --arg v "$v" '.fields[$k]=$v' <<<"$body")
  done
  jira_curl PUT "$(_jira_issue_path)/$key" "$body"
  echo "updated $key"
}

_jira_issue_delete() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira issue delete <KEY>" >&2; return 1; }
  jira_curl DELETE "$(_jira_issue_path)/$key"
  echo "deleted $key"
}

_jira_issue_comment() {
  local key="${1:-}" text="${2:-}"
  [[ -z "$key" || -z "$text" ]] && { echo "Usage: jira issue comment <KEY> <TEXT>" >&2; return 1; }
  local body
  body=$(jq -nc --argjson b "$(jira_text_to_adf "$text")" '{body:$b}')
  jira_curl POST "$(_jira_issue_path)/$key/comment" "$body" | jira_lean '.'
}

_jira_issue_transitions() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira issue transitions <KEY>" >&2; return 1; }
  jira_curl GET "$(_jira_issue_path)/$key/transitions" | jira_lean '{transitions: [.transitions[] | {id, name}]}'
}

_jira_issue_transition() {
  local key="${1:-}" want="${2:-}"
  [[ -z "$key" || -z "$want" ]] && { echo "Usage: jira issue transition <KEY> <NAME|ID>" >&2; return 1; }
  local id
  if [[ "$want" =~ ^[0-9]+$ ]]; then
    id="$want"
  else
    local list matches n
    list=$(jira_curl GET "$(_jira_issue_path)/$key/transitions") || return 1
    matches=$(jq -c --arg w "$want" '[.transitions[] | select((.name|ascii_downcase)==($w|ascii_downcase)) | .id]' <<<"$list")
    n=$(jq 'length' <<<"$matches")
    if [[ "$n" -eq 0 ]]; then
      echo "jira: no transition matches '$want'. Available: $(jq -r '[.transitions[].name]|join(", ")' <<<"$list")" >&2
      return 1
    elif [[ "$n" -gt 1 ]]; then
      echo "jira: transition '$want' is ambiguous (matched ids: $(jq -r 'join(", ")' <<<"$matches"))" >&2
      return 1
    fi
    id=$(jq -r '.[0]' <<<"$matches")
  fi
  jira_curl POST "$(_jira_issue_path)/$key/transitions" "$(jq -nc --arg i "$id" '{transition:{id:$i}}')"
  echo "transitioned $key -> $id"
}

_jira_issue_assign() {
  local key="${1:-}" who="${2:-}"
  [[ -z "$key" || -z "$who" ]] && { echo "Usage: jira issue assign <KEY> <ACCOUNT_ID|->" >&2; return 1; }
  jira_curl PUT "$(_jira_issue_path)/$key/assignee" "$(_jira_assignee_obj "$who")"
  echo "assigned $key"
}
