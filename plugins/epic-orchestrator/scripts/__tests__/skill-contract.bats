#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
SKILL="$ROOT/skills/epic-orchestrator/SKILL.md"
CMD="$ROOT/commands/epic.md"

@test "skill invokes bun CLIs via host plugin root" {
  grep -Fq 'ROOT="${CLAUDE_PLUGIN_ROOT}"' "$SKILL"
  grep -Fq 'S="${ROOT}/scripts"' "$SKILL"
  grep -Fq 'bun "$S/epic-graph.ts"' "$SKILL"
  grep -Fq 'bun "$S/launch-issue.ts"' "$SKILL"
  grep -Fq 'bun "$S/epic-watch.ts"' "$SKILL"
  grep -Fq 'bun "$S/merge-gate.ts"' "$SKILL"
  grep -Fq 'bun "$S/route.ts"' "$SKILL"
  grep -Fq 'bun "$S/epic-close.ts"' "$SKILL"
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

@test "every script the skill names exists" {
  while IFS= read -r script; do
    [ -f "$ROOT/scripts/$script" ] || { echo "missing $script"; return 1; }
  done < <(grep -oE '"\$S/[a-z_-]+\.(ts|sh)"' "$SKILL" | sed -E 's#"\$S/(.*)"#\1#' | sort -u)
}

@test "brief placeholders are all filled by the launcher" {
  brief="$ROOT/skills/epic-orchestrator/references/worker-brief.md"
  while IFS= read -r ph; do
    grep -Fq "    $ph:" "$ROOT/scripts/launch-issue.ts" || { echo "unfilled {{$ph}}"; return 1; }
  done < <(grep -oE '\{\{[A-Z_]+\}\}' "$brief" | tr -d '{}' | sort -u)
}
