#!/bin/bash
# Pre-tool-use hook dispatcher (entrypoint).
# Runs all scripts in .claude/hooks/pre-tools/modules/*.sh sequentially.
# Each script receives $input and $tool_name as env vars plus $input on stdin.
#
# Dispatch, decision short-circuit (permissionDecision:"deny"), advisory
# merging, and module exit-code semantics live in lib/dispatch.sh.

HOOK_LIB="$(cd "$(dirname "$0")/../lib" && pwd)"
# shellcheck source=../lib/config.sh
. "$HOOK_LIB/config.sh"
# shellcheck source=../lib/dispatch.sh
. "$HOOK_LIB/dispatch.sh"
# shellcheck source=../lib/detect.sh
. "$HOOK_LIB/detect.sh"
# shellcheck source=../lib/registry.sh
. "$HOOK_LIB/registry.sh"

if ! toolu_enabled hooks pre-tools; then
  cat > /dev/null 2>&1 || true
  exit 0
fi

# Exported only once the hook is enabled, so disabled runs keep the env clean.
export TOOLU_LIB_DIR="$HOOK_LIB"
TOOLU_CONFIG_DIR="$(toolu_config_root)"
export TOOLU_CONFIG_DIR

input=$(cat)
tool_name=$(jq -r '.tool_name // ""' <<<"$input" 2>/dev/null || echo "")

export input tool_name

HOOK_DIR="$(cd "$(dirname "$0")" && pwd)/modules"

# Exits 2 if a module hard-blocks via exit code 2 (stderr forwarded).
toolu_dispatch_hook "$HOOK_DIR" "PreToolUse" "$(toolu_registry_event_dir PreToolUse)"
