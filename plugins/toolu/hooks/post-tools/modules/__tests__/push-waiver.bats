#!/usr/bin/env bats
# Tests for hooks/post-tools/modules/push-waiver.sh.
#
# Real repo, real branch diff, real SHAs — the module's entire job is deciding
# whether a pending marker matches the code that was just pushed.

HOOK="${BATS_TEST_DIRNAME}/../push-waiver.sh"

setup() {
  TMP=$(mktemp -d)
  SANDBOX="$TMP/repo"
  mkdir -p "$SANDBOX"
  cd "$SANDBOX"
  git init -q -b development
  git config user.email t@t
  git config user.name t
  echo base > base.txt && git add base.txt && git commit -qm "base"
  git checkout -qb feat/example
  echo feature > feature.txt && git add feature.txt && git commit -qm "feature"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
  export STATE_DIR="$SANDBOX/.claude/tmp/push-review"
  mkdir -p "$STATE_DIR"
  export PUSH_REVIEW_BASE=development
  SHA=$(git diff --no-color development...HEAD | git hash-object --stdin)
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

pend() {
  local sha="${1:-$SHA}"
  STATE_DIR="$STATE_DIR" bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/push-waiver.sh"
    push_waiver_pend "'"$SANDBOX"'" feat_example "'"$sha"'" development no-state
  '
}

# Run the module with a PostToolUse payload. $1 command, $2 exit code ("" for
# a host that reports none), $3 interrupted flag.
run_module() {
  local cmd="$1" code="${2:-0}" interrupted="${3:-false}"
  local payload
  payload=$(jq -n --arg c "$cmd" --arg code "$code" --argjson int "$interrupted" '
    {tool_name:"Bash", tool_input:{command:$c},
     tool_response: ({stdout:"", interrupted:$int}
       + (if $code == "" then {} else {exit_code: ($code|tonumber)} end))}')
  tool_name=Bash input="$payload" run bash "$HOOK" <<<"$payload"
}

@test "a successful push promotes the pending marker" {
  pend
  run_module "git push" 0
  [ "$status" -eq 0 ]
  [ -f "$STATE_DIR/feat_example.waiver.json" ]
  [ "$(jq -r '.diff_sha' "$STATE_DIR/feat_example.waiver.json")" = "$SHA" ]
  [ ! -f "$STATE_DIR/feat_example.pending-waiver.json" ]
}

@test "a failed push leaves the pending marker for the retry" {
  pend
  run_module "git push" 1
  [ "$status" -eq 0 ]
  [ ! -f "$STATE_DIR/feat_example.waiver.json" ]
  [ -f "$STATE_DIR/feat_example.pending-waiver.json" ]
}

@test "an interrupted push promotes nothing" {
  pend
  run_module "git push" "" true
  [ ! -f "$STATE_DIR/feat_example.waiver.json" ]
}

@test "a host that reports no exit code is treated as success" {
  pend
  run_module "git push" ""
  [ -f "$STATE_DIR/feat_example.waiver.json" ]
}

@test "a pending marker for older code is not cashed in" {
  pend
  echo more > more.txt && git add more.txt && git commit -qm "more"
  run_module "git push" 0
  [ ! -f "$STATE_DIR/feat_example.waiver.json" ]
  [ -f "$STATE_DIR/feat_example.pending-waiver.json" ]
}

@test "no pending marker means no waiver" {
  run_module "git push" 0
  [ "$status" -eq 0 ]
  [ ! -f "$STATE_DIR/feat_example.waiver.json" ]
}

@test "a non-push command is ignored" {
  pend
  run_module "git status" 0
  [ ! -f "$STATE_DIR/feat_example.waiver.json" ]
}

@test "a push mentioned only inside a heredoc body is ignored" {
  pend
  run_module 'cat <<EOF > notes.txt
git push
EOF' 0
  [ ! -f "$STATE_DIR/feat_example.waiver.json" ]
}

@test "git -C <path> push is recognised" {
  pend
  run_module "git -C $SANDBOX push" 0
  [ -f "$STATE_DIR/feat_example.waiver.json" ]
}

@test "the module never writes to stdout" {
  pend
  run_module "git push" 0
  [ -z "$output" ]
}

@test "a non-Bash tool exits silently" {
  pend
  payload=$(jq -n '{tool_name:"Edit",tool_input:{file_path:"a.ts"},tool_response:{}}')
  tool_name=Edit input="$payload" run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  [ ! -f "$STATE_DIR/feat_example.waiver.json" ]
}
