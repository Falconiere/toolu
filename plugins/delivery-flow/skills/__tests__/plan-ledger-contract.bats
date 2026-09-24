#!/usr/bin/env bats
#
# Plan-ledger reference prose contract — asserts each private phase carries the
# marker string its phase is responsible for, so the plan↔execution ledger
# contract stays wired into the prose (not just the scripts).

setup() {
  SKILLS="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
  ROOT="$(cd "$SKILLS/../../.." && pwd)"
}

@test "plan emits a machine-readable steps block" {
  grep -q 'Steps (machine-readable)' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan.md"
}

@test "plan-review asserts every step has a runnable check and rejects empty steps" {
  grep -q 'non-empty runnable' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
  grep -q 'empty steps' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
}

@test "execution reads status and records each step via run <plan_doc> --step <id>" {
  grep -q 'plan-ledger.sh" status' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/execution.md"
  # the engine requires the plan-doc positional arg, so the doc must show it
  grep -q 'run <plan_doc>' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/execution.md"
  grep -q -- '--step <id>' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/execution.md"
}

# --- s8: enrichment docs in sync (ac_refs/depends_on/input + AC-<n> + coverage) ---

@test "spec documents the **AC-<n>:** id convention" {
  grep -q 'AC-<n>' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec.md"
}

@test "spec authors delivery-critical evidence and impact sections" {
  grep -q '^## Failure modes and edge cases$' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec.md"
  grep -q '^## Acceptance evidence$' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec.md"
  grep -q '^## Documentation impact$' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec.md"
}

@test "spec-review validates authored evidence and non-blocking questions" {
  grep -q 'Failure modes and edge cases' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec-review.md"
  grep -q 'Acceptance evidence' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec-review.md"
  grep -q 'Documentation impact' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec-review.md"
  grep -q 'observable real-data' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec-review.md"
  grep -q 'non-blocking' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/spec-review.md"
}

@test "shared plan reference documents optional ac_refs/depends_on/input fields" {
  grep -q 'ac_refs' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/ledger.md"
  grep -q 'depends_on' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/ledger.md"
  grep -q 'input' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/ledger.md"
}

@test "shared plan reference uses the supported final verification command" {
  grep -q 'plan-ledger.sh run <plan_doc> --verify' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/ledger.md"
}

@test "plan is evidence-first and keeps detailed steps only in the ledger" {
  grep -q 'Evidence first' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan.md"
  grep -q 'Mechanical work' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan.md"
  grep -q 'sole detailed step list' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan.md"
  grep -q 'ledger.md' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan.md"
  [ -f "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/ledger.md" ]
}

@test "plan delegates the full ledger schema to its shared reference" {
  ! grep -q 'each item has non-empty' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan.md"
  grep -q 'canonical ledger shape' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan.md"
}

@test "plan-review asserts ac_refs resolve via pl_check_ac_refs" {
  grep -q 'ac_refs' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
  grep -q 'pl_check_ac_refs' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
}

@test "plan-review checks delivery-ready evidence and coverage" {
  grep -q 'AC-to-step coverage' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
  grep -q 'real input' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
  grep -q 'Path declarations' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
  grep -q 'PR-delivery readiness' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/plan-review.md"
}

@test "test is an execution-time method with a behavior-to-evidence map" {
  grep -q 'reusable execution-time method' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/test.md"
  grep -q 'Behavior-to-evidence map' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/test.md"
  grep -q 'mock-substitute' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/test.md"
  grep -q 'happy-path-only' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/test.md"
  ! grep -q 'mocks/' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/test.md"
}

@test "workflow docs show execution-time real-data tests" {
  grep -qi 'execution with real-data tests' "$ROOT/README.md"
  grep -qi 'execution with real-data tests' "$ROOT/docs/toolu/README.md"
}

@test "execution reads AC coverage from status" {
  grep -q 'AC-coverage' "$ROOT/plugins/delivery-flow/skills/delivery-flow/references/execution.md"
}
