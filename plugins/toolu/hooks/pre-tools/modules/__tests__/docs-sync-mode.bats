#!/usr/bin/env bats
# Tests for docs-sync.sh's `docsSync.mode` (advise|block|off) promotion.
# Real git fixtures (no mocks), real project toolu.config.json files.
# docs-sync.bats covers the advise-default behavior in depth (surfaces,
# attestation, excludes, etc.); this file covers the mode gate itself.

bats_require_minimum_version 1.5.0

load docs-sync-helpers

setup() { setup_sandbox; }
teardown() { teardown_sandbox; }

# Slug-keyed telemetry file path, mirroring write_attestation's own slugging
# (branch_slug in lib/detect.sh) and telemetry_append's default location
# ($(detect_project_root)/.claude/tmp/telemetry — repo_root here is $SANDBOX,
# since the test's cwd never leaves it).
_telemetry_file() {
  local branch slug
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  [[ -z "$slug" ]] && slug="_default"
  printf '%s' "$SANDBOX/.claude/tmp/telemetry/${slug}.jsonl"
}

_set_mode() {
  mkdir -p "$SANDBOX/.claude"
  printf '{"docsSync":{"mode":"%s"}}' "$1" > "$SANDBOX/.claude/toolu.config.json"
}

# (a) mode=block, code-without-doc-or-attestation -> push denied, reason mentions docs.
@test "docs-sync mode=block: code-without-doc, no attestation -> push denied, reason mentions docs" {
  _set_mode block
  commit_file lib/foo.sh "echo hi"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.hookEventName == "PreToolUse"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("docs-sync")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("attest")'
  [ -f "$(_telemetry_file)" ]
  [ "$(jq -r 'select(.event=="docs_nudge") | .event' "$(_telemetry_file)" | tail -1)" = "docs_nudge" ]
}

# (b) default (no config) is advise: existing nudge behavior intact + docs_nudge event.
@test "docs-sync default mode (advise): allow + advisory context + docs_nudge telemetry" {
  commit_file lib/foo.sh "echo hi"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'
  echo "$output" | jq -e '.hookSpecificOutput | has("permissionDecision") | not'
  [ -f "$(_telemetry_file)" ]
  [ "$(jq -r 'select(.event=="docs_nudge") | .event' "$(_telemetry_file)" | tail -1)" = "docs_nudge" ]
}

# (c) mode=block + a valid attestation at the module's own resolution path -> allow + docs_attested.
@test "docs-sync mode=block: valid attestation (module's own path resolution) -> allow + docs_attested" {
  _set_mode block
  commit_file lib/foo.sh "echo hi"
  write_attestation "$(current_diff_sha)"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ -f "$(_telemetry_file)" ]
  [ "$(jq -r 'select(.event=="docs_attested") | .event' "$(_telemetry_file)" | tail -1)" = "docs_attested" ]
}

# (d) mode=block + a doc file present in the diff -> allow (nothing to flag at all).
@test "docs-sync mode=block: doc file present in diff -> allow, no output, no deny" {
  _set_mode block
  commit_file lib/foo.sh "echo hi"
  commit_file README.md "# updated"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# (e) mode=off -> allow, no advisory, no telemetry at all (not even docs_nudge).
@test "docs-sync mode=off: allow, no advisory, no telemetry file" {
  _set_mode off
  commit_file lib/foo.sh "echo hi"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -d "$SANDBOX/.claude/tmp/telemetry" ]
}

# (e-cont) off must be indistinguishable from "doesn't exist" even when an
# attestation IS present and would otherwise fire docs_attested telemetry.
@test "docs-sync mode=off: valid attestation present still emits NO telemetry (off means off)" {
  _set_mode off
  commit_file lib/foo.sh "echo hi"
  write_attestation "$(current_diff_sha)"
  run_hook "Bash" "$(build_input 'git push')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -d "$SANDBOX/.claude/tmp/telemetry" ]
}

# (f) an unrecognized mode value warns (toolu_string contract) and falls back to advise.
@test "docs-sync mode=junk: warns on stderr and falls back to advise behavior" {
  _set_mode junk
  commit_file lib/foo.sh "echo hi"
  payload="$(build_input 'git push')"
  # --separate-stderr: the docsSync.mode rejection warning must not corrupt
  # the JSON stdout the dispatcher parses.
  run --separate-stderr env tool_name="Bash" input="$payload" \
    DOCS_SYNC_BASE=development DOCS_SYNC_STATE_DIR="$STATE_DIR" \
    TOOLU_CONFIG_DIR="$EMPTY_CFG_DIR" TOOLU_PROJECT_DIR="$SANDBOX" \
    bash "$HOOK_SCRIPT" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$stderr" | grep -q 'docsSync.mode'
  echo "$stderr" | grep -q 'is not an allowed value'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'
  echo "$output" | jq -e '.hookSpecificOutput | has("permissionDecision") | not'
}
