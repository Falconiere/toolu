#!/usr/bin/env bash
# Shared bats helpers for docs-sync.sh tests.
#
# Each test gets a sandbox: a temp git repo with a `development` base branch and
# a `feat/example` feature branch (no feature commit yet — tests commit the
# files they want in the diff). Config is isolated from the real user config via
# an empty TOOLU_CONFIG_DIR; the project root is the sandbox itself.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
HOOK_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/docs-sync.sh"

setup_sandbox() {
  SANDBOX="$(mktemp -d)"
  export SANDBOX
  export STATE_DIR="$SANDBOX/.claude/tmp/docs-sync"
  export EMPTY_CFG_DIR="$SANDBOX/agent"   # no toolu.config.json -> defaults
  mkdir -p "$STATE_DIR" "$EMPTY_CFG_DIR"

  cd "$SANDBOX" || return 1
  git init -q -b development .
  git config user.email "test@example.com"
  git config user.name "Test"
  echo "base" > base.txt
  git add base.txt
  git commit -q -m "base commit"
  git checkout -q -b feat/example
}

teardown_sandbox() {
  [[ -n "${SANDBOX:-}" && -d "$SANDBOX" ]] && rm -rf "$SANDBOX"
}

# Commit a file (creating parent dirs) into the current branch.
# Usage: commit_file path/to/file.ext "contents"
commit_file() {
  local path="$1" body="${2:-x}"
  mkdir -p "$(dirname "$path")"
  printf '%s\n' "$body" > "$path"
  git add "$path"
  git commit -q -m "add $path"
}

build_input() {
  jq -n --arg cmd "$1" '{ tool_name: "Bash", tool_input: { command: $cmd } }'
}

# Run the module against a payload. Forces base=development and isolates config.
# Usage: run_hook "Bash" "$(build_input 'git push')"
run_hook() {
  local tool_name="$1" payload="$2"
  tool_name="$tool_name" input="$payload" \
    DOCS_SYNC_BASE=development DOCS_SYNC_STATE_DIR="$STATE_DIR" \
    TOOLU_CONFIG_DIR="$EMPTY_CFG_DIR" TOOLU_PROJECT_DIR="$SANDBOX" \
    run bash "$HOOK_SCRIPT" <<<"$payload"
}

# Diff sha the module keys the attestation on.
current_diff_sha() {
  git diff --no-color "development...HEAD" | git hash-object --stdin
}

# Write an attestation state file for the current branch.
# Usage: write_attestation <diff_sha> [decision]
write_attestation() {
  local sha="$1" decision="${2:-not-needed}" branch slug
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  [[ -z "$slug" ]] && slug="_default"
  jq -n --arg sha "$sha" --arg d "$decision" --arg b "$branch" '{
    version: 1, branch: $b, diff_sha: $sha, base_branch: "development",
    decision: $d, note: "covered by test", attested_at: "2026-06-15T00:00:00Z"
  }' > "$STATE_DIR/${slug}.json"
}

# Assert the module did NOT emit a blocking decision (advisory-only invariant).
refute_deny() {
  [ "$status" -eq 0 ]
  if [ -n "$output" ]; then
    run_jq=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')
    [ "$run_jq" = "none" ]
  fi
}
