#!/usr/bin/env bats
# Tests for the model-routing surface: the SessionStart injection
# (hooks/docs/model-routing.md rendered by session-start.sh) and the tier
# pinned in each pre-built agent's frontmatter. Runs the REAL hook. No mocks.

bats_require_minimum_version 1.5.0

HOOK="${BATS_TEST_DIRNAME}/../session-start.sh"
AGENTS="${BATS_TEST_DIRNAME}/../../agents"

setup() {
  TMP=$(mktemp -d)
  CFG="$TMP/cfg"
  mkdir -p "$CFG"
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
}

teardown() {
  cd /
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Run the hook with an isolated config dir so the developer's own
# ~/.claude/toolu.config.json cannot leak into the assertions. stderr is kept
# separate: config warnings must never be confused with injected context.
_run_hook() {
  run --separate-stderr env CLAUDE_CONFIG_DIR="$CFG" TOOLU_CONFIG_DIR="$CFG" \
      TOOLU_PROJECT_DIR="$TMP" bash "$HOOK" <<<"{\"source\":\"${1:-startup}\"}"
}

@test "session-start: injects the routing block with the default tiers" {
  _run_hook startup
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'Model routing (mandatory)'
  echo "$output" | grep -q 'mechanical (`haiku`)'
  echo "$output" | grep -q 'exploration (`sonnet`)'
  echo "$output" | grep -q 'architecture (`opus`)'
}

@test "session-start: leaves no unsubstituted model placeholder" {
  _run_hook startup
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q '{{model_'
}

@test "session-start: routing block survives a compact" {
  # Post-compaction is exactly where delegation otherwise reverts to one model
  # for everything, so the block must be re-injected on that event too.
  _run_hook compact
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'Model routing (mandatory)'
}

@test "session-start: a config remap changes the injected tier" {
  echo '{"version":1,"models":{"review":"opus","mechanical":"sonnet"}}' > "$CFG/toolu.config.json"
  _run_hook startup
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'review (`opus`)'
  echo "$output" | grep -q 'mechanical (`sonnet`)'
}

@test "session-start: an unroutable alias falls back to the default tier" {
  echo '{"version":1,"models":{"review":"gpt-9"}}' > "$CFG/toolu.config.json"
  _run_hook startup
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'review (`sonnet`)'
  # The rejected value must never reach the injected context — only the warning.
  ! echo "$output" | grep -q 'gpt-9'
  echo "$stderr" | grep -q "models.review: 'gpt-9' is not a model alias"
}

@test "session-start: models.enabled=false suppresses the block" {
  echo '{"version":1,"models":{"enabled":false}}' > "$CFG/toolu.config.json"
  _run_hook startup
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'Model routing (mandatory)'
  # The rest of the protocol must still be injected.
  echo "$output" | grep -q 'Session Protocol'
}

@test "agents: the tier ladder is pinned in frontmatter, cheapest to most capable" {
  grep -q '^model: haiku$'  "$AGENTS/quick-task.md"
  grep -q '^model: sonnet$' "$AGENTS/deep-explore.md"
  grep -q '^model: sonnet$' "$AGENTS/research-agent.md"
  grep -q '^model: sonnet$' "$AGENTS/implementer.md"
  grep -q '^model: opus$'   "$AGENTS/architect.md"
}

@test "agents: every pre-built agent declares exactly one model" {
  for f in "$AGENTS"/*.md; do
    [ "$(grep -c '^model: ' "$f")" -eq 1 ]
  done
}

@test "agents: the cheap and mid tiers document the escalation path" {
  # quick-task and implementer must hand work up rather than guess — that is
  # what makes routing down to them safe.
  grep -q 'ESCALATE' "$AGENTS/quick-task.md"
  grep -q 'ESCALATE' "$AGENTS/implementer.md"
}

@test "agents: the top tier is read-only (no edit tools)" {
  ! grep -E '^tools:.*(Edit|Write)' "$AGENTS/architect.md"
}

@test "routing rubric: the reference doc exists and is cited by the workflow skills" {
  local ref="plugins/toolu/skills/orchestrator/references/model-routing.md"
  local root="${BATS_TEST_DIRNAME}/../../../.."
  [ -f "$root/$ref" ]
  for skill in orchestrator brainstorm plan execution; do
    grep -q 'model-routing.md' "$root/plugins/toolu/skills/$skill/SKILL.md"
  done
}
