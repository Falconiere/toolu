#!/usr/bin/env bash
# Shared bats helpers for push-review.sh tests.
#
# Each test gets a sandbox: a temp git repo with a `development` base branch,
# a feature branch with one commit, and a writable `.claude/tmp/push-review/`
# dir. The hook module is invoked as a subprocess, with the standard
# dispatcher env vars (`tool_name`, `input`) exported, and JSON payload on
# stdin.

# Four levels up from modules/__tests__/ is the dir that CONTAINS hooks/ —
# now plugins/toolu (was the repo root before the Plan 2 reorg).
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
HOOK_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/push-review.sh"

setup_sandbox() {
  export SANDBOX="$(mktemp -d)"
  export STATE_DIR="$SANDBOX/.claude/tmp/push-review"
  mkdir -p "$STATE_DIR"

  # Config isolation: point both config layers at the sandbox so a real
  # ~/.claude/toolu.config.json on the machine running the suite cannot change
  # what mode these tests resolve.
  export HOME="$SANDBOX/home"
  export TOOLU_PROJECT_DIR="$SANDBOX"
  export CLAUDE_PROJECT_DIR="$SANDBOX"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_HOST_OVERRIDE
  mkdir -p "$HOME/.claude" "$SANDBOX/.claude"

  cd "$SANDBOX"
  git init -q -b development .
  git config user.email "test@example.com"
  git config user.name "Test"
  echo "base" > base.txt
  git add base.txt
  git commit -q -m "base commit"

  git checkout -q -b feat/example
  echo "feature" > feature.txt
  git add feature.txt
  git commit -q -m "feature commit"
}

# Pin the gate preset (or a single gate's mode) for a test.
#   gate_config '{"gates":{"preset":"strict"}}'
# Most of the pre-existing cases below describe BLOCK behavior, which is now
# the `strict` preset rather than the built-in default — they say so explicitly
# instead of relying on whatever ships as the default.
gate_config() {
  printf '%s' "$1" > "$SANDBOX/.claude/toolu.config.json"
}

# Shorthand for the common case.
use_strict_preset() {
  gate_config '{"version":1,"gates":{"preset":"strict"}}'
}

# Opt-in prompt path: waivers and ask-mode delivery only fire when asked.
use_ask_mode() {
  gate_config '{"version":1,"gates":{"pushReview":{"mode":"ask"}}}'
}

teardown_sandbox() {
  [[ -n "${SANDBOX:-}" && -d "$SANDBOX" ]] && rm -rf "$SANDBOX"
}

# Build a PreToolUse JSON payload for a given command.
# Usage: build_input "git push origin feat/example"
build_input() {
  local cmd="$1"
  jq -n --arg cmd "$cmd" '{
    tool_name: "Bash",
    tool_input: { command: $cmd }
  }'
}

# Run the hook against a payload. Output goes to $output; status to $status.
# Tests use the `development` base branch fixture, so PUSH_REVIEW_BASE forces
# the project-agnostic detect_base_branch fallback to honor that.
# Usage: run_hook "Bash" "$(build_input 'git push')"
run_hook() {
  local tool_name="$1"
  local payload="$2"
  tool_name="$tool_name" input="$payload" PUSH_REVIEW_BASE=development \
    run bash "$HOOK_SCRIPT" <<<"$payload"
}

# Compute the current branch diff SHA the same way the hook does.
current_diff_sha() {
  git diff --no-color "development...HEAD" | git hash-object --stdin
}

# Write a state file with given SHA, findings count, and (optional) round.
# reviewed_files is computed from the CURRENT real diff against development
# (not from $sha, which callers sometimes set to a stale/bogus value on
# purpose) — it always reflects actual file coverage for whatever the branch
# looks like at call time.
# Usage: write_state <sha> <findings_count> [<review_round>]
write_state() {
  local sha="$1"
  local count="$2"
  local round="${3:-1}"
  local branch
  branch=$(git rev-parse --abbrev-ref HEAD)
  local slug
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  [[ -z "$slug" ]] && slug="_default"
  local reviewed_files
  reviewed_files=$(git diff --no-color "development...HEAD" --name-only \
    | jq -R -s -c 'split("\n") | map(select(length > 0))')
  jq -n \
    --arg branch "$branch" \
    --arg sha "$sha" \
    --argjson count "$count" \
    --argjson round "$round" \
    --argjson reviewed_files "$reviewed_files" \
    '{
      version: 2,
      branch: $branch,
      diff_sha: $sha,
      base_branch: "development",
      reviewed_at: "2026-06-07T00:00:00Z",
      reviewers: ["code-review"],
      findings_count: $count,
      review_round: $round,
      reviewed_files: $reviewed_files,
      findings: []
    }' > "$STATE_DIR/${slug}.json"
}
