#!/usr/bin/env bash
# Shared runtime registry for hook modules contributed by other plugins.
# Modules placed under <root>/<event>.d/<plugin-spec>__<name>.sh are executed
# by the core dispatcher after its built-in modules, gated on the owning plugin
# being installed. The "__" separator is load-bearing: plugin specs may contain
# "." (e.g. name@git.example.com) but must not contain "__". The registry holds
# GENERATED state (synced each session by each plugin's register.sh); the
# source of truth for a module is its own plugin.

# toolu_registry_root -> prints the registry root dir (not created).
toolu_registry_root() {
  local agent_dir="${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}"
  printf '%s/toolu' "$agent_dir"
}

# toolu_registry_event_dir EVENT -> prints "<root>/<event-slug>.d".
# Known events map to the repo's existing naming; others are CamelCase->kebab.
toolu_registry_event_dir() {
  local event="$1" slug
  case "$event" in
    PreToolUse)  slug="pre-tools" ;;
    PostToolUse) slug="post-tools" ;;
    *)
      # Best-effort kebab-casing: consecutive-uppercase runs are NOT split
      # (e.g. "MCPNotification" -> "mcpnotification"). Fine for the current
      # Claude Code event names; extend the case table for anything odd.
      slug=$(printf '%s' "$event" \
        | sed -E 's/([a-z0-9])([A-Z])/\1-\2/g' \
        | tr '[:upper:]' '[:lower:]')
      ;;
  esac
  printf '%s/%s.d' "$(toolu_registry_root)" "$slug"
}
