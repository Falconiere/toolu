#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
SKILL="$ROOT/skills/epic-orchestrator/SKILL.md"
CMD="$ROOT/commands/epic.md"

@test "skill invokes bun CLIs via CLAUDE_PLUGIN_ROOT" {
  grep -Fq 'S="${CLAUDE_PLUGIN_ROOT}/scripts"' "$SKILL"
  grep -Fq 'bun "$S/epic-graph.ts"' "$SKILL"
  grep -Fq 'bun "$S/launch-issue.ts"' "$SKILL"
  grep -Fq 'bun "$S/epic-watch.ts"' "$SKILL"
  grep -Fq 'bun "$S/merge-gate.ts"' "$SKILL"
  ! grep -q 'python3' "$SKILL"
  ! grep -qE '\.py' "$SKILL" "$ROOT/skills/epic-orchestrator/references/recovery.md"
  ! grep -q '~/.claude/skills/epic-orchestrator' "$SKILL"
}

@test "command points at plugin skill and bun scripts" {
  grep -Fq 'CLAUDE_PLUGIN_ROOT' "$CMD"
  grep -Fq 'bun' "$CMD"
}

@test "preflight requires bun" {
  grep -Fq 'command -v bun' "$SKILL"
}
