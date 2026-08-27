#!/usr/bin/env bats
# skill-use.sh against real PostToolUse JSON (spec AC-8, AC-10).

HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/skill-use.sh"

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
  _seed_skill deploy-staging
}

_seed_skill() {
  local name="${1:-deploy-staging}" now
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  mkdir -p "$REPO/.toolu/skills/$name" "$REPO/.toolu/skills/.archive"
  printf '%s\n' '.archive/' '.usage.json' >"$REPO/.toolu/skills/.gitignore"
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
  jq -n --arg n "$name" --arg t "$now" \
    '{($n):{origin:"agent",created_at:$t,use_count:0,patch_count:0,last_used_at:null,last_patched_at:null,state:"active",pinned:false,first_seen_at:$t}}' \
    >"$REPO/.toolu/skills/.usage.json"
}

teardown() { rm -rf "$TMP"; }

_payload() {
  local tool="$1" path="$2"
  jq -n --arg t "$tool" --arg p "$path" --arg c "$REPO" \
    '{tool_name:$t, cwd:$c, tool_input:{file_path:$p, target_file:$p}}'
}

@test "AC-8: Read of project SKILL.md increments use_count" {
  skill="$REPO/.toolu/skills/deploy-staging/SKILL.md"
  run bash "$HOOK" < <(_payload Read "$skill")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].use_count==1 and (.["deploy-staging"].last_used_at | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))' \
    "$REPO/.toolu/skills/.usage.json" >/dev/null
}

@test "AC-8: Read of README.md or plugin SKILL.md does not increment" {
  run bash "$HOOK" < <(_payload Read "$REPO/README.md")
  [ "$status" -eq 0 ]
  plugin="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/agent-memory" && pwd)/SKILL.md"
  run bash "$HOOK" < <(_payload Read "$plugin")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].use_count==0' "$REPO/.toolu/skills/.usage.json" >/dev/null
}

@test "AC-8: Write of project skill increments patch_count and reactivates stale" {
  jq '.["deploy-staging"].state="stale"' "$REPO/.toolu/skills/.usage.json" >"$TMP/u.json"
  mv "$TMP/u.json" "$REPO/.toolu/skills/.usage.json"
  skill="$REPO/.toolu/skills/deploy-staging/SKILL.md"
  run bash "$HOOK" < <(_payload Write "$skill")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].patch_count==1 and .["deploy-staging"].state=="active"' \
    "$REPO/.toolu/skills/.usage.json" >/dev/null
}

@test "AC-8: .. in a path is canonicalized and does not invent a skill name" {
  sneak="$REPO/.toolu/skills/.archive/../deploy-staging/SKILL.md"
  run bash "$HOOK" < <(_payload Read "$sneak")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].use_count==1' "$REPO/.toolu/skills/.usage.json" >/dev/null
}

@test "AC-8: SKILL.md symlink into .archive is not counted as use" {
  mkdir -p "$REPO/.toolu/skills/.archive/old"
  mv "$REPO/.toolu/skills/deploy-staging/SKILL.md" "$REPO/.toolu/skills/.archive/old/SKILL.md"
  ln -s "$REPO/.toolu/skills/.archive/old/SKILL.md" "$REPO/.toolu/skills/deploy-staging/SKILL.md"
  run bash "$HOOK" < <(_payload Read "$REPO/.toolu/skills/deploy-staging/SKILL.md")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].use_count==0' "$REPO/.toolu/skills/.usage.json" >/dev/null
}

@test "AC-8: read_file (Codex/Grok) also counts as use" {
  skill="$REPO/.toolu/skills/deploy-staging/SKILL.md"
  run bash "$HOOK" < <(_payload read_file "$skill")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].use_count==1' "$REPO/.toolu/skills/.usage.json" >/dev/null
}

@test "AC-10: skills.comemory=false leaves usage untouched" {
  printf '%s\n' '{"version":1,"skills":{"comemory":false}}' >"$CLAUDE_CONFIG_DIR/toolu.config.json"
  skill="$REPO/.toolu/skills/deploy-staging/SKILL.md"
  run bash "$HOOK" < <(_payload Read "$skill")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].use_count==0' "$REPO/.toolu/skills/.usage.json" >/dev/null
}

@test "AC-10: projectSkills.enabled=false leaves usage untouched" {
  printf '%s\n' '{"version":1,"projectSkills":{"enabled":false}}' >"$CLAUDE_CONFIG_DIR/toolu.config.json"
  skill="$REPO/.toolu/skills/deploy-staging/SKILL.md"
  run bash "$HOOK" < <(_payload Read "$skill")
  [ "$status" -eq 0 ]
  jq -e '.["deploy-staging"].use_count==0' "$REPO/.toolu/skills/.usage.json" >/dev/null
}
