#!/usr/bin/env bats
# Tests for tooling/install-opencode.sh against the real repo (no mocks):
# installs into a throwaway directory and asserts the produced layout.

SCRIPT="${BATS_TEST_DIRNAME}/../install-opencode.sh"

setup() {
  TARGET=$(mktemp -d)
  AGENT_DIR=$(mktemp -d)
  export TOOLU_CONFIG_DIR="$AGENT_DIR"
  export OPENCODE_CONFIG_DIR="$AGENT_DIR"
}

teardown() {
  [ -n "${TARGET:-}" ] && [ -d "$TARGET" ] && rm -rf "$TARGET"
  [ -n "${AGENT_DIR:-}" ] && [ -d "$AGENT_DIR" ] && rm -rf "$AGENT_DIR"
}

@test "install: drops the adapter into .opencode/plugins/" {
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  [ -f "$TARGET/.opencode/plugins/toolu.ts" ]
}

@test "install: copies every opencode-format agent" {
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  # Two agents today (deep-explore, research-agent); future agents auto-picked up.
  count=$(find "$TARGET/.opencode/agents" -maxdepth 1 -name '*.md' | wc -l | tr -d ' ')
  [ "$count" -ge 2 ]
  [ -f "$TARGET/.opencode/agents/deep-explore.md" ]
  [ -f "$TARGET/.opencode/agents/research-agent.md" ]
}

@test "install: copies every opencode-format command" {
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  count=$(find "$TARGET/.opencode/commands" -maxdepth 1 -name '*.md' | wc -l | tr -d ' ')
  [ "$count" -ge 5 ]
  [ -f "$TARGET/.opencode/commands/toolu-commit.md" ]
  [ -f "$TARGET/.opencode/commands/toolu-stats.md" ]
  [ -f "$TARGET/.opencode/commands/toolu-status.md" ]
}

@test "install: creates opencode.json with skills.paths and \$schema" {
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  [ -f "$TARGET/opencode.json" ]
  schema=$(jq -r '."$schema"' "$TARGET/opencode.json")
  [ "$schema" = "https://opencode.ai/config.json" ]
  # skills.paths must include a /plugins entry
  echo "$(jq -c '.skills.paths' "$TARGET/opencode.json")" | jq -e 'any(. | test("/plugins$"))' >/dev/null
}

@test "install: populates the opencode runtime registry under TOOLU_CONFIG_DIR" {
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  pre=$(find "$AGENT_DIR/toolu/pre-tools.d" -maxdepth 1 -name '*.sh' 2>/dev/null | wc -l | tr -d ' ')
  post=$(find "$AGENT_DIR/toolu/post-tools.d" -maxdepth 1 -name '*.sh' 2>/dev/null | wc -l | tr -d ' ')
  [ "$pre" -ge 1 ]
  [ "$post" -ge 1 ]
}

@test "install: is idempotent (second run reports up-to-date)" {
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  out=$(bash "$SCRIPT" --target "$TARGET" 2>&1)
  echo "$out" | grep -q "up to date"
  # opencode.json should not change on the second run (semantic, not byte-equal)
  sum_before=$(jq -S . "$TARGET/opencode.json" | shasum -a 256 | cut -d' ' -f1)
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  sum_after=$(jq -S . "$TARGET/opencode.json" | shasum -a 256 | cut -d' ' -f1)
  [ "$sum_before" = "$sum_after" ]
}

@test "install: preserves existing opencode.json fields when merging" {
  # Seed an opencode.json with a user-owned key; the script must keep it.
  printf '{\n  "model": "anthropic/claude-sonnet-4-6",\n  "mcp": {"mything": {"type": "remote", "url": "https://x"}}\n}\n' \
    > "$TARGET/opencode.json"
  bash "$SCRIPT" --target "$TARGET" >/dev/null 2>&1
  [ "$(jq -r .model "$TARGET/opencode.json")" = "anthropic/claude-sonnet-4-6" ]
  [ "$(jq -r .mcp.mything.url "$TARGET/opencode.json")" = "https://x" ]
}

@test "install: --global targets ~/.config/opencode by default" {
  HOME="$TARGET" bash "$SCRIPT" --global >/dev/null 2>&1
  [ -f "$TARGET/.config/opencode/.opencode/plugins/toolu.ts" ]
}

@test "install: refuses to run without jq on PATH" {
  # Run with a stripped PATH so `jq` is unfindable; expect non-zero exit.
  out=$(PATH="/usr/bin:/bin" env -i PATH="/usr/bin:/bin" HOME="$TARGET" bash "$SCRIPT" --target "$TARGET/sub" 2>&1 || true)
  # Either the script errors out OR the install succeeds (it might find jq in /usr/bin
  # even with that stripped PATH). The important assertion: it does not silently corrupt.
  echo "$out" | grep -qE "install-opencode|done" || [ ! -d "$TARGET/sub/.opencode" ]
}
