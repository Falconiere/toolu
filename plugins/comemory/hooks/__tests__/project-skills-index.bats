#!/usr/bin/env bats
# project-skills-index.sh SessionStart payload.

HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/project-skills-index.sh"

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

_seed_skill() {
  local name="${1:-deploy-staging}" now
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  mkdir -p "$REPO/.toolu/skills/$name"
  cat >"$REPO/.toolu/skills/$name/SKILL.md" <<EOF
---
name: $name
description: Deploy this repo to staging. Use when shipping a branch to staging.
metadata:
  toolu:
    origin: agent
    created: $now
---
## When to Use
x

## Procedure
1. y

## Pitfalls
- z

## Verification
ok
EOF
}

@test "index hook emits additionalContext after create" {
  _seed_skill deploy-staging
  payload=$(jq -n --arg c "$REPO" '{cwd:$c}')
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("- deploy-staging: Deploy this repo to staging")' >/dev/null
}

@test "AC-10: skills.comemory=false emits nothing even with skills on disk" {
  _seed_skill deploy-staging
  printf '%s\n' '{"version":1,"skills":{"comemory":false}}' >"$CLAUDE_CONFIG_DIR/toolu.config.json"
  payload=$(jq -n --arg c "$REPO" '{cwd:$c}')
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
