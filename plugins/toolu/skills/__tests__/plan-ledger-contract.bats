#!/usr/bin/env bats
#
# Plan-ledger skill prose contract — asserts each workflow SKILL.md carries the
# marker string its phase is responsible for, so the plan↔execution ledger
# contract stays wired into the prose (not just the scripts).

setup() {
  SKILLS="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
}

@test "plan emits a machine-readable steps block" {
  grep -q 'Steps (machine-readable)' "$SKILLS/plan/SKILL.md"
}

@test "plan-review asserts every step has a runnable check and rejects empty steps" {
  grep -q 'every step has a' "$SKILLS/plan-review/SKILL.md"
  grep -q 'empty steps' "$SKILLS/plan-review/SKILL.md"
}

@test "execution reads status and records each step via run <plan_doc> --step <id>" {
  grep -q 'plan-ledger.sh status' "$SKILLS/execution/SKILL.md"
  # the engine requires the plan-doc positional arg, so the doc must show it
  grep -q 'run <plan_doc>' "$SKILLS/execution/SKILL.md"
  grep -q -- '--step <id>' "$SKILLS/execution/SKILL.md"
}

@test "execution-review confirms all steps fresh-green" {
  grep -q 'fresh-green' "$SKILLS/execution-review/SKILL.md"
}
