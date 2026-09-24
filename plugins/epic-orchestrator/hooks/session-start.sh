#!/usr/bin/env bash
# OpenCode bootstrap entry (pluginBootstrapScript prefers register.sh, else
# session-start.sh). Claude/Codex still invoke check-deps.sh via hooks.json.
exec "$(cd "$(dirname "$0")" && pwd)/check-deps.sh"
