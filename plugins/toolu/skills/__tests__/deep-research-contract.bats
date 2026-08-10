#!/usr/bin/env bats
#
# Deep-research skill contract — pins AC-1..AC-7 from the approved spec against
# the real shipped SKILL.md (and its doc-sync surfaces) so the skill's prose
# and structure can't drift out from under the acceptance criteria.

setup() {
  SKILLS="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
  ROOT="$(cd "$SKILLS/../../.." && pwd)"
}

@test "AC-1: frontmatter name and description carry the trigger phrases and deflections" {
  grep -q 'name: deep-research' "$SKILLS/deep-research/SKILL.md"
  grep -q 'deep research' "$SKILLS/deep-research/SKILL.md"
  grep -q 'cited report' "$SKILLS/deep-research/SKILL.md"
  grep -q 'research-agent' "$SKILLS/deep-research/SKILL.md"
  grep -q 'deep-explore' "$SKILLS/deep-research/SKILL.md"
}

@test "AC-2: approval gate blocks research behind AskUserQuestion" {
  grep -q 'AskUserQuestion' "$SKILLS/deep-research/SKILL.md"
  grep -q 'no research runs before approval' "$SKILLS/deep-research/SKILL.md"
}

@test "AC-3: fan-out uses research-agent workers with both engines and the routing rubric" {
  grep -q 'research-agent' "$SKILLS/deep-research/SKILL.md"
  grep -q 'exa-search' "$SKILLS/deep-research/SKILL.md"
  grep -q 'context7' "$SKILLS/deep-research/SKILL.md"
  grep -q 'model-routing.md' "$SKILLS/deep-research/SKILL.md"
}

@test "AC-4: deliverable is a cited report under docs/research/ with the required sections" {
  grep -q 'docs/research/' "$SKILLS/deep-research/SKILL.md"
  grep -q 'TL;DR' "$SKILLS/deep-research/SKILL.md"
  grep -q 'Reliability notes' "$SKILLS/deep-research/SKILL.md"
  grep -q 'never overwrite' "$SKILLS/deep-research/SKILL.md"
}

@test "AC-5: verification never silently drops findings and flags no-live-source runs" {
  grep -q 'never silently kept' "$SKILLS/deep-research/SKILL.md"
  grep -q 'unverified — no live sources' "$SKILLS/deep-research/SKILL.md"
}

@test "AC-6: deep-research is synced across the repo's doc surfaces" {
  grep -q 'deep-research' "$ROOT/plugins/toolu/README.md"
  grep -q 'deep-research' "$ROOT/docs/toolu/README.md"
  grep -q 'deep-research' "$ROOT/README.md"
}

@test "AC-7: SKILL.md stays within the 120-line skill budget" {
  local lines
  lines="$(wc -l < "$SKILLS/deep-research/SKILL.md")"
  [ "$lines" -le 120 ]
}
