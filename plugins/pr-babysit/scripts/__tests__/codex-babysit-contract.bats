#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
SKILL="$ROOT/skills/babysit/SKILL.md"
WORKFLOW="$ROOT/workflows/babysit.md"

@test "Codex babysit skill uses one durable goal and bounded continuation cycles" {
  [ -f "$SKILL" ]
  grep -Fq 'create_goal' "$SKILL"
  grep -Fq 'get_goal' "$SKILL"
  grep -Fq 'update_goal' "$SKILL"
  grep -Fq 'wait' "$SKILL"
  grep -qi '60 seconds' "$SKILL"
  grep -qi 'one active goal' "$SKILL"
}

@test "Codex babysit state and worktrees are isolated by repository and PR" {
  grep -Fq '.codex/tmp/pr-babysit' "$WORKFLOW"
  grep -Fq 'git worktree' "$WORKFLOW"
  grep -qi 'one slot per repository/PR' "$WORKFLOW"
}

@test "Codex controller documents start resume and cancel without Claude scheduling tools" {
  grep -qi 'Codex start or resume' "$WORKFLOW"
  grep -qi 'Codex cancel' "$WORKFLOW"
  ! rg -n 'Cron(Create|Delete|List)|EnterWorktree|ExitWorktree' "$SKILL"
}

@test "Codex skill and Claude command consume the same canonical clearance workflow" {
  grep -Fq '../../workflows/babysit.md' "$SKILL"
  grep -Fq 'workflows/babysit.md' "$ROOT/commands/babysit.md"
}
