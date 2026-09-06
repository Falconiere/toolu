#!/usr/bin/env bats
# Adaptive Brainstorm contract — keeps triage and its public guidance aligned.

setup() {
  SKILLS="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
  ROOT="$(cd "$SKILLS/../../.." && pwd)"
  BRAINSTORM="$SKILLS/brainstorm/SKILL.md"
}

@test "Brainstorm triages work into skip, compact, and full paths" {
  grep -Fq '## Triage' "$BRAINSTORM"
  grep -Fq '**Skip**' "$BRAINSTORM"
  grep -Fq '**Compact**' "$BRAINSTORM"
  grep -Fq '**Full**' "$BRAINSTORM"
  grep -Fq 'mechanical work with no design decision' "$BRAINSTORM"
}

@test "Brainstorm defaults and asks only one highest-blast-radius question when necessary" {
  grep -Fq 'one structured question (2–3 options)' "$BRAINSTORM"
  grep -Fq 'highest-blast-radius' "$BRAINSTORM"
  grep -Fq 'record defaults and risks for the rest' "$BRAINSTORM"
  grep -Fq 'goal-defining or hard-to-reverse fork' "$BRAINSTORM"
}

@test "Brainstorm grounds design in targeted evidence and delegates conditionally" {
  grep -Fq 'memory recall, one targeted structural or exact-text search' "$BRAINSTORM"
  grep -Fq 'inspect the best hits' "$BRAINSTORM"
  grep -Fq 'Delegate only when the search needs a broad map' "$BRAINSTORM"
  grep -Fq 'final trade-off decision' "$BRAINSTORM"
}

@test "Brainstorm compact path records its required capsule and hands off appropriately" {
  grep -Fq 'Outcome' "$BRAINSTORM"
  grep -Fq 'Material defaults/non-goal' "$BRAINSTORM"
  grep -Fq 'Repository evidence' "$BRAINSTORM"
  grep -Fq 'Risk' "$BRAINSTORM"
  grep -Fq 'Handoff' "$BRAINSTORM"
  grep -Fq 'handoff to `spec`' "$BRAINSTORM"
  grep -Fq 'straight to `plan`' "$BRAINSTORM"
}

@test "Brainstorm preserves host-neutral structured-question guidance" {
  grep -Fq 'structured question' "$ROOT/plugins/toolu/workflows/host-mapping.md"
  grep -Fq 'goal-defining or hard-to-reverse fork' "$ROOT/plugins/toolu/workflows/host-mapping.md"
  ! grep -Fq 'AskUserQuestion` —' "$BRAINSTORM"
}

@test "Brainstorm documentation describes materiality-based triage" {
  for doc in "$ROOT/README.md" "$ROOT/plugins/toolu/README.md" "$ROOT/docs/toolu/README.md"; do
    grep -Fq 'Brainstorm' "$doc"
    grep -Fq 'default-and-proceed' "$doc"
  done
}
