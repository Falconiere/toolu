#!/usr/bin/env bats
#
# Plan-ledger skill prose contract — asserts each workflow SKILL.md carries the
# marker string its phase is responsible for, so the plan↔execution ledger
# contract stays wired into the prose (not just the scripts).

setup() {
  SKILLS="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
  ROOT="$(cd "$SKILLS/../../.." && pwd)"
}

@test "plan emits a machine-readable steps block" {
  grep -q 'Steps (machine-readable)' "$SKILLS/plan/SKILL.md"
}

@test "plan-review asserts every step has a runnable check and rejects empty steps" {
  grep -q 'non-empty runnable' "$SKILLS/plan-review/SKILL.md"
  grep -q 'empty steps' "$SKILLS/plan-review/SKILL.md"
}

@test "execution reads status and records each step via run <plan_doc> --step <id>" {
  grep -q 'plan-ledger.sh status' "$SKILLS/execution/SKILL.md"
  # the engine requires the plan-doc positional arg, so the doc must show it
  grep -q 'run <plan_doc>' "$SKILLS/execution/SKILL.md"
  grep -q -- '--step <id>' "$SKILLS/execution/SKILL.md"
}

# --- s8: enrichment docs in sync (ac_refs/depends_on/input + AC-<n> + coverage) ---

@test "spec documents the **AC-<n>:** id convention" {
  grep -q 'AC-<n>' "$SKILLS/spec/SKILL.md"
}

@test "spec authors delivery-critical evidence and impact sections" {
  grep -q '^## Failure modes and edge cases$' "$SKILLS/spec/SKILL.md"
  grep -q '^## Acceptance evidence$' "$SKILLS/spec/SKILL.md"
  grep -q '^## Documentation impact$' "$SKILLS/spec/SKILL.md"
}

@test "spec-review validates authored evidence and non-blocking questions" {
  grep -q 'Failure modes and edge cases' "$SKILLS/spec-review/SKILL.md"
  grep -q 'Acceptance evidence' "$SKILLS/spec-review/SKILL.md"
  grep -q 'Documentation impact' "$SKILLS/spec-review/SKILL.md"
  grep -q 'observable real-data' "$SKILLS/spec-review/SKILL.md"
  grep -q 'non-blocking' "$SKILLS/spec-review/SKILL.md"
}

@test "shared plan reference documents optional ac_refs/depends_on/input fields" {
  grep -q 'ac_refs' "$SKILLS/plan/references/ledger.md"
  grep -q 'depends_on' "$SKILLS/plan/references/ledger.md"
  grep -q 'input' "$SKILLS/plan/references/ledger.md"
}

@test "shared plan reference uses the supported final verification command" {
  grep -q 'plan-ledger.sh run <plan_doc> --verify' "$SKILLS/plan/references/ledger.md"
}

@test "plan is evidence-first and keeps detailed steps only in the ledger" {
  grep -q 'Evidence first' "$SKILLS/plan/SKILL.md"
  grep -q 'Mechanical work' "$SKILLS/plan/SKILL.md"
  grep -q 'sole detailed step list' "$SKILLS/plan/SKILL.md"
  grep -q 'references/ledger.md' "$SKILLS/plan/SKILL.md"
  [ -f "$SKILLS/plan/references/ledger.md" ]
}

@test "plan delegates the full ledger schema to its shared reference" {
  ! grep -q 'each item has non-empty' "$SKILLS/plan/SKILL.md"
  grep -q 'canonical ledger shape' "$SKILLS/plan/SKILL.md"
}

@test "plan-review asserts ac_refs resolve via pl_check_ac_refs" {
  grep -q 'ac_refs' "$SKILLS/plan-review/SKILL.md"
  grep -q 'pl_check_ac_refs' "$SKILLS/plan-review/SKILL.md"
}

@test "plan-review checks delivery-ready evidence and coverage" {
  grep -q 'AC-to-step coverage' "$SKILLS/plan-review/SKILL.md"
  grep -q 'real input' "$SKILLS/plan-review/SKILL.md"
  grep -q 'Path declarations' "$SKILLS/plan-review/SKILL.md"
  grep -q 'PR-delivery readiness' "$SKILLS/plan-review/SKILL.md"
}

@test "test is an execution-time method with a behavior-to-evidence map" {
  grep -q 'reusable execution-time method' "$SKILLS/test/SKILL.md"
  grep -q 'Behavior-to-evidence map' "$SKILLS/test/SKILL.md"
  grep -q 'mock-substitute' "$SKILLS/test/SKILL.md"
  grep -q 'happy-path-only' "$SKILLS/test/SKILL.md"
  ! grep -q 'mocks/' "$SKILLS/test/SKILL.md"
}

@test "workflow diagrams show test feeding execution-time evidence" {
  grep -Fq 'T(test, reusable execution-time method) -.-> E' "$ROOT/README.md"
  grep -Fq 'T(test, reusable execution-time method) -.-> E' "$ROOT/docs/toolu/README.md"
}

@test "execution reads AC coverage from status" {
  grep -q 'AC-coverage' "$SKILLS/execution/SKILL.md"
}
