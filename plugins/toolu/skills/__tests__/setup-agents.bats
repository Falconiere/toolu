#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
SCRIPT="$ROOT/plugins/toolu/skills/setup/scripts/setup.sh"
TEMPLATES="$ROOT/plugins/toolu/assets/agents"

setup() {
  TMP=$(mktemp -d)
  TEST_HOME="$TMP/home"
  mkdir -p "$TEST_HOME"
}

teardown() {
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

@test "agent templates contain the required models efforts sandboxes and instructions" {
  expected=(
    "quick-task|gpt-5.6-luna|medium|read-only"
    "deep-explore|gpt-5.6-terra|medium|read-only"
    "research-agent|gpt-5.6-terra|medium|read-only"
    "implementer|gpt-5.6-terra|medium|workspace-write"
    "architect|gpt-5.6-sol|high|read-only"
  )

  for row in "${expected[@]}"; do
    IFS='|' read -r name model effort sandbox <<<"$row"
    file="$TEMPLATES/$name.toml"
    [ -f "$file" ]
    grep -Fqx "name = \"$name\"" "$file"
    grep -Fqx "model = \"$model\"" "$file"
    grep -Fqx "model_reasoning_effort = \"$effort\"" "$file"
    grep -Fqx "sandbox_mode = \"$sandbox\"" "$file"
    grep -Fq 'developer_instructions = """' "$file"
  done
}

@test "preview falls back to HOME/.codex when CODEX_HOME is unset and writes nothing" {
  run env -u CODEX_HOME HOME="$TEST_HOME" bash "$SCRIPT" preview
  [ "$status" -eq 0 ]
  [[ "$output" == *"$TEST_HOME/.codex/agents"* ]]
  [[ "$output" == *"PLAN quick-task install"* ]]
  [ ! -e "$TEST_HOME/.codex/agents" ]
}

@test "install writes all five profiles to an explicit CODEX_HOME" {
  codex_home="$TMP/codex"
  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" bash "$SCRIPT" install
  [ "$status" -eq 0 ]
  [[ "$output" == *"INSTALLED 5"* ]]
  [[ "$output" == *"Restart Codex"* ]]
  for name in quick-task deep-explore research-agent implementer architect; do
    cmp "$TEMPLATES/$name.toml" "$codex_home/agents/$name.toml"
  done
}

@test "unchanged profiles are not backed up or rewritten" {
  codex_home="$TMP/codex"
  env CODEX_HOME="$codex_home" HOME="$TEST_HOME" bash "$SCRIPT" install >/dev/null
  before=$(stat -c %Y "$codex_home/agents/quick-task.toml")

  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" bash "$SCRIPT" install

  [ "$status" -eq 0 ]
  [[ "$output" == *"UNCHANGED 5"* ]]
  [ "$before" = "$(stat -c %Y "$codex_home/agents/quick-task.toml")" ]
  [ ! -e "$codex_home/agents/.toolu-backups" ]
}

@test "a changed managed profile is updated after a timestamped backup" {
  codex_home="$TMP/codex"
  stamp="20260813T190000Z"
  env CODEX_HOME="$codex_home" HOME="$TEST_HOME" bash "$SCRIPT" install >/dev/null
  sed -i 's/model_reasoning_effort = "medium"/model_reasoning_effort = "low"/' \
    "$codex_home/agents/quick-task.toml"

  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" TOOLU_TIMESTAMP="$stamp" \
    bash "$SCRIPT" install

  [ "$status" -eq 0 ]
  [[ "$output" == *"UPDATED 1"* ]]
  grep -Fqx 'model_reasoning_effort = "low"' \
    "$codex_home/agents/.toolu-backups/$stamp/quick-task.toml"
  cmp "$TEMPLATES/quick-task.toml" "$codex_home/agents/quick-task.toml"
}

@test "install refuses an unmanaged conflict without partially installing profiles" {
  codex_home="$TMP/codex"
  mkdir -p "$codex_home/agents"
  printf '%s\n' 'name = "personal"' > "$codex_home/agents/quick-task.toml"

  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" bash "$SCRIPT" install

  [ "$status" -eq 2 ]
  [[ "$output" == *"PLAN quick-task conflict"* ]]
  grep -Fqx 'name = "personal"' "$codex_home/agents/quick-task.toml"
  [ ! -e "$codex_home/agents/architect.toml" ]
}

@test "confirmed force install backs up and replaces an unmanaged conflict" {
  codex_home="$TMP/codex"
  stamp="20260813T190100Z"
  mkdir -p "$codex_home/agents"
  printf '%s\n' 'name = "personal"' > "$codex_home/agents/quick-task.toml"

  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" TOOLU_TIMESTAMP="$stamp" \
    bash "$SCRIPT" install --force

  [ "$status" -eq 0 ]
  grep -Fqx 'name = "personal"' \
    "$codex_home/agents/.toolu-backups/$stamp/quick-task.toml"
  cmp "$TEMPLATES/quick-task.toml" "$codex_home/agents/quick-task.toml"
}

@test "remove requires explicit confirmation and then preserves recoverable backups" {
  codex_home="$TMP/codex"
  stamp="20260813T190200Z"
  env CODEX_HOME="$codex_home" HOME="$TEST_HOME" bash "$SCRIPT" install >/dev/null

  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" bash "$SCRIPT" remove
  [ "$status" -eq 2 ]
  [ -f "$codex_home/agents/quick-task.toml" ]

  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" TOOLU_TIMESTAMP="$stamp" \
    bash "$SCRIPT" remove --yes
  [ "$status" -eq 0 ]
  [[ "$output" == *"REMOVED 5"* ]]
  [ ! -e "$codex_home/agents/quick-task.toml" ]
  [ -f "$codex_home/agents/.toolu-backups/$stamp/quick-task.toml" ]
}

@test "invalid template TOML fails before the destination is created" {
  template_dir="$TMP/templates"
  mkdir -p "$template_dir"
  cp "$TEMPLATES"/*.toml "$template_dir/"
  printf '%s\n' 'invalid = [' >> "$template_dir/architect.toml"
  codex_home="$TMP/codex"

  run env CODEX_HOME="$codex_home" HOME="$TEST_HOME" \
    TOOLU_AGENT_TEMPLATE_DIR="$template_dir" bash "$SCRIPT" install

  [ "$status" -eq 1 ]
  [[ "$output" == *"invalid agent template"* ]]
  [ ! -e "$codex_home/agents" ]
}
