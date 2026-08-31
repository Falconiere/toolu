#!/usr/bin/env bash
# Probe: a hook `ask` survives a blanket permissions allowlist.
#
# The relaxed denylist is only useful if `permissionDecision: "ask"` still
# reaches the user after toolu writes `Bash(*)` into settings.local.json. Claude
# Code's changelog settles the rule — `permissions.deny` overrides a hook's
# ask, and nothing says `permissions.allow` does — but a rule you have not
# exercised is a rule you are guessing at.
#
# This probe runs the real PreToolUse pipeline against a real command in a
# repo that HAS the blanket allowlist on disk, and asserts an ask comes out. It
# proves toolu's half end to end. The host's half (does the CLI render the
# prompt) can only be seen by a human running the CLI; the changelog evidence
# and the manual check are recorded in the spec's Open Questions.
#
# Usage: bash tooling/probe-ask.sh
# Exit 0 = ask emitted; exit 1 = something else came out (details on stderr).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MOD="$REPO_ROOT/plugins/toolu/hooks/pre-tools/mod.sh"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/repo/.claude" "$TMP/home/.claude" "$TMP/settings"
cd "$TMP/repo" || exit 1
git init -q -b main
git config user.email probe@example.com
git config user.name probe
git commit -q --allow-empty -m init

# The blanket allowlist under test.
cat > "$TMP/repo/.claude/settings.local.json" <<'JSON'
{"permissions":{"allow":["Bash(*)","Edit","Write"]}}
JSON

# A denylist with one rule, and bashCommands pinned to ask (the shipped
# default advises; this probe is about whether an ask still survives the
# blanket allowlist).
printf 'node -e\n' > "$TMP/settings/bash-denylist.txt"
: > "$TMP/settings/bash-allowlist.txt"
printf '%s' '{"version":1,"gates":{"bashCommands":{"mode":"ask"}}}' > "$TMP/repo/.claude/toolu.config.json"

payload=$(jq -n '{tool_name:"Bash",tool_input:{command:"node -e \"console.log(1)\""}}')

output=$(
  cd "$TMP/repo" && HOME="$TMP/home" \
    CLAUDE_PROJECT_DIR="$TMP/repo" TOOLU_PROJECT_DIR="$TMP/repo" \
    TOOLU_SETTINGS_DIR="$TMP/settings" \
    bash "$MOD" <<<"$payload" 2>/dev/null
)

decision=$(printf '%s' "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"' 2>/dev/null || echo "unparseable")

if [ "$decision" = "ask" ]; then
  printf 'probe-ask: ask emitted with Bash(*) allowlisted — the gate can still prompt.\n'
  exit 0
fi

printf 'probe-ask: expected an ask decision, got "%s".\n' "$decision" >&2
printf 'probe-ask: raw hook output was: %s\n' "$output" >&2
exit 1
