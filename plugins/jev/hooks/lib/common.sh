#!/usr/bin/env bash
# Shared by jev's hooks. Resolves the host-native config root, the stable
# published wrapper path, and the local prerequisites, and emits hook output.
# No network calls: everything here must be safe to run on every prompt.
#
# Source via:  . "$HOOK_DIR/lib/common.sh"

# jev_config_root -> writable config root for the active host.
# An explicit TOOLU_CONFIG_DIR wins; Codex is detected by TOOLU_HOST_OVERRIDE
# or by its native PLUGIN_ROOT variable; otherwise Claude Code.
jev_config_root() {
  if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
    printf '%s\n' "$TOOLU_CONFIG_DIR"
  elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] ||
       { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
    printf '%s\n' "${CODEX_HOME:-$HOME/.codex}"
  else
    printf '%s\n' "${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
  fi
}

# jev_published_path -> the stable wrapper path the agent's shell can reach.
jev_published_path() {
  printf '%s/jev/jev.sh\n' "$(jev_config_root)"
}

# jev_missing_prereqs WRAPPER -> space-prefixed list of what is missing, or "".
jev_missing_prereqs() {
  local dst="$1" missing="" tool
  for tool in jq curl; do
    command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
  done
  [ -n "${TYPESAFE_API_KEY:-}" ] || missing="$missing TYPESAFE_API_KEY"
  [ -x "$dst" ] || missing="$missing executable-wrapper"
  printf '%s' "$missing"
}

# jev_emit EVENT CONTEXT -> hookSpecificOutput JSON; plain text without jq.
# Both hosts accept the JSON shape for SessionStart and UserPromptSubmit.
jev_emit() {
  local event="$1" context="$2"
  [ -n "$context" ] || return 0
  if command -v jq >/dev/null 2>&1; then
    jq -nc --arg event "$event" --arg context "$context" \
      '{hookSpecificOutput: {hookEventName: $event, additionalContext: $context}}'
  else
    printf '%s\n' "$context"
  fi
}
