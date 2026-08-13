#!/bin/bash
# Post-tool-use hook dispatcher (entrypoint).
# Runs all scripts in .claude/hooks/post-tools/modules/*.sh sequentially.
# Each script receives $input, $tool_name, and $PROJECT_ROOT as env vars plus
# $input on stdin.
#
# Dispatch, decision short-circuit (PostToolUse uses decision:"block" +
# reason; permissionDecision is PreToolUse-only and ignored here), advisory
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

if ! toolu_enabled hooks post-tools; then
  cat > /dev/null 2>&1 || true
  exit 0
fi

# Exported only once the hook is enabled, so disabled runs keep the env clean.
export TOOLU_LIB_DIR="$HOOK_LIB"
TOOLU_CONFIG_DIR="$(toolu_config_root)"
export TOOLU_CONFIG_DIR

input=$(cat 2>/dev/null || echo "{}")
tool_name=$(jq -r '.tool_name // ""' <<<"$input" 2>/dev/null || echo "")

PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
export PATH="$PROJECT_ROOT/node_modules/.bin:$PATH"

export input tool_name PROJECT_ROOT

HOOK_DIR="$(cd "$(dirname "$0")" && pwd)/modules"

# Exits 2 if a module hard-blocks via exit code 2 (stderr forwarded).
toolu_dispatch_hook "$HOOK_DIR" "PostToolUse" "$(toolu_registry_event_dir PostToolUse)"
