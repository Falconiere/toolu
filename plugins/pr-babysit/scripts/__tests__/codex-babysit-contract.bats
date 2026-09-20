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

@test "Codex branch runs the same shipped helper with the Codex slot path and bounded waits from the result" {
  grep -Fq 'scripts/babysit-tick.sh' "$SKILL"
  grep -Fq 'references/helper.md' "$SKILL"
  grep -Fq -- '--state-file' "$SKILL"
  codex=$(awk '/^### Codex start or resume/{f=1} /^### Codex cancel/{f=0} f' "$WORKFLOW")
  grep -q 'backoff.waitSeconds' <<<"$codex"
  grep -q '60 seconds' <<<"$codex"
  grep -Fq 'record.sh status' "$WORKFLOW"
  ! grep -iE '(write|create|implement) (a |your own |the )?(python |bash )?(polling )?(script|controller)' "$SKILL" | grep -viq 'never'
}

@test "Codex Step 3 delegates fixes through spawn_agent at the routed tier" {
  step3=$(awk '/^## Step 3/{f=1} /^## Step 4/{f=0} f' "$WORKFLOW")
  grep -q 'spawn_agent' <<<"$step3"
  grep -q 'Luna' <<<"$step3"; grep -q 'Terra' <<<"$step3"; grep -q 'Sol' <<<"$step3"
}

