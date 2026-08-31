#!/usr/bin/env bash
# Exercise the real Codex plugin CLI against every plugin in this repository.
set -euo pipefail

ROOT="${CODEX_SMOKE_REPO:-$(cd "${BASH_SOURCE%/*}/.." && pwd)}"
TMP_ROOT="${CODEX_SMOKE_TMP_ROOT:-${TMPDIR:-/tmp}}"
KEEP_HOME="${CODEX_SMOKE_KEEP_HOME:-0}"

fail() {
  printf 'codex-smoke: %s\n' "$*" >&2
  exit 1
}

command -v codex >/dev/null 2>&1 || fail 'codex CLI is required'
command -v jq >/dev/null 2>&1 || fail 'jq is required'
[ -d "$ROOT/.agents/plugins" ] || fail "not a Codex marketplace: $ROOT"
[ -d "$TMP_ROOT" ] || fail "temporary root does not exist: $TMP_ROOT"
[ ! -L "$TMP_ROOT" ] || fail "temporary root must not be a symlink: $TMP_ROOT"

TMP_ROOT_REAL="$(cd "$TMP_ROOT" && pwd -P)"
SMOKE_HOME="$(mktemp -d "$TMP_ROOT_REAL/toolu-codex-smoke.XXXXXX")"
SMOKE_PROJECT="$SMOKE_HOME/project"
mkdir -p "$SMOKE_PROJECT"

cleanup() {
  local target parent base
  [ "$KEEP_HOME" = 0 ] || {
    printf 'codex-smoke: preserved %s\n' "$SMOKE_HOME"
    return 0
  }
  target="${SMOKE_HOME:?}"
  [ -d "$target" ] || return 0
  [ ! -L "$target" ] || return 0
  parent="$(cd "$(dirname "$target")" && pwd -P)"
  base="$(basename "$target")"
  [ "$parent" = "$TMP_ROOT_REAL" ] || return 0
  case "$base" in
    toolu-codex-smoke.*) rm -rf -- "$target" ;;
  esac
}
trap cleanup EXIT

run_codex() {
  CODEX_HOME="$SMOKE_HOME" codex "$@"
}

run_codex plugin marketplace add "$ROOT" --json > "$SMOKE_HOME/marketplace-add.json"
[ "$(jq -r '.marketplaceName' "$SMOKE_HOME/marketplace-add.json")" = toolu ] || fail 'marketplace registered under an unexpected name'
run_codex plugin marketplace list > "$SMOKE_HOME/marketplaces.txt"
grep -Eq '^toolu[[:space:]]' "$SMOKE_HOME/marketplaces.txt" || fail 'toolu marketplace is not listed'

run_codex plugin list --available --json > "$SMOKE_HOME/available.json"
available_count="$(jq '.available | length' "$SMOKE_HOME/available.json")"
[ "$available_count" -eq 13 ] || fail "expected 13 available plugins, found $available_count"
expected_names="$(for manifest in "$ROOT"/plugins/*/.codex-plugin/plugin.json; do jq -r '.name' "$manifest"; done | sort)"
available_names="$(jq -r '.available[].name' "$SMOKE_HOME/available.json" | sort)"
[ "$available_names" = "$expected_names" ] || fail 'available plugin names differ from checked-in manifests'
printf 'codex-smoke: available=%d\n' "$available_count"

# Install the core first so every dependent plugin observes the supported order.
run_codex plugin add toolu@toolu --json > "$SMOKE_HOME/install-toolu.json"
for plugin_name in $expected_names; do
  [ "$plugin_name" = toolu ] && continue
  run_codex plugin add "$plugin_name@toolu" --json > "$SMOKE_HOME/install-$plugin_name.json"
done

run_codex plugin list --json > "$SMOKE_HOME/installed.json"
installed_count="$(jq '.installed | length' "$SMOKE_HOME/installed.json")"
installed_names="$(jq -r '.installed[].name' "$SMOKE_HOME/installed.json" | sort)"
[ "$installed_count" -eq 13 ] || fail "expected 13 installed plugins, found $installed_count"
[ "$installed_names" = "$expected_names" ] || fail 'installed plugin names differ from checked-in manifests'
printf 'codex-smoke: installed=%d\n' "$installed_count"

# Codex validates hook schemas during installation. Execute every SessionStart
# command once as well, using the native Codex environment and an isolated repo.
session_start_count=0
for hook_file in "$ROOT"/plugins/*/hooks/hooks.json; do
  [ -f "$hook_file" ] || continue
  plugin_root="${hook_file%/hooks/hooks.json}"
  while IFS= read -r hook_command; do
    hook_path="${hook_command#'${CLAUDE_PLUGIN_ROOT}/'}"
    [ "$hook_path" != "$hook_command" ] || hook_path="${hook_command#'${PLUGIN_ROOT}/'}"
    [ "$hook_path" != "$hook_command" ] || fail "unsupported SessionStart command: $hook_command"
    [ -x "$plugin_root/$hook_path" ] || fail "SessionStart command is not executable: $plugin_root/$hook_path"
    printf '%s\n' '{"hook_event_name":"SessionStart","source":"startup"}' | (
      cd "$SMOKE_PROJECT"
      env CODEX_HOME="$SMOKE_HOME" \
        PLUGIN_ROOT="$plugin_root" \
        CLAUDE_PLUGIN_ROOT="$plugin_root" \
        TOOLU_HOST_OVERRIDE=codex \
        TOOLU_PROJECT_DIR="$SMOKE_PROJECT" \
        "$plugin_root/$hook_path" >/dev/null
    )
    session_start_count=$((session_start_count + 1))
  done < <(jq -r '.hooks.SessionStart[]?.hooks[]? | select(.type == "command") | .command' "$hook_file")
done
[ "$session_start_count" -eq 19 ] || fail "expected 19 SessionStart commands, ran $session_start_count"
printf 'codex-smoke: session-start=%d\n' "$session_start_count"

removed_count=0
for plugin_name in $expected_names; do
  [ "$plugin_name" = toolu ] && continue
  run_codex plugin remove "$plugin_name@toolu" --json > "$SMOKE_HOME/remove-$plugin_name.json"
  removed_count=$((removed_count + 1))
done
run_codex plugin remove toolu@toolu --json > "$SMOKE_HOME/remove-toolu.json"
removed_count=$((removed_count + 1))
run_codex plugin list --json > "$SMOKE_HOME/after-remove.json"
[ "$(jq '.installed | length' "$SMOKE_HOME/after-remove.json")" -eq 0 ] || fail 'plugins remain installed after removal smoke test'
printf 'codex-smoke: removed=%d\n' "$removed_count"
