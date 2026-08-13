#!/usr/bin/env bash
# Shared runtime registry for hook modules contributed by other plugins.
# Modules placed under <root>/<event>.d/<plugin-spec>__<name>.sh are executed
# by the core dispatcher after its built-in modules, gated on the owning plugin
# being installed. The "__" separator is load-bearing: plugin specs may contain
# "." (e.g. name@git.example.com) but must not contain "__". The registry holds
# GENERATED state (synced each session by each plugin's register.sh); the
# source of truth for a module is its own plugin.

_TOOLU_REGISTRY_LIB_DIR="$(cd "${BASH_SOURCE%/*}" && pwd)"
# shellcheck source=host.sh
. "$_TOOLU_REGISTRY_LIB_DIR/host.sh"

# toolu_registry_root -> prints the registry root dir (not created).
toolu_registry_root() {
  local agent_dir
  agent_dir=$(toolu_config_root)
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

# Remove generated registry modules whose owning plugin is definitively absent
# from Codex's ready SessionStart snapshot. Missing/malformed/indeterminate
# snapshots prune nothing. Symlinks and un-namespaced files are never removed.
toolu_registry_prune_inactive() {
  [ "$(toolu_host)" = codex ] || return 0
  command -v jq >/dev/null 2>&1 || return 0

  local snapshot root event_dir module base spec
  snapshot=$(toolu_codex_plugin_snapshot_path)
  [ -f "$snapshot" ] || return 0
  jq -e '.version == 1 and .status == "ready" and (.plugins | type == "array")' \
    "$snapshot" >/dev/null 2>&1 || return 0

  root=$(toolu_registry_root)
  for event_dir in "$root/pre-tools.d" "$root/post-tools.d"; do
    [ -d "$event_dir" ] || continue
    for module in "$event_dir"/*.sh; do
      [ -f "$module" ] || continue
      [ -L "$module" ] && continue
      [ "${module%/*}" = "$event_dir" ] || continue
      base=$(basename "$module")
      case "$base" in
        ?*__*.sh) spec=${base%%__*} ;;
        *) continue ;;
      esac
      [ -n "$spec" ] || continue
      if ! jq -e --arg spec "$spec" 'any(.plugins[]; . == $spec)' "$snapshot" >/dev/null 2>&1; then
        rm -f -- "$module"
      fi
    done
  done
  return 0
}
