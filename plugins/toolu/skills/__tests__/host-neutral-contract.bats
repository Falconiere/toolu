#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"

@test "published helper paths name unambiguous defaults for both hosts" {
  skills=(
    "plugins/agent-browser/skills/agent-browser/SKILL.md"
    "plugins/comemory/skills/agent-memory/SKILL.md"
    "plugins/context7/skills/context7/SKILL.md"
    "plugins/exa-search/skills/exa-search/SKILL.md"
    "plugins/git-better/skills/git-better/SKILL.md"
    "plugins/jira/skills/jira/SKILL.md"
    "plugins/toolu-review/skills/review/SKILL.md"
    "plugins/toolu/agents/research-agent.md"
  )

  for skill in "${skills[@]}"; do
    grep -Fq '${CODEX_HOME:-$HOME/.codex}' "$ROOT/$skill"
    grep -Fq '${CLAUDE_CONFIG_DIR:-$HOME/.claude}' "$ROOT/$skill"
    ! grep -Fq '${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}' "$ROOT/$skill"
  done
}

@test "Codex-only command skills pass an explicit host to shared shell workflows" {
  grep -Fq 'TOOLU_HOST_OVERRIDE=codex' \
    "$ROOT/plugins/statusline/skills/status/SKILL.md"
  grep -Fq 'TOOLU_HOST_OVERRIDE=codex' \
    "$ROOT/plugins/comemory/skills/setup/SKILL.md"
  grep -Fq 'TOOLU_HOST_OVERRIDE=claude' \
    "$ROOT/plugins/comemory/commands/setup.md"
  grep -Fq 'TOOLU_HOST_OVERRIDE=codex' \
    "$ROOT/plugins/toolu-review/skills/review/SKILL.md"
  grep -Fq 'TOOLU_HOST_OVERRIDE=claude' \
    "$ROOT/plugins/toolu-review/skills/review/SKILL.md"
}

@test "toolu skills map structured questions and delegation through the host reference" {
  for skill in \
    "$ROOT/plugins/toolu/skills/brainstorm/SKILL.md" \
    "$ROOT/plugins/toolu/skills/deep-research/SKILL.md" \
    "$ROOT/plugins/toolu/skills/orchestrator/SKILL.md" \
    "$ROOT/plugins/toolu/skills/plan/SKILL.md"; do
    grep -Fq 'workflows/host-mapping.md' "$skill"
  done

  ! grep -Fq 'alone holds the Agent tool' "$ROOT/plugins/toolu/skills/orchestrator/SKILL.md"
  ! grep -Fq 'AskUserQuestion` —' "$ROOT/plugins/toolu/skills/brainstorm/SKILL.md"
}

@test "host-neutral skills do not instruct Codex to invoke Claude slash commands" {
  ! rg -n 'Run `/comemory:setup|re-run `/comemory:setup' \
    "$ROOT/plugins/comemory/skills/agent-memory/SKILL.md"
}

@test "published helpers and registry modules preserve Codex config-root precedence" {
  files=(
    "plugins/ast-grep/hooks/post-tools.d/byte-savings.sh"
    "plugins/git-better/hooks/pre-tools.d/git-conventions-nudge.sh"
    "plugins/git-better/hooks/post-tools.d/git-byte-savings.sh"
    "plugins/git-better/scripts/lib/conventions-cache.sh"
    "plugins/comemory/hooks/pre-tools.d/comemory-scope.sh"
    "plugins/jira/skills/jira/scripts/lib/plan-run.sh"
  )

  for file in "${files[@]}"; do
    grep -Fq 'CODEX_HOME' "$ROOT/$file" || {
      echo "missing Codex config-root fallback: $file"
      return 1
    }
  done
}
