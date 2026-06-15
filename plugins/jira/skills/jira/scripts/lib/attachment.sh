#!/usr/bin/env bash
# attachment.sh — Jira issue attachments: add (multipart upload), list, get.
# Sourced; depends on http.sh (jira_curl/jira_lean and the resolved transport
# state _JIRA_AUTH_MODE/_JIRA_VER/_JIRA_BASE). The multipart `add` builds its own
# curl (jira_curl only speaks JSON), reusing the same auth resolution.

jira_attachment() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    add)  _jira_attachment_add "$@";;
    list) _jira_attachment_list "$@";;
    get)  _jira_attachment_get "$@";;
    *) echo "Usage: jira attachment <add|list|get> ..." >&2; return 1;;
  esac
}

# _jira_attachment_add KEY FILE: multipart-upload FILE to issue KEY's attachments.
_jira_attachment_add() {
  local key="${1:-}" file="${2:-}"
  [[ -z "$key" || -z "$file" ]] && { echo "Usage: jira attachment add <KEY> <FILE>" >&2; return 1; }
  [[ -f "$file" ]] || { echo "attachment add: file not found: $file" >&2; return 1; }
  local args=(-sS --fail-with-body -X POST -H "X-Atlassian-Token: no-check" \
    -H "Accept: application/json" -F "file=@$file")
  if [[ "${_JIRA_AUTH_MODE:-}" == "bearer" ]]; then
    args+=(-H "Authorization: Bearer ${JIRA_PAT}")
  else
    args+=(-u "${JIRA_EMAIL}:${JIRA_API_TOKEN}")
  fi
  curl "${args[@]}" "${_JIRA_BASE}/rest/api/${_JIRA_VER}/issue/${key}/attachments" | jira_lean '.'
}

# _jira_attachment_list KEY: list KEY's attachments (id, filename, size).
_jira_attachment_list() {
  local key="${1:-}"
  [[ -z "$key" ]] && { echo "Usage: jira attachment list <KEY>" >&2; return 1; }
  jira_curl GET "/rest/api/${_JIRA_VER}/issue/${key}?fields=attachment" \
    | jira_lean '{attachments:[.fields.attachment[]|{id, filename, size}]}'
}

# _jira_attachment_get ATTACHMENT_ID: fetch one attachment's metadata.
_jira_attachment_get() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira attachment get <ATTACHMENT_ID>" >&2; return 1; }
  jira_curl GET "/rest/api/${_JIRA_VER}/attachment/${id}" | jira_lean '.'
}
