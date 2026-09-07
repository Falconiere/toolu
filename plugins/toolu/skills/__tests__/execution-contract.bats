#!/usr/bin/env bats

# Contract tests for the execution skill's delivery-owned readiness protocol.

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
SKILL="$ROOT/plugins/toolu/skills/execution/SKILL.md"

@test "execution verifies every ledger step against the final branch diff" {
  grep -Fq 'plan-ledger.sh run <plan_doc> --verify' "$SKILL"
  grep -qi 'AC coverage' "$SKILL"
}

@test "execution requires per-step real-data evidence and documentation sync" {
  grep -qi 'per-step.*real-data' "$SKILL"
  grep -qi 'Docs in sync' "$SKILL"
}

@test "execution owns the v2 review state and green unified verdict" {
  grep -qiE 'toolu-review.*version: 2|version: 2.*toolu-review' "$SKILL"
  grep -Fq 'verdict.sh status' "$SKILL"
  grep -qi 'overall: green' "$SKILL"
}

@test "execution attests the committed branch before delivery" {
  local commit_line ledger_line review_line verdict_line push_line
  [ "$(grep -Fc 'plan-ledger.sh run <plan_doc> --verify' "$SKILL")" -eq 1 ]
  commit_line="$(grep -ni 'commit the scoped changes' "$SKILL" | cut -d: -f1)"
  ledger_line="$(grep -n 'plan-ledger.sh run <plan_doc> --verify' "$SKILL" | cut -d: -f1)"
  review_line="$(grep -n 'toolu-review:review' "$SKILL" | cut -d: -f1)"
  verdict_line="$(grep -n 'verdict.sh status' "$SKILL" | cut -d: -f1)"
  push_line="$(grep -ni 'push the non-default feature branch' "$SKILL" | cut -d: -f1)"
  [ "$commit_line" -lt "$ledger_line" ]
  [ "$ledger_line" -lt "$review_line" ]
  [ "$review_line" -lt "$verdict_line" ]
  [ "$verdict_line" -lt "$push_line" ]
}

@test "execution only delivers with authorization and all delivery prerequisites" {
  grep -qi 'delivery authorization' "$SKILL"
  grep -qi 'GitHub auth' "$SKILL"
  grep -qi 'non-default branch' "$SKILL"
  grep -qi 'pr-babysit.*installed' "$SKILL"
}

@test "execution makes brainstorm optional upstream triage" {
  grep -qiE 'brainstorm.*optional upstream triage' "$SKILL"
}

@test "execution commits pushes creates or finds a default-branch PR then invokes babysit" {
  grep -qiE 'commit.*scoped changes' "$SKILL"
  grep -qi 'push' "$SKILL"
  grep -qiE 'locate.*or create.*pull request|create.*or locate.*pull request' "$SKILL"
  grep -qi 'repository default branch' "$SKILL"
  grep -qiE 'verify.*PR.*number.*head/base|verify.*head/base.*branches' "$SKILL"
  grep -Fq '$pr-babysit:babysit' "$SKILL"
}

@test "execution has no execution-review or terminal-test handoff" {
  ! grep -qi 'execution-review' "$SKILL"
  ! grep -qi 'then to `test` for the final pass' "$SKILL"
}
