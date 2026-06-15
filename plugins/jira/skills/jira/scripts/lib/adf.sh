#!/usr/bin/env bash
# adf.sh — render a plain-text comment/description body for the resolved API
# version: a minimal ADF document for v3 (Cloud), a plain JSON string for v2
# (Server/DC). Sourced; defines a function only.

# jira_text_to_adf TEXT: print the JSON *value* for a body field (not wrapped).
# v3 -> {"type":"doc","version":1,"content":[{"type":"paragraph",...}]}
# v2 -> the text as a JSON string. Callers wrap it, e.g. {"body": <value>}.
jira_text_to_adf() {
  local text="$1"
  if [[ "${_JIRA_VER:-3}" == "2" ]]; then
    jq -n --arg t "$text" '$t'
  else
    jq -n --arg t "$text" \
      '{type:"doc",version:1,content:[{type:"paragraph",content:[{type:"text",text:$t}]}]}'
  fi
}
