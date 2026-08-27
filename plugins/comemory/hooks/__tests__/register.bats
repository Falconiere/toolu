#!/usr/bin/env bats
# register.sh syncs this plugin's pre-tools.d modules into the toolu
# runtime registry as <spec>__<name>.sh, prunes its own stale entries, and
# never touches other plugins' files.

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/register.sh"
  SRC_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../pre-tools.d" && pwd)"
}

teardown() { rm -rf "$TMP"; }

@test "register: syncs every module into pre-tools.d with the spec prefix" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  for src in "$SRC_DIR"/*.sh; do
    name=$(basename "$src")
    dst="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d/comemory@toolu__${name}"
    [ -f "$dst" ]
    cmp -s "$src" "$dst"
  done
}

@test "register: prunes its own stale entries but not other plugins'" {
  regdir="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d"
  mkdir -p "$regdir"
  echo '#!/usr/bin/env bash' > "$regdir/comemory@toolu__removed-module.sh"
  echo '#!/usr/bin/env bash' > "$regdir/other@market__keep.sh"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -f "$regdir/comemory@toolu__removed-module.sh" ]
  [ -f "$regdir/other@market__keep.sh" ]
}

@test "register: refreshes a registry copy that drifted from source" {
  regdir="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d"
  mkdir -p "$regdir"
  echo 'stale content' > "$regdir/comemory@toolu__comemory-scope.sh"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  cmp -s "$SRC_DIR/comemory-scope.sh" "$regdir/comemory@toolu__comemory-scope.sh"
}

@test "register: idempotent (second run changes nothing, exits 0)" {
  bash "$REGISTER" <<<'{}'
  before=$(ls "$CLAUDE_CONFIG_DIR/toolu/pre-tools.d" | sort)
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  after=$(ls "$CLAUDE_CONFIG_DIR/toolu/pre-tools.d" | sort)
  [ "$before" = "$after" ]
}

@test "register: writes are atomic (no *.tmp.* leftovers)" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  found=$(find "$CLAUDE_CONFIG_DIR/toolu" -name '*.tmp.*' | wc -l | tr -d ' ')
  [ "$found" = "0" ]
}

@test "register: emits nothing on stdout (SessionStart hygiene)" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "register: publishes skills.sh next to comemory.sh" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ -L "$CLAUDE_CONFIG_DIR/comemory/skills.sh" ]
  [ -L "$CLAUDE_CONFIG_DIR/comemory/comemory.sh" ]
}

@test "register e2e: synced comemory-scope denies an unscoped call through the core dispatcher when installed" {
  CORE_MOD="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/pre-tools" && pwd)/mod.sh"
  bash "$REGISTER" <<<'{}'
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{"comemory@toolu":{}}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$CLAUDE_CONFIG_DIR" HOME="$TMP" \
    bash "$CORE_MOD" <<<'{"tool_name":"Bash","tool_input":{"command":"comemory search foo"}}'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
}

@test "register e2e: synced modules are gated off when the plugin is definitively absent" {
  CORE_MOD="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/pre-tools" && pwd)/mod.sh"
  bash "$REGISTER" <<<'{}'
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$CLAUDE_CONFIG_DIR" HOME="$TMP" \
    bash "$CORE_MOD" <<<'{"tool_name":"Bash","tool_input":{"command":"comemory search foo"}}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "register: publishes the wrapper symlink at \$CLAUDE_CONFIG_DIR/comemory/comemory.sh" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  dst="$CLAUDE_CONFIG_DIR/comemory/comemory.sh"
  [ -L "$dst" ]
  # The symlink target must point at the plugin-root wrapper.
  src="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/agent-memory/scripts" && pwd)/comemory.sh"
  [ "$(readlink "$dst")" = "$src" ]
  # And it must be executable (dereferences through the symlink).
  [ -x "$dst" ]
}

@test "register: refreshes a stale wrapper symlink to the current target" {
  pub_root="$CLAUDE_CONFIG_DIR/comemory"
  mkdir -p "$pub_root"
  ln -s /nonexistent/old/comemory.sh "$pub_root/comemory.sh"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  src="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/agent-memory/scripts" && pwd)/comemory.sh"
  [ "$(readlink "$pub_root/comemory.sh")" = "$src" ]
}

@test "register: NEVER clobbers a real file the user placed at the wrapper path" {
  pub_root="$CLAUDE_CONFIG_DIR/comemory"
  mkdir -p "$pub_root"
  printf '#!/usr/bin/env bash\necho user-override\n' > "$pub_root/comemory.sh"
  chmod +x "$pub_root/comemory.sh"
  before=$(cat "$pub_root/comemory.sh")
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -L "$pub_root/comemory.sh" ]
  [ "$(cat "$pub_root/comemory.sh")" = "$before" ]
}

@test "register: published wrapper actually executes (smoke: -h-style usage)" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  dst="$CLAUDE_CONFIG_DIR/comemory/comemory.sh"
  # Stub `comemory` so the wrapper's bin-presence guard finds it; the wrapper's
  # `list` path is exec'd via comemory, so the stub's argv is its own assertion.
  stub_dir="$TMP/stub"
  mkdir -p "$stub_dir"
  cat >"$stub_dir/comemory" <<'EOF'
#!/usr/bin/env bash
echo "argv: $*"
EOF
  chmod +x "$stub_dir/comemory"
  run env PATH="$stub_dir:$PATH" MY_CLAUDE_COMEMORY_REPO=stub-repo bash "$dst" list
  [ "$status" -eq 0 ]
  [[ "$output" == *"--repo stub-repo"* ]]
}

@test "register: clears its own AGED orphaned tmp residue, keeps fresh + foreign tmp" {
  regdir="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d"
  mkdir -p "$regdir"
  # Aged orphan (ours): from a crashed run — must be removed.
  echo 'partial' > "$regdir/comemory@toolu__comemory-scope.sh.tmp.12345"
  touch -t 202601010000 "$regdir/comemory@toolu__comemory-scope.sh.tmp.12345"
  # Fresh tmp (ours): a concurrent SessionStart mid-write — must survive.
  echo 'partial' > "$regdir/comemory@toolu__fresh-module.sh.tmp.777"
  # Foreign tmp: never ours to touch, fresh or aged.
  echo 'partial' > "$regdir/other@market__keep.sh.tmp.99"
  touch -t 202601010000 "$regdir/other@market__keep.sh.tmp.99"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -f "$regdir/comemory@toolu__comemory-scope.sh.tmp.12345" ]
  [ -f "$regdir/comemory@toolu__fresh-module.sh.tmp.777" ]
  [ -f "$regdir/other@market__keep.sh.tmp.99" ]
}
