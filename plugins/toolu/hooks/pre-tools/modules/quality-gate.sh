#!/usr/bin/env bash
# Pre-tool check: Quality gate enforcement.
# DENIES non-fix actions when the quality-gate state file says "failing".
# Project-agnostic: tooling is auto-detected; missing tooling → silent skip.
# Honors MY_CLAUDE_QUALITY=off to disable the gate entirely.
#
# Inputs (from parent dispatcher pre-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload on stdin

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"

# Owner kill-switch.
[ "${MY_CLAUDE_QUALITY:-}" = "off" ] && exit 0

# Guard tooling.
command -v jq  >/dev/null 2>&1 || exit 0
command -v git >/dev/null 2>&1 || exit 0

project_root="$(detect_project_root)"
[ -z "$project_root" ] && project_root="$(pwd)"

gate_file="$project_root/.claude/tmp/quality-gate-status.json"

# Cheap stat/jq checks FIRST. This module runs on every single tool call and the
# gate is absent or passing almost always; the linked-worktree probe below costs
# two `git rev-parse` processes (~13ms measured), so running it ahead of these
# burned that on every call for nothing.
[[ ! -f "$gate_file" ]] && exit 0
[[ "$(jq -r '.status // ""' "$gate_file" 2>/dev/null)" != "failing" ]] && exit 0

# Skip enforcement in git linked worktrees — quality state lives on the main checkout.
git_dir="$(git -C "$project_root" rev-parse --path-format=absolute --git-dir 2>/dev/null || true)"
common_dir="$(git -C "$project_root" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
git_dir="${git_dir%/}"
common_dir="${common_dir%/}"
if [[ -n "$git_dir" && -n "$common_dir" && "$git_dir" != "$common_dir" ]]; then
  exit 0
fi

# Allowed quality-fix command patterns are auto-generated per package manager.
pm="$(detect_node_pm)"
has_rust="$(detect_rust)"
allow_pattern=""

case "$pm" in
  bun)
    if command -v bun >/dev/null 2>&1; then
      allow_pattern='(bun run (check|check:fix|check:duplication|lint|lint:fix|format|format:fix|build|test|typecheck)|bun test|vitest|tsc|oxlint|oxfmt)'
    fi
    ;;
  pnpm)
    if command -v pnpm >/dev/null 2>&1; then
      allow_pattern='(pnpm run (check|lint|lint:fix|format|format:fix|build|test|typecheck)|pnpm test|vitest|tsc)'
    fi
    ;;
  yarn)
    if command -v yarn >/dev/null 2>&1; then
      allow_pattern='(yarn (check|lint|lint:fix|format|format:fix|build|test|typecheck)|vitest|tsc)'
    fi
    ;;
  npm)
    if command -v npm >/dev/null 2>&1; then
      allow_pattern='(npm run (check|lint|lint:fix|format|format:fix|build|test|typecheck)|npm test|vitest|tsc)'
    fi
    ;;
esac

if [ "$has_rust" = "rust" ] && command -v cargo >/dev/null 2>&1; then
  rust_pattern='(cargo (check|clippy|fmt|build|nextest))'
  allow_pattern="${allow_pattern:+$allow_pattern|}$rust_pattern"
fi

# Always allow generic test-runners + linters if present on PATH.
extra_pattern='(vitest|tsc|oxlint|oxfmt|ruff|mypy)'
allow_pattern="${allow_pattern:+$allow_pattern|}$extra_pattern"

# Gate is failing — determine if this action is allowed.
#
# Allow patterns must match the FIRST shell statement of the command. The old
# behavior (unanchored grep) let `cat tsconfig.json` slip through via the
# `tsc` substring AND let `rm -rf node_modules && bun run check` ride the
# allow rule from behind a destructive prefix. Anchor at start-of-string with
# optional leading whitespace; terminate with a statement boundary so trailing
# composition (`bun run check && bun run build`) still passes.
ANCHOR_PREFIX='^[[:space:]]*'
ANCHOR_SUFFIX='([[:space:]]|$|&&|\|\||;)'

if [[ "$tool_name" == "Bash" || "$tool_name" == "Shell" ]]; then
  command=$(echo "$input" | jq -r '.tool_input.command // ""')
  cmd_only=$(printf '%s\n' "$command" | strip_heredocs)

  # allow_pattern joins components with top-level `|`; wrap it in an extra
  # group so ANCHOR_PREFIX/ANCHOR_SUFFIX bind to EVERY alternative, not just
  # the first/last. Without the group, middle components (e.g. cargo in a
  # bun+rust repo) matched unanchored anywhere in the string.
  if [ -n "$allow_pattern" ] && echo "$cmd_only" | grep -qE "${ANCHOR_PREFIX}(${allow_pattern})${ANCHOR_SUFFIX}"; then
    exit 0
  fi

  # Always allow non-destructive git inspection commands. `commit` and `add`
  # are intentionally EXCLUDED here — committing while the gate is failing
  # defeats the gate's stated purpose. To fix, run the failing check first
  # so the gate clears.
  if echo "$cmd_only" | grep -qE "${ANCHOR_PREFIX}git[[:space:]]+(status|diff|log|branch|stash)"; then
    exit 0
  fi
fi

# Allow Edit/Write on code/config files (to fix violations).
if [[ "$tool_name" == "Edit" || "$tool_name" == "Write" || "$tool_name" == "MultiEdit" ]]; then
  file_path=$(echo "$input" | jq -r '.tool_input.file_path // .tool_input.path // ""')
  if [[ "$file_path" =~ \.(ts|tsx|js|mjs|sh|json|rs|py|toml|yaml|yml)$ ]]; then
    exit 0
  fi
fi

# Allow Read, Grep, Glob (non-destructive).
if [[ "$tool_name" == "Read" || "$tool_name" == "Grep" || "$tool_name" == "Glob" ]]; then
  exit 0
fi

# Strict-deny stance: anything else gets blocked with the gate's reason.
reason=$(jq -r '.reason // "Quality gate failing"' "$gate_file" 2>/dev/null || echo "Quality gate failing")
violations=$(jq -r '.violations // ""' "$gate_file" 2>/dev/null || echo "")

jq -n --arg reason "$reason" --arg violations "$violations" '{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": ("BLOCKED: Quality gate failing. Fix violations first.\n" + $reason + "\n" + $violations)
  }
}'
exit 0
