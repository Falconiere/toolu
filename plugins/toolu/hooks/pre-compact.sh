#!/bin/bash
# PreCompact hook

HOOK_DIR="$(dirname "$0")"

# shellcheck source=lib/config.sh
. "$HOOK_DIR/lib/config.sh"

if ! toolu_enabled hooks pre-compact; then
  exit 0
fi

# PreCompact has no shared context-output shape, so a successful no-op is silent.

exit 0
