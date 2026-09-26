#!/usr/bin/env bats
# Public brainstorm skill contract.

setup() {
  SKILLS="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
  ROOT="$(cd "$SKILLS/../../.." && pwd)"
  BRAINSTORM="$SKILLS/brainstorm/SKILL.md"
  QUESTIONS="$SKILLS/brainstorm/references/design-questions.md"
}

@test "brainstorm is a public skill with a design question bank" {
  [ -f "$BRAINSTORM" ]
  [ -f "$QUESTIONS" ]
  grep -Fxq 'name: brainstorm' "$BRAINSTORM"
  grep -Fq 'references/design-questions.md' "$BRAINSTORM"
  grep -Fq '## Material axes' "$QUESTIONS"
}

@test "Brainstorm triages work into minimal, compact, and full paths" {
  grep -Fq '## Triage' "$BRAINSTORM"
  grep -Fq '**Minimal**' "$BRAINSTORM"
  grep -Fq '**Compact**' "$BRAINSTORM"
  grep -Fq '**Full**' "$BRAINSTORM"
  grep -Fq 'mechanical work with no design decision' "$BRAINSTORM"
}

@test "Brainstorm defaults and asks only one highest-blast-radius question when necessary" {
  grep -Fq 'one structured question (2–3 options)' "$BRAINSTORM"
  grep -Fq 'highest-blast-radius' "$BRAINSTORM"
  grep -Fq 'record defaults and risks for the rest' "$BRAINSTORM"
  grep -Fq 'goal-defining or hard-to-reverse fork' "$BRAINSTORM"
  grep -qi 'default-and-proceed' "$BRAINSTORM"
}

@test "Brainstorm grounds design in targeted evidence and delegates conditionally" {
  grep -Fq 'memory recall, one targeted structural or exact-text search' "$BRAINSTORM"
  grep -Fq 'inspect the best hits' "$BRAINSTORM"
  grep -Fq 'Delegate only when the search needs a broad map' "$BRAINSTORM"
  grep -Fq 'final trade-off decision' "$BRAINSTORM"
}

@test "Brainstorm records the capsule fields" {
  for field in 'Outcome' 'Material defaults/non-goal' 'Repository evidence' 'Risk' 'Handoff'; do
    grep -Fq "**$field:**" "$BRAINSTORM"
  done
}

@test "Brainstorm runs standalone without delivery and hands off to spec inside delivery-flow" {
  grep -Fq '## Modes' "$BRAINSTORM"
  grep -Fq '**Standalone**' "$BRAINSTORM"
  grep -Fq '**Delivery**' "$BRAINSTORM"
  grep -Fq 'never edits code, commits, or starts delivery' "$BRAINSTORM"
  grep -Fq 'hand off to `spec`' "$BRAINSTORM"
}

@test "Brainstorm always posts the capsule and writes a file only on the Full path or request" {
  grep -Fq 'Always post the capsule in chat' "$BRAINSTORM"
  grep -Fq 'docs/toolu/brainstorms/<YYYY-MM-DD>-<slug>.md' "$BRAINSTORM"
  grep -Fq 'only when the Full path runs or the user asks for a file' "$BRAINSTORM"
  grep -Fq 'never overwrite' "$BRAINSTORM"
}

@test "Brainstorm maps host capabilities without naming one host's tool as the only path" {
  grep -Fq 'AskUserQuestion' "$BRAINSTORM"
  grep -Fq 'request_user_input' "$BRAINSTORM"
  grep -Fq 'Jev' "$BRAINSTORM"
  ! grep -Fq 'AskUserQuestion` —' "$BRAINSTORM"
}

@test "brainstorm manifests share identity and declare no dependencies" {
  claude="$ROOT/plugins/brainstorm/.claude-plugin/plugin.json"
  codex="$ROOT/plugins/brainstorm/.codex-plugin/plugin.json"
  [ "$(jq -r .name "$claude")" = brainstorm ]
  [ "$(jq -r .version "$claude")" = "$(jq -r .version "$codex")" ]
  [ "$(jq -r .description "$claude")" = "$(jq -r .description "$codex")" ]
  [ "$(jq -r .skills "$codex")" = "./skills/" ]
  [ "$(jq '.dependencies // [] | length' "$claude")" -eq 0 ]
}

@test "docs describe the brainstorm plugin and the delivery-flow dependency" {
  for doc in "$ROOT/README.md" "$ROOT/docs/plugins/index.md" "$ROOT/docs/toolu/README.md" "$ROOT/docs/opencode.md" "$ROOT/plugins/delivery-flow/README.md"; do
    grep -Fq 'brainstorm' "$doc"
    grep -Eq '`brainstorm`|\*\*brainstorm\*\*|brainstorm:brainstorm' "$doc" \
      || { echo "$doc: does not name the brainstorm plugin" >&2; return 1; }
  done
  grep -Fq 'also depends on `toolu-review`, `pr-babysit`, and `brainstorm`' "$ROOT/docs/cli.md"
  grep -Fq '/brainstorm:brainstorm' "$ROOT/README.md"
  grep -Eq '── brainstorm/ +# ' "$ROOT/README.md"
}
