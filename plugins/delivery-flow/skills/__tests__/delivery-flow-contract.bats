#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
SKILL="$ROOT/plugins/delivery-flow/skills/delivery-flow/SKILL.md"
REFS="$ROOT/plugins/delivery-flow/skills/delivery-flow/references"

@test "one public skill owns every required phase and private guidance" {
  [ -f "$SKILL" ]
  [ "$(find "$ROOT/plugins/delivery-flow/skills" -name SKILL.md | wc -l | tr -d ' ')" -eq 1 ]
  for phase in brainstorm spec spec-review plan plan-review execution test; do
    [ -f "$REFS/$phase.md" ]
    [ ! -e "$ROOT/plugins/toolu/skills/$phase/SKILL.md" ]
    grep -Fq "references/$phase.md" "$SKILL"
  done
  grep -Fq 'brainstorm → spec → spec review → plan → plan review → execution' "$SKILL"
}

@test "delivery stops at failed reviews and resumes from the failed phase" {
  grep -Fq 'Status: Needs changes' "$SKILL"
  grep -Fq 'resume from that phase' "$SKILL"
  grep -Fq 'plan-ledger.sh" preflight' "$SKILL"
  grep -Fq 'real-data' "$SKILL"
}

@test "invocation authorizes checked delivery and prerequisite failures block it" {
  grep -Fq 'Invoking this skill authorizes' "$SKILL"
  grep -Fq 'gh api user' "$SKILL"
  grep -Fq 'non-default branch' "$SKILL"
  grep -Fq 'toolu-review:review' "$SKILL"
  grep -Fq 'verdict.sh" status' "$SKILL"
  grep -Fq 'overall: ready' "$SKILL"
  grep -Fq 'PR' "$SKILL"
  grep -Fq 'pr-babysit:babysit' "$SKILL"
}
