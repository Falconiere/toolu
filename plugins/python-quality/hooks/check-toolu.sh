#!/usr/bin/env bash
# Codex lacks a plugin-dependency manifest field. Warn when this dependent
# plugin is installed without the toolu core, using Codex's authoritative list.
set -u

[ "${TOOLU_HOST_OVERRIDE:-}" != claude ] || exit 0
[ -n "${PLUGIN_ROOT:-}" ] || exit 0
command -v codex >/dev/null 2>&1 || exit 0
command -v jq >/dev/null 2>&1 || exit 0

installed=$(codex plugin list --json 2>/dev/null) || exit 0
jq -e 'any(.installed[]?; .pluginId == "toolu@toolu" and
  ((has("installed") | not) or (.installed == true)) and
  ((has("enabled") | not) or (.enabled == true)))' \
  <<<"$installed" >/dev/null 2>&1 && exit 0

jq -n '{hookSpecificOutput:{
  hookEventName:"SessionStart",
  additionalContext:"WARN: this plugin requires the toolu core. Install it first with: codex plugin add toolu@toolu"
}}'
