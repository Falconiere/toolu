#!/usr/bin/env bash
# attachment.sh — Jira issue attachments: add (multipart upload), list, get
# (metadata), download (binary content), read (download + surface text content
# as context). Sourced; depends on http.sh (jira_curl/jira_lean/jira_download and
# the resolved transport state). The multipart `add` builds its own curl
# (jira_curl only speaks JSON), reusing the same auth resolution.

jira_attachment() {
  local action="${1:-}"; [[ $# -gt 0 ]] && shift
  case "$action" in
    add)      _jira_attachment_add "$@";;
    list)     _jira_attachment_list "$@";;
    get)      _jira_attachment_get "$@";;
    download) _jira_attachment_download "$@";;
    read)     _jira_attachment_read "$@";;
    *) echo "Usage: jira attachment <add|list|get|download|read> ..." >&2; return 1;;
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

# _jira_attachment_download ATTACHMENT_ID [-o OUTFILE]: download the binary
# content. Without -o, the filename is taken from the attachment metadata.
_jira_attachment_download() {
  local id="${1:-}"; [[ $# -gt 0 ]] && shift
  local out=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -o|--output) out="$2"; shift 2;;
      *) echo "attachment download: unknown option '$1'" >&2; return 1;;
    esac
  done
  [[ -z "$id" ]] && { echo "Usage: jira attachment download <ATTACHMENT_ID> [-o OUTFILE]" >&2; return 1; }
  if [[ -z "$out" ]]; then
    local fn; fn=$(jira_curl GET "/rest/api/${_JIRA_VER}/attachment/${id}" | jq -r '.filename // empty')
    out="${fn:-attachment-${id}}"
  fi
  jira_download "/rest/api/${_JIRA_VER}/attachment/content/${id}" "$out"
  echo "downloaded attachment ${id} -> ${out}"
}

# _jira_attachment_read ATTACHMENT_ID: download the content and, for text-like
# types, print it (so it becomes usable context); for binary types, report the
# type/size and the saved path instead of dumping bytes.
_jira_attachment_read() {
  local id="${1:-}"
  [[ -z "$id" ]] && { echo "Usage: jira attachment read <ATTACHMENT_ID>" >&2; return 1; }
  local meta mime fn
  meta=$(jira_curl GET "/rest/api/${_JIRA_VER}/attachment/${id}") || return 1
  mime=$(jq -r '.mimeType // ""' <<<"$meta")
  fn=$(jq -r '.filename // "attachment"' <<<"$meta")
  local tmp; tmp="$(mktemp -t jira-attachment.XXXXXX)"
  jira_download "/rest/api/${_JIRA_VER}/attachment/content/${id}" "$tmp" || { rm -f "$tmp"; return 1; }
  case "$mime" in
    text/*|application/json|application/xml|application/javascript|application/x-yaml|application/yaml|*+json|*+xml)
      printf '# %s (%s)\n\n' "$fn" "$mime"
      cat "$tmp"
      rm -f "$tmp"
      ;;
    *)
      local size; size=$(wc -c < "$tmp" | tr -d ' ')
      printf '%s — binary attachment (%s, %s bytes) saved to %s\n' "$fn" "${mime:-unknown}" "$size" "$tmp"
      printf 'Run `jira attachment download %s -o <path>` to save it elsewhere.\n' "$id"
      ;;
  esac
}
