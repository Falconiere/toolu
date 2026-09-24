#!/usr/bin/env bash
# Codex lacks a plugin-dependency manifest field. Warn when this dependent
# plugin is installed without toolu or pr-babysit, using Codex's list.
set -u

[ "${TOOLU_HOST_OVERRIDE:-}" != claude ] || exit 0
[ -n "${PLUGIN_ROOT:-}" ] || exit 0
command -v codex >/dev/null 2>&1 || exit 0
command -v jq >/dev/null 2>&1 || exit 0

installed=$(codex plugin list --json 2>/dev/null) || exit 0

plugin_ok() {
  local id="$1"
  jq -e --arg id "$id" 'any(.installed[]?; .pluginId == $id and
    ((has("installed") | not) or (.installed == true)) and
    ((has("enabled") | not) or (.enabled == true)))' \
    <<<"$installed" >/dev/null 2>&1
}

missing=()
plugin_ok "toolu@toolu" || missing+=("toolu@toolu")
plugin_ok "pr-babysit@toolu" || missing+=("pr-babysit@toolu")
[ "${#missing[@]}" -eq 0 ] && exit 0

msg="WARN: this plugin requires"
for id in "${missing[@]}"; do
  msg+=" $id (install with: codex plugin add $id)"
done
jq -n --arg ctx "$msg" '{hookSpecificOutput:{
  hookEventName:"SessionStart",
  additionalContext:$ctx
}}'
