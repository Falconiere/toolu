#!/usr/bin/env bash
# Host adapter shared by toolu's hook runtime.
#
# Public API:
#   toolu_host                         claude | codex
#   toolu_config_root                  host-native writable config/data root
#   toolu_project_root                 explicit/project git root, if any
#   toolu_project_dirname              .claude | .codex (overrideable)
#   toolu_project_config               project toolu.config.json path
#   toolu_project_state_root           <repo>/<host-dir>/tmp
#   toolu_project_state_dir NAME       state root child
#   toolu_plugin_root / _data          plugin paths supplied by the host
#   toolu_invocation NS NAME           /ns:name | $ns:name
#   toolu_plugin_install_command SPEC  host-native exact install command
#   toolu_codex_plugin_snapshot_path   canonical snapshot path
#   toolu_snapshot_codex_plugins       refresh snapshot at SessionStart
#   toolu_codex_plugin_installed SPEC  tri-state lookup (0 hit/miss, 2 unknown)

_toolu_host_warn() {
  printf 'toolu-host: %s\n' "$1" >&2
}

toolu_host() {
  case "${TOOLU_HOST_OVERRIDE:-}" in
    claude|codex) printf '%s\n' "$TOOLU_HOST_OVERRIDE"; return 0 ;;
    '') ;;
    *) _toolu_host_warn "invalid TOOLU_HOST_OVERRIDE '${TOOLU_HOST_OVERRIDE}' (using environment detection)" ;;
  esac

  # Codex sets PLUGIN_ROOT and also exports CLAUDE_PLUGIN_ROOT for compatibility.
  # The Codex-native signal must therefore be checked first.
  if [ -n "${PLUGIN_ROOT:-}" ]; then
    printf 'codex\n'
  else
    printf 'claude\n'
  fi
}

toolu_config_root() {
  if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
    printf '%s\n' "$TOOLU_CONFIG_DIR"
  elif [ "$(toolu_host)" = codex ]; then
    printf '%s\n' "${CODEX_HOME:-$HOME/.codex}"
  elif [ -n "${CLAUDE_CONFIG_DIR:-}" ]; then
    printf '%s\n' "$CLAUDE_CONFIG_DIR"
  else
    printf '%s/.claude\n' "$HOME"
  fi
}

toolu_project_root() {
  local root="${TOOLU_PROJECT_DIR:-}"
  if [ -z "$root" ] && [ "$(toolu_host)" = claude ]; then
    root="${CLAUDE_PROJECT_DIR:-}"
  fi
  [ -n "$root" ] || root=$(git rev-parse --show-toplevel 2>/dev/null || true)
  [ -n "$root" ] && printf '%s\n' "$root"
}

toolu_project_dirname() {
  if [ -n "${TOOLU_PROJECT_CONFIG_DIRNAME:-}" ]; then
    printf '%s\n' "$TOOLU_PROJECT_CONFIG_DIRNAME"
  elif [ "$(toolu_host)" = codex ]; then
    printf '.codex\n'
  else
    printf '.claude\n'
  fi
}

toolu_project_config() {
  local root
  root=$(toolu_project_root)
  [ -n "$root" ] || return 0
  printf '%s/%s/toolu.config.json\n' "$root" "$(toolu_project_dirname)"
}

toolu_project_state_root() {
  local root="${1:-}"
  [ -n "$root" ] || root=$(toolu_project_root)
  [ -n "$root" ] || return 0
  printf '%s/%s/tmp\n' "$root" "$(toolu_project_dirname)"
}

toolu_project_state_dir() {
  local name="${1:-}" root="${2:-}" base
  [ -n "$name" ] || return 1
  base=$(toolu_project_state_root "$root")
  [ -n "$base" ] || return 0
  printf '%s/%s\n' "$base" "$name"
}

toolu_plugin_root() {
  if [ "$(toolu_host)" = codex ] && [ -n "${PLUGIN_ROOT:-}" ]; then
    printf '%s\n' "$PLUGIN_ROOT"
  elif [ -n "${CLAUDE_PLUGIN_ROOT:-}" ]; then
    printf '%s\n' "$CLAUDE_PLUGIN_ROOT"
  fi
}

toolu_plugin_data() {
  if [ "$(toolu_host)" = codex ] && [ -n "${PLUGIN_DATA:-}" ]; then
    printf '%s\n' "$PLUGIN_DATA"
  elif [ -n "${CLAUDE_PLUGIN_DATA:-}" ]; then
    printf '%s\n' "$CLAUDE_PLUGIN_DATA"
  fi
}

toolu_invocation() {
  local namespace="${1:-}" name="${2:-}"
  [ -n "$namespace" ] && [ -n "$name" ] || return 1
  if [ "$(toolu_host)" = codex ]; then
    printf '$%s:%s\n' "$namespace" "$name"
  else
    printf '/%s:%s\n' "$namespace" "$name"
  fi
}

toolu_plugin_install_command() {
  local spec="${1:-}"
  [ -n "$spec" ] || return 1
  if [ "$(toolu_host)" = codex ]; then
    printf 'codex plugin add %s\n' "$spec"
  else
    printf '/plugin install %s\n' "$spec"
  fi
}

toolu_codex_plugin_snapshot_path() {
  printf '%s\n' "${TOOLU_CODEX_PLUGIN_SNAPSHOT:-$(toolu_config_root)/toolu/codex-plugins.json}"
}

_toolu_write_codex_snapshot() {
  local path="$1" payload="$2" dir tmp
  dir=$(dirname "$path")
  mkdir -p "$dir" 2>/dev/null || return 0
  tmp="${path}.tmp.$$"
  if printf '%s\n' "$payload" > "$tmp" 2>/dev/null; then
    mv "$tmp" "$path" 2>/dev/null || rm -f "$tmp"
  else
    rm -f "$tmp"
  fi
}

# Snapshot once on Codex SessionStart. Failures are represented explicitly so
# hot-path readers never invoke the CLI and never mistake an old cache for a
# definitive installed/absent result.
toolu_snapshot_codex_plugins() {
  [ "$(toolu_host)" = codex ] || return 0
  local path raw canonical
  path=$(toolu_codex_plugin_snapshot_path)
  canonical='{"version":1,"status":"indeterminate","plugins":[]}'

  if command -v codex >/dev/null 2>&1 && raw=$(codex plugin list --json 2>/dev/null); then
    if canonical=$(jq -ce '
      if (.installed | type) != "array" then error("installed must be an array") else
        {
          version: 1,
          status: "ready",
          plugins: ([.installed[]
            | select((has("installed") | not) or (.installed == true))
            | select((has("enabled") | not) or (.enabled == true))
            | (.pluginId // (if (.name | type) == "string" and (.marketplaceName | type) == "string"
                              then (.name + "@" + .marketplaceName) else empty end))
            | select(type == "string" and length > 0)] | unique | sort)
        }
      end
    ' <<<"$raw" 2>/dev/null); then
      :
    else
      canonical='{"version":1,"status":"indeterminate","plugins":[]}'
    fi
  fi
  _toolu_write_codex_snapshot "$path" "$canonical"
  return 0
}

# Return contract matches detect_plugin_installed:
#   0 + spec => installed, 0 + empty => definitively absent, 2 => unknown.
toolu_codex_plugin_installed() {
  local spec="${1:-}" path status
  [ -n "$spec" ] || return 0
  command -v jq >/dev/null 2>&1 || return 2
  path=$(toolu_codex_plugin_snapshot_path)
  [ -f "$path" ] || return 2
  jq -e '.version == 1 and (.status | type == "string") and (.plugins | type == "array")' \
    "$path" >/dev/null 2>&1 || return 2
  status=$(jq -r '.status' "$path" 2>/dev/null) || return 2
  [ "$status" = ready ] || return 2
  if jq -e --arg spec "$spec" 'any(.plugins[]; . == $spec)' "$path" >/dev/null 2>&1; then
    printf '%s\n' "$spec"
  fi
  return 0
}

# toolu_supports_ask -> 0 iff this host can prompt the user for a PreToolUse
# decision (`permissionDecision: "ask"`).
#
# Claude Code prompts; Codex's hook contract has no ask, so a gate that asked
# there would emit a decision the host drops on the floor — silently disabling
# the gate. Callers (toolu_gate_mode) degrade `ask` to `advise` instead, which
# still reaches the agent.
toolu_supports_ask() {
  [ "$(toolu_host)" = codex ] && return 1
  return 0
}
