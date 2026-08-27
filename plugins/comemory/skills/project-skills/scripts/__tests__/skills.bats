#!/usr/bin/env bats
# Real-repo tests for skills.sh (spec AC-1..7). No mocks.

SKILLS_SH="${BATS_TEST_DIRNAME}/../skills.sh"

setup() {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  command -v git >/dev/null 2>&1 || skip "git not installed"
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
  BODY="$TMP/body.md"
  cat >"$BODY" <<'EOF'
## When to Use
Shipping a branch to staging.

## Procedure
1. Run the deploy script.

## Pitfalls
- Wrong environment.

## Verification
The staging health endpoint returns 200.
EOF
}

teardown() { rm -rf "$TMP"; }

_desc() { printf 'Deploy this repo to staging. Use when shipping a branch to staging.'; }

_create() {
  local name="${1:-deploy-staging}"
  git -C "$REPO" bash -c "cd '$REPO' && bash '$SKILLS_SH' create '$name' --description '$(_desc)' --file '$BODY'"
}

_iso_ago() {
  python3 -c "from datetime import datetime,timedelta,timezone; print((datetime.now(timezone.utc)-timedelta(days=$1)).strftime('%Y-%m-%dT%H:%M:%SZ'))"
}

@test "AC-1: create writes SKILL.md, gitignore, usage; skill is tracked" {
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' create deploy-staging --description '$(_desc)' --file '$BODY'"
  [ "$status" -eq 0 ]
  [ -f "$REPO/.toolu/skills/deploy-staging/SKILL.md" ]
  grep -q 'origin: agent' "$REPO/.toolu/skills/deploy-staging/SKILL.md"
  grep -qx '.archive/' "$REPO/.toolu/skills/.gitignore"
  grep -qx '.usage.json' "$REPO/.toolu/skills/.gitignore"
  [ -f "$REPO/.toolu/skills/.usage.json" ]
  jq -e '.["deploy-staging"].origin=="agent" and .["deploy-staging"].state=="active" and .["deploy-staging"].use_count==0' \
    "$REPO/.toolu/skills/.usage.json" >/dev/null
  git -C "$REPO" check-ignore -q .toolu/skills/.usage.json
  git -C "$REPO" check-ignore -q .toolu/skills/.archive
  run git -C "$REPO" check-ignore -q .toolu/skills/deploy-staging/SKILL.md
  [ "$status" -eq 1 ]
}

@test "AC-2: create rejects collision, uppercase, 31-word description, missing heading" {
  bash -c "cd '$REPO' && bash '$SKILLS_SH' create deploy-staging --description '$(_desc)' --file '$BODY'"
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' create deploy-staging --description '$(_desc)' --file '$BODY'"
  [ "$status" -ne 0 ]

  run bash -c "cd '$REPO' && bash '$SKILLS_SH' create Deploy --description '$(_desc)' --file '$BODY'"
  [ "$status" -ne 0 ]

  long="one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty thirtyone"
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' create other-skill --description '$long' --file '$BODY'"
  [ "$status" -ne 0 ]
  [ ! -d "$REPO/.toolu/skills/other-skill" ]

  bad="$TMP/bad.md"
  printf '%s\n' '## When to Use' 'x' '## Pitfalls' 'y' '## Verification' 'z' >"$bad"
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' create no-proc --description '$(_desc)' --file '$bad'"
  [ "$status" -ne 0 ]
  [ ! -d "$REPO/.toolu/skills/no-proc" ]
}

@test "AC-3: unmanaged raw Write is not archived; adopt then idle 91d archives" {
  mkdir -p "$REPO/.toolu/skills/hand-runbook"
  {
    printf '%s\n' '---' 'name: hand-runbook' 'description: Hand written runbook for deploys.' '---'
    cat "$BODY"
  } >"$REPO/.toolu/skills/hand-runbook/SKILL.md"
  mkdir -p "$REPO/.toolu/skills"
  printf '{}\n' >"$REPO/.toolu/skills/.usage.json"
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' curate"
  [ "$status" -eq 0 ]
  [ -d "$REPO/.toolu/skills/hand-runbook" ]
  [ ! -d "$REPO/.toolu/skills/.archive/hand-runbook" ]

  run bash -c "cd '$REPO' && bash '$SKILLS_SH' adopt hand-runbook"
  [ "$status" -eq 0 ]
  grep -q 'origin: agent' "$REPO/.toolu/skills/hand-runbook/SKILL.md"
  ago=$(_iso_ago 91)
  jq --arg t "$ago" '.["hand-runbook"].last_used_at=$t | .["hand-runbook"].first_seen_at=$t | .["hand-runbook"].created_at=$t' \
    "$REPO/.toolu/skills/.usage.json" >"$TMP/u.json"
  mv "$TMP/u.json" "$REPO/.toolu/skills/.usage.json"
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' curate"
  [ "$status" -eq 0 ]
  [ -d "$REPO/.toolu/skills/.archive/hand-runbook" ]
  [ ! -d "$REPO/.toolu/skills/hand-runbook" ]
}

@test "AC-4: pin skips archive; unpin then curate archives" {
  bash -c "cd '$REPO' && bash '$SKILLS_SH' create deploy-staging --description '$(_desc)' --file '$BODY'"
  ago=$(_iso_ago 91)
  jq --arg t "$ago" '.["deploy-staging"].last_used_at=$t | .["deploy-staging"].first_seen_at=$t' \
    "$REPO/.toolu/skills/.usage.json" >"$TMP/u.json"
  mv "$TMP/u.json" "$REPO/.toolu/skills/.usage.json"
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' pin deploy-staging"
  [ "$status" -eq 0 ]
  bash -c "cd '$REPO' && bash '$SKILLS_SH' curate"
  [ -d "$REPO/.toolu/skills/deploy-staging" ]
  state=$(jq -r '.["deploy-staging"].state' "$REPO/.toolu/skills/.usage.json")
  [ "$state" = "active" ]
  bash -c "cd '$REPO' && bash '$SKILLS_SH' unpin deploy-staging"
  bash -c "cd '$REPO' && bash '$SKILLS_SH' curate"
  [ -d "$REPO/.toolu/skills/.archive/deploy-staging" ]
}

@test "AC-5: restore after archive; second restore fails" {
  bash -c "cd '$REPO' && bash '$SKILLS_SH' create deploy-staging --description '$(_desc)' --file '$BODY'"
  ago=$(_iso_ago 91)
  jq --arg t "$ago" '.["deploy-staging"].last_used_at=$t | .["deploy-staging"].first_seen_at=$t' \
    "$REPO/.toolu/skills/.usage.json" >"$TMP/u.json"
  mv "$TMP/u.json" "$REPO/.toolu/skills/.usage.json"
  bash -c "cd '$REPO' && bash '$SKILLS_SH' curate"
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' restore deploy-staging"
  [ "$status" -eq 0 ]
  [ -d "$REPO/.toolu/skills/deploy-staging" ]
  state=$(jq -r '.["deploy-staging"].state' "$REPO/.toolu/skills/.usage.json")
  [ "$state" = "active" ]
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' restore deploy-staging"
  [ "$status" -ne 0 ]
}

@test "AC-6: missing usage.json seeds first_seen_at and archives none" {
  mkdir -p "$REPO/.toolu/skills/one" "$REPO/.toolu/skills/two" "$REPO/.toolu/skills/three"
  for n in one two three; do
    {
      printf '%s\n' '---' "name: $n" 'description: Class-level procedure for this repo task.' \
        'metadata:' '  toolu:' '    origin: agent' '---'
      cat "$BODY"
    } >"$REPO/.toolu/skills/$n/SKILL.md"
  done
  [ ! -f "$REPO/.toolu/skills/.usage.json" ]
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' curate"
  [ "$status" -eq 0 ]
  [ -d "$REPO/.toolu/skills/one" ]
  [ -d "$REPO/.toolu/skills/two" ]
  [ -d "$REPO/.toolu/skills/three" ]
  [ -f "$REPO/.toolu/skills/.usage.json" ]
  jq -e '.one.first_seen_at and .two.first_seen_at and .three.first_seen_at' \
    "$REPO/.toolu/skills/.usage.json" >/dev/null
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' curate"
  [ "$status" -eq 0 ]
  [ -d "$REPO/.toolu/skills/one" ]
  [ ! -d "$REPO/.toolu/skills/.archive/one" ]
}

@test "AC-7: index honors indexCap=20 by newest last_used_at; empty tree is empty" {
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' index"
  [ "$status" -eq 0 ]
  [ -z "$output" ]

  mkdir -p "$REPO/.claude"
  printf '%s\n' '{"version":1,"projectSkills":{"indexCap":20}}' >"$REPO/.claude/toolu.config.json"
  bash -c "cd '$REPO' && bash '$SKILLS_SH' create skill-01 --description '$(_desc)' --file '$BODY'"
  [ -f "$REPO/.toolu/skills/skill-01/SKILL.md" ]
  i=2
  while [ "$i" -le 25 ]; do
    n=$(printf 'skill-%02d' "$i")
    cp -a "$REPO/.toolu/skills/skill-01" "$REPO/.toolu/skills/$n"
    sed -i "s/^name: skill-01$/name: $n/" "$REPO/.toolu/skills/$n/SKILL.md"
    i=$((i + 1))
  done
  # Oldest 01 .. newest 25
  python3 - "$REPO/.toolu/skills/.usage.json" <<'PY'
import json, sys
from datetime import datetime, timedelta, timezone
path = sys.argv[1]
data = json.load(open(path))
now = datetime.now(timezone.utc)
base = data.get("skill-01") or {
    "origin": "agent", "use_count": 0, "patch_count": 0,
    "state": "active", "pinned": False,
}
for i in range(1, 26):
    name = f"skill-{i:02d}"
    ts = (now - timedelta(days=26-i)).strftime("%Y-%m-%dT%H:%M:%SZ")
    entry = dict(base)
    entry["last_used_at"] = ts
    entry["created_at"] = ts
    entry["first_seen_at"] = ts
    data[name] = entry
json.dump(data, open(path, "w"))
PY
  run bash -c "cd '$REPO' && bash '$SKILLS_SH' index"
  [ "$status" -eq 0 ]
  lines=$(printf '%s\n' "$output" | grep -c '^- skill-')
  [ "$lines" -eq 20 ]
  printf '%s\n' "$output" | grep -q '^- skill-25:'
  printf '%s\n' "$output" | grep -q '^- skill-06:'
  ! printf '%s\n' "$output" | grep -q '^- skill-05:'
}

@test "create fails outside a git repo" {
  run bash -c "cd '$TMP' && bash '$SKILLS_SH' create deploy-staging --description '$(_desc)' --file '$BODY'"
  [ "$status" -ne 0 ]
}
