#!/usr/bin/env bats
# project-skills-curate.sh stamp + detach (spec AC-9, AC-10).

HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/project-skills-curate.sh"

setup() {
  TMP=$(mktemp -d)
  export HOME="$TMP/home"
  export CLAUDE_CONFIG_DIR="$HOME/.claude"
  mkdir -p "$CLAUDE_CONFIG_DIR/toolu"
  WRAP="$TMP/bin"
  mkdir -p "$WRAP"
  MARK="$TMP/curate.mark"
  printf '#!/bin/sh\necho ran >>"%s"\n' "$MARK" >"$WRAP/skills.sh"
  chmod +x "$WRAP/skills.sh"
  export PATH="$WRAP:$PATH"
}

teardown() { rm -rf "$TMP"; }

@test "AC-9: today's stamp does not invoke curate" {
  printf '%s' "$(date -u +%Y%m%d)" >"$CLAUDE_CONFIG_DIR/toolu/.project-skills-last-curate"
  run bash "$HOOK" </dev/null
  [ "$status" -eq 0 ]
  [ ! -f "$MARK" ]
}

@test "AC-9: stale stamp backgrounds curate; hook returns first" {
  printf '%s' "19990101" >"$CLAUDE_CONFIG_DIR/toolu/.project-skills-last-curate"
  run bash "$HOOK" </dev/null
  [ "$status" -eq 0 ]
  [ "$(cat "$CLAUDE_CONFIG_DIR/toolu/.project-skills-last-curate")" = "$(date -u +%Y%m%d)" ]
  # Detached — wait briefly for the wrapper to land.
  i=0
  while [ "$i" -lt 20 ] && [ ! -f "$MARK" ]; do
    sleep 0.05
    i=$((i + 1))
  done
  [ -f "$MARK" ]
}

@test "AC-10: skills.comemory=false does not curate" {
  printf '%s\n' '{"version":1,"skills":{"comemory":false}}' >"$CLAUDE_CONFIG_DIR/toolu.config.json"
  rm -f "$CLAUDE_CONFIG_DIR/toolu/.project-skills-last-curate"
  run bash "$HOOK" </dev/null
  [ "$status" -eq 0 ]
  [ ! -f "$MARK" ]
  [ ! -f "$CLAUDE_CONFIG_DIR/toolu/.project-skills-last-curate" ]
}

@test "AC-10: projectSkills.enabled=false does not curate" {
  printf '%s\n' '{"version":1,"projectSkills":{"enabled":false}}' >"$CLAUDE_CONFIG_DIR/toolu.config.json"
  rm -f "$CLAUDE_CONFIG_DIR/toolu/.project-skills-last-curate"
  run bash "$HOOK" </dev/null
  [ "$status" -eq 0 ]
  [ ! -f "$MARK" ]
}
