#!/usr/bin/env bats
# Tests for hooks/pre-tools/modules/docs-sync.sh — advisory docs-sync backstop.
# Real git fixtures (no mocks). Acceptance criteria from the spec are tagged.

load docs-sync-helpers

setup() { setup_sandbox; }
teardown() { teardown_sandbox; }

# --- criterion 4 / 4a: degenerate states are silent no-ops -------------------

@test "docs-sync: non-Bash tool exits silently" {
  commit_file lib/foo.sh "echo hi"
  run_hook "Edit" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: Bash but not git push exits silently" {
  commit_file lib/foo.sh "echo hi"
  run_hook "Bash" "$(build_input 'ls -la')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: empty diff against base is a silent no-op" {
  # No feature commit diverged from base.
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: detached HEAD is a silent no-op" {
  commit_file lib/foo.sh "echo hi"
  git checkout -q "$(git rev-parse HEAD)"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: missing base branch is a silent no-op" {
  commit_file lib/foo.sh "echo hi"
  tool_name="Bash" input="$(build_input 'git push')" \
    DOCS_SYNC_BASE=nope DOCS_SYNC_STATE_DIR="$STATE_DIR" \
    TOOLU_CONFIG_DIR="$EMPTY_CFG_DIR" TOOLU_PROJECT_DIR="$SANDBOX" \
    run bash "$HOOK_SCRIPT" <<<"$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# --- criterion 1 + 5: code-only diff fires advisory, never a deny ------------

@test "docs-sync: code change with no doc surface fires an advisory" {
  commit_file lib/foo.sh "echo hi"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'
  echo "$output" | jq -e '.hookSpecificOutput.hookEventName == "PreToolUse"'
}

@test "docs-sync: advisory is never a blocking decision (advisory-only invariant)" {
  commit_file lib/foo.sh "echo hi"
  run_hook "Bash" "$(build_input 'git push')"
  refute_deny
  # An advisory was indeed produced...
  echo "$output" | jq -e 'has("hookSpecificOutput")'
  # ...with no permissionDecision field at all.
  echo "$output" | jq -e '.hookSpecificOutput | has("permissionDecision") | not'
}

# --- criterion 2: a doc touch silences it ------------------------------------

@test "docs-sync: code change WITH a README touch is silent" {
  commit_file lib/foo.sh "echo hi"
  commit_file README.md "# updated"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: code change WITH a nested docs/ guide touch is silent" {
  commit_file lib/foo.sh "echo hi"
  commit_file docs/guide/usage.md "how to"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: code change WITH a SKILL.md trigger touch is silent" {
  commit_file lib/foo.sh "echo hi"
  commit_file skills/thing/SKILL.md "trigger text"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# --- criterion 7: release notes do NOT satisfy the check ---------------------

@test "docs-sync: code change with ONLY a release-note touch still fires" {
  commit_file lib/foo.sh "echo hi"
  commit_file docs/releases/v9.9.0.md "notes"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'
}

# --- criterion 3: attestation suppresses; stale attestation does not ---------

@test "docs-sync: a fresh diff-sha attestation silences the advisory" {
  commit_file lib/foo.sh "echo hi"
  write_attestation "$(current_diff_sha)"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: a stale attestation (diff changed) does NOT silence it" {
  commit_file lib/foo.sh "echo hi"
  write_attestation "$(current_diff_sha)"
  # Diff changes -> stored sha no longer matches.
  commit_file lib/bar.sh "echo bye"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'
}

@test "docs-sync: a corrupted attestation file falls safe (still fires)" {
  commit_file lib/foo.sh "echo hi"
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  printf '%s' 'not json at all' > "$STATE_DIR/${slug}.json"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'
}

# --- criterion 6: config override of the surface set --------------------------

@test "docs-sync: a project surfaces override is honored" {
  mkdir -p "$SANDBOX/.claude"
  printf '%s' '{"docsSync":{"surfaces":["wiki/*.md"]}}' \
    > "$SANDBOX/.claude/toolu.config.json"
  # A wiki page now counts as a doc surface -> silent.
  commit_file lib/foo.sh "echo hi"
  commit_file wiki/page.md "doc"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "docs-sync: under a surfaces override, README no longer counts" {
  mkdir -p "$SANDBOX/.claude"
  printf '%s' '{"docsSync":{"surfaces":["wiki/*.md"]}}' \
    > "$SANDBOX/.claude/toolu.config.json"
  commit_file lib/foo.sh "echo hi"
  commit_file README.md "# updated"   # not a surface under the override
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'
}

# --- doc-only change does not fire (nothing user-facing to flag) --------------

@test "docs-sync: a doc-only change (no code) is silent" {
  commit_file README.md "# updated"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# --- dispatcher auto-discovery ------------------------------------------------

@test "docs-sync: module is picked up by the modules/*.sh glob" {
  shopt -s nullglob
  local found=0 m
  for m in "$REPO_ROOT"/hooks/pre-tools/modules/*.sh; do
    [[ "$m" == *docs-sync.sh ]] && found=1
  done
  [ "$found" -eq 1 ]
}
