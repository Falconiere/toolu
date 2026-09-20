#!/bin/bash
# PreCompact hook

HOOK_DIR="$(dirname "$0")"

# shellcheck source=lib/config.sh
. "$HOOK_DIR/lib/config.sh"

# Drain stdin with a shell builtin so Codex does not see EPIPE from a fast exit.
while IFS= read -r; do :; done

if ! toolu_enabled hooks pre-compact; then
  exit 0
fi

# PreCompact has no shared context-output shape, so a successful no-op is silent.

exit 0
