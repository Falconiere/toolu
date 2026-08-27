#!/usr/bin/env bats
# project-skills-index.sh SessionStart payload.

HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/project-skills-index.sh"
SKILLS_SH="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/project-skills/scripts" && pwd)/skills.sh"

setup() {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  mkdir -p "$CLAUDE_CONFIG_DIR"
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  git init -q "$REPO"
  git -C "$REPO" config user.email t@t.t
  git -C "$REPO" config user.name t
  printf 'init\n' >"$REPO/README.md"
  git -C "$REPO" add README.md
  git -C "$REPO" commit -q -m init
}

teardown() { rm -rf "$TMP"; }

@test "index hook emits nothing when the tree is missing" {
  payload=$(jq -n --arg c "$REPO" '{cwd:$c}')
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "index hook emits additionalContext after create" {
  BODY="$TMP/body.md"
  cat >"$BODY" <<'EOF'
## When to Use
x

## Procedure
1. y

## Pitfalls
- z

## Verification
ok
EOF
  DESC='Deploy this repo to staging. Use when shipping a branch to staging.'
  bash -c "cd '$REPO' && bash '$SKILLS_SH' create deploy-staging --description '$DESC' --file '$BODY'"
  payload=$(jq -n --arg c "$REPO" '{cwd:$c}')
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("- deploy-staging: Deploy this repo to staging")' >/dev/null
}

@test "AC-10: skills.comemory=false emits nothing even with skills on disk" {
  BODY="$TMP/body.md"
  cat >"$BODY" <<'EOF'
## When to Use
x

## Procedure
1. y

## Pitfalls
- z

## Verification
ok
EOF
  DESC='Deploy this repo to staging. Use when shipping a branch to staging.'
  bash -c "cd '$REPO' && bash '$SKILLS_SH' create deploy-staging --description '$DESC' --file '$BODY'"
  printf '%s\n' '{"version":1,"skills":{"comemory":false}}' >"$CLAUDE_CONFIG_DIR/toolu.config.json"
  payload=$(jq -n --arg c "$REPO" '{cwd:$c}')
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
