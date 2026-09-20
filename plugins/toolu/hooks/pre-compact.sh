#!/bin/bash
# PreCompact hook

HOOK_DIR="$(dirname "$0")"

# shellcheck source=lib/config.sh
. "$HOOK_DIR/lib/config.sh"

if ! toolu_enabled hooks pre-compact; then
  cat > /dev/null 2>&1 || true
  exit 0
fi

# Both hosts send hook input via stdin. PreCompact has no shared context-output
# shape, so a successful no-op must be silent.
cat > /dev/null 2>&1 || true

exit 0
